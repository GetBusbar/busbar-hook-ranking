// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! THE RANKING HOOK'S CONFORMANCE: its door, BOTH WAYS (#2 rule (1): compiled in or dropped in,
//! one contract, one loading path), and RANKING NEVER PENDS (ARCHITECT Q-SO9).
//!
//! * [`the_linked_and_the_dropped_in_door_rank_every_shape_the_same`]: the linked door
//!   (`linked::door`) and this crate's dropped-in image (its built cdylib, the same door behind
//!   `export_door!`), each admitted through the one loader, run ONE script — a refused open,
//!   then every strategy word opened and asked to `decide` over every candidate shape the ranking
//!   parity cases use. The two transcripts (outcome, verbs, the order written) must be identical.
//! * RED ARM, kept: [`red_a_door_that_ranks_the_other_way_answers_differently`] runs the same script
//!   over the same door with its verdict reversed. Its transcript differs at the orders, so a door
//!   that ranked differently cannot pass for this one.
//! * [`every_strategy_word_answers_decide_ready_on_the_first_poll_never_pending`]: through both
//!   doors, `decide` answers READY on the FIRST poll for every word and shape. Ranking is pure
//!   compute: an answer of PENDING would hand the call to the dispatcher's deadline, where 1.5.5's
//!   in-process ranking could never time out. RED by making the door's `decide` answer
//!   `Poll::Pending`: this test fails on the first word.

use std::sync::Arc;
use std::task::Poll;

use busbar_contract::abi::hook::{
    slot, CandidateDynamic, ConfigureIn, ConfigureOut, DecideOut, SignalEntry,
    SignalValue as WireSignalValue, CANDIDATE_HAS_LATENCY_MS, CANDIDATE_HAS_RATE_HEADROOM,
    SIGNAL_TAG_BOOL, SIGNAL_TAG_F64, SIGNAL_TAG_STR, SIGNAL_TAG_U64, VERB_ABSTAIN, VERB_PREFER,
};
use busbar_contract::abi::host::hook::{DecideFrame, DecideView};
use busbar_contract::abi::mechanism::call::{AbiStr, Blob, Outcome, BLOB_JSON};
use busbar_contract::abi::mechanism::door::DoorFn;
use busbar_contract::abi::mechanism::lifecycle::{slot as life, OpenIn, OpenOut, ReleaseIn};
use busbar_contract::abi::sdk::door::blank_out;
use busbar_contract::abi::sdk::exchange::Op;
use busbar_contract::abi::sdk::hook::{
    Decoded, DecodedCandidate, Hook as HookImpl, HookOpen, Verdict,
};
use busbar_contract::hooks::{Candidate, RoutingContext, RoutingRequest};
use busbar_contract::SignalBag;
use busbar_hook_ranking::WORDS;
use busbar_plugin_loader::dispatch::kinds::hook::Hook;
use busbar_plugin_loader::dispatch::{
    in_head, load_dropped, load_linked, out_head, rendering_of, Bind, ConnTable, DispatchConfig,
    Dispatcher, Frame, LinkedRow, NoSink, Plugin,
};

fn checked_call<
    I: busbar_plugin_loader::dispatch::InFrame,
    O: busbar_plugin_loader::dispatch::OutFrame,
>(
    plugin: &Plugin<Hook>,
    slot: u32,
    frame: &mut Frame<I, O>,
) -> busbar_plugin_loader::dispatch::Called {
    use std::sync::atomic::Ordering;
    let entered = plugin.inner.crossings.load(Ordering::Relaxed);
    let resumed = plugin.inner.resumes.load(Ordering::Relaxed);
    let called = plugin.call(slot, frame);
    let host_refusal = (slot == life::OPEN || slot == slot::DECIDE || slot == slot::TRANSFORM)
        && matches!(called.outcome, Outcome::Refused | Outcome::Fault);
    assert_eq!(
        plugin.inner.crossings.load(Ordering::Relaxed) - entered,
        u64::from(!host_refusal),
        "one physical plugin entry for ranking slot {slot}"
    );
    assert_eq!(
        plugin.inner.resumes.load(Ordering::Relaxed) - resumed,
        0,
        "ranking is pure compute and must not resume"
    );
    called
}

fn bind(d: &Dispatcher) -> Bind {
    Bind {
        instance: Arc::from("ranking"),
        max_inflight_cap: 64,
        sink: Arc::new(NoSink),
        dispatcher: d.adopter(),
        conns: ConnTable::NoNeeds,
    }
}

/// `door`, linked and bound on `d`.
fn linked_row(door: DoorFn, d: &Dispatcher) -> Plugin<Hook> {
    let row = LinkedRow::of(door).expect("the door states itself");
    load_linked::<Hook>(&row, bind(d)).expect("the door loads")
}

/// The linked door, bound on `d`.
fn linked(d: &Dispatcher) -> Plugin<Hook> {
    linked_row(busbar_hook_ranking::linked::door, d)
}

/// This crate's dropped-in image, the freshly built cdylib (`busbar_hook_ranking_plugin`) named by
/// the absolute `BUSBAR_PLUGIN_CDYLIB` path, admitted against the Statement the linked door
/// renders. A missing artifact is a failure, never a skip: this IS the dropped-in door's proof.
fn dropped(d: &Dispatcher) -> Plugin<Hook> {
    let path = {
        let path = std::env::var_os("BUSBAR_PLUGIN_CDYLIB")
            .map(std::path::PathBuf::from)
            .expect("set BUSBAR_PLUGIN_CDYLIB to the freshly built ranking cdylib");
        assert!(
            path.is_absolute() && path.is_file(),
            "actual ranking cdylib required: {}",
            path.display()
        );
        path
    };
    let stated =
        rendering_of(busbar_hook_ranking::linked::door).expect("the door renders its Statement");
    load_dropped::<Hook>(&path, &stated, bind(d)).expect("the dropped door loads")
}

/// `open` over `settings`: its outcome.
fn open_answer(p: &Plugin<Hook>, settings: &str) -> busbar_plugin_loader::dispatch::Called {
    let mut f = Frame::new(
        OpenIn {
            head: in_head(),
            host: std::ptr::null(),
            settings: Blob {
                ptr: settings.as_ptr(),
                len: settings.len(),
                fmt: BLOB_JSON,
                flags: 0,
            },
            secrets: std::ptr::null(),
            secrets_len: 0,
            generation: 1,
            err_buf: std::ptr::null_mut(),
            err_cap: 0,
        },
        OpenOut {
            head: out_head(),
            instance: std::ptr::null_mut(),
            err_len: 0,
        },
    );
    checked_call(p, life::OPEN, &mut f)
}

fn open(p: &Plugin<Hook>, settings: &str) -> Outcome {
    open_answer(p, settings).outcome
}

fn request() -> RoutingRequest<'static> {
    RoutingRequest {
        request_id: 1,
        pool: "p",
        ingress_protocol: "wire-a",
        requested_model: None,
        message_count: 1,
        tool_count: 0,
        has_tools: false,
        total_chars: 10,
        system_chars: 0,
        max_tokens: None,
        stream: false,
        prompt: None,
        identity: None,
        signals: SignalBag::new(),
        session: None,
    }
}

