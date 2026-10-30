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

use busbar_contract::abi::sdk::hook::{DecodedCandidate, Verdict};

// The strategy names: the `rank` match arms and [`WORDS`] both read them.
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
    // 0: a number, 1: a `NaN` (it does not compare with itself), 2: absent.
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
/// Candidates whose key is `None` are demoted to the end but still ranked among themselves by `idx`,
/// never dropped, so a member with missing signal data is reachable. A `NaN` key is present: it
/// sorts after every number and before every absent key ([`key_order`]). Returns `Abstain` if the signal is ABSENT on every candidate.
fn rank_by<K: PartialOrd + Copy>(
    candidates: &[DecodedCandidate<'_>],
    descending: bool,
    key: impl Fn(&DecodedCandidate<'_>) -> Option<K>,
) -> Verdict {
    let mut keyed: Vec<(usize, Option<K>)> = candidates.iter().map(|c| (c.idx, key(c))).collect();
    // Abstain only when the signal is ABSENT on every candidate.
    if keyed.iter().all(|(_, k)| k.is_none()) {
        return Verdict::Abstain;
    }
    // Present keys in order, then every absent key; ties by idx ([`key_order`]).
    keyed.sort_by(|a, b| key_order(a, b, descending));
    Verdict::Prefer(keyed.into_iter().map(|(idx, _)| idx).collect())
}

/// Rank `candidates` by the strategy `name`, one of [`WORDS`]; `None` for a word this plugin does
/// not claim (the door answers such a call `Abstain`, no opinion, the default SWRR).
///
/// * `cheapest`: lowest operator-declared `cost_per_mtok`; a member with no cost is demoted.
/// * `fastest`: lowest measured rolling-EWMA `latency_ms`; a member with no sample is demoted.
/// * `least_busy`: most `available_concurrency`; the signal is always present, so it Abstains only
///   on an empty pool.
/// * `usage`: most `rate_headroom` (the largest fraction of the governance rate budget still
///   available this window); a member with no headroom signal is demoted, and every member lacking
///   it Abstains (no rate limit in play).
#[must_use]
pub fn rank(name: &str, candidates: &[DecodedCandidate<'_>]) -> Option<Verdict> {
    Some(match name {
        POLICY_NAME_CHEAPEST => rank_by(candidates, false, |c| c.cost_per_mtok),
        POLICY_NAME_FASTEST => rank_by(candidates, false, |c| c.latency_ms),
        POLICY_NAME_LEAST_BUSY => rank_by(candidates, true, |c| Some(c.available_concurrency)),
        POLICY_NAME_USAGE => rank_by(candidates, true, |c| c.rate_headroom),
        _ => return None,
    })
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
