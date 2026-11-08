// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! THE DOOR: the ranking hook on the hook kind's memory ABI (`hook_door!`), on the SDK's safe
//! surface. No `unsafe` here.
//!
//! * `open` reads a `policy` from [`crate::WORDS`] and an optional `expected_units` map keyed by
//!   opaque billing class. Other keys, a non-object document, a non-string policy or an invalid
//!   `expected_units` map (including one too large for the complete configure checkpoint) refuse
//!   in words, as does an unknown word. An empty document is `{}`, so
//!   it refuses as a missing policy.
//! * `configure` validates the pushed map in `open`'s grammar (with `policy` optional) before
//!   comparing versions: a map this hook refuses is a NACK, even when stale, whose reason is
//!   written to its log under [`CONFIGURE_REFUSED`]. A valid version older than the last applied
//!   version is acknowledged without being applied. Equal or newer valid pushes commit on ack:
//!   absent fields retain their live values; a policy/profile update becomes live together. An
//!   empty `expected_units` object restores plane defaults. Every acknowledged push returns the
//!   complete configuration checkpoint. The SDK refuses a non-object push before calling this
//!   hook, without its diagnostic.
//! * `decide` ranks the view's candidates by the live strategy (`cheapest` with the live
//!   expected-units profile). It reads no prompt and no caller (the Statement's tail asks for
//!   none) and never pends: no need is declared.
//! * `transform` / `notify` / `status` / `describe` are the SDK's defaults.
//!
//! The Statement claims the strategy words ([`crate::WORDS`]) as hook word marks.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;

use busbar_contract::abi::hook::{Tail, CLASS_GATE, PROMPT_NO, USER_NO};
use busbar_contract::abi::mechanism::door::{MarkWord, Rewrite, Statement, MARK_WORD_HOOK};
use busbar_contract::abi::sdk::door::{abi_str, alias, statement};
use busbar_contract::abi::sdk::exchange::Op;
use busbar_contract::abi::sdk::hook::{
    statement_with_tail, tail, ConfigureCheckpoint, Decoded, Hook, HookOpen, Verdict,
};

use crate::{rank_with_expected_units, ALIAS, NAME, WORDS};

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

busbar_contract::statement_extensions!(
    EXTENSIONS: [
        (
            busbar_contract::abi::mechanism::extensions::EXT_DECLARES,
            include_bytes!("../declares.json")
        ),
    ]
);

/// The Statement: name, version, the hook tail, the alias and the claimed [`WORDS`].
pub const STATEMENT: Statement = Statement {
    settings_schema: busbar_contract::abi::mechanism::call::Blob::json(include_bytes!(
        "../settings.schema.json"
    )),
    extensions: EXTENSIONS,
    rewrites: REWRITES.as_ptr(),
    rewrites_len: REWRITES.len(),
    mark_words: MARKS.as_ptr(),
    mark_words_len: MARKS.len(),
    ..statement_with_tail(
        statement(NAME, env!("CARGO_PKG_VERSION"), MAX_INFLIGHT),
        TAIL,
    )
};

#[derive(Debug, Default)]
struct Configuration {
    version: u64,
    revision: u64,
    expected_units: Arc<BTreeMap<String, f64>>,
}

/// One opened ranking: its strategy index and the last applied version with its immutable
/// expected-units profile. Cheapest snapshots strategy and profile under the configure mutex,
/// then releases it before sorting; other strategies read only the atomic index. No logging or
/// foreign call runs under this mutex.
#[derive(Debug)]
pub struct Ranking(AtomicUsize, Mutex<Configuration>);

impl Ranking {
    fn word(&self) -> &'static str {
        WORDS[self.0.load(Ordering::Acquire)]
    }

    fn apply_configuration(
        &self,
        settings: &serde_json::Map<String, serde_json::Value>,
        version: u64,
    ) -> Result<ConfigureCheckpoint, String> {
        let mut applied = self.1.lock().unwrap_or_else(|e| e.into_inner());
        let result = (|| {
            let selected_policy = configured(settings)?;
            let selected_units = expected_units(settings)?;
            if version < applied.version {
                return checkpoint(
                    applied.revision,
                    applied.version,
                    self.word(),
                    &applied.expected_units,
                );
            }
            let index = selected_policy.unwrap_or_else(|| self.0.load(Ordering::Acquire));
            let units = selected_units
                .map(Arc::new)
                .unwrap_or_else(|| Arc::clone(&applied.expected_units));
            let revision = applied
                .revision
                .checked_add(1)
                .ok_or_else(|| "ranking configuration revision exhausted".to_string())?;
            let snapshot = checkpoint(revision, version, WORDS[index], &units)?;
            self.0.store(index, Ordering::Release);
            applied.expected_units = units;
            applied.version = version;
            applied.revision = revision;
            Ok(snapshot)
        })();
        drop(applied);
        if let Err(why) = &result {
            tracing::warn!(diag = %CONFIGURE_REFUSED, "{ALIAS}: configure() rejected: {why}");
        }
        result
    }
}

fn checkpoint(
    revision: u64,
    version: u64,
    policy: &str,
    units: &BTreeMap<String, f64>,
) -> Result<ConfigureCheckpoint, String> {
    let mut bytes = b"{\"policy\":".to_vec();
    serde_json::to_writer(&mut bytes, policy)
        .map_err(|_| "ranking configuration checkpoint encoding failed".to_string())?;
    bytes.extend_from_slice(b",\"expected_units\":");
    serde_json::to_writer(&mut bytes, units)
        .map_err(|_| "ranking configuration checkpoint encoding failed".to_string())?;
    bytes.push(b'}');
    ConfigureCheckpoint::new(revision, version, bytes)
}

