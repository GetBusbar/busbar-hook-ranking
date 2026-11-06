// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! Tests for `hook-ranking/src/lib.rs`.

use super::*;

fn cand(
    idx: usize,
    cost: Option<f64>,
    lat: Option<f64>,
    conc: usize,
    budget: Option<i64>,
) -> DecodedCandidate<'static> {
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
        signals: {
            let mut signals = busbar_contract::SignalBag::new();
            route_cost::append(
                &mut signals,
                [route_cost::Term {
                    class: "work",
                    price_per_unit: cost,
                    expected_units: 1.0,
                }],
            );
            signals
        },
    }
}

#[test]
fn cheapest_orders_by_cost_demoting_unknown() {
    let cands = [
        cand(0, Some(15.0), None, 1, None),
        cand(1, Some(3.0), None, 1, None),
        cand(2, None, None, 1, None),
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
        cand_rate(0, None, None, 1, None, Some(0.10)),
        cand_rate(1, None, None, 1, None, Some(0.90)),
        cand_rate(2, None, None, 1, None, None),
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

/// The claimed words are exactly the four supported strategies.
#[test]
fn the_claimed_words_are_the_four_strategies() {
    assert_eq!(WORDS, ["cheapest", "fastest", "least_busy", "usage"]);
}

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
    let d = rank("cheapest", &[cand(0, Some(5.0), None, 1, None)]).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![0]));

    let d = rank("fastest", &[cand(0, None, Some(30.0), 1, None)]).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![0]));

    let d = rank("least_busy", &[cand(0, None, None, 3, None)]).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![0]));

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
        cand(1, None, None, 1, None),
        cand(2, None, Some(40.0), 1, None),
    ];
    let d = rank("fastest", &cands).unwrap();
    assert_eq!(d, Verdict::Prefer(vec![2, 0, 1]));
}

/// A deterministic stream of ordered keys (no randomness crate): `n` draws over finite values,
/// both zeros and both infinities. The ordering test adds absent keys separately.
fn keys(seed: u64, n: usize) -> Vec<Option<f64>> {
    let mut x = seed;
    (0..n)
        .map(|_| {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let pick = (x >> 33) % 8;
            match pick {
                0 => Some(0.0),
                1 => Some(-0.0),
                2 => Some(f64::INFINITY),
                3 => Some(f64::NEG_INFINITY),
                _ => Some(((x >> 40) % 7) as f64 - 3.0),
            }
        })
        .collect()
}

