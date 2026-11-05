// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

use super::*;

#[test]
fn settings_schema_matches_lifecycle() {
    let bytes = include_bytes!("../../settings.schema.json");
    let schema: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(
        STATEMENT.settings_schema.fmt,
        busbar_contract::abi::mechanism::call::BLOB_JSON
    );
    assert_eq!(STATEMENT.settings_schema.len(), bytes.len());
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        schema["properties"]["policy"]["enum"],
        serde_json::json!(WORDS)
    );
    assert_eq!(
        schema["properties"]["expected_units"]["default"],
        serde_json::json!({})
    );
    let validator = jsonschema::validator_for(&schema).unwrap();
    for word in WORDS {
        let value = serde_json::json!({"policy": word});
        assert!(validator.is_valid(&value));
        assert!(Open::open(&value.to_string()).is_ok());
    }
    for (text, accepted) in [
        (r#"{"policy":"cheapest","expected_units":{}}"#, true),
        (
            r#"{"policy":"cheapest","expected_units":{"input":0,"output":1.5,"large":1e308}}"#,
            true,
        ),
        ("{}", false),
        ("null", false),
        ("[]", false),
        (r#"{"policy":null}"#, false),
        (r#"{"policy":"weighted"}"#, false),
        (r#"{"policy":"usage","unknown":1}"#, false),
        (r#"{"policy":"usage","expected_units":null}"#, false),
        (r#"{"policy":"usage","expected_units":[]}"#, false),
        (r#"{"policy":"usage","expected_units":{"":1}}"#, false),
        (r#"{"policy":"usage","expected_units":{"input":-1}}"#, false),
        (
            r#"{"policy":"usage","expected_units":{"input":"1"}}"#,
            false,
        ),
    ] {
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(validator.is_valid(&value), accepted, "schema: {text}");
        assert_eq!(Open::open(text).is_ok(), accepted, "lifecycle: {text}");
    }
    assert!(Open::open("").is_err());
    assert!(Open::open(r#"{"policy":"usage","expected_units":{"input":1e309}}"#).is_err());
}
