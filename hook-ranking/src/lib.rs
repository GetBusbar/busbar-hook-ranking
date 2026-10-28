// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! The RANKING hook: `cheapest` / `fastest` / `least_busy` / `usage`, each a small sync sort over
//! the live signals a `decide` view already carries (`Decoded::candidates`). A hook plugin on the
//! hook kind's memory ABI ([`door`]); the four strategy words are the hook words its Statement
//! claims. `weighted` is NOT a word of this plugin: it is the engine's non-removable inline SWRR
//! floor, and the door never ranks it (it claims only [`WORDS`]).
//!
//! All rankings are SYNC and never touch async or I/O, and rank exactly as 1.5.5 did on every input
//! 1.5.5 could be given (hook behaviour is frozen at 1.5.5; v1.5.5
//! `crates/hooks-ranking/src/lib.rs:64-106`). An absent key is demoted to the end, still reachable;
//! present keys compare by `partial_cmp`, so `-0.0` ties `+0.0` and an infinity is ordered. Only
//! when the signal is ABSENT on every candidate is the answer `Abstain` (no opinion, default SWRR).
//! Ties break by `idx`, so an order is the same on every call.
//!
//! A `NaN` KEY is ordered as 1.5.5 ordered it: `partial_cmp(..).unwrap_or(Equal)` then the index
//! (v1.5.5 `crates/hooks-ranking/src/lib.rs:76-79`, `:98-101`), so a `NaN` ties every number and the
//! order among a pool holding one is whatever that comparator and the standard sort give. Hook
//! behaviour is frozen at 1.5.5 (THE DESIGN §2, "A port keeps 1.5.5's surface"; Part 1), and no
//! recorded ruling allows another order. No `NaN` reaches a ranking in practice: every key is finite
//! upstream (a cost is a rate card's mean, and the validator refuses a rate that is not finite and
//! non-negative, v1.5.5 `crates/busbar/src/config_validate/mod.rs:1575`; a latency EWMA drops every
//! sample that is not finite and positive; a rate headroom is clamped into `[0, 1]`; concurrency
//! is an integer).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod door;

use busbar_contract::abi::sdk::hook::{DecodedCandidate, Verdict};

// ── Policy-name constants ─────────────────────────────────────────────────────────────────────────
// Single source of truth for the strategy names: the `rank` match arms and [`WORDS`] (the hook
// words the Statement claims) both read them.
const POLICY_NAME_CHEAPEST: &str = "cheapest";
const POLICY_NAME_FASTEST: &str = "fastest";
const POLICY_NAME_LEAST_BUSY: &str = "least_busy";
const POLICY_NAME_USAGE: &str = "usage";

/// THE ONE KEY ORDER both directions share, 1.5.5's comparator: present keys by `partial_cmp`
/// (`reverse` for the descending rankings; a pair that does not compare, a `NaN` among them, is
/// `Equal`), then the index; a present key before an absent one; absent keys by index.
fn key_order<K: PartialOrd>(
    (ia, ka): &(usize, Option<K>),
    (ib, kb): &(usize, Option<K>),
    reverse: bool,
) -> std::cmp::Ordering {
    use std::cmp::Ordering::{Equal, Greater, Less};
    match (ka, kb) {
        (Some(a), Some(b)) => {
            let (x, y) = if reverse { (b, a) } else { (a, b) };
            x.partial_cmp(y).unwrap_or(Equal).then(ia.cmp(ib))
        }
        (Some(_), None) => Less,
        (None, Some(_)) => Greater,
        (None, None) => ia.cmp(ib),
    }
}

/// Rank candidates by a key, exactly as 1.5.5 ranked them (v1.5.5 `crates/hooks-ranking/src/lib.rs:64-85`;
/// hook behaviour is frozen at 1.5.5): smallest key first, or with `descending` the LARGEST signal
/// first (e.g. most free concurrency, most budget remaining). Candidates whose key is `None` are
/// demoted to the end (lowest preference) but still ranked among themselves by `idx`; never
/// dropped, so a member with missing signal data is reachable, not stranded. A `NaN` key ties every
/// number, as in 1.5.5 ([`key_order`], the crate doc). Returns `Abstain` if the signal is ABSENT on
/// every candidate (no opinion → default SWRR).
fn rank_by<K: PartialOrd + Copy>(
    candidates: &[DecodedCandidate<'_>],
    descending: bool,
    key: impl Fn(&DecodedCandidate<'_>) -> Option<K>,
) -> Verdict {
    let mut keyed: Vec<(usize, Option<K>)> = candidates.iter().map(|c| (c.idx, key(c))).collect();
    // Abstain only when the signal is ABSENT on every candidate (1.5.5's rule).
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

/// The plugin's name: its Statement's, and the linked row's.
pub const NAME: &str = "hooks-ranking";

/// THE LINKED ENTRY: what a build that links the ranking hook registers on the hook axis — its
/// door, the same door a dropped-in build of it exports. Its hook words are [`WORDS`].
pub mod linked {
    pub use crate::door::{door, MACRO_DOOR};
}

#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;