/// `(idx, cost, latency, concurrency, rate headroom)`.
type Row = (usize, Option<f64>, Option<f64>, usize, Option<f64>);

/// The candidate shapes the ranking parity cases rank: signals present, partly absent, all absent,
/// a non-finite key, all saturated, one member, none.
const SHAPES: &[&[Row]] = &[
    &[
        (0, Some(15.0), Some(120.0), 2, Some(0.10)),
        (1, Some(3.0), Some(40.0), 9, Some(0.90)),
        (2, None, None, 5, None),
    ],
    &[(0, None, None, 1, None), (1, None, None, 1, None)],
    &[
        (0, Some(f64::NAN), Some(f64::INFINITY), 0, Some(f64::NAN)),
        (1, Some(1.0), None, 0, None),
    ],
    &[(0, Some(5.0), Some(30.0), 3, Some(0.5))],
    &[],
];

fn cost_signal(price: Option<f64>) -> SignalBag {
    use busbar_contract::signal::route_cost::{append, Term};
    let mut signals = SignalBag::new();
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

fn candidates(rows: &[Row]) -> Vec<Candidate<'static>> {
    rows.iter()
        .map(|&(idx, cost, lat, conc, rate)| Candidate {
            idx,
            model: "m",
            provider: "p",
            weight: 1,
            context_max: None,
            tier: None,
            cost_per_mtok: cost,
            tags: &[],
            latency_ms: lat,
            available_concurrency: conc,
            budget_remaining: None,
            rate_headroom: rate,
            signals: cost_signal(cost),
        })
        .collect()
}

fn ctx() -> RoutingContext<'static> {
    RoutingContext {
        pool: "p",
        budget_remaining: None,
        budget: &[],
    }
}

/// One `decide` over `rows`: its outcome, verbs and the order it wrote.
fn decide(p: &Plugin<Hook>, rows: &[Row]) -> (Outcome, u32, Vec<usize>) {
    let frame = DecideFrame::first(DecideView::build(&request(), &candidates(rows), &ctx()));
    let mut f = Frame::new(frame.input(), blank_out::<DecideOut>());
    let outcome = checked_call(p, slot::DECIDE, &mut f).outcome;
    (outcome, f.out.verbs, frame.order(f.out.order_written))
}

/// Lend candidate signals through the existing physical ABI, without requiring new Rust
/// catalogue variants in this test. This makes a new projection an executable old-API regression
/// rather than a missing-symbol compiler failure.
fn decide_with_signals(
    p: &Plugin<Hook>,
    rows: &[Row],
    signals: &[&[SignalEntry]],
) -> (Outcome, u32, Vec<usize>) {
    assert_eq!(rows.len(), signals.len());
    let req = request();
    let cands = candidates(rows);
    let context = ctx();
    let frame = DecideFrame::first(DecideView::build(&req, &cands, &context));
    let dynamics: Vec<_> = rows
        .iter()
        .zip(signals)
        .map(
            |(&(_, _, latency, concurrency, rate), signals)| CandidateDynamic {
                latency_ms: latency.unwrap_or(0.0),
                available_concurrency: concurrency as u64,
                budget_remaining: 0,
                rate_headroom: rate.unwrap_or(0.0),
                signals: signals.as_ptr(),
                signals_len: signals.len(),
                present: if latency.is_some() {
                    CANDIDATE_HAS_LATENCY_MS
                } else {
                    0
                } | if rate.is_some() {
                    CANDIDATE_HAS_RATE_HEADROOM
                } else {
                    0
                },
                _reserved: 0,
            },
        )
        .collect();
    let mut input = frame.input();
    input.candidate_dynamics = dynamics.as_ptr();
    let mut f = Frame::new(input, blank_out::<DecideOut>());
    let outcome = checked_call(p, slot::DECIDE, &mut f).outcome;
    (outcome, f.out.verbs, frame.order(f.out.order_written))
}

const ROUTE_COST_ID: u32 = 10;

fn recipe(terms: &[(&str, Option<u64>, f64)]) -> Vec<SignalEntry> {
    let mut entries = vec![SignalEntry {
        id: ROUTE_COST_ID,
        tag: SIGNAL_TAG_BOOL,
        value: WireSignalValue { boolean: 1 },
    }];
    for &(class, price, units) in terms {
        entries.push(SignalEntry {
            id: ROUTE_COST_ID,
            tag: SIGNAL_TAG_STR,
            value: WireSignalValue {
                str_: AbiStr {
                    ptr: class.as_ptr(),
                    len: class.len(),
                },
            },
        });
        entries.push(SignalEntry {
            id: ROUTE_COST_ID,
            tag: if price.is_some() {
                SIGNAL_TAG_U64
            } else {
                SIGNAL_TAG_BOOL
            },
            value: price.map_or(WireSignalValue { boolean: 0 }, |u64_| WireSignalValue {
                u64_,
            }),
        });
        entries.push(SignalEntry {
            id: ROUTE_COST_ID,
            tag: SIGNAL_TAG_F64,
            value: WireSignalValue { f64_: units },
        });
    }
    entries
}

fn configure(p: &Plugin<Hook>, settings: &str, version: u64) -> (Outcome, u64) {
    let mut f = Frame::new(
        ConfigureIn {
            head: in_head(),
            version,
            settings: Blob {
                ptr: settings.as_ptr(),
                len: settings.len(),
                fmt: BLOB_JSON,
                flags: 0,
            },
            name: AbiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
        },
        blank_out::<ConfigureOut>(),
    );
    let called = checked_call(p, slot::CONFIGURE, &mut f);
    if called.lease != 0 {
        let mut release = Frame::new(
            ReleaseIn {
                head: in_head(),
                lease: called.lease,
            },
            out_head(),
        );
        assert_eq!(
            checked_call(p, life::RELEASE, &mut release).outcome,
            Outcome::Ready
        );
    }
    (called.outcome, f.out.acked_version)
}

const COST_ROWS: &[Row] = &[
    (0, Some(1.0), Some(900.0), 1, None),
    (1, Some(9.0), Some(10.0), 1, None),
];

