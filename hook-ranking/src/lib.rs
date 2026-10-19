// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! The RANKING hook: `cheapest` / `fastest` / `least_busy` / `usage`, each a small sync sort over
//! the live signals a `decide` view already carries (`Decoded::candidates`). A hook plugin on the
//! hook kind's memory ABI ([`door`]); the four strategy words are the hook words its Statement
//! claims. `weighted` is NOT a word of this plugin: it is the engine's non-removable inline SWRR
//! floor (the name stays in [`rank`] as the explicit Abstaining form).
//!
//! All rankings are SYNC and never touch async or I/O, and rank exactly as 1.5.5 did on every input
//! 1.5.5 could be given (hook behaviour is frozen at 1.5.5; v1.5.5
//! `crates/hooks-ranking/src/lib.rs:64-106`). An absent key is demoted to the end, still reachable;
//! present keys compare by `partial_cmp`, so `-0.0` ties `+0.0` and an infinity is ordered. Only
//! when the signal is ABSENT on every candidate is the answer `Abstain` (no opinion, default SWRR).
//! Ties break by `idx`, so an order is the same on every call.
//!
//! A `NaN` KEY (coordinator ruling 2026-10-18, a robustness fix with no reachable difference) sorts
//! after every number, as if absent, and all `NaN`s compare equal, so the comparator is a total
//! order and no sort can panic on it. 1.5.5's `partial_cmp(..).unwrap_or(Equal)` (`:76-79`,
//! `:98-101`) is no total order once a `NaN` sits among numbers, and no `NaN` ever reaches a
//! ranking: every key is finite upstream. A cost is a rate card's mean, and the validator refuses a
//! rate that is not finite and non-negative (v1.5.5 `crates/busbar/src/config_validate/mod.rs:1575`).
//! A latency EWMA drops every sample that is not finite and positive (busbar
//! `crates/busbar-kernel/src/store/in_memory/availability.rs:207`, and
//! `crates/busbar/src/root/model_egress.rs:295`). A rate headroom is clamped into `[0, 1]`
//! (`crates/busbar-kernel/src/governance/state.rs:917`). Concurrency is an integer.

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

/// THE ONE KEY ORDER both directions share: present keys by `partial_cmp` (`reverse` for the
/// descending rankings), a `NaN` after every number and equal to every other `NaN`, then the index.
/// A key that does not compare with itself is a `NaN` (for any `PartialOrd` key).
fn key_order<K: PartialOrd>(
    (ia, ka): &(usize, Option<K>),
    (ib, kb): &(usize, Option<K>),
    reverse: bool,
) -> std::cmp::Ordering {
    use std::cmp::Ordering::{Equal, Greater, Less};
    fn orderable<K: PartialOrd>(k: &Option<K>) -> Option<&K> {
        k.as_ref().filter(|v| v.partial_cmp(v).is_some())
    }
    let by_key = match (orderable(ka), orderable(kb)) {
        (Some(a), Some(b)) => {
            let (x, y) = if reverse { (b, a) } else { (a, b) };
            x.partial_cmp(y).unwrap_or(Equal)
        }
        (Some(_), None) => Less,
        (None, Some(_)) => Greater,
        (None, None) => Equal,
    };
    by_key.then(ia.cmp(ib))
}

/// Rank candidates by a key, ascending (smallest key first), exactly as 1.5.5 ranked them (v1.5.5
/// `crates/hooks-ranking/src/lib.rs:64-85`; hook behaviour is frozen at 1.5.5). Candidates whose key
/// is `None` are demoted to the end (lowest preference) but still ranked among themselves by `idx`;
/// never dropped, so a member with missing signal data is reachable, not stranded. A `NaN` key sorts
/// as absent ([`key_order`], the crate doc). Returns `Abstain` if the signal is ABSENT on every
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
    // Present keys ascending, then every absent or `NaN` key; ties by idx ([`key_order`]).
    keyed.sort_by(|a, b| key_order(a, b, false));
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
    keyed.sort_by(|a, b| key_order(a, b, true));
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
