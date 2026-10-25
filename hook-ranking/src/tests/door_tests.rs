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
    // compare this refusal with the door's.
    assert_eq!(m, "hook-ranking: settings.policy is required");
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
