// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! Tests for `crates/hooks-ranking/src/lib.rs`.

use super::*;

fn cand(
    idx: usize,
    cost: Option<f64>,
    lat: Option<f64>,
    conc: usize,
    budget: Option<i64>,
) -> DecodedCandidate<'static> {
    // Most native tests don't exercise the `usage` signal; default `rate_headroom` to `None`.
    // The `usage` tests build candidates with `cand_rate` to set it explicitly.
    cand_rate(idx, cost, lat, conc, budget, None)
}

fn cand_rate(
    idx: usize,
    cost: Option<f64>,
    lat: Option<f64>,
    conc: usize,
    budget: Option<i64>,
    rate: Option<f64>,
) -> DecodedCandidate<'static> {
    DecodedCandidate {
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
        budget_remaining: budget,
        rate_headroom: rate,
        signals: Default::default(),
    }
}

#[test]
fn cheapest_orders_by_cost_demoting_unknown() {
    let cands = [
        cand(0, Some(15.0), None, 1, None),
        cand(1, Some(3.0), None, 1, None),
        cand(2, None, None, 1, None), // no cost -> demoted to last
    ];
    let d = rank("cheapest", &cands).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![1, 0, 2]));
}

#[test]
fn cheapest_all_unknown_abstains() {
    let cands = [cand(0, None, None, 1, None), cand(1, None, None, 1, None)];
    let d = rank("cheapest", &cands).unwrap();
    assert_eq!(d, Verdict::Abstain);
}

#[test]
fn fastest_orders_by_latency() {
    let cands = [
        cand(0, None, Some(120.0), 1, None),
        cand(1, None, Some(40.0), 1, None),
        cand(2, None, Some(80.0), 1, None),
    ];
    let d = rank("fastest", &cands).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![1, 2, 0]));
}

#[test]
fn least_busy_prefers_most_headroom() {
    let cands = [
        cand(0, None, None, 2, None),
        cand(1, None, None, 9, None),
        cand(2, None, None, 5, None),
    ];
    let d = rank("least_busy", &cands).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![1, 2, 0]));
}

/// `usage` ranks DESCENDING by `rate_headroom` (most rate-limit headroom first), demoting a
/// candidate with no headroom signal (`None`) to last but keeping it reachable.
#[test]
fn usage_orders_by_rate_headroom_demoting_unknown() {
    let cands = [
        cand_rate(0, None, None, 1, None, Some(0.10)), // nearly at the cap
        cand_rate(1, None, None, 1, None, Some(0.90)), // most headroom
        cand_rate(2, None, None, 1, None, None),       // no signal -> demoted to last
        cand_rate(3, None, None, 1, None, Some(0.50)),
    ];
    let d = rank("usage", &cands).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![1, 3, 0, 2]));
}

/// `usage` Abstains when EVERY candidate lacks the rate-headroom signal (no rate limit in play),
/// so selection falls through to the default SWRR.
#[test]
fn usage_all_unknown_abstains() {
    let cands = [
        cand_rate(0, None, None, 1, Some(100), None),
        cand_rate(1, None, None, 1, None, None),
        cand_rate(2, None, None, 1, Some(5000), None),
    ];
    let d = rank("usage", &cands).unwrap();
    assert_eq!(d, Verdict::Abstain);
}

/// Every word this plugin claims ranks; anything else, `weighted` included, is not a strategy of this
/// plugin.
#[test]
fn rank_knows_its_words_and_no_others() {
    let one = [cand(0, Some(1.0), Some(1.0), 1, None)];
    assert!(rank("weighted", &one).is_none());
    for word in WORDS {
        assert!(rank(word, &one).is_some(), "{word}");
    }
    assert!(rank("nonexistent", &one).is_none());
}

/// The claimed words are exactly the four 1.5.5 strategy spellings.
#[test]
fn the_claimed_words_are_the_four_strategies() {
    assert_eq!(WORDS, ["cheapest", "fastest", "least_busy", "usage"]);
}

// ── edge cases: empty pool / single candidate / all-saturated / all-unknown ──────────────────

