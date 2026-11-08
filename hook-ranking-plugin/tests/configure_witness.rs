// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! THE RANKING HOOK'S `configure`, BOTH WAYS (the linked door and the dropped-in cdylib, through
//! the one loader): a current-version ACK commits the pushed settings, and a NACK says why.
//! A valid older version is acknowledged without application, preserving the newer live policy;
//! an invalid push is a NACK even when stale.
//!
//! * [`a_committed_configure_moves_the_next_decide`]: a `cheapest` instance pushed
//!   `{"policy": "fastest"}` acknowledges the version and its next `decide` ranks by latency. RED
//!   on the SDK's default `configure`, which acknowledged every push and kept ranking `cheapest`.
//! * [`a_refused_configure_is_a_nack_and_is_logged`]: an unknown word, a non-string policy and an
//!   unknown key are each a NACK (FAILED), the reason is a warn in the plugin's log, and the live
//!   strategy is unchanged. RED on the default, which acknowledged all three silently.

use std::sync::{Arc, Mutex};

use busbar_contract::abi::hook::{slot, ConfigureIn, ConfigureOut, DecideOut};
use busbar_contract::abi::host::hook::{DecideFrame, DecideView};
use busbar_contract::abi::mechanism::call::{AbiStr, Blob, Outcome, BLOB_JSON};
use busbar_contract::abi::mechanism::lifecycle::{slot as life, OpenIn, OpenOut, ReleaseIn};
use busbar_contract::abi::sdk::door::blank_out;
use busbar_contract::hooks::{Candidate, RoutingContext, RoutingRequest};
use busbar_contract::SignalBag;
use busbar_plugin_loader::dispatch::kinds::hook::Hook;
use busbar_plugin_loader::dispatch::{
    in_head, load_dropped, load_linked, out_head, rendering_of, Bind, ConnTable, Diagnostic,
    DispatchConfig, Dispatcher, Dropped, EnvelopeSink, Frame, LinkedRow, Metric, Plugin,
};

/// The host's envelope sink, recording every diagnostic as `(severity, text)`.
#[derive(Default)]
struct Recorder(Mutex<Vec<(u8, String)>>);

impl Recorder {
    /// The texts of the warn (severity 1) diagnostics so far.
    fn warns(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(s, _)| *s == 1)
            .map(|(_, t)| t.clone())
            .collect()
    }
}

impl EnvelopeSink for Recorder {
    fn metric(&self, _: Metric<'_>) {}
    fn diag(&self, d: Diagnostic<'_>) {
        self.0
            .lock()
            .unwrap()
            .push((d.severity, String::from_utf8_lossy(d.text).into_owned()));
    }
    fn dropped(&self, _: Dropped) {}
}

fn blob(text: &str) -> Blob {
    Blob {
        ptr: text.as_ptr(),
        len: text.len(),
        fmt: BLOB_JSON,
        flags: 0,
    }
}

/// One door, bound with a recording sink.
struct Bound {
    recorder: Arc<Recorder>,
    plugin: Plugin<Hook>,
    _dispatcher: Dispatcher,
}

/// The linked door (`false`) or the dropped-in cdylib (`true`), bound.
fn bind(dropped: bool) -> Bound {
    let dispatcher = Dispatcher::new(
        DispatchConfig::default(),
        busbar_contract::services::test_caller_binding(),
    );
    let recorder = Arc::new(Recorder::default());
    let bind = Bind {
        instance: Arc::from("ranking"),
        max_inflight_cap: 64,
        sink: recorder.clone(),
        dispatcher: dispatcher.adopter(),
        conns: ConnTable::NoNeeds,
    };
    let plugin = if dropped {
        let path = {
            let path = std::env::var_os("BUSBAR_PLUGIN_CDYLIB")
                .map(std::path::PathBuf::from)
                .expect("set BUSBAR_PLUGIN_CDYLIB to the freshly built ranking cdylib");
            assert!(
                path.is_absolute() && path.is_file(),
                "actual ranking cdylib required: {}",
                path.display()
            );
            path
        };
        let stated = rendering_of(busbar_hook_ranking::linked::door)
            .expect("the door renders its Statement");
        load_dropped::<Hook>(&path, &stated, bind)
    } else {
        load_linked::<Hook>(
            &LinkedRow::of(busbar_hook_ranking::linked::door).expect("the door states itself"),
            bind,
        )
    }
    .expect("the door loads");
    Bound {
        recorder,
        plugin,
        _dispatcher: dispatcher,
    }
}

impl Bound {
    fn open(&self, settings: &str) -> Outcome {
        let mut f = Frame::new(
            OpenIn {
                head: in_head(),
                host: std::ptr::null(),
                settings: blob(settings),
                secrets: std::ptr::null(),
                secrets_len: 0,
                generation: 1,
                err_buf: std::ptr::null_mut(),
                err_cap: 0,
            },
            OpenOut {
                head: out_head(),
                instance: std::ptr::null_mut(),
                err_len: 0,
            },
        );
        self.plugin.call(life::OPEN, &mut f).outcome
    }

