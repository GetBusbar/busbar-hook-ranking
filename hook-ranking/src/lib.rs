// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! The RANKING hook: `cheapest` / `fastest` / `least_busy` / `usage`, each a small sync sort over
//! the live signals a `decide` view already carries (`Decoded::candidates`). A hook plugin on the
//! hook kind's memory ABI ([`door`]); the four strategy words are the hook words its Statement
//! claims. `weighted` is NOT a word of this plugin: it is the engine's non-removable inline SWRR
//! floor (the name stays in [`rank`] as the explicit Abstaining form).
//!
//! All rankings are SYNC and never touch async or I/O, and rank exactly as 1.5.5 did (hook
//! behaviour is frozen at 1.5.5; v1.5.5 `crates/hooks-ranking/src/lib.rs:64-106`). An absent key is
//! demoted to the end, still reachable; a key that is not orderable (`NaN`) compares equal to every
//! key (1.5.5's `partial_cmp(..).unwrap_or(Equal)`, `:76-79`), so the index decides; an infinity is
//! ordered by `partial_cmp`. Only when the signal is ABSENT on every candidate is the answer
//! `Abstain` (no opinion, default SWRR); an all-`NaN` pool states a key, so it ranks by `idx`.
//! Ties break by `idx`, so an order is the same on every call.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod door;

use busbar_contract::abi::sdk::hook::{DecodedCandidate, Verdict};

// ── Policy-name constants ─────────────────────────────────────────────────────────────────────────
// Single source of truth for the strategy names: the `rank` match arms and [`WORDS`] (the hook
// words the Statement claims) both read them.
const POLICY_NAME_WEIGHTED: &str = "weighted";
const POLICY_NAME_CHEAPEST: &str = "cheapest";
const POLICY_NAME_FASTEST: &str = "fastest";
const POLICY_NAME_LEAST_BUSY: &str = "least_busy";
const POLICY_NAME_USAGE: &str = "usage";

/// Rank candidates by a key, ascending (smallest key first), exactly as 1.5.5 ranked them (v1.5.5
/// `crates/hooks-ranking/src/lib.rs:64-85`; hook behaviour is frozen at 1.5.5). Candidates whose key
/// is `None` are demoted to the end (lowest preference) but still ranked among themselves by `idx`;
/// never dropped, so a member with missing signal data is reachable, not stranded. A key `partial_cmp`
/// cannot order (`NaN`) compares EQUAL to every key, as 1.5.5's `unwrap_or(Equal)` made it
/// (`:76-79`), so the index breaks the tie. Returns `Abstain` if the signal is ABSENT on every
/// candidate (no opinion → default SWRR).
fn rank_ascending_by<K: PartialOrd + Copy>(
    candidates: &[DecodedCandidate<'_>],
    key: impl Fn(&DecodedCandidate<'_>) -> Option<K>,
) -> Verdict {
    let mut keyed: Vec<(usize, Option<K>)> = candidates.iter().map(|c| (c.idx, key(c))).collect();
    // Abstain only when the signal is ABSENT on every candidate (1.5.5's rule): an all-`NaN` pool
    // states the signal, so it ranks by index as 1.5.5 did.
    if keyed.iter().all(|(_, k)| k.is_none()) {
        return Verdict::Abstain;
    }
    // Sort: Some(k) before None; among Some, ascending by k; ties (and None/None) by idx for a
    // deterministic, stable order. A `NaN` compares Equal to every key (v1.5.5 lib.rs:76-79), so
    // the index decides, byte for byte the order 1.5.5 answered.
    keyed.sort_by(|(ia, ka), (ib, kb)| match (ka, kb) {
        (Some(a), Some(b)) => a
            .partial_cmp(b)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(ia.cmp(ib)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => ia.cmp(ib),
    });
    Verdict::Prefer(keyed.into_iter().map(|(idx, _)| idx).collect())
}

/// Rank descending (largest key first) — the same shape as `rank_ascending_by` but preferring the
/// LARGEST signal (e.g. most free concurrency, most budget remaining).
fn rank_descending_by<K: PartialOrd + Copy>(
    candidates: &[DecodedCandidate<'_>],
    key: impl Fn(&DecodedCandidate<'_>) -> Option<K>,
) -> Verdict {
    let mut keyed: Vec<(usize, Option<K>)> = candidates.iter().map(|c| (c.idx, key(c))).collect();
    // Abstain only when the signal is ABSENT on every candidate (1.5.5's rule): an all-`NaN` pool
    // states the signal, so it ranks by index as 1.5.5 did.
    if keyed.iter().all(|(_, k)| k.is_none()) {
        return Verdict::Abstain;
    }
    keyed.sort_by(|(ia, ka), (ib, kb)| match (ka, kb) {
        (Some(a), Some(b)) => b
            .partial_cmp(a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(ia.cmp(ib)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => ia.cmp(ib),
    });
    Verdict::Prefer(keyed.into_iter().map(|(idx, _)| idx).collect())
}

/// Rank `candidates` by the strategy `name` (`weighted` is the explicit Abstaining form). `None`
/// for a word this plugin does not claim.
///
/// * `cheapest`: lowest operator-declared `cost_per_mtok`; a member with no cost is demoted.
/// * `fastest`: lowest measured rolling-EWMA `latency_ms`; a member with no sample is demoted.
/// * `least_busy`: most `available_concurrency`; the signal is always present, so never Abstains.
/// * `usage`: most `rate_headroom` (the largest fraction of the governance rate budget still
///   available this window); a member with no headroom signal is demoted, and every member lacking
///   it Abstains (no rate limit in play).
#[must_use]
pub fn rank(name: &str, candidates: &[DecodedCandidate<'_>]) -> Option<Verdict> {
    Some(match name {
        POLICY_NAME_WEIGHTED => Verdict::Abstain,
        POLICY_NAME_CHEAPEST => rank_ascending_by(candidates, |c| c.cost_per_mtok),
        POLICY_NAME_FASTEST => rank_ascending_by(candidates, |c| c.latency_ms),
        POLICY_NAME_LEAST_BUSY => rank_descending_by(candidates, |c| Some(c.available_concurrency)),
        POLICY_NAME_USAGE => rank_descending_by(candidates, |c| c.rate_headroom),
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

/// The plugin's name: its Statement's, and the linked row's.
pub const NAME: &str = "hooks-ranking";

/// THE LINKED ENTRY: what a build that links the ranking hook registers on the hook axis — its
/// door, the same door a dropped-in build of it exports. Its hook words are [`WORDS`].
pub mod linked {
    pub use crate::door::door;
}

#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;