/// An empty candidate pool yields `Abstain` for every native (no candidates → no opinion → SWRR).
#[test]
fn all_natives_abstain_on_empty_pool() {
    let empty: [DecodedCandidate<'_>; 0] = [];
    for name in ["cheapest", "fastest", "least_busy", "usage"] {
        let d = rank(name, &empty).unwrap();
        assert_eq!(
            d,
            Verdict::Abstain,
            "{name} must Abstain on an empty candidate pool"
        );
    }
}

/// A single candidate carrying the relevant signal ranks to a one-element `Prefer` for the
/// signal-bearing natives; the always-present `least_busy` likewise prefers it.
#[test]
fn single_candidate_prefers_it() {
    // cheapest: one candidate with a cost.
    let d = rank("cheapest", &[cand(0, Some(5.0), None, 1, None)]).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![0]));
    // fastest: one candidate with a latency sample.
    let d = rank("fastest", &[cand(0, None, Some(30.0), 1, None)]).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![0]));
    // least_busy: always has data.
    let d = rank("least_busy", &[cand(0, None, None, 3, None)]).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![0]));
    // usage: one candidate with rate headroom.
    let d = rank("usage", &[cand_rate(0, None, None, 1, None, Some(0.5))]).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![0]));
}

/// `least_busy` with EVERY candidate saturated (`available_concurrency == 0`) does NOT Abstain —
/// the concurrency signal is always present, so it ranks all lanes (all-zero → ordered by `idx`).
/// The ordered walk + breaker machinery downstream is what skips a truly-unusable lane; the native
/// only ORDERS and must never strand a lane by dropping it.
#[test]
fn least_busy_all_saturated_ranks_by_idx() {
    let cands = [
        cand(0, None, None, 0, None),
        cand(1, None, None, 0, None),
        cand(2, None, None, 0, None),
    ];
    let d = rank("least_busy", &cands).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![0, 1, 2]));
}

/// `usage` with EVERY candidate at the cap (`rate_headroom == Some(0.0)`) still ranks (the signal
/// is present on all), tie-breaking by `idx` — it does NOT Abstain (Abstain is only for all-`None`).
#[test]
fn usage_all_at_cap_ranks_by_idx() {
    let cands = [
        cand_rate(0, None, None, 1, None, Some(0.0)),
        cand_rate(1, None, None, 1, None, Some(0.0)),
    ];
    let d = rank("usage", &cands).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![0, 1]));
}

/// `cheapest` with EVERY candidate at weight-0 / no declared cost Abstains (no cost signal at all
/// → fall through to SWRR). Weight is irrelevant to `cheapest`; it ranks on cost.
#[test]
fn cheapest_all_weight_zero_no_cost_abstains() {
    let mut a = cand(0, None, None, 1, None);
    a.weight = 0;
    let mut b = cand(1, None, None, 1, None);
    b.weight = 0;
    let d = rank("cheapest", &[a, b]).unwrap();
    assert_eq!(d, Verdict::Abstain);
}

/// `fastest` with EVERY candidate lacking a latency sample Abstains (mirrors `cheapest_all_unknown`).
#[test]
fn fastest_all_unknown_latency_abstains() {
    let cands = [cand(0, None, None, 1, None), cand(1, None, None, 1, None)];
    let d = rank("fastest", &cands).unwrap();
    assert_eq!(d, Verdict::Abstain);
}

/// `fastest` demotes a candidate with no latency sample to last (reachable, not dropped), exactly
/// as `cheapest` demotes a no-cost candidate. Mirrors `cheapest_orders_by_cost_demoting_unknown`.
#[test]
fn fastest_orders_by_latency_demoting_unknown() {
    let cands = [
        cand(0, None, Some(120.0), 1, None),
        cand(1, None, None, 1, None), // no latency sample -> demoted to last
        cand(2, None, Some(40.0), 1, None),
    ];
    let d = rank("fastest", &cands).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![2, 0, 1]));
}

/// A deterministic stream of keys (no randomness crate): `n` draws over finite values, both zeros
/// and both infinities, plus `NaN` and an absent key when `with_nan`.
fn keys(seed: u64, n: usize, with_nan: bool) -> Vec<Option<f64>> {
    let mut x = seed;
    (0..n)
        .map(|_| {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let pick = (x >> 33) % if with_nan { 10 } else { 8 };
            match pick {
                0 => Some(0.0),
                1 => Some(-0.0),
                2 => Some(f64::INFINITY),
                3 => Some(f64::NEG_INFINITY),
                4..=7 => Some(((x >> 40) % 7) as f64 - 3.0),
                8 => Some(f64::NAN),
                _ => None,
            }
        })
        .collect()
}