    /// `configure`: the outcome and the version acknowledged.
    fn configure(&self, settings: &str, version: u64) -> (Outcome, u64) {
        let (outcome, version, _) = self.configure_snapshot(settings, version);
        (outcome, version)
    }

    fn configure_snapshot(
        &self,
        settings: &str,
        version: u64,
    ) -> (Outcome, u64, Option<(u64, u64, serde_json::Value)>) {
        let mut f = Frame::new(
            ConfigureIn {
                head: in_head(),
                version,
                settings: blob(settings),
                name: AbiStr {
                    ptr: std::ptr::null(),
                    len: 0,
                },
            },
            blank_out::<ConfigureOut>(),
        );
        let called = self.plugin.call(slot::CONFIGURE, &mut f);
        let blob = f.out.head.extensions;
        let bytes = if blob.ptr.is_null() || blob.len == 0 {
            Vec::new()
        } else {
            // SAFETY: the validated reply's lease remains live until the explicit RELEASE below.
            unsafe { std::slice::from_raw_parts(blob.ptr, blob.len) }.to_vec()
        };
        if called.lease != 0 {
            let mut release = Frame::new(
                ReleaseIn {
                    head: in_head(),
                    lease: called.lease,
                },
                out_head(),
            );
            assert_eq!(
                self.plugin.call(life::RELEASE, &mut release).outcome,
                Outcome::Ready
            );
        }
        let snapshot = busbar_contract::abi::mechanism::extensions::get(
            &bytes,
            "hook.configure_checkpoint.v1",
        )
        .map(|value| {
            assert!(value.len() > 16);
            let revision = u64::from_le_bytes(value[..8].try_into().unwrap());
            let version = u64::from_le_bytes(value[8..16].try_into().unwrap());
            let settings = serde_json::from_slice(&value[16..]).expect("complete JSON settings");
            (revision, version, settings)
        });
        (called.outcome, f.out.acked_version, snapshot)
    }