impl Hook for Ranking {
    fn decide(
        &self,
        view: &Decoded<'_>,
        _: &Op<'_, busbar_contract::plugin::markers::HookKind>,
    ) -> Poll<Verdict> {
        let word = self.word();
        if word != crate::POLICY_NAME_CHEAPEST {
            return Poll::Ready(crate::rank(word, &view.candidates).unwrap_or(Verdict::Abstain));
        }
        let (word, expected_units) = {
            let applied = self.1.lock().unwrap_or_else(|e| e.into_inner());
            (self.word(), Arc::clone(&applied.expected_units))
        };
        Poll::Ready(
            rank_with_expected_units(word, &view.candidates, &expected_units)
                .unwrap_or(Verdict::Abstain),
        )
    }

    fn configure(
        &self,
        settings: &serde_json::Map<String, serde_json::Value>,
        version: u64,
    ) -> Result<(), String> {
        self.apply_configuration(settings, version).map(|_| ())
    }

    fn configure_checkpoint(
        &self,
        settings: &serde_json::Map<String, serde_json::Value>,
        version: u64,
    ) -> Result<Option<ConfigureCheckpoint>, String> {
        self.apply_configuration(settings, version).map(Some)
    }
}

/// The refusal (without the alias prefix: the caller adds it once) for an unknown setting.
fn unknown_key(settings: &serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
    match settings.keys().find(|k| !matches!(k.as_str(), "policy" | "expected_units")) {
        Some(k) => Err(format!(
            "unknown setting '{}' (settings are `policy` and `expected_units`; policy is one of {WORDS:?})",
            k.escape_debug()
        )),
        None => Ok(()),
    }
}

/// A present override replaces the profile as a whole; an empty object restores plane defaults.
fn expected_units(
    settings: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<BTreeMap<String, f64>>, String> {
    let Some(value) = settings.get("expected_units") else {
        return Ok(None);
    };
    let object = value.as_object().ok_or_else(|| {
        "settings.expected_units must be an object mapping class names to finite non-negative numbers"
            .to_string()
    })?;
    let mut units = BTreeMap::new();
    for (class, value) in object {
        if class.is_empty() {
            return Err("settings.expected_units class names must not be empty".to_string());
        }
        let number = value
            .as_f64()
            .filter(|v| v.is_finite() && *v >= 0.0)
            .ok_or_else(|| {
                format!(
                    "settings.expected_units['{}'] must be a finite non-negative number",
                    class.escape_debug()
                )
            })?;
        units.insert(class.clone(), number);
    }
    Ok(Some(units))
}

/// `policy`'s value to its index in [`WORDS`], or the refusal in words (no alias prefix).
fn policy_index(value: &serde_json::Value) -> Result<usize, String> {
    let word = value
        .as_str()
        .ok_or_else(|| format!("settings.policy must be a string, one of {WORDS:?}"))?;
    WORDS
        .iter()
        .position(|w| *w == word)
        .ok_or_else(|| format!("no strategy '{}' (one of {WORDS:?})", word.escape_debug()))
}

/// The strategy part of `configure`: `Ok(None)` retains the word (no `policy` pushed),
/// `Ok(Some(i))` selects [`WORDS`]`[i]`, and `Err` is the bare NACK reason. The caller validates
/// the expected-units profile before committing either part.
fn configured(
    settings: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<usize>, String> {
    unknown_key(settings)?;
    settings.get("policy").map(policy_index).transpose()
}

/// Opens a [`Ranking`] from `{"policy": "<word>"}`, with an optional `expected_units` map.
#[derive(Debug)]
pub struct Open;

impl HookOpen for Open {
    fn open(settings: &str) -> Result<Box<dyn Hook>, String> {
        let settings = if settings.trim().is_empty() {
            "{}"
        } else {
            settings
        };
        let doc: serde_json::Value = serde_json::from_str(settings)
            .map_err(|e| format!("{ALIAS}: the settings are not JSON: {e}"))?;
        let obj = doc.as_object().ok_or_else(|| {
            format!(
                "{ALIAS}: the settings must be an object {{\"policy\": \"<word>\"}}, optionally with `expected_units`; policy is one of {WORDS:?}"
            )
        })?;
        unknown_key(obj).map_err(|why| format!("{ALIAS}: {why}"))?;
        let index = policy_index(
            obj.get("policy")
                .ok_or_else(|| format!("{ALIAS}: settings.policy is required"))?,
        )
        .map_err(|why| format!("{ALIAS}: {why}"))?;
        let units = expected_units(obj)
            .map_err(|why| format!("{ALIAS}: {why}"))?
            .unwrap_or_default();
        // Every later configure answers with the complete checkpoint, so a profile that cannot be
        // checkpointed with the opened policy would open and then NACK every push that keeps it.
        // Refuse it here, in configure's words.
        checkpoint(1, 0, WORDS[index], &units).map_err(|why| format!("{ALIAS}: {why}"))?;
        Ok(Box::new(Ranking(
            AtomicUsize::new(index),
            Mutex::new(Configuration {
                version: 0,
                revision: 0,
                expected_units: Arc::new(units),
            }),
        )))
    }
}

busbar_contract::hook_door! {
    open: Open,
    statement: STATEMENT,
}

#[cfg(test)]
#[path = "tests/door_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/settings_schema_tests.rs"]
mod settings_schema_tests;