/// 1.5.5's comparator over `keys`, ascending (v1.5.5 `crates/hooks-ranking/src/lib.rs:76-85`).
fn order_1_5_5(keys: &[Option<f64>], descending: bool) -> Vec<usize> {
    let mut keyed: Vec<(usize, Option<f64>)> = keys.iter().copied().enumerate().collect();
    keyed.sort_by(|(ia, ka), (ib, kb)| match (ka, kb) {
        (Some(a), Some(b)) => if descending {
            b.partial_cmp(a)
        } else {
            a.partial_cmp(b)
        }
        .unwrap_or(std::cmp::Ordering::Equal)
        .then(ia.cmp(ib)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => ia.cmp(ib),
    });
    keyed.into_iter().map(|(i, _)| i).collect()
}

/// PARITY ON EVERY REACHABLE INPUT: over pools with no `NaN` (finite keys, `-0.0` and `+0.0`, both
/// infinities, absent keys), the ranking is 1.5.5's comparator's order, byte for byte, ascending
/// and descending, at every size across the sort's small-slice boundary.
#[test]
fn a_pool_without_nan_ranks_as_1_5_5_ranked_it() {
    for seed in 1..=64_u64 {
        for n in [1, 2, 3, 7, 19, 20, 21, 33, 64, 100] {
            let ks: Vec<Option<f64>> = keys(seed * 977 + n as u64, n, false)
                .into_iter()
                .enumerate()
                .map(|(i, k)| {
                    if (seed + i as u64).is_multiple_of(9) {
                        None
                    } else {
                        k
                    }
                })
                .collect();
            if ks.iter().all(Option::is_none) {
                continue;
            }
            let cost: Vec<_> = ks
                .iter()
                .enumerate()
                .map(|(i, k)| cand(i, *k, None, 1, None))
                .collect();
            assert_eq!(
                rank("cheapest", &cost).unwrap(),
                Verdict::Prefer(order_1_5_5(&ks, false)),
                "ascending, seed {seed}, n {n}: {ks:?}"
            );
            let rate: Vec<_> = ks
                .iter()
                .enumerate()
                .map(|(i, k)| cand_rate(i, None, None, 1, None, *k))
                .collect();
            assert_eq!(
                rank("usage", &rate).unwrap(),
                Verdict::Prefer(order_1_5_5(&ks, true)),
                "descending, seed {seed}, n {n}: {ks:?}"
            );
        }
    }
}

/// Candidates in a NON-monotone idx order (`idx = (i * 7) % n`, `n` coprime to 7), the key of each
/// from `key(idx)`, so only the comparator's idx tie-break, never the input order, can produce a
/// sorted answer.
fn shuffled(n: usize, key: impl Fn(usize) -> Option<f64>) -> Vec<Option<f64>> {
    (0..n).map(|i| key((i * 7) % n)).collect()
}

fn pool_cost(n: usize, key: impl Fn(usize) -> Option<f64>) -> Vec<DecodedCandidate<'static>> {
    let ks = shuffled(n, &key);
    (0..n)
        .map(|i| cand((i * 7) % n, ks[i], None, 1, None))
        .collect()
}

fn pool_rate(n: usize, key: impl Fn(usize) -> Option<f64>) -> Vec<DecodedCandidate<'static>> {
    let ks = shuffled(n, &key);
    (0..n)
        .map(|i| cand_rate((i * 7) % n, None, None, 1, None, ks[i]))
        .collect()
}

/// A `NaN` KEY IS AN ABSENT KEY. `NaN` makes `partial_cmp` partial, so a comparator that let it tie
/// every number is no total order and the standard sort may panic past its insertion-sort range
/// (more than 20 candidates). A `NaN` joins the absent group, ordered by idx behind every number,
/// so the comparator is total. 48 candidates in a non-monotone idx order, `NaN`, absent and finite
/// keys mixed, ascending (`cheapest`) and descending (`usage`), the exact order asserted.
#[test]
fn a_nan_key_ranks_with_the_absent_group() {
    const N: usize = 48;
    // idx % 3 == 0: NaN; idx % 5 == 0 (not 3): absent; else finite, many ties.
    let key = |idx: usize| {
        if idx.is_multiple_of(3) {
            Some(f64::NAN)
        } else if idx.is_multiple_of(5) {
            None
        } else {
            Some((idx % 4) as f64)
        }
    };
    let finite = |idx: usize| !idx.is_multiple_of(3) && !idx.is_multiple_of(5);
    let tail: Vec<usize> = (0..N).filter(|i| !finite(*i)).collect();
    let mut present: Vec<usize> = (0..N).filter(|i| finite(*i)).collect();

    present.sort_by_key(|i| (i % 4, *i));
    let want: Vec<usize> = present
        .iter()
        .copied()
        .chain(tail.iter().copied())
        .collect();
    assert_eq!(
        rank("cheapest", &pool_cost(N, key)).unwrap(),
        Verdict::Prefer(want),
        "ascending"
    );

    present.sort_by_key(|i| (std::cmp::Reverse(i % 4), *i));
    let want: Vec<usize> = present
        .iter()
        .copied()
        .chain(tail.iter().copied())
        .collect();
    assert_eq!(
        rank("usage", &pool_rate(N, key)).unwrap(),
        Verdict::Prefer(want),
        "descending"
    );
}

