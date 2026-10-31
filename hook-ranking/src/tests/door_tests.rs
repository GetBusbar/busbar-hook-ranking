// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! Tests for `hook-ranking/src/door.rs`.

use super::*;

fn refusal(settings: &str) -> String {
    match <Open as HookOpen>::open(settings) {
        Ok(_) => panic!("settings {settings:?} should have been refused"),
        Err(e) => e,
    }
}

fn assert_lists_words(msg: &str) {
    for w in WORDS {
        assert!(msg.contains(w), "{msg:?} should list {w:?}");
    }
}

#[test]
fn an_unknown_key_is_refused_by_name() {
    let m = refusal(r#"{"policy":"cheapest","fallback":"fastest"}"#);
    assert!(m.contains("unknown setting 'fallback'"), "{m}");
    assert!(m.contains("cheapest"), "{m}");
    assert_lists_words(&m);
}

#[test]
fn a_misspelt_policy_key_is_refused_by_name() {
    let m = refusal(r#"{"polcy":"cheapest"}"#);
    assert!(m.contains("unknown setting 'polcy'"), "{m}");
}

#[test]
fn a_non_string_policy_is_refused_as_such() {
    for s in [r#"{"policy":5}"#, r#"{"policy":{"name":"cheapest"}}"#] {
        let m = refusal(s);
        assert!(m.contains("settings.policy must be a string"), "{s}: {m}");
        assert_lists_words(&m);
    }
}

#[test]
fn a_non_object_document_is_refused_as_such() {
    for s in ["[]", r#""cheapest""#, "null"] {
        let m = refusal(s);
        assert!(m.contains("must be an object"), "{s}: {m}");
        assert_lists_words(&m);
    }
}

#[test]
fn a_missing_policy_is_required() {
    let m = refusal("{}");
    // Byte for byte: busbar's ranking double and its parity test (`linked_hook_ranking.rs`)
    // compare this refusal with the door's. The prefix is the alias an operator names the hook
    // by (`ranking`), as the webrequest hook's refusals start `webrequest: `.
    assert_eq!(m, "ranking: settings.policy is required");
}

#[test]
fn non_json_is_refused() {
    let m = refusal("not json");
    assert!(m.contains("not JSON"), "{m}");
}

#[test]
fn an_unknown_word_is_refused() {
    let m = refusal(r#"{"policy":"nope"}"#);
    assert!(m.contains("no strategy 'nope'"), "{m}");
}

#[test]
fn every_word_opens() {
    for w in WORDS {
        let s = format!(r#"{{"policy":"{w}"}}"#);
        assert!(<Open as HookOpen>::open(&s).is_ok(), "{w}");
    }
}

#[test]
fn an_empty_document_is_a_missing_policy() {
    // The SDK reads an empty settings blob as `{}`; so does the door.
    for s in ["", "  \n"] {
        assert_eq!(refusal(s), "ranking: settings.policy is required", "{s:?}");
    }
}

fn map(settings: &str) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str(settings).expect("a JSON object")
}

#[test]
fn configure_without_a_policy_acknowledges_and_changes_nothing() {
    assert_eq!(configured(&map("{}")), Ok(None));
}

#[test]
fn configure_with_a_word_moves_the_live_strategy() {
    for (i, w) in WORDS.iter().enumerate() {
        assert_eq!(
            configured(&map(&format!(r#"{{"policy":"{w}"}}"#))),
            Ok(Some(i))
        );
    }
}

#[test]
fn configure_refuses_in_open_s_words_with_the_operator_s_text_escaped() {
    for (s, want) in [
        (r#"{"policy":"nope"}"#, "no strategy \"nope\""),
        (r#"{"policy":5}"#, "settings.policy must be a string"),
        (
            r#"{"policy":"cheapest","fallback":"x"}"#,
            "unknown setting \"fallback\"",
        ),
    ] {
        let m = configured(&map(s)).expect_err(s);
        assert!(m.contains(want), "{s}: {m}");
        assert_lists_words(&m);
    }
}

#[test]
fn a_nacked_configure_leaves_the_live_strategy() {
    let hook = Ranking {
        word: AtomicUsize::new(1),
        applied: Mutex::new(0),
    };
    assert!(!hook.configure(&map(r#"{"policy":"nope"}"#), 2));
    assert_eq!(hook.word(), "fastest");
    assert!(hook.configure(&map(r#"{"policy":"usage"}"#), 3));
    assert_eq!(hook.word(), "usage");
    assert!(!hook.configure(&map(r#"{"policy":"cheapest","x":1}"#), 4));
    assert_eq!(hook.word(), "usage", "a NACK commits nothing");
    assert!(hook.configure(&map("{}"), 5));
    assert_eq!(hook.word(), "usage");
}

#[test]
fn every_settings_refusal_starts_with_the_alias() {
    let refusals = [
        refusal("not json"),
        refusal("[]"),
        refusal("{}"),
        refusal(r#"{"policy":5}"#),
        refusal(r#"{"policy":"nope"}"#),
        refusal(r#"{"policy":"cheapest","x":1}"#),
    ];
    for m in refusals {
        assert!(m.starts_with("ranking: "), "{m}");
    }
}
