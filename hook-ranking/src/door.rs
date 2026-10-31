// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! THE DOOR: the ranking hook on the hook kind's memory ABI (`hook_door!`), on the SDK's safe
//! surface. No `unsafe` here.
//!
//! * `open` reads the strategy from the settings document, `{"policy": "<word>"}`, one of
//!   [`crate::WORDS`]; any other key, a non-object document, or a non-string policy refuses in
//!   words, as does an unknown word. An empty document is `{}` (the SDK's settings rule), so it
//!   refuses as a missing policy.
//! * `configure` merges a pushed settings map, commit-on-ack, in `open`'s grammar: an absent
//!   `policy` acknowledges and changes nothing; a valid one becomes the live strategy; any
//!   refusal is a NACK whose reason is written to the plugin's log.
//! * `decide` ranks the view's candidates ([`crate::rank`]). It reads no prompt and no caller (the
//!   Statement's tail asks for none) and never pends: no need is declared.
//! * `transform` / `notify` / `status` / `describe` are the SDK's defaults.
//!
//! The Statement claims the strategy words ([`crate::WORDS`]) as hook word marks.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::task::Poll;

use busbar_contract::abi::hook::{Tail, CLASS_GATE, PROMPT_NO, USER_NO};
use busbar_contract::abi::mechanism::door::{MarkWord, Rewrite, Statement, MARK_WORD_HOOK};
use busbar_contract::abi::sdk::door::{abi_str, alias, statement};
use busbar_contract::abi::sdk::exchange::Op;
use busbar_contract::abi::sdk::hook::{
    statement_with_tail, tail, Decoded, Hook, HookOpen, Verdict,
};

use crate::{rank, ALIAS, NAME, WORDS};

/// How many ops one instance holds in flight; the host clamps. Ranking is CPU-only.
pub const MAX_INFLIGHT: u32 = 64;

/// An order gate that reads neither the prompt nor the caller.
const TAIL: &Tail = &tail(CLASS_GATE, PROMPT_NO, USER_NO);

const fn word(w: &'static str) -> MarkWord {
    MarkWord {
        class: MARK_WORD_HOOK,
        _reserved: 0,
        word: abi_str(w),
    }
}

/// The hook words, one mark each: built from [`WORDS`] by mapping, so a word added there is claimed
/// here (and `rank` must answer it: `rank_knows_its_words_and_no_others` iterates [`WORDS`]).
const fn marks() -> [MarkWord; WORDS.len()] {
    let mut out = [word(WORDS[0]); WORDS.len()];
    let mut i = 1;
    while i < WORDS.len() {
        out[i] = word(WORDS[i]);
        i += 1;
    }
    out
}

const MARKS: &[MarkWord] = &marks();

/// The module name an operator writes, as the alias the registry holds beside [`NAME`].
const REWRITES: &[Rewrite] = &[alias(ALIAS)];

/// The catalogue code a refused `configure` is raised under (declared in `declares.json`).
pub const CONFIGURE_REFUSED: &str = "BUSBAR-7150";

/// The Statement: name, version, the hook tail, the alias and the claimed [`WORDS`].
pub const STATEMENT: Statement = Statement {
    rewrites: REWRITES.as_ptr(),
    rewrites_len: REWRITES.len(),
    mark_words: MARKS.as_ptr(),
    mark_words_len: MARKS.len(),
    ..statement_with_tail(
        statement(NAME, env!("CARGO_PKG_VERSION"), MAX_INFLIGHT),
        TAIL,
    )
};

/// One opened ranking: the index into [`WORDS`] of the strategy its settings named, swapped by
/// `configure`.
#[derive(Debug)]
pub struct Ranking {
    word: AtomicUsize,
    /// The highest `configure` version applied; held across the check and the swap, so a
    /// concurrent older push can never overwrite a newer one.
    applied: Mutex<u64>,
}

impl Ranking {
    fn word(&self) -> &'static str {
        WORDS[self.word.load(Ordering::Acquire)]
    }
}

impl Hook for Ranking {
    fn decide(&self, view: &Decoded<'_>, _: &Op<'_>) -> Poll<Verdict> {
        Poll::Ready(rank(self.word(), &view.candidates).unwrap_or(Verdict::Abstain))
    }