    /// One `decide` over two candidates, idx 0 cheaper and idx 1 faster: the order written.
    fn order(&self) -> Vec<usize> {
        let request = RoutingRequest {
            request_id: 1,
            pool: "p",
            ingress_protocol: "wire-a",
            requested_model: None,
            message_count: 1,
            tool_count: 0,
            has_tools: false,
            total_chars: 10,
            system_chars: 0,
            max_tokens: None,
            stream: false,
            prompt: None,
            identity: None,
            signals: SignalBag::new(),
            session: None,
        };
        let candidate = |idx, cost, latency| Candidate {
            idx,
            model: "m",
            provider: "p",
            weight: 1,
            context_max: None,
            tier: None,
            cost_per_mtok: Some(cost),
            tags: &[],
            latency_ms: Some(latency),
            available_concurrency: 1,
            budget_remaining: None,
            rate_headroom: None,
            signals: {
                use busbar_contract::signal::route_cost::{append, Term};
                let mut signals = SignalBag::new();
                append(
                    &mut signals,
                    [Term {
                        class: "work",
                        price_per_unit: Some(cost),
                        expected_units: 1.0,
                    }],
                );
                signals
            },
        };
        let candidates = [candidate(0, 1.0, 900.0), candidate(1, 9.0, 10.0)];
        let ctx = RoutingContext {
            pool: "p",
            budget_remaining: None,
            budget: &[],
        };
        let frame = DecideFrame::first(DecideView::build(&request, &candidates, &ctx));
        let mut f = Frame::new(frame.input(), blank_out::<DecideOut>());
        assert_eq!(
            self.plugin.call(slot::DECIDE, &mut f).outcome,
            Outcome::Ready
        );
        frame.order(f.out.order_written)
    }
}

#[test]
fn complete_checkpoints_preserve_partial_profiles_and_equal_version_changes_both_ways() {
    for dropped in [false, true] {
        let bound = bind(dropped);
        assert_eq!(
            bound.open(r#"{"policy":"cheapest","expected_units":{"work":10}}"#),
            Outcome::Ready
        );
        let first = bound.configure_snapshot(r#"{"policy":"fastest"}"#, 8);
        assert_eq!(
            first,
            (
                Outcome::Ready,
                8,
                Some((
                    1,
                    8,
                    serde_json::json!({"policy":"fastest","expected_units":{"work":10.0}})
                ))
            )
        );
        let second = bound.configure_snapshot(r#"{"expected_units":{"span":2}}"#, 8);
        assert_eq!(
            second,
            (
                Outcome::Ready,
                8,
                Some((
                    2,
                    8,
                    serde_json::json!({"policy":"fastest","expected_units":{"span":2.0}})
                ))
            )
        );
        let stale = bound.configure_snapshot(r#"{"policy":"cheapest","expected_units":{}}"#, 7);
        assert_eq!(stale, (Outcome::Ready, 7, second.2.clone()));
        assert!(bound.recorder.warns().is_empty());
        for settings in [
            r#"{"unknown":"refused"}"#,
            r#"{"policy":"nope"}"#,
            r#"{"expected_units":{"span":-1}}"#,
        ] {
            let refused = bound.configure_snapshot(settings, 7);
            assert_eq!(
                refused,
                (Outcome::Failed, 0, None),
                "no checkpoint is published on refusal"
            );
            assert_eq!(bound.order(), vec![1, 0]);
        }
        let unchanged = bound.configure_snapshot("{}", 7);
        assert_eq!(unchanged, (Outcome::Ready, 7, second.2.clone()));
        assert_eq!(
            bound
                .configure_snapshot(r#"{"policy":"cheapest","expected_units":{"span":-1}}"#, 9)
                .0,
            Outcome::Failed
        );
        let restored = bind(dropped);
        assert_eq!(restored.open(r#"{"policy":"cheapest"}"#), Outcome::Ready);
        let snapshot = second.2.unwrap();
        assert_eq!(
            restored.configure(&snapshot.2.to_string(), snapshot.1),
            (Outcome::Ready, 8)
        );
        assert_eq!(restored.order(), vec![1, 0]);
        let reset = bound.configure_snapshot(r#"{"expected_units":{}}"#, 9);
        assert_eq!(
            reset,
            (
                Outcome::Ready,
                9,
                Some((
                    3,
                    9,
                    serde_json::json!({"policy":"fastest","expected_units":{}})
                ))
            )
        );
    }
}

#[test]
fn a_committed_configure_moves_the_next_decide() {
    for dropped in [false, true] {
        let b = bind(dropped);
        assert_eq!(b.open(r#"{"policy": "cheapest"}"#), Outcome::Ready);
        assert_eq!(b.order(), vec![0, 1], "cheapest first");
        assert_eq!(
            b.configure(r#"{"policy": "fastest"}"#, 2),
            (Outcome::Ready, 2)
        );
        assert_eq!(
            b.order(),
            vec![1, 0],
            "the ACKed strategy is live (dropped={dropped})"
        );

        assert_eq!(b.configure("{}", 3), (Outcome::Ready, 3));
        assert_eq!(b.order(), vec![1, 0]);
        assert!(b.recorder.warns().is_empty(), "{:?}", b.recorder.warns());
    }
}

#[test]
fn a_refused_configure_is_a_nack_and_is_logged() {
    for dropped in [false, true] {
        let b = bind(dropped);
        assert_eq!(b.open(r#"{"policy": "cheapest"}"#), Outcome::Ready);
        for (n, (settings, why)) in [
            (r#"{"policy": "nope"}"#, "no strategy 'nope'"),
            (r#"{"policy": "weighted"}"#, "no strategy 'weighted'"),
            (r#"{"policy": "Cheapest"}"#, "no strategy 'Cheapest'"),
            (r#"{"policy": " cheapest"}"#, "no strategy ' cheapest'"),
            (r#"{"policy": "cheapest "}"#, "no strategy 'cheapest '"),
            (r#"{"policy": ""}"#, "no strategy ''"),
            (r#"{"policy": null}"#, "settings.policy must be a string"),
            (r#"{"policy": 5}"#, "settings.policy must be a string"),
            (r#"{"policy": "fastest", "x": 1}"#, "unknown setting 'x'"),
        ]
        .into_iter()
        .enumerate()
        {
            let before = b.recorder.warns().len();
            let (outcome, _) = b.configure(settings, 2 + n as u64);
            assert_eq!(outcome, Outcome::Failed, "{settings} (dropped={dropped})");
            let warns = b.recorder.warns();
            let this = &warns[before..];
            assert!(
                this.iter()
                    .any(|w| w.contains("ranking: configure() rejected:")
                        && w.contains(why)
                        && w.contains("BUSBAR-7150")),
                "{settings}: {this:?}"
            );
        }
        assert_eq!(b.order(), vec![0, 1], "a NACK commits nothing");
    }
}

#[test]
fn open_refuses_unclaimed_policy_spellings_and_null_both_ways() {
    for dropped in [false, true] {
        for policy in [
            serde_json::json!("weighted"),
            serde_json::json!("Cheapest"),
            serde_json::json!(" cheapest"),
            serde_json::json!("cheapest "),
            serde_json::json!(""),
            serde_json::Value::Null,
        ] {
            let b = bind(dropped);
            let settings = serde_json::json!({"policy": policy}).to_string();
            assert_eq!(
                b.open(&settings),
                Outcome::Failed,
                "{settings} (dropped={dropped})"
            );
            assert_eq!(b.open(r#"{"policy":"cheapest"}"#), Outcome::Ready);
            assert_eq!(b.order(), vec![0, 1]);
        }
    }
}

/// The refusal's code is declared: `declares.json` carries 7150 as an actionable diagnostic.
#[test]
fn the_configure_refusal_code_is_declared() {
    let declares: serde_json::Value =
        serde_json::from_str(include_str!("../../hook-ranking/declares.json")).unwrap();
    let diags = declares["diagnostics"].as_array().expect("diagnostics");
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0]["code"], 7150);
    assert_eq!(format!("BUSBAR-{}", diags[0]["code"]), "BUSBAR-7150");
    assert_eq!(diags[0]["severity"], "actionable");
    assert_eq!(diags[0]["since"], "1.6.0");
}

#[test]
fn stale_configures_preserve_the_live_strategy_and_diagnostics() {
    for dropped in [false, true] {
        let b = bind(dropped);
        assert_eq!(b.open(r#"{"policy":"cheapest"}"#), Outcome::Ready);
        assert_eq!(b.configure(r#"{"policy":"fastest"}"#, 8).0, Outcome::Ready);
        // A stale but valid push is acknowledged and not applied.
        assert_eq!(b.configure(r#"{"policy":"cheapest"}"#, 7).0, Outcome::Ready);
        assert_eq!(b.order(), vec![1, 0], "v8 remains live (dropped={dropped})");
        assert!(b.recorder.warns().is_empty(), "{:?}", b.recorder.warns());
        // A stale INVALID push is still a NACK and still logged: any refusal is a NACK.
        assert_eq!(b.configure(r#"{"policy":"nope"}"#, 7).0, Outcome::Failed);
        assert_eq!(b.order(), vec![1, 0], "v8 remains live (dropped={dropped})");
        let warns = b.recorder.warns();
        assert!(
            warns
                .iter()
                .any(|w| w.contains("no strategy 'nope'") && w.contains("BUSBAR-7150")),
            "{warns:?}"
        );
        assert_eq!(b.configure(r#"{"policy":"nope"}"#, 9).0, Outcome::Failed);
        assert_eq!(b.order(), vec![1, 0]);
        assert_eq!(b.configure(r#"{"policy":"cheapest"}"#, 9).0, Outcome::Ready);
        assert_eq!(b.order(), vec![0, 1]);
    }
}

#[test]
fn hostile_policy_words_and_keys_cannot_forge_diagnostics() {
    for dropped in [false, true] {
        let b = bind(dropped);
        assert_eq!(b.open(r#"{"policy":"cheapest"}"#), Outcome::Ready);
        for settings in [
            r#"{"policy":"x\r\nBUSBAR-9999 forged"}"#,
            r#"{"x\r\nBUSBAR-9999 forged":1}"#,
            r#"{"expected_units":{"x\r\nBUSBAR-9999 forged":-1}}"#,
        ] {
            assert_eq!(b.configure(settings, 2).0, Outcome::Failed);
            assert_eq!(b.order(), vec![0, 1]);
        }
        let warns = b.recorder.warns();
        assert_eq!(warns.len(), 3);
        for warning in warns {
            assert!(!warning.contains(['\r', '\n']), "{warning:?}");
            assert!(warning.contains("\\r\\n"), "{warning:?}");
            assert!(warning.contains("BUSBAR-9999 forged"), "{warning:?}");
        }
    }
}