/// A pool whose only keys are `NaN` states no signal: Abstain, as an all-absent pool does.
#[test]
fn an_all_nan_pool_abstains_as_an_all_absent_one_does() {
    let cands = [
        cand(0, Some(f64::NAN), Some(f64::NAN), 1, None),
        cand(1, Some(f64::NAN), None, 1, None),
    ];
    assert_eq!(rank("cheapest", &cands).unwrap(), Verdict::Abstain);
    assert_eq!(rank("fastest", &cands).unwrap(), Verdict::Abstain);
}

/// THE IDX TIE-BREAK, with candidates handed in NON-idx order: equal keys order by idx, in the
/// ascending and the descending rankings, and the absent group orders by idx, so the answer is the
/// comparator's and not the input's.
#[test]
fn ties_and_the_absent_group_break_by_idx_whatever_the_input_order() {
    // Ascending: idx 4 and 1 tie at 1.0, idx 3 is cheaper, 5 and 0 and 2 are absent.
    let cands = [
        cand(4, Some(1.0), None, 1, None),
        cand(5, None, None, 1, None),
        cand(1, Some(1.0), None, 1, None),
        cand(2, None, None, 1, None),
        cand(3, Some(0.5), None, 1, None),
        cand(0, None, None, 1, None),
    ];
    assert_eq!(
        rank("cheapest", &cands).unwrap(),
        Verdict::Prefer(vec![3, 1, 4, 0, 2, 5])
    );
    // Descending: `usage` (largest first) and `least_busy`, ties and the absent group by idx.
    let rates = [
        cand_rate(4, None, None, 1, None, Some(0.5)),
        cand_rate(5, None, None, 1, None, None),
        cand_rate(1, None, None, 1, None, Some(0.5)),
        cand_rate(2, None, None, 1, None, None),
        cand_rate(3, None, None, 1, None, Some(0.9)),
        cand_rate(0, None, None, 1, None, None),
    ];
    assert_eq!(
        rank("usage", &rates).unwrap(),
        Verdict::Prefer(vec![3, 1, 4, 0, 2, 5])
    );
    let busy = [
        cand(2, None, None, 3, None),
        cand(0, None, None, 3, None),
        cand(1, None, None, 7, None),
    ];
    assert_eq!(
        rank("least_busy", &busy).unwrap(),
        Verdict::Prefer(vec![1, 0, 2])
    );
}

/// AN INFINITE SIGNAL IS ORDERED AS 1.5.5 ORDERED IT. 1.5.5's ranking compared every present key
/// with `partial_cmp`, and an infinity is orderable: `cheapest` put an infinite cost behind every
/// finite one and ahead of a member with no cost, and preferred it over an absent signal rather than
/// abstaining. A port keeps 1.5.5's behaviour (THE DESIGN §2, "A port keeps 1.5.5's surface"); only
/// a NaN, which has no order at all, ranks as absent.
#[test]
fn an_infinite_signal_ranks_as_1_5_5_ranked_it() {
    let cands = [
        cand(0, Some(f64::INFINITY), None, 1, None),
        cand(1, None, None, 1, None),
    ];
    assert_eq!(
        rank("cheapest", &cands).unwrap(),
        Verdict::Prefer(vec![0, 1]),
        "an infinite cost is a cost, not an absent one"
    );
    let cands = [
        cand(0, Some(f64::INFINITY), None, 1, None),
        cand(1, Some(2.0), None, 1, None),
        cand(2, None, None, 1, None),
    ];
    assert_eq!(
        rank("cheapest", &cands).unwrap(),
        Verdict::Prefer(vec![1, 0, 2])
    );
    let cands = [
        cand(0, None, Some(f64::INFINITY), 1, None),
        cand(1, None, None, 1, None),
    ];
    assert_eq!(
        rank("fastest", &cands).unwrap(),
        Verdict::Prefer(vec![0, 1])
    );
}