    fn configure(
        &self,
        settings: &serde_json::Map<String, serde_json::Value>,
        version: u64,
    ) -> bool {
        // Held across validate, check and swap: the highest applied version wins.
        let mut applied = self.applied.lock().unwrap_or_else(|e| e.into_inner());
        match configured(settings) {
            // A push older than the applied one is acknowledged and not applied.
            Ok(_) if version < *applied => true,
            Ok(None) => {
                *applied = version;
                true
            }
            Ok(Some(i)) => {
                self.word.store(i, Ordering::Release);
                *applied = version;
                true
            }
            Err(why) => {
                tracing::warn!(diag = %CONFIGURE_REFUSED, "{ALIAS}: configure() rejected: {why}");
                false
            }
        }
    }
}

/// The refusal (without the alias prefix: the caller adds it once) for a settings key other than `policy`, if `settings` holds one.
fn unknown_key(
    settings: &serde_json::Map<String, serde_json::Value>,
    escape: bool,
) -> Result<(), String> {
    match settings.keys().find(|k| k.as_str() != "policy") {
        Some(k) if escape => Err(format!(
            "unknown setting {k:?} (the one setting is `policy`, one of {WORDS:?})"
        )),
        Some(k) => Err(format!(
            "unknown setting '{k}' (the one setting is `policy`, one of {WORDS:?})"
        )),
        None => Ok(()),
    }
}

/// `policy`'s value to its index in [`WORDS`], or the refusal in words (no alias prefix).
fn policy_index(value: &serde_json::Value, escape: bool) -> Result<usize, String> {
    let word = value
        .as_str()
        .ok_or_else(|| format!("settings.policy must be a string, one of {WORDS:?}"))?;
    WORDS.iter().position(|w| *w == word).ok_or_else(|| {
        if escape {
            format!("no strategy {word:?} (one of {WORDS:?})")
        } else {
            format!("no strategy '{word}' (one of {WORDS:?})")
        }
    })
}

/// `configure`'s merge, in `open`'s grammar: `Ok(None)` acknowledges and changes nothing (no
/// `policy` pushed), `Ok(Some(i))` acknowledges with [`WORDS`]`[i]` the new live strategy, `Err`
/// is the NACK's reason, without the alias prefix (the live strategy is untouched).
fn configured(
    settings: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<usize>, String> {
    // `configure`'s refusal lands in the log: the operator's word is Debug-escaped there.
    unknown_key(settings, true)?;
    settings
        .get("policy")
        .map(|v| policy_index(v, true))
        .transpose()
}

/// Opens a [`Ranking`] from `{"policy": "<word>"}`.
#[derive(Debug)]
pub struct Open;

impl HookOpen for Open {
    fn open(settings: &str) -> Result<Box<dyn Hook>, String> {
        // An empty document is `{}`, as the SDK reads every settings blob.
        let settings = if settings.trim().is_empty() {
            "{}"
        } else {
            settings
        };
        let doc: serde_json::Value = serde_json::from_str(settings)
            .map_err(|e| format!("{ALIAS}: the settings are not JSON: {e}"))?;
        let obj = doc.as_object().ok_or_else(|| {
            format!(
                "{ALIAS}: the settings must be an object {{\"policy\": \"<word>\"}}, one of {WORDS:?}"
            )
        })?;
        unknown_key(obj, false).map_err(|why| format!("{ALIAS}: {why}"))?;
        let index = policy_index(
            obj.get("policy")
                // The host's double mirrors this text byte for byte (busbar `ranking_double.rs`).
                .ok_or_else(|| format!("{ALIAS}: settings.policy is required"))?,
            false,
        )
        .map_err(|why| format!("{ALIAS}: {why}"))?;
        Ok(Box::new(Ranking {
            word: AtomicUsize::new(index),
            applied: Mutex::new(0),
        }))
    }
}

busbar_contract::hook_door! {
    open: Open,
    statement: STATEMENT,
}

#[cfg(test)]
#[path = "tests/door_tests.rs"]
mod tests;
