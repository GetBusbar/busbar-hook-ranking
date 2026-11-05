// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! Ranking ties resolve by the candidate's stable `idx`, using `key_order` in
//! `hook-ranking/src/lib.rs`. Non-monotone input indices distinguish this from preserving input
//! order. Both full and partial ties are checked through `rank`, the public ranking entry point.

use busbar_contract::abi::sdk::hook::{DecodedCandidate, Verdict};
use busbar_hook_ranking::rank;

mod allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    pub struct Counting;

    thread_local! {
        static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
    }

    fn record() {
        let _ = COUNT.try_with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });
    }

    // SAFETY: every allocation operation delegates unchanged to the system allocator.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            record();
            // SAFETY: the caller supplies the layout required by GlobalAlloc.
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            record();
            // SAFETY: the caller supplies the layout required by GlobalAlloc.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            record();
            // SAFETY: pointer, layout and size retain the caller's GlobalAlloc contract.
            unsafe { System.realloc(ptr, layout, size) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: pointer and layout retain the caller's GlobalAlloc contract.
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    pub fn during<T>(call: impl FnOnce() -> T) -> (T, usize) {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                COUNT.with(|count| count.set(None));
            }
        }
        COUNT.with(|count| assert!(count.replace(Some(0)).is_none()));
        let reset = Reset;
        let value = call();
        let count = COUNT.with(|count| count.get().unwrap());
        drop(reset);
        (value, count)
    }
}

#[global_allocator]
static ALLOCATOR: allocations::Counting = allocations::Counting;

fn cost_signal(price: Option<f64>) -> busbar_contract::SignalBag {
    use busbar_contract::signal::route_cost::{append, Term};
    let mut signals = busbar_contract::SignalBag::new();
    append(
        &mut signals,
        [Term {
            class: "work",
            price_per_unit: price,
            expected_units: 1.0,
        }],
    );
    signals
}

fn cand_tied(idx: usize, rate: Option<f64>) -> DecodedCandidate<'static> {
    DecodedCandidate {
        idx,
        model: "m".into(),
        provider: "p".into(),
        weight: 1,
        context_max: None,
        tier: None,

        cost_per_mtok: Some(7.0),
        tags: Vec::new(),
        latency_ms: Some(50.0),

        available_concurrency: 4,
        budget_remaining: None,
        rate_headroom: rate,
        signals: cost_signal(Some(7.0)),
    }
}

/// For every native ranking policy that ranks on a signal (cheapest, fastest, least_busy, usage),
/// a full tie on the primary key must resolve to idx-ascending order, identically on every call.
#[test]
fn ranking_is_deterministic_under_full_ties_across_all_signal_natives() {
    let cands = [
        cand_tied(3, Some(0.5)),
        cand_tied(1, Some(0.5)),
        cand_tied(2, Some(0.5)),
        cand_tied(0, Some(0.5)),
    ];
    let expected = Verdict::Prefer(vec![0, 1, 2, 3]);

    for name in ["cheapest", "fastest", "least_busy", "usage"] {
        let decision =
            rank(name, &cands).unwrap_or_else(|| panic!("{name} must be a known strategy"));
        assert_eq!(
            decision, expected,
            "{name} must break a full tie by ascending idx"
        );
    }
}

/// A partial tie (some candidates share the primary key, one does not) still resolves the tied
/// subgroup by idx-ascending, with the untied member ranked by its own key as usual. This pins the
/// tiebreak rule precisely (idx, not e.g. insertion-reversed or hash-order) rather than merely
/// "some order that happens to be stable in one process".
#[test]
fn partial_tie_group_is_ordered_by_idx_within_the_group() {
    let cands = [
        DecodedCandidate {
            idx: 5,
            cost_per_mtok: Some(3.0),
            signals: cost_signal(Some(3.0)),
            ..cand_tied(5, None)
        },
        DecodedCandidate {
            idx: 9,
            cost_per_mtok: Some(1.0),
            signals: cost_signal(Some(1.0)),
            ..cand_tied(9, None)
        },
        DecodedCandidate {
            idx: 2,
            cost_per_mtok: Some(3.0),
            signals: cost_signal(Some(3.0)),
            ..cand_tied(2, None)
        },
    ];
    let expected = Verdict::Prefer(vec![9, 2, 5]);
    let decision = rank("cheapest", &cands).unwrap();
    assert_eq!(decision, expected);
}

#[test]
fn all_absent_signals_abstain_without_allocating() {
    let cands = [
        DecodedCandidate {
            cost_per_mtok: None,
            signals: cost_signal(None),
            latency_ms: None,
            ..cand_tied(9, None)
        },
        DecodedCandidate {
            cost_per_mtok: None,
            signals: cost_signal(None),
            latency_ms: None,
            ..cand_tied(2, None)
        },
    ];
    for name in ["cheapest", "fastest", "usage"] {
        let (decision, count) = allocations::during(|| {
            std::hint::black_box(rank(
                std::hint::black_box(name),
                std::hint::black_box(&cands),
            ))
        });
        assert_eq!(decision, Some(Verdict::Abstain), "{name}");
        assert_eq!(count, 0, "{name} allocated while abstaining");
    }

    let present = [cand_tied(9, Some(0.5)), cand_tied(2, Some(0.5))];
    let (decision, count) =
        allocations::during(|| std::hint::black_box(rank("usage", std::hint::black_box(&present))));
    assert_eq!(decision, Some(Verdict::Prefer(vec![2, 9])));
    assert!(count > 0, "the allocating control must be observed");
}

#[test]
fn authoritative_unknown_costs_abstain_without_allocating() {
    use busbar_contract::{Signal, SignalBag, SignalValue};
    let signal = Signal::CandidateRouteCost;
    let mut empty = SignalBag::new();
    empty.push(signal, SignalValue::Bool(true));
    let mut unknown = empty.clone();
    unknown.push(signal, SignalValue::Str("work".into()));
    unknown.push(signal, SignalValue::Bool(false));
    unknown.push(signal, SignalValue::F64(1.0));
    for signals in [empty, unknown] {
        let cands = [
            DecodedCandidate {
                signals: signals.clone(),
                ..cand_tied(9, None)
            },
            DecodedCandidate {
                signals,
                ..cand_tied(2, None)
            },
        ];
        let (decision, count) = allocations::during(|| {
            std::hint::black_box(rank("cheapest", std::hint::black_box(&cands)))
        });
        assert_eq!(decision, Some(Verdict::Abstain));
        assert_eq!(
            count, 0,
            "unknown generic costs must not allocate while abstaining"
        );
    }
}