#[test]
fn generic_costs_preserve_the_configured_float_order_both_ways() {
    let mut a = recipe(&[("input", Some(100), 0.5), ("output", Some(200), 0.5)]);
    let mut b = recipe(&[("input", Some(150), 0.5), ("output", Some(150), 0.5)]);
    for (entries, prices) in [(&mut a, [0.1, 0.2]), (&mut b, [0.15, 0.15])] {
        for (offset, price) in [2, 5].into_iter().zip(prices) {
            entries[offset].tag = SIGNAL_TAG_F64;
            entries[offset].value = WireSignalValue { f64_: price };
        }
    }
    let rows = [
        (0, Some((0.1 + 0.2) / 2.0), None, 1, None),
        (1, Some((0.15 + 0.15) / 2.0), None, 1, None),
    ];
    for load in [linked as fn(&Dispatcher) -> Plugin<Hook>, dropped] {
        let d = Dispatcher::new(DispatchConfig::default());
        let p = load(&d);
        assert_eq!(open(&p, r#"{"policy":"cheapest"}"#), Outcome::Ready);
        assert_eq!(decide(&p, &rows), (Outcome::Ready, VERB_PREFER, vec![1, 0]));
        assert_eq!(
            decide_with_signals(&p, &rows, &[&a, &b]),
            (Outcome::Ready, VERB_PREFER, vec![1, 0]),
            "the recipe must preserve distinct configured routing prices"
        );
        assert_eq!(
            configure(&p, r#"{"expected_units":{"input":1,"output":0}}"#, 2).0,
            Outcome::Ready
        );
        assert_eq!(
            decide_with_signals(&p, &rows, &[&a, &b]),
            (Outcome::Ready, VERB_PREFER, vec![0, 1]),
            "overrides still evaluate the generic classes"
        );
    }
}

#[test]
fn missing_recipes_never_use_scalar_costs_both_ways() {
    for load in [linked as fn(&Dispatcher) -> Plugin<Hook>, dropped] {
        let d = Dispatcher::new(DispatchConfig::default());
        let p = load(&d);
        assert_eq!(open(&p, r#"{"policy":"cheapest"}"#), Outcome::Ready);
        assert_eq!(
            decide_with_signals(&p, COST_ROWS, &[&[], &[]]),
            (Outcome::Ready, VERB_ABSTAIN, vec![])
        );
        assert_eq!(
            configure(&p, r#"{"expected_units":{"work":0}}"#, 2).0,
            Outcome::Ready
        );
        assert_eq!(
            decide_with_signals(&p, COST_ROWS, &[&[], &[]]),
            (Outcome::Ready, VERB_ABSTAIN, vec![])
        );
    }
}

#[test]
fn cheapest_uses_the_plane_recipe_instead_of_the_legacy_scalar_both_ways() {
    let a = recipe(&[("work", Some(2), 1.0), ("span", Some(10), 1.0)]);
    let b = recipe(&[("work", Some(6), 1.0), ("span", Some(1), 1.0)]);
    // Each strategy orders these differently: the recipe makes idx 1 cheaper (7 versus 12) though
    // its legacy scalar is dearer, idx 0 is faster, and idx 1 has more free concurrency.
    const ROWS: &[Row] = &[
        (0, Some(1.0), Some(10.0), 1, None),
        (1, Some(9.0), Some(900.0), 2, None),
    ];
    for load in [linked as fn(&Dispatcher) -> Plugin<Hook>, dropped] {
        let d = Dispatcher::new(DispatchConfig::default());
        let p = load(&d);
        assert_eq!(open(&p, r#"{"policy":"cheapest"}"#), Outcome::Ready);

        assert_eq!(
            decide_with_signals(&p, ROWS, &[&a, &b]),
            (Outcome::Ready, VERB_PREFER, vec![1, 0])
        );

        assert_eq!(
            configure(&p, r#"{"policy":"fastest"}"#, 2),
            (Outcome::Ready, 2)
        );
        assert_eq!(
            decide_with_signals(&p, ROWS, &[&a, &b]),
            (Outcome::Ready, VERB_PREFER, vec![0, 1])
        );
        assert_eq!(
            configure(&p, r#"{"policy":"least_busy"}"#, 3).0,
            Outcome::Ready
        );
        assert_eq!(
            decide_with_signals(&p, ROWS, &[&a, &b]),
            (Outcome::Ready, VERB_PREFER, vec![1, 0])
        );
    }
}

#[test]
fn per_hook_expected_units_override_and_configure_are_atomic_both_ways() {
    let a = recipe(&[("work", Some(2), 1.0), ("span", Some(10), 1.0)]);
    let b = recipe(&[("work", Some(6), 1.0), ("span", Some(1), 1.0)]);
    for load in [linked as fn(&Dispatcher) -> Plugin<Hook>, dropped] {
        let d = Dispatcher::new(DispatchConfig::default());
        let p = load(&d);
        let other = load(&d);
        assert_eq!(
            open(&p, r#"{"policy":"cheapest","expected_units":{"work":10}}"#),
            Outcome::Ready
        );
        assert_eq!(open(&other, r#"{"policy":"cheapest"}"#), Outcome::Ready);
        let order = || decide_with_signals(&p, COST_ROWS, &[&a, &b]).2;
        assert_eq!(order(), vec![0, 1], "30 versus61 after the override");
        assert_eq!(
            decide_with_signals(&other, COST_ROWS, &[&a, &b]).2,
            vec![1, 0],
            "per-instance profile"
        );
        assert_eq!(
            configure(
                &p,
                r#"{"policy":"fastest","expected_units":{"work":-1}}"#,
                2
            )
            .0,
            Outcome::Failed
        );
        assert_eq!(
            order(),
            vec![0, 1],
            "a NACK commits neither strategy nor profile"
        );
        assert_eq!(
            configure(&p, r#"{"expected_units":{}}"#, 2),
            (Outcome::Ready, 2)
        );
        assert_eq!(
            order(),
            vec![1, 0],
            "an empty override restores plane defaults"
        );
        assert_eq!(
            configure(&p, r#"{"expected_units":{"work":10}}"#, 8),
            (Outcome::Ready, 8)
        );
        assert_eq!(order(), vec![0, 1]);
        assert_eq!(
            configure(&p, r#"{"expected_units":{"work":-1}}"#, 7).0,
            Outcome::Failed
        );
        assert_eq!(
            order(),
            vec![0, 1],
            "stale invalid profile is refused without changing the live profile"
        );
        assert_eq!(configure(&p, "{}", 7), (Outcome::Ready, 7));
        assert_eq!(order(), vec![0, 1], "valid stale partial push is a no-op");
        assert_eq!(
            configure(&p, r#"{"policy":"fastest"}"#, 9),
            (Outcome::Ready, 9)
        );
        assert_eq!(order(), vec![1, 0]);
        assert_eq!(
            configure(&p, r#"{"policy":"cheapest"}"#, 10),
            (Outcome::Ready, 10)
        );
        assert_eq!(
            order(),
            vec![0, 1],
            "an omitted override retains the live profile"
        );
        for settings in [
            r#"{"expected_units":null}"#,
            r#"{"expected_units":[]}"#,
            r#"{"expected_units":{"work":"10"}}"#,
        ] {
            assert_eq!(configure(&p, settings, 11).0, Outcome::Failed, "{settings}");
            assert_eq!(order(), vec![0, 1]);
        }
    }
}

#[test]
fn unknown_empty_malformed_and_overflow_costs_never_become_free_both_ways() {
    let known = recipe(&[("work", Some(7), 1.0)]);
    let mut malformed = recipe(&[("work", Some(1), 1.0)]);
    malformed.pop();
    let mut late_error = recipe(&[("work", Some(1), 1.0)]);
    late_error.extend(recipe(&[]));
    let mut unknown_tag = recipe(&[]);
    unknown_tag[0].tag = u32::MAX;
    for bad in [
        recipe(&[]),
        recipe(&[("work", None, 1.0)]),
        recipe(&[("work", None, 0.0)]),
        recipe(&[("work", Some(u64::MAX), f64::MAX)]),
        recipe(&[("work", Some(1), f64::NAN)]),
        recipe(&[("work", Some(1), -1.0)]),
        malformed,
        late_error,
        unknown_tag,
    ] {
        for load in [linked as fn(&Dispatcher) -> Plugin<Hook>, dropped] {
            let d = Dispatcher::new(DispatchConfig::default());
            let p = load(&d);
            assert_eq!(open(&p, r#"{"policy":"cheapest"}"#), Outcome::Ready);
            assert_eq!(
                decide_with_signals(&p, COST_ROWS, &[&bad, &known]),
                (Outcome::Ready, VERB_PREFER, vec![1, 0])
            );
            assert_eq!(
                decide_with_signals(&p, COST_ROWS, &[&bad, &bad]),
                (Outcome::Ready, VERB_ABSTAIN, vec![])
            );
        }
    }
}

#[test]
fn known_zero_and_duplicate_recipe_terms_keep_their_meaning_both_ways() {
    let zero = recipe(&[("work", Some(0), 5.0)]);
    let duplicate = recipe(&[("work", Some(2), 1.0), ("work", Some(3), 1.0)]);
    let nine = recipe(&[("other", Some(9), 1.0)]);
    for load in [linked as fn(&Dispatcher) -> Plugin<Hook>, dropped] {
        let d = Dispatcher::new(DispatchConfig::default());
        let p = load(&d);
        assert_eq!(open(&p, r#"{"policy":"cheapest"}"#), Outcome::Ready);
        assert_eq!(
            decide_with_signals(&p, COST_ROWS, &[&zero, &nine]),
            (Outcome::Ready, VERB_PREFER, vec![0, 1])
        );
        assert_eq!(
            decide_with_signals(&p, COST_ROWS, &[&duplicate, &nine]),
            (Outcome::Ready, VERB_PREFER, vec![0, 1])
        );
        assert_eq!(
            configure(&p, r#"{"expected_units":{"work":2}}"#, 2).0,
            Outcome::Ready
        );
        assert_eq!(
            decide_with_signals(&p, COST_ROWS, &[&duplicate, &nine]),
            (Outcome::Ready, VERB_PREFER, vec![1, 0]),
            "both duplicate terms use the override:4+6=10"
        );
        let unknown = recipe(&[("work", None, 1.0)]);
        assert_eq!(
            configure(&p, r#"{"expected_units":{"work":0}}"#, 3).0,
            Outcome::Ready
        );
        assert_eq!(
            decide_with_signals(&p, COST_ROWS, &[&unknown, &nine]),
            (Outcome::Ready, VERB_PREFER, vec![1, 0]),
            "an override of zero never invents an unknown price"
        );
        let hundred = recipe(&[("other", Some(100), 1.0)]);
        assert_eq!(
            decide_with_signals(&p, COST_ROWS, &[&hundred, &[]]),
            (Outcome::Ready, VERB_PREFER, vec![0, 1]),
            "a missing generic estimate never falls back to a differently scaled legacy value"
        );
    }
}

/// THE SCRIPT over the door `load` binds, one line per answer: an open that names no strategy,
/// then every strategy word opened and asked to rank every shape.
fn script(load: impl Fn(&Dispatcher) -> Plugin<Hook>) -> Vec<String> {
    let d = Dispatcher::new(DispatchConfig::default());
    let mut lines = vec![format!(
        "open nope: {:?}",
        open(&load(&d), r#"{"policy": "nope"}"#)
    )];
    for word in WORDS {
        let p = load(&d);
        lines.push(format!(
            "open {word}: {:?}",
            open(&p, &format!(r#"{{"policy": "{word}"}}"#))
        ));
        for (n, rows) in SHAPES.iter().enumerate() {
            let (outcome, verbs, order) = decide(&p, rows);
            lines.push(format!(
                "{word} shape {n}: {outcome:?} verbs={verbs:#x} order={order:?}"
            ));
        }
    }
    lines
}

/// The order the pure ranking function gives `word` over `rows`: `Some(order)` for a Prefer, `None`
/// for an Abstain.
fn expected_order(word: &str, rows: &[Row]) -> Option<Vec<usize>> {
    let decoded: Vec<DecodedCandidate<'static>> = rows
        .iter()
        .map(|&(idx, cost, lat, conc, rate)| DecodedCandidate {
            idx,
            model: "m".into(),
            provider: "p".into(),
            weight: 1,
            context_max: None,
            tier: None,
            cost_per_mtok: cost,
            tags: Vec::new(),
            latency_ms: lat,
            available_concurrency: conc,
            budget_remaining: None,
            rate_headroom: rate,
            signals: cost_signal(cost),
        })
        .collect();
    match busbar_hook_ranking::rank(word, &decoded) {
        Some(Verdict::Prefer(order)) => Some(order),
        None | Some(Verdict::Abstain) => None,
        Some(other) => panic!("rank answered {other:?}"),
    }
}

#[test]
fn the_linked_and_the_dropped_in_door_rank_every_shape_the_same() {
    let linked = script(linked);
    assert_eq!(
        linked[0], "open nope: Failed",
        "a word no strategy spells opens nothing"
    );
    // Every word's open -> decide wiring is pinned to the pure ranking function, so a door that
    // wired a word to the wrong strategy (the same in both doors) cannot pass on parity alone.
    let verbs_of = |line: &str| -> String {
        let at = line.find("verbs=").expect("verbs in line") + "verbs=".len();
        line[at..].split(' ').next().unwrap().to_string()
    };
    let (mut abstain_verbs, mut prefer_verbs) = (Vec::new(), Vec::new());
    for word in WORDS {
        for (n, rows) in SHAPES.iter().enumerate() {
            let want = expected_order(word, rows);
            let prefix = format!("{word} shape {n}: ");
            let line = linked
                .iter()
                .find(|l| l.starts_with(&prefix))
                .unwrap_or_else(|| panic!("no line for {prefix}: {linked:#?}"));
            let order = want.clone().unwrap_or_default();
            assert!(
                line.ends_with(&format!("order={order:?}")),
                "{line} (want order={order:?})"
            );
            match want {
                None => abstain_verbs.push(verbs_of(line)),
                Some(_) => prefer_verbs.push(verbs_of(line)),
            }
        }
    }
    // An Abstain and a Prefer are told apart by the verbs the door writes, not by the order alone:
    // a door answering Prefer([]) where rank Abstains must fail here.
    assert!(!abstain_verbs.is_empty(), "no shape abstains");
    assert!(!prefer_verbs.is_empty(), "no shape prefers");
    assert!(
        abstain_verbs.iter().all(|v| *v == abstain_verbs[0]),
        "every Abstain writes the same verbs: {abstain_verbs:?}"
    );
    assert!(
        prefer_verbs.iter().all(|v| *v != abstain_verbs[0]),
        "a Prefer writes verbs unlike an Abstain ({abstain_verbs:?}): {prefer_verbs:?}"
    );
    assert_eq!(script(dropped), linked, "the dropped-in door");
}

/// Absolute results through each loading path: agreement alone could preserve the same wrong
/// direction in both doors. These expectations include absent signals, NaN, ties and empty pools.
#[test]
fn every_strategy_has_its_expected_orders_through_both_doors() {
    const EXPECTED: [(&str, [&[usize]; 5]); 4] = [
        ("cheapest", [&[1, 0, 2], &[], &[1, 0], &[0], &[]]),
        ("fastest", [&[1, 0, 2], &[], &[0, 1], &[0], &[]]),
        ("least_busy", [&[1, 2, 0], &[0, 1], &[0, 1], &[0], &[]]),
        ("usage", [&[1, 0, 2], &[], &[0, 1], &[0], &[]]),
    ];
    for load in [linked as fn(&Dispatcher) -> Plugin<Hook>, dropped] {
        let d = Dispatcher::new(DispatchConfig::default());
        for (word, orders) in EXPECTED {
            let p = load(&d);
            assert_eq!(
                open(&p, &format!(r#"{{"policy":"{word}"}}"#)),
                Outcome::Ready,
                "{word} opens"
            );
            for (shape, (rows, order)) in SHAPES.iter().zip(orders).enumerate() {
                let verb = if order.is_empty() {
                    VERB_ABSTAIN
                } else {
                    VERB_PREFER
                };
                assert_eq!(
                    decide(&p, rows),
                    (Outcome::Ready, verb, order.to_vec()),
                    "{word}, shape {shape}"
                );
            }
        }
    }
}

/// The ranking door with its verdict REVERSED: the same Statement, the same strategies, every
/// preferred order read back to front. The red arm's door.
mod reversed {
    use super::{Decoded, HookImpl, HookOpen, Op, Poll, Verdict};

    struct Reversed(Box<dyn HookImpl>);

    impl HookImpl for Reversed {
        fn decide(&self, view: &Decoded<'_>, op: &Op<'_>) -> Poll<Verdict> {
            self.0.decide(view, op).map(|v| match v {
                Verdict::Prefer(mut order) => {
                    order.reverse();
                    Verdict::Prefer(order)
                }
                other => other,
            })
        }
    }

    pub struct Open;

    impl HookOpen for Open {
        fn open(settings: &str) -> Result<Box<dyn HookImpl>, String> {
            busbar_hook_ranking::door::Open::open(settings)
                .map(|h| Box::new(Reversed(h)) as Box<dyn HookImpl>)
        }
    }

    busbar_contract::hook_door! {
        open: Open,
        statement: busbar_hook_ranking::door::STATEMENT,
    }
}

#[test]
fn red_a_door_that_ranks_the_other_way_answers_differently() {
    let honest = script(linked);
    let red = script(|d| linked_row(reversed::door, d));
    assert_eq!(red.len(), honest.len());
    let differ: Vec<(&String, &String)> = honest.iter().zip(&red).filter(|(a, b)| a != b).collect();
    assert!(
        !differ.is_empty(),
        "a reversed ranking must answer differently"
    );
    assert!(
        differ.iter().all(|(a, _)| a.contains(" shape ")),
        "the two differ at the orders, nowhere else: {differ:#?}"
    );
}

#[test]
fn every_strategy_word_answers_decide_ready_on_the_first_poll_never_pending() {
    for load in [linked as fn(&Dispatcher) -> Plugin<Hook>, dropped] {
        let d = Dispatcher::new(DispatchConfig::default());
        for word in WORDS {
            let p = load(&d);
            assert_eq!(
                open(&p, &format!(r#"{{"policy": "{word}"}}"#)),
                Outcome::Ready,
                "{word}"
            );
            for rows in SHAPES {
                let (outcome, ..) = decide(&p, rows);
                assert_eq!(
                    outcome,
                    Outcome::Ready,
                    "`{word}` must answer READY on its first poll (pure compute), never \
                     {outcome:?}, over {} candidates",
                    rows.len()
                );
            }
        }
    }
}

/// THE RENDERED STATEMENT CLAIMS THE FOUR WORDS, as hook word marks, in [`WORDS`]' order: the
/// rendering carries the mark count and then, per mark, its class (`MARK_WORD_HOOK`), the word's
/// length and the word.
#[test]
fn the_rendered_statement_claims_every_word_as_a_hook_mark() {
    use busbar_contract::abi::mechanism::door::MARK_WORD_HOOK;
    let rendering = rendering_of(busbar_hook_ranking::linked::door).expect("the door renders");
    let mut want = Vec::new();
    want.extend_from_slice(&(WORDS.len() as u32).to_le_bytes());
    for w in WORDS {
        want.extend_from_slice(&MARK_WORD_HOOK.to_le_bytes());
        want.extend_from_slice(&(w.len() as u32).to_le_bytes());
        want.extend_from_slice(w.as_bytes());
    }
    assert!(
        rendering.windows(want.len()).any(|w| w == want.as_slice()),
        "the word marks are not in the rendering: {:?}",
        String::from_utf8_lossy(&rendering)
    );
}

fn json(text: &str) -> Blob {
    Blob {
        ptr: text.as_ptr(),
        len: text.len(),
        fmt: BLOB_JSON,
        flags: 0,
    }
}

fn refresh(p: &Plugin<Hook>, settings: &str) -> busbar_plugin_loader::dispatch::Called {
    use busbar_contract::abi::mechanism::lifecycle::RefreshIn;
    let mut f = Frame::new(
        RefreshIn {
            head: in_head(),
            generation: 2,
            settings: json(settings),
            secrets: std::ptr::null(),
            secrets_len: 0,
        },
        out_head(),
    );
    checked_call(p, life::REFRESH, &mut f)
}

// This is the actual host view and its host-owned buffers, submitted with custody on a ticket.
// A short answer is resubmitted once on that same ticket, as production drives the hook.
fn ticketed_decide(
    p: &Plugin<Hook>,
    d: &Dispatcher,
    rows: &[Row],
    cap: usize,
    short_again: bool,
    expected_entries: u64,
) -> (Outcome, u32, Vec<usize>) {
    use busbar_contract::abi::host::hook::Caps;
    use busbar_contract::abi::mechanism::call::DeadlineClass;
    use std::sync::atomic::Ordering::SeqCst;
    use std::time::Duration;
    let view = DecideView::build(&request(), &candidates(rows), &ctx());
    let caps = Caps {
        order: cap,
        ..Caps::initial(&view)
    };
    let first = DecideFrame::new(Arc::clone(&view), caps);
    let ticket = d.mint(0).expect("an actual ranking ticket");
    let entered = p.inner.crossings.load(SeqCst);
    let resumed = p.inner.resumes.load(SeqCst);
    let deadline = busbar_plugin_loader::dispatch::now_ns().saturating_add(10_000_000_000);
    let mut done = d
        .submit_watched(
            p,
            ticket,
            slot::DECIDE,
            Frame::new(first.input(), blank_out::<DecideOut>()),
            DeadlineClass::Call,
            deadline,
            Arc::clone(&first) as busbar_plugin_loader::dispatch::Lent,
            Duration::from_secs(10),
        )
        .wait_done();
    let mut owned = first;
    if done.short {
        assert_eq!(done.outcome, Outcome::Failed);
        let out = done.frame.as_ref().expect("short frame").out;
        assert_eq!(out.order_needed, rows.len());
        assert_eq!(out.order_written, 0);
        owned = if short_again {
            DecideFrame::new(Arc::clone(&view), caps)
        } else {
            owned.regrown_decide(&out)
        };
        done = d
            .submit_watched(
                p,
                ticket,
                slot::DECIDE,
                Frame::new(owned.input(), blank_out::<DecideOut>()),
                DeadlineClass::Call,
                deadline,
                Arc::clone(&owned) as busbar_plugin_loader::dispatch::Lent,
                Duration::from_secs(10),
            )
            .wait_done();
        if short_again {
            assert_eq!(
                done.outcome,
                Outcome::Fault,
                "a second short response faults"
            );
        }
    }
    d.recycle(ticket);
    assert_eq!(p.inner.crossings.load(SeqCst) - entered, expected_entries);
    assert_eq!(p.inner.resumes.load(SeqCst) - resumed, 0);
    assert_eq!(done.lease, 0);
    match done.frame {
        Some(frame) => (
            done.outcome,
            frame.out.verbs,
            owned.order(frame.out.order_written),
        ),
        None => {
            assert_ne!(
                done.outcome,
                Outcome::Ready,
                "Ready returns its output frame"
            );
            (done.outcome, 0, Vec::new())
        }
    }
}

#[test]
fn ordinary_lifecycle_and_management_preserve_the_published_ranking_contract() {
    use busbar_contract::abi::hook::{DescribeOut, StatusOut, TransformOut};
    use busbar_contract::abi::host::hook::NotifyFrame;
    use busbar_contract::abi::mechanism::call::{DeadlineClass, InHead, OutHead};
    use busbar_contract::abi::mechanism::lifecycle::{TickIn, TickOut, ValidateIn};
    use std::sync::atomic::Ordering::SeqCst;
    use std::time::Duration;
    const ROWS: &[Row] = &[
        (3, Some(5.0), Some(200.0), 2, Some(0.2)),
        (9, Some(2.0), Some(50.0), 1, None),
        (7, None, Some(90.0), 4, Some(0.8)),
    ];
    for load in [linked as fn(&Dispatcher) -> Plugin<Hook>, dropped] {
        let d = Dispatcher::new(DispatchConfig::default());
        let p = load(&d);
        let before_facts = p.inner.crossings.load(SeqCst);
        assert_eq!(p.kind(), busbar_contract::abi::mechanism::KindCode::Hook);
        assert_eq!(p.max_inflight(), 64);
        assert!(!p.name().is_empty());
        assert_eq!(p.inner.crossings.load(SeqCst), before_facts);
        const NOT_AN_OBJECT: &str = "settings: must be a JSON object";
        let bad_settings = [
            (
                "not json",
                NOT_AN_OBJECT,
                "ranking: the settings are not JSON",
            ),
            (
                r#"["cheapest"]"#,
                NOT_AN_OBJECT,
                "ranking: the settings must be an object {\"policy\": \"<word>\"}, optionally with `expected_units`; policy is one of [",
            ),
            (
                "null",
                NOT_AN_OBJECT,
                "ranking: the settings must be an object {\"policy\": \"<word>\"}, optionally with `expected_units`; policy is one of [",
            ),
            (
                r#"{"policy":"bogus"}"#,
                "ranking: no strategy 'bogus'",
                "ranking: no strategy 'bogus'",
            ),
            (
                r#"{"policy":"cheapest","x":1}"#,
                "ranking: unknown setting 'x'",
                "ranking: unknown setting 'x'",
            ),
        ];
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("conformance.json")).unwrap();
        assert_eq!(
            fixture["hook"]["bad_settings"],
            serde_json::json!(bad_settings.map(|(bad, _, _)| bad)),
            "conformance.json's bad settings are the ones exercised here"
        );
        for (bad, validate_why, open_why) in bad_settings {
            let mut reason = [0u8; 1024];
            let mut frame: Frame<ValidateIn, OutHead> = Frame::new(
                ValidateIn {
                    head: in_head(),
                    settings: json(bad),
                    err_buf: reason.as_mut_ptr(),
                    err_cap: reason.len(),
                },
                out_head(),
            );
            let validation = checked_call(&p, life::VALIDATE, &mut frame);
            assert_eq!(validation.outcome, Outcome::Failed);
            assert_eq!(validation.lease, 0);
            let why = String::from_utf8_lossy(validation.error.as_deref().unwrap_or_default());
            assert!(why.contains(validate_why), "validate {bad}: {why}");
            let opened = open_answer(&p, bad);
            assert_eq!(opened.outcome, Outcome::Failed);
            assert_eq!(opened.lease, 0);
            let why = String::from_utf8_lossy(opened.error.as_deref().unwrap_or_default());
            assert!(why.contains(open_why), "open {bad}: {why}");
        }
        assert_ne!(ticketed_decide(&p, &d, ROWS, 2, false, 0).0, Outcome::Ready);
        let mut valid: Frame<ValidateIn, OutHead> = Frame::new(
            ValidateIn {
                head: in_head(),
                settings: json(r#"{"policy":"cheapest"}"#),
                err_buf: std::ptr::null_mut(),
                err_cap: 0,
            },
            out_head(),
        );
        assert_eq!(
            checked_call(&p, life::VALIDATE, &mut valid).outcome,
            Outcome::Ready
        );
        assert_eq!(open(&p, r#"{"policy":"cheapest"}"#), Outcome::Ready);
        let before_ready = p.inner.crossings.load(SeqCst);
        p.ready(&d, Duration::from_secs(30))
            .expect("actual ranking door ready");
        assert_eq!(
            p.inner.crossings.load(SeqCst) - before_ready,
            u64::from(p.has_ready())
        );
        assert_eq!(
            ticketed_decide(&p, &d, ROWS, 2, false, 2),
            (Outcome::Ready, VERB_PREFER, vec![9, 3, 7])
        );
        assert_eq!(
            ticketed_decide(
                &p,
                &d,
                &[
                    (3, None, Some(200.0), 2, None),
                    (9, None, Some(50.0), 1, None)
                ],
                2,
                false,
                1
            ),
            (Outcome::Ready, VERB_ABSTAIN, vec![])
        );
        assert_eq!(
            ticketed_decide(&p, &d, &ROWS[..2], 2, false, 1),
            (Outcome::Ready, VERB_PREFER, vec![9, 3])
        );
        let transform_view = DecideFrame::first(DecideView::build(&request(), &[], &ctx()));
        let ticket = d.mint(0).unwrap();
        let entered = p.inner.crossings.load(SeqCst);
        let done = d
            .submit_watched(
                &p,
                ticket,
                slot::TRANSFORM,
                Frame::new(transform_view.input(), blank_out::<TransformOut>()),
                DeadlineClass::Call,
                busbar_plugin_loader::dispatch::now_ns().saturating_add(10_000_000_000),
                Arc::clone(&transform_view) as busbar_plugin_loader::dispatch::Lent,
                Duration::from_secs(10),
            )
            .wait_done();
        d.recycle(ticket);
        assert_eq!(done.outcome, Outcome::Ready);
        assert_eq!(done.lease, 0);
        assert_eq!(done.frame.unwrap().out.verbs, VERB_ABSTAIN);
        assert_eq!(p.inner.crossings.load(SeqCst) - entered, 1);
        assert_eq!(
            ticketed_decide(&p, &d, ROWS, 2, false, 2),
            (Outcome::Ready, VERB_PREFER, vec![9, 3, 7])
        );
        assert_eq!(
            configure(&p, r#"{"policy":"cheapest"}"#, 2),
            (Outcome::Ready, 2)
        );
        let notify = NotifyFrame::build(&request(), None, false);
        let ticket = d.mint(0).unwrap();
        let entered = p.inner.crossings.load(SeqCst);
        let frame: Frame<_, OutHead> = Frame::new(notify.input(), out_head());
        let done = d
            .submit_watched(
                &p,
                ticket,
                slot::NOTIFY,
                frame,
                DeadlineClass::Call,
                busbar_plugin_loader::dispatch::now_ns().saturating_add(10_000_000_000),
                Arc::clone(&notify) as busbar_plugin_loader::dispatch::Lent,
                Duration::from_secs(10),
            )
            .wait_done();
        d.recycle(ticket);
        assert_eq!(done.outcome, Outcome::Ready);
        assert_eq!(done.lease, 0);
        assert_eq!(p.inner.crossings.load(SeqCst) - entered, 1);
        for operation in [slot::STATUS, slot::DESCRIBE] {
            let (called, blob) = if operation == slot::STATUS {
                let mut f: Frame<InHead, StatusOut> = Frame::new(in_head(), blank_out());
                let called = checked_call(&p, operation, &mut f);
                (called, f.out.status)
            } else {
                let mut f: Frame<InHead, DescribeOut> = Frame::new(in_head(), blank_out());
                let called = checked_call(&p, operation, &mut f);
                (called, f.out.describe)
            };
            assert_eq!(called.outcome, Outcome::Ready);
            assert_ne!(called.lease, 0);
            assert_eq!(blob.fmt, BLOB_JSON);
            assert!(!blob.ptr.is_null() && blob.len > 0);
            // SAFETY: the loader validated the real output range, held by its unreleased lease.
            let bytes = unsafe { std::slice::from_raw_parts(blob.ptr, blob.len) };
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(bytes)
                    .expect("management answer is JSON"),
                serde_json::json!({}),
                "status and describe answer {{}}"
            );
            let mut release = Frame::new(
                ReleaseIn {
                    head: in_head(),
                    lease: called.lease,
                },
                out_head(),
            );
            assert_eq!(
                checked_call(&p, life::RELEASE, &mut release).outcome,
                Outcome::Ready
            );
        }
        assert_eq!(ticketed_decide(&p, &d, ROWS, 2, true, 2).0, Outcome::Fault);
        let refused = refresh(&p, "not json");
        assert_eq!(refused.outcome, Outcome::Failed);
        assert_eq!(refused.lease, 0);
        assert_eq!(
            refresh(&p, r#"{"policy":"fastest"}"#).outcome,
            Outcome::Ready
        );
        let mut tick = Frame::new(
            TickIn {
                head: in_head(),
                now_ns: 1,
            },
            TickOut {
                head: out_head(),
                next_tick_ns: 7,
            },
        );
        assert_eq!(
            checked_call(&p, life::TICK, &mut tick).outcome,
            Outcome::Ready
        );
        assert_eq!(
            ticketed_decide(&p, &d, ROWS, 2, false, 2),
            (Outcome::Ready, VERB_PREFER, vec![9, 7, 3])
        );
        let mut close: Frame<InHead, OutHead> = Frame::new(in_head(), out_head());
        assert_eq!(
            checked_call(&p, life::CLOSE, &mut close).outcome,
            Outcome::Ready
        );
        assert_ne!(ticketed_decide(&p, &d, ROWS, 2, false, 0).0, Outcome::Ready);
        assert_eq!(p.inner.resumes.load(SeqCst), 0);
        assert_eq!(d.stats().deadline_without_wake, 0);
    }
}

#[test]
fn declared_needs_and_requested_profile_match_the_actual_ranking_image() {
    assert!(
        !(cfg!(debug_assertions) && std::env::var_os("BUSBAR_EXPECT_RELEASE").is_some()),
        "BUSBAR_EXPECT_RELEASE requires cargo test --release"
    );
    let declared: serde_json::Value =
        serde_json::from_str(include_str!("../../hook-ranking/declares.json")).unwrap();
    assert!(declared
        .get("needs")
        .is_none_or(|needs| needs.as_array().is_some_and(Vec::is_empty)));
    let row = LinkedRow::of(busbar_hook_ranking::linked::door).expect("actual ranking Statement");
    assert!(
        busbar_contract::abi::mechanism::rendering::read(&row.statement)
            .unwrap()
            .needs
            .is_empty()
    );
    // The dropped image is admitted only against the linked rendering, so it states the same needs.
    let d = Dispatcher::new(DispatchConfig::default());
    for p in [linked(&d), dropped(&d)] {
        assert_eq!(p.kind(), busbar_contract::abi::mechanism::KindCode::Hook);
    }
}

fn different_valid_statement(statement: &[u8]) -> Vec<u8> {
    let version = busbar_contract::abi::mechanism::rendering::read(statement)
        .unwrap()
        .version;
    assert!(!version.is_empty());
    let at = statement
        .windows(version.len())
        .position(|bytes| bytes == version.as_bytes())
        .expect("the actual version is present in its rendering");
    let mut changed = statement.to_vec();
    changed[at] = if changed[at] == b'9' { b'8' } else { b'9' };
    busbar_contract::abi::mechanism::rendering::read(&changed)
        .expect("a changed version remains a valid Statement rendering");
    changed
}

// These negative inputs retain this product's actual Statement and operation table. Only the
// numerical ABI/layout declaration or version string is changed; no plugin behavior is copied.
extern "C" fn metadata_door<const CASE: usize>(
) -> *const busbar_contract::abi::mechanism::door::Door {
    use busbar_contract::abi::mechanism::door::{Door, Statement};
    static DOORS: [std::sync::OnceLock<usize>; 5] = [const { std::sync::OnceLock::new() }; 5];
    *DOORS[CASE].get_or_init(|| {
        let mut parts = busbar_hook_ranking::linked::door().parts();
        let kind = busbar_contract::abi::mechanism::KindCode::Hook;
        match CASE {
            0 => parts.kind_abi = kind.abi_version() + 1,
            1 => parts.kind_abi = kind.abi_version().wrapping_sub(1),
            2 => parts.layout = kind.layout_fingerprint() ^ 1,
            3 => parts.layout = 0,
            4 => {
                // SAFETY: the actual product's immutable Statement and all nested ranges are static.
                let statement = unsafe { parts.statement.read_unaligned() };
                const OTHER: &str = "0.0.0-conformance-restated";
                parts.statement = Box::leak(Box::new(Statement {
                    version: busbar_contract::abi::mechanism::call::AbiStr {
                        ptr: OTHER.as_ptr(),
                        len: OTHER.len(),
                    },
                    ..statement
                }));
            }
            _ => unreachable!(),
        }
        // SAFETY: the actual product's full table and nested Statement ranges remain immutable
        // and static. Changed metadata is deliberately invalid and judged before any operation.
        let door = unsafe { Door::__from_parts(parts) };
        Box::into_raw(Box::new(door)) as usize
    }) as *const Door
}

fn metadata_image() -> std::path::PathBuf {
    let image = std::path::PathBuf::from(
        std::env::var_os("BUSBAR_PLUGIN_CDYLIB")
            .expect("BUSBAR_PLUGIN_CDYLIB must name the freshly built actual image"),
    );
    assert!(image.is_absolute() && image.is_file());
    image
}

#[test]
fn the_actual_kind_abi_is_checked_by_pack_linked_and_dropped_origins() {
    use busbar_contract::abi::sdk::door::RawDoorFn;
    use busbar_plugin_loader::dispatch::LoadError;
    let kind = busbar_contract::abi::mechanism::KindCode::Hook;
    let host = kind.abi_version();
    assert_eq!(busbar_hook_ranking::linked::door().parts().kind_abi, host);
    let d = Dispatcher::new(Default::default());
    let honest = LinkedRow::of(busbar_hook_ranking::linked::door).expect("actual Statement");
    let _ = load_linked::<Hook>(&honest, bind(&d)).expect("honest linked twin");
    let _ = load_dropped::<Hook>(&metadata_image(), &honest.statement, bind(&d))
        .expect("honest dropped twin");
    for (door, abi) in [
        (metadata_door::<0> as RawDoorFn, host + 1),
        (metadata_door::<1>, host.wrapping_sub(1)),
    ] {
        let want = LoadError::KindAbi {
            kind,
            door: abi,
            host,
        };
        let packed = busbar_plugin_loader::dispatch::load::rendering_of_raw(door)
            .expect_err("pack refuses another ABI");
        assert_eq!(packed, want);
        let linked = LinkedRow::of_raw(door)
            .and_then(|row| load_linked::<Hook>(&row, bind(&d)).map(|_| ()))
            .expect_err("linked refuses another ABI");
        assert_eq!(linked, want);
        assert!(linked.to_string().contains("rebuild"));
        let mut statement = honest.statement.clone();
        let at = busbar_contract::abi::mechanism::rendering::RENDERING_MAGIC.len() + 8;
        statement[at..at + 4].copy_from_slice(&abi.to_le_bytes());
        assert_eq!(
            load_dropped::<Hook>(&metadata_image(), &statement, bind(&d))
                .map(|_| ())
                .err(),
            Some(LoadError::ManifestKindAbi { stated: abi, host })
        );
    }
}

#[test]
fn the_actual_kind_layout_is_checked_under_honest_and_own_rows() {
    use busbar_contract::abi::sdk::door::RawDoorFn;
    use busbar_plugin_loader::dispatch::LoadError;
    let kind = busbar_contract::abi::mechanism::KindCode::Hook;
    let host = kind.layout_fingerprint();
    assert_eq!(busbar_hook_ranking::linked::door().parts().layout, host);
    let d = Dispatcher::new(Default::default());
    let honest = LinkedRow::of(busbar_hook_ranking::linked::door).unwrap();
    let _ = load_linked::<Hook>(&honest, bind(&d)).unwrap();
    let _ = load_dropped::<Hook>(&metadata_image(), &honest.statement, bind(&d)).unwrap();
    for (door, stated) in [
        (metadata_door::<2> as RawDoorFn, host ^ 1),
        (metadata_door::<3>, 0),
    ] {
        let refusal = busbar_plugin_loader::dispatch::load::rendering_of_raw(door)
            .expect_err("pack refuses incompatible layout");
        let LoadError::KindLayout {
            kind: named,
            stated: said,
            host: read,
            ..
        } = &refusal
        else {
            panic!("{refusal:?}")
        };
        assert_eq!((*named, *said, *read), (kind, stated, host));
        assert!(refusal
            .to_string()
            .contains("rebuild against this busbar-contract"));
        let row = LinkedRow {
            statement: honest.statement.clone(),
            door,
        };
        assert_eq!(
            load_linked::<Hook>(&row, bind(&d)).map(|_| ()).err(),
            Some(refusal.clone())
        );
        assert_eq!(
            LinkedRow::of_raw(door)
                .and_then(|row| load_linked::<Hook>(&row, bind(&d)).map(|_| ()))
                .err(),
            Some(refusal)
        );
    }
}

#[test]
fn the_actual_statement_matches_each_row_manifest_and_restated_version() {
    use busbar_plugin_loader::dispatch::LoadError;
    let d = Dispatcher::new(Default::default());
    let honest = LinkedRow::of(busbar_hook_ranking::linked::door).unwrap();
    let _ = load_linked::<Hook>(&honest, bind(&d)).unwrap();
    let _ = load_dropped::<Hook>(&metadata_image(), &honest.statement, bind(&d)).unwrap();
    let changed = different_valid_statement(&honest.statement);
    let row = LinkedRow {
        statement: changed.clone(),
        door: honest.door,
    };
    assert_eq!(
        load_linked::<Hook>(&row, bind(&d)).map(|_| ()).err(),
        Some(LoadError::StatementMismatch)
    );
    assert_eq!(
        load_dropped::<Hook>(&metadata_image(), &changed, bind(&d))
            .map(|_| ())
            .err(),
        Some(LoadError::StatementMismatch)
    );
    let row = LinkedRow {
        statement: honest.statement,
        door: metadata_door::<4>,
    };
    assert_eq!(
        load_linked::<Hook>(&row, bind(&d)).map(|_| ()).err(),
        Some(LoadError::StatementMismatch)
    );
}

fn other_kind_is_refused<K: busbar_plugin_loader::dispatch::Kind>() {
    use busbar_plugin_loader::dispatch::LoadError;
    let d = Dispatcher::new(Default::default());
    let row = LinkedRow::of(busbar_hook_ranking::linked::door).unwrap();
    let want = LoadError::ManifestKind {
        stated: busbar_contract::abi::mechanism::KindCode::Hook,
        want: K::CODE,
    };
    assert_eq!(
        load_linked::<K>(&row, bind(&d)).map(|_| ()).err(),
        Some(want.clone())
    );
    assert_eq!(
        load_dropped::<K>(&metadata_image(), &row.statement, bind(&d))
            .map(|_| ())
            .err(),
        Some(want)
    );
}

#[test]
fn the_actual_image_is_refused_as_every_other_kind_through_both_origins() {
    other_kind_is_refused::<busbar_plugin_loader::dispatch::kinds::auth::Auth>();
    other_kind_is_refused::<busbar_plugin_loader::dispatch::kinds::export::Export>();
    other_kind_is_refused::<busbar_plugin_loader::dispatch::kinds::plane::Plane>();
    other_kind_is_refused::<busbar_plugin_loader::dispatch::kinds::secret::Secret>();
    other_kind_is_refused::<busbar_plugin_loader::dispatch::kinds::store::Store>();
    other_kind_is_refused::<busbar_plugin_loader::dispatch::kinds::transport::Transport>();
}
