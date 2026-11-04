// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! The RANKING hook: `cheapest` / `fastest` / `least_busy` / `usage`, each a small sync sort over
//! the live signals a `decide` view already carries (`Decoded::candidates`). A hook plugin on the
//! hook kind's memory ABI ([`door`]); the four strategy words are the hook words its Statement
//! claims. `weighted` is NOT a word of this plugin: it is the engine's non-removable inline SWRR
//! floor, and the door never ranks it (it claims only [`WORDS`]).
//!
//! All rankings are SYNC and never touch async or I/O. An absent key is demoted to the end, still
//! reachable; present keys compare by `partial_cmp`, so `-0.0` ties `+0.0` and an infinity is
//! ordered. Only when the signal is ABSENT on every candidate is the answer `Abstain` (no opinion,
//! default SWRR). Ties break by `idx`, so an order is the same on every call.
//!
//! A `NaN` key is a PRESENT key that has no order against a number: it sorts after every number
//! and before every absent key, `NaN`s among themselves by `idx`. That keeps the comparator a total
//! order (the standard sort may panic on one that is not) and keeps 1.5.5's outcomes for a pool of
//! `NaN`s (`Prefer` in idx order, not `Abstain`). No `NaN` reaches a ranking in practice (the
//! validator refuses a rate that is not finite and non-negative; a latency EWMA drops every sample
//! that is not finite and positive; a rate headroom is clamped into `[0, 1]`; concurrency is an
//! integer); this keeps a stray one harmless.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod door;

use std::collections::BTreeMap;

use busbar_contract::abi::sdk::hook::{DecodedCandidate, Verdict};
use busbar_contract::signal::route_cost;
use busbar_contract::Signal;

const POLICY_NAME_CHEAPEST: &str = "cheapest";
const POLICY_NAME_FASTEST: &str = "fastest";
const POLICY_NAME_LEAST_BUSY: &str = "least_busy";
const POLICY_NAME_USAGE: &str = "usage";

/// The one key order both directions share, a total order in three tiers: a number by
/// `partial_cmp` (`reverse` for the descending rankings), then the index; then a `NaN` key, by
/// index; then an absent key, by index.
fn key_order<K: PartialOrd>(
    (ia, ka): &(usize, Option<K>),
    (ib, kb): &(usize, Option<K>),
    reverse: bool,
) -> std::cmp::Ordering {
    let tier = |k: &Option<K>| match k {
        Some(k) if k.partial_cmp(k).is_some() => 0_u8,
        Some(_) => 1,
        None => 2,
    };
    tier(ka).cmp(&tier(kb)).then_with(|| match (ka, kb) {
        (Some(a), Some(b)) if tier(ka) == 0 => {
            let (x, y) = if reverse { (b, a) } else { (a, b) };
            x.partial_cmp(y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(ia.cmp(ib))
        }
        _ => ia.cmp(ib),
    })
}

/// Rank candidates by a key: smallest key first, or with `descending` the LARGEST signal first.
fn rank_by<K: PartialOrd + Copy>(
    candidates: &[DecodedCandidate<'_>],
    descending: bool,
    key: impl Fn(&DecodedCandidate<'_>) -> Option<K>,
) -> Verdict {
    let mut keyed: Vec<(usize, Option<K>)> = candidates.iter().map(|c| (c.idx, key(c))).collect();
    if keyed.iter().all(|(_, k)| k.is_none()) {
        return Verdict::Abstain;
    }

    keyed.sort_by(|a, b| key_order(a, b, descending));
    Verdict::Prefer(keyed.into_iter().map(|(idx, _)| idx).collect())
}

/// Estimated routing cost from the plane's recipe. Unknown or malformed data never becomes
/// a zero price, including when an override selects zero units. This is a routing estimate only;
/// it never changes the card or the legacy per-million projection.
fn estimated_cost(
    candidate: &DecodedCandidate<'_>,
    expected_units: &BTreeMap<String, f64>,
    generic: bool,
) -> Option<f64> {
    let mut total = 0.0;
    let mut saw_term = false;
    let mut known = true;
    let decoded = route_cost::visit(&candidate.signals, |term| {
        saw_term = true;
        let Some(rate) = term.price_per_unit else {
            known = false;
            return;
        };
        let units = expected_units
            .get(term.class)
            .copied()
            .unwrap_or(term.expected_units);
        let next = total + rate * units;
        if !units.is_finite() || units < 0.0 || !next.is_finite() {
            known = false;
        } else {
            total = next;
        }
    });
    match decoded {
        Ok(false) if !generic => candidate.cost_per_mtok,
        Ok(true) if saw_term && known => Some(total),
        _ => None,
    }
}

/// Rank with the opened hook's per-class expected-units overrides.
pub(crate) fn rank_with_expected_units(
    name: &str,
    candidates: &[DecodedCandidate<'_>],
    expected_units: &BTreeMap<String, f64>,
) -> Option<Verdict> {
    Some(match name {
        POLICY_NAME_CHEAPEST => {
            let generic = candidates
                .iter()
                .any(|candidate| candidate.signals.get(Signal::CandidateRouteCost).is_some());
            rank_by(candidates, false, |candidate| {
                estimated_cost(candidate, expected_units, generic)
            })
        }
        POLICY_NAME_FASTEST => rank_by(candidates, false, |c| c.latency_ms),
        POLICY_NAME_LEAST_BUSY => rank_by(candidates, true, |c| Some(c.available_concurrency)),
        POLICY_NAME_USAGE => rank_by(candidates, true, |c| c.rate_headroom),
        _ => return None,
    })
}

/// Rank `candidates` by the strategy `name`, one of [`WORDS`]; `None` for a word this plugin does
/// not claim (the door answers such a call `Abstain`, no opinion, the default SWRR).
///
/// * `cheapest`: lowest estimated total cost from the plane's declared recipe and card-priced
///   candidate signals. Empty, unknown or malformed estimates are demoted. A legacy host with no
///   generic projection retains the original `cost_per_mtok` ordering without rescaling it.
///   This function uses each term's plane-declared expected units; the opened hook's
///   `expected_units` overrides apply only through the door's `decide`.
/// * `fastest`: lowest measured rolling-EWMA `latency_ms`; a member with no sample is demoted.
/// * `least_busy`: most `available_concurrency`; the signal is always present, so it Abstains only
///   on an empty pool.
/// * `usage`: most `rate_headroom` (the largest fraction of the governance rate budget still
///   available this window); a member with no headroom signal is demoted, and every member lacking
///   it Abstains (no rate limit in play).
#[must_use]
pub fn rank(name: &str, candidates: &[DecodedCandidate<'_>]) -> Option<Verdict> {
    rank_with_expected_units(name, candidates, &BTreeMap::new())
}

/// The strategy words this plugin claims (its Statement's hook words).
pub const WORDS: [&str; 4] = [
    POLICY_NAME_CHEAPEST,
    POLICY_NAME_FASTEST,
    POLICY_NAME_LEAST_BUSY,
    POLICY_NAME_USAGE,
];

/// The plugin's name: its Statement's, and the linked row's (`busbar-<kind>-<name>`, as every
/// plugin's).
pub const NAME: &str = "busbar-hook-ranking";

/// The alias a hook reference names this plugin by (`module: ranking`), and the prefix of its
/// settings refusals.
pub const ALIAS: &str = "ranking";

/// THE LINKED ENTRY: what a build that links the ranking hook registers on the hook axis — its
/// door, the same door a dropped-in build of it exports. Its hook words are [`WORDS`].
pub mod linked {
    pub use crate::door::{door, MACRO_DOOR};
}

#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;
