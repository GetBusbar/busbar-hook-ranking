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
    assert_lists_words(&m);
}

#[test]
fn a_misspelt_policy_key_is_refused_by_name() {
    let m = refusal(r#"{"polcy":"cheapest"}"#);
    assert!(m.contains("unknown setting 'polcy'"), "{m}");
}

#[test]
fn a_non_string_policy_is_refused_as_such() {
    for s in [
        r#"{"policy":5}"#,
        r#"{"policy":{"name":"cheapest"}}"#,
        r#"{"policy":null}"#,
    ] {
        let m = refusal(s);
        assert!(m.contains("settings.policy must be a string"), "{s}: {m}");
        assert_lists_words(&m);
    }
}

#[test]
fn a_non_object_document_is_refused_as_such() {
    for s in ["[]", r#""cheapest""#, "null"] {
        let m = refusal(s);
        assert_eq!(
            m,
            format!(
                "ranking: the settings must be an object {{\"policy\": \"<word>\"}}, optionally with `expected_units`; policy is one of {WORDS:?}"
            ),
            "{s}"
        );
        assert_lists_words(&m);
    }
}

#[test]
fn a_missing_policy_is_required() {
    let m = refusal("{}");

    assert_eq!(m, "ranking: settings.policy is required");
}

#[test]
fn non_json_is_refused() {
    let m = refusal("not json");
    assert!(m.contains("not JSON"), "{m}");
}

#[test]
fn an_unknown_word_is_refused() {
    for word in ["nope", "weighted", "Cheapest", " cheapest", "cheapest ", ""] {
        let settings = serde_json::json!({"policy": word}).to_string();
        let m = refusal(&settings);
        assert!(m.contains(&format!("no strategy '{word}'")), "{m}");
        assert_lists_words(&m);
    }
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
    let hook = Ranking(AtomicUsize::new(2), Mutex::new(Configuration::default()));
    hook.configure(&map("{}"), 2).unwrap();
    assert_eq!(
        hook.word(),
        "least_busy",
        "a policyless push never resets to WORDS[0]"
    );
}

#[test]
fn configure_with_a_word_moves_the_live_strategy() {
    for (i, w) in WORDS.iter().enumerate() {
        assert_eq!(
            configured(&map(&format!(r#"{{"policy":"{w}"}}"#))),
            Ok(Some(i))
        );
    }
    let hook = Ranking(AtomicUsize::new(0), Mutex::new(Configuration::default()));
    for (version, w) in (1..).zip(WORDS.iter().rev()) {
        hook.configure(&map(&format!(r#"{{"policy":"{w}"}}"#)), version)
            .unwrap();
        assert_eq!(hook.word(), *w);
    }
}

#[test]
fn open_refuses_an_invalid_expected_units_map_in_words() {
    let object = "ranking: settings.expected_units must be an object mapping class names to finite non-negative numbers";
    let number = "ranking: settings.expected_units['work'] must be a finite non-negative number";
    for (units, want) in [
        ("null", object),
        ("[]", object),
        ("10", object),
        (r#"{"work":"1"}"#, number),
        (r#"{"work":null}"#, number),
        (r#"{"work":-1}"#, number),
        (r#"{"work":[1]}"#, number),
        (
            r#"{"":1}"#,
            "ranking: settings.expected_units class names must not be empty",
        ),
    ] {
        let settings = format!(r#"{{"policy":"cheapest","expected_units":{units}}}"#);
        assert_eq!(refusal(&settings), want, "{settings}");
    }
}

#[test]
fn an_empty_class_name_is_refused_and_commits_nothing() {
    let hook = Ranking(AtomicUsize::new(1), Mutex::new(Configuration::default()));
    assert_eq!(
        hook.configure(&map(r#"{"policy":"usage","expected_units":{"":1}}"#), 2),
        Err("settings.expected_units class names must not be empty".to_string())
    );
    assert_eq!(hook.word(), "fastest");
    let state = hook.1.lock().unwrap();
    assert_eq!((state.version, state.revision), (0, 0));
    assert!(state.expected_units.is_empty());
}

#[test]
fn a_hostile_class_name_is_escaped_in_the_refusal() {
    let hook = Ranking(AtomicUsize::new(0), Mutex::new(Configuration::default()));
    let why = hook
        .configure(
            &map(r#"{"expected_units":{"x\r\nBUSBAR-9999 forged":-1}}"#),
            2,
        )
        .expect_err("a negative quantity is refused");
    assert_eq!(
        why,
        r"settings.expected_units['x\r\nBUSBAR-9999 forged'] must be a finite non-negative number"
    );
    assert!(!why.contains(['\r', '\n']), "{why:?}");
}

/// The host restores a quarantined instance by pushing its saved checkpoint to a fresh one and
/// requires the fresh checkpoint to be byte-identical. These quantities are ones serde_json's
/// default (not correctly rounded) float parser reads one ULP off, so they fail without
/// `float_roundtrip`.
#[test]
fn a_checkpoint_pushed_to_a_fresh_instance_reproduces_its_bytes() {
    let pushed = r#"{"policy":"fastest","expected_units":{"a":1e-30,"b":968730.2436888135,"c":3.2839432695173066e-133,"d":1.2345678901234567e-30}}"#;
    let hook = <Open as HookOpen>::open(r#"{"policy":"cheapest"}"#).unwrap();
    let saved = hook.configure_checkpoint(&map(pushed), 4).unwrap().unwrap();
    let units: serde_json::Value = serde_json::from_slice(saved.settings()).unwrap();
    assert_eq!(units["expected_units"]["a"].as_f64(), Some(1e-30));
    assert_eq!(
        units["expected_units"]["b"].as_f64(),
        Some(968_730.243_688_813_5)
    );
    assert_eq!(
        units["expected_units"]["c"].as_f64(),
        Some(3.283_943_269_517_306_6e-133)
    );
    assert_eq!(
        units["expected_units"]["d"].as_f64(),
        Some(1.234_567_890_123_456_7e-30)
    );
    let fresh = <Open as HookOpen>::open(r#"{"policy":"cheapest"}"#).unwrap();
    let settings: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(saved.settings()).unwrap();
    let restored = fresh
        .configure_checkpoint(&settings, saved.settings_version)
        .unwrap()
        .unwrap();
    assert_eq!(restored.settings_version, saved.settings_version);
    assert_eq!(
        String::from_utf8_lossy(restored.settings()),
        String::from_utf8_lossy(saved.settings())
    );
}

#[test]
fn configure_refuses_in_open_s_words() {
    for (s, want) in [
        (r#"{"policy":"nope"}"#, "no strategy 'nope'"),
        (r#"{"policy":5}"#, "settings.policy must be a string"),
        (
            r#"{"policy":"cheapest","fallback":"x"}"#,
            "unknown setting 'fallback'",
        ),
    ] {
        let m = configured(&map(s)).expect_err(s);
        assert!(m.contains(want), "{s}: {m}");
        assert_lists_words(&m);
    }
}

#[test]
fn configure_returns_the_bare_refusal_and_preserves_the_live_policy() {
    let hook = Ranking(AtomicUsize::new(1), Mutex::new(Configuration::default()));
    for settings in [
        r#"{"policy":"nope"}"#,
        r#"{"policy":null}"#,
        r#"{"policy":"cheapest","x":1}"#,
    ] {
        let expected = refusal(settings)
            .strip_prefix("ranking: ")
            .expect("OPEN names the hook")
            .to_owned();
        assert_eq!(hook.configure(&map(settings), 1), Err(expected));
        assert_eq!(hook.word(), "fastest");
    }
}

#[test]
fn a_nacked_configure_leaves_the_live_strategy() {
    let hook = Ranking(AtomicUsize::new(1), Mutex::new(Configuration::default()));
    assert!(hook
        .configure(&map(r#"{"policy":"nope"}"#), 2)
        .unwrap_err()
        .contains("no strategy 'nope'"));
    assert_eq!(hook.word(), "fastest");
    hook.configure(&map(r#"{"policy":"usage"}"#), 3).unwrap();
    assert_eq!(hook.word(), "usage");
    assert!(hook
        .configure(&map(r#"{"policy":"cheapest","x":1}"#), 4)
        .unwrap_err()
        .contains("unknown setting 'x'"));
    assert_eq!(hook.word(), "usage", "a NACK commits nothing");
    hook.configure(&map("{}"), 5).unwrap();
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

#[test]
fn checkpoint_revision_advances_on_version_zero_and_equal_version_partial_pushes() {
    let hook =
        <Open as HookOpen>::open(r#"{"policy":"cheapest","expected_units":{"work":10}}"#).unwrap();
    let first = hook.configure_checkpoint(&map("{}"), 0).unwrap().unwrap();
    assert_eq!((first.revision, first.settings_version), (1, 0));
    let second = hook
        .configure_checkpoint(&map(r#"{"policy":"fastest"}"#), 0)
        .unwrap()
        .unwrap();
    assert_eq!((second.revision, second.settings_version), (2, 0));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(second.settings()).unwrap(),
        serde_json::json!({"policy":"fastest","expected_units":{"work":10.0}})
    );
    let replayed = <Open as HookOpen>::open(r#"{"policy":"cheapest"}"#).unwrap();
    let settings: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(second.settings()).unwrap();
    let replay = replayed
        .configure_checkpoint(&settings, second.settings_version)
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(replay.settings()).unwrap(),
        serde_json::from_slice::<serde_json::Value>(second.settings()).unwrap()
    );
}

#[test]
fn checkpoint_exhaustion_and_size_refusals_commit_nothing() {
    let hook = Ranking(
        AtomicUsize::new(1),
        Mutex::new(Configuration {
            version: 7,
            revision: u64::MAX,
            expected_units: Arc::new(BTreeMap::from([("work".to_string(), 3.0)])),
        }),
    );
    assert_eq!(
        hook.configure(&map(r#"{"policy":"cheapest","expected_units":{}}"#), 8),
        Err("ranking configuration revision exhausted".to_string())
    );
    assert_eq!(hook.word(), "fastest");
    let state = hook.1.lock().unwrap();
    assert_eq!((state.version, state.revision), (7, u64::MAX));
    assert_eq!(state.expected_units.get("work"), Some(&3.0));
    drop(state);

    let hook = Ranking(AtomicUsize::new(1), Mutex::new(Configuration::default()));
    let settings = serde_json::json!({
        "policy":"cheapest",
        "expected_units":{ ("x".repeat(busbar_contract::abi::sdk::hook::configure_checkpoint::MAX_BYTES + 1)):1.0 }
    }).as_object().unwrap().clone();
    let why = hook
        .configure(&settings, 9)
        .expect_err("oversized complete reply refuses before commit");
    assert_eq!(why, "invalid complete hook configuration checkpoint");
    assert_eq!(hook.word(), "fastest");
    let state = hook.1.lock().unwrap();
    assert_eq!((state.version, state.revision), (0, 0));
    assert!(state.expected_units.is_empty());
}

#[test]
fn a_stale_invalid_configure_is_still_refused_and_changes_nothing() {
    let hook = Ranking(AtomicUsize::new(0), Mutex::new(Configuration::default()));
    hook.configure(
        &map(r#"{"policy":"fastest","expected_units":{"work":3}}"#),
        5,
    )
    .unwrap();
    for (settings, why) in [
        (r#"{"policy":"bogus","x":1}"#, "unknown setting 'x'"),
        (r#"{"policy":"bogus"}"#, "no strategy 'bogus'"),
        (r#"{"policy":null}"#, "settings.policy must be a string"),
        (r#"{"expected_units":{"work":-1}}"#, "finite non-negative"),
    ] {
        let reason = hook
            .configure(&map(settings), 3)
            .expect_err("invalid stale settings are a NACK");
        assert!(reason.contains(why), "{reason}");
        assert_eq!(hook.word(), "fastest");
        let state = hook.1.lock().unwrap();
        assert_eq!((state.version, state.revision), (5, 1));
        assert_eq!(
            state.expected_units.as_ref(),
            &BTreeMap::from([("work".to_owned(), 3.0)])
        );
    }
    let stale = hook
        .configure_checkpoint(&map(r#"{"policy":"usage","expected_units":{}}"#), 4)
        .unwrap()
        .unwrap();
    assert_eq!((stale.settings_version, stale.revision), (5, 1));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(stale.settings()).unwrap(),
        serde_json::json!({"policy":"fastest","expected_units":{"work":3.0}})
    );
    assert_eq!(hook.word(), "fastest");
}

#[test]
fn a_configure_refusal_is_the_bare_reason_without_the_alias() {
    let hook = Ranking(AtomicUsize::new(0), Mutex::new(Configuration::default()));
    let why = hook
        .configure(&map(r#"{"policy":"bogus"}"#), 2)
        .expect_err("refused");
    assert!(!why.starts_with("ranking:"), "{why}");
    assert!(!why.contains("configure() rejected"), "{why}");
    assert!(why.contains("no strategy 'bogus'"), "{why}");
}

#[test]
fn a_push_at_the_applied_version_is_applied() {
    let hook = Ranking(AtomicUsize::new(0), Mutex::new(Configuration::default()));
    hook.configure(&map(r#"{"policy":"fastest"}"#), 5).unwrap();
    assert_eq!(hook.word(), "fastest");
    hook.configure(&map(r#"{"policy":"cheapest"}"#), 5).unwrap();
    assert_eq!(hook.word(), "cheapest", "an equal version is not stale");
    let fresh = Ranking(AtomicUsize::new(0), Mutex::new(Configuration::default()));
    fresh.configure(&map(r#"{"policy":"fastest"}"#), 0).unwrap();
    assert_eq!(fresh.word(), "fastest");
}

#[test]
fn open_refuses_a_profile_no_configure_could_ever_checkpoint() {
    // A profile whose complete checkpoint exceeds the SDK bound would open, then NACK every later
    // configure (even a policy-only or stale push). Open refuses it in words instead.
    let class = "x".repeat(busbar_contract::abi::sdk::hook::configure_checkpoint::MAX_BYTES + 1);
    let settings = serde_json::json!({ "policy": "fastest", "expected_units": { class: 1.0 } });
    let Err(why) = Open::open(&settings.to_string()) else {
        panic!("an uncheckpointable profile should have been refused");
    };
    assert!(why.starts_with("ranking: "), "{why}");
    assert!(
        why.contains("invalid complete hook configuration checkpoint"),
        "{why}"
    );
}

#[test]
fn an_opened_profile_survives_a_later_policy_only_push() {
    let hook = Open::open(r#"{"policy":"cheapest","expected_units":{"work":0}}"#).unwrap();
    hook.configure(&map(r#"{"policy":"fastest"}"#), 1)
        .expect("a policy-only push keeps the opened profile");
}

#[test]
fn every_word_is_marked_once_in_order_as_a_hook_word() {
    // `marks()` is normally evaluated only at compile time for `MARKS`; run it here so its loop is
    // checked directly. The four words have distinct lengths, so the length sequence pins both the
    // words and their order (a skipped, repeated or shifted word changes it).
    let runtime = marks();
    assert_eq!(runtime.len(), WORDS.len());
    assert_eq!(MARKS.len(), WORDS.len());
    for ((mark, compiled), w) in runtime.iter().zip(MARKS).zip(WORDS) {
        assert_eq!(mark.class, MARK_WORD_HOOK, "{w}");
        assert_eq!(mark._reserved, 0, "{w}");
        assert_eq!(mark.word.len, w.len(), "{w}");
        assert_eq!(compiled.class, mark.class, "{w}");
        assert_eq!(compiled.word.len, mark.word.len, "{w}");
    }
    let one = word("cheapest");
    assert_eq!(
        (one.class, one._reserved, one.word.len),
        (MARK_WORD_HOOK, 0, 8)
    );
}