/// Numeric ordering with stable index ties; the input contains no NaN.
fn numeric_order(keys: &[Option<f64>], descending: bool) -> Vec<usize> {
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

/// Ascending and descending signal rankings use numeric order and index ties across the
/// sort's small-slice boundary, including zeros, infinities and absent signals.
#[test]
fn a_pool_without_nan_uses_numeric_order_and_index_ties() {
    for seed in 1..=64_u64 {
        for n in [1, 2, 3, 7, 19, 20, 21, 33, 64, 100] {
            let ks: Vec<Option<f64>> = keys(seed * 977 + n as u64, n)
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
            let mut latency: Vec<_> = ks
                .iter()
                .enumerate()
                .map(|(i, k)| cand(i, None, *k, 1, None))
                .collect();

            latency.reverse();
            assert_eq!(
                rank("fastest", &latency).unwrap(),
                Verdict::Prefer(numeric_order(&ks, false)),
                "ascending, seed {seed}, n {n}: {ks:?}"
            );
            let mut rate: Vec<_> = ks
                .iter()
                .enumerate()
                .map(|(i, k)| cand_rate(i, None, None, 1, None, *k))
                .collect();
            rate.reverse();
            assert_eq!(
                rank("usage", &rate).unwrap(),
                Verdict::Prefer(numeric_order(&ks, true)),
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

fn pool_latency(n: usize, key: impl Fn(usize) -> Option<f64>) -> Vec<DecodedCandidate<'static>> {
    let ks = shuffled(n, &key);
    (0..n)
        .map(|i| cand((i * 7) % n, None, ks[i], 1, None))
        .collect()
}

fn pool_rate(n: usize, key: impl Fn(usize) -> Option<f64>) -> Vec<DecodedCandidate<'static>> {
    let ks = shuffled(n, &key);
    (0..n)
        .map(|i| cand_rate((i * 7) % n, None, None, 1, None, ks[i]))
        .collect()
}

/// A `NaN` KEY SORTS AFTER EVERY NUMBER AND BEFORE EVERY ABSENT KEY. `NaN` makes `partial_cmp`
#[test]
fn a_nan_key_ranks_after_the_numbers_and_before_the_absent() {
    const N: usize = 48;

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
    let nan: Vec<usize> = (0..N).filter(|i| i.is_multiple_of(3)).collect();
    let absent: Vec<usize> = (0..N)
        .filter(|i| !i.is_multiple_of(3) && i.is_multiple_of(5))
        .collect();
    let tail: Vec<usize> = nan.iter().chain(absent.iter()).copied().collect();
    let mut present: Vec<usize> = (0..N).filter(|i| finite(*i)).collect();

    present.sort_by_key(|i| (i % 4, *i));
    let want: Vec<usize> = present
        .iter()
        .copied()
        .chain(tail.iter().copied())
        .collect();
    assert_eq!(
        rank("fastest", &pool_latency(N, key)).unwrap(),
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

/// A NaN latency is kept in a deterministic tier after numbers and before absent signals.
#[test]
fn an_all_nan_pool_ranks_by_index_and_ahead_of_absent() {
    let cands = [
        cand(1, Some(f64::NAN), Some(f64::NAN), 1, None),
        cand(0, Some(f64::NAN), Some(f64::NAN), 1, None),
    ];
    assert_eq!(rank("cheapest", &cands).unwrap(), Verdict::Abstain);
    assert_eq!(
        rank("fastest", &cands).unwrap(),
        Verdict::Prefer(vec![0, 1])
    );
    let mixed = [
        cand(0, None, None, 1, None),
        cand(1, None, Some(f64::NAN), 1, None),
        cand(2, None, Some(5.0), 1, None),
    ];
    assert_eq!(
        rank("fastest", &mixed).unwrap(),
        Verdict::Prefer(vec![2, 1, 0])
    );
}

/// THE IDX TIE-BREAK, with candidates handed in NON-idx order: equal keys order by idx, in the
/// ascending and the descending rankings, and the absent group orders by idx, so the answer is the
/// comparator's and not the input's.
#[test]
fn ties_and_the_absent_group_break_by_idx_whatever_the_input_order() {
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

/// Infinite latency remains orderable; invalid recipe prices are unknown, never cheap.
#[test]
fn infinite_latency_is_ordered_and_nonfinite_costs_are_unknown() {
    for invalid in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN, -1.0] {
        let cands = [
            cand(0, Some(invalid), None, 1, None),
            cand(1, Some(2.0), None, 1, None),
            cand(2, None, None, 1, None),
        ];
        assert_eq!(
            rank("cheapest", &cands).unwrap(),
            Verdict::Prefer(vec![1, 0, 2])
        );
        assert_eq!(rank("cheapest", &cands[..1]).unwrap(), Verdict::Abstain);
    }
    let cands = [
        cand(0, None, Some(f64::INFINITY), 1, None),
        cand(1, None, Some(2.0), 1, None),
        cand(2, None, None, 1, None),
    ];
    assert_eq!(
        rank("fastest", &cands).unwrap(),
        Verdict::Prefer(vec![1, 0, 2])
    );
}

#[test]
fn missing_recipes_never_use_scalar_costs_or_ignore_overrides() {
    let mut cands = [
        cand(0, Some(1.0), None, 1, None),
        cand(1, Some(9.0), None, 1, None),
    ];
    for candidate in &mut cands {
        candidate.signals = busbar_contract::SignalBag::new();
    }
    assert_eq!(rank("cheapest", &cands), Some(Verdict::Abstain));
    let overrides = BTreeMap::from([("work".to_owned(), 0.0)]);
    assert_eq!(
        rank_with_expected_units("cheapest", &cands, &overrides),
        Some(Verdict::Abstain)
    );
    // A known recipe stays ahead of a lower scalar on a candidate missing its recipe.
    cands[1] = cand(1, Some(9.0), None, 1, None);
    assert_eq!(rank("cheapest", &cands), Some(Verdict::Prefer(vec![1, 0])));
}

#[test]
fn rank_by_evaluates_each_candidate_key_exactly_once() {
    // Only the last candidate carries a key: the all-absent check must not decode the leading
    // keys a second time when ranking.
    let pool: Vec<_> = (0..5)
        .map(|i| cand(i, None, (i == 4).then_some(1.0), 0, None))
        .collect();
    let calls = std::cell::Cell::new(0_usize);
    let verdict = rank_by(&pool, false, |c| {
        calls.set(calls.get() + 1);
        c.latency_ms
    });
    assert_eq!(verdict, Verdict::Prefer(vec![4, 0, 1, 2, 3]));
    assert_eq!(calls.get(), pool.len());

    let calls = std::cell::Cell::new(0_usize);
    let none: Vec<_> = (0..3).map(|i| cand(i, None, None, 0, None)).collect();
    let verdict = rank_by(&none, false, |c| {
        calls.set(calls.get() + 1);
        c.latency_ms
    });
    assert_eq!(verdict, Verdict::Abstain);
    assert_eq!(calls.get(), none.len());
}
