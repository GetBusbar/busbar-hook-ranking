// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! THE RANKING HOOK'S `configure`, BOTH WAYS (the linked door and the dropped-in cdylib, through
//! the one loader): an ACK means the pushed settings are live, and a NACK says why.
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
use busbar_contract::abi::mechanism::lifecycle::{slot as life, OpenIn, OpenOut};
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
    let dispatcher = Dispatcher::new(DispatchConfig::default());
    let recorder = Arc::new(Recorder::default());
    let bind = Bind {
        instance: Arc::from("ranking"),
        max_inflight_cap: 64,
        sink: recorder.clone(),
        dispatcher: dispatcher.adopter(),
        conns: ConnTable::NoNeeds,
    };
    let plugin = if dropped {
        let path = busbar_plugin_loader::conformance::cdylib_of("busbar_hook_ranking_plugin");
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
        let outcome = self.plugin.call(slot::CONFIGURE, &mut f).outcome;
        (outcome, f.out.acked_version)
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
            signals: SignalBag::new(),
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
        // A push that names no policy acknowledges and changes nothing.
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
            (r#"{"policy": 5}"#, "settings.policy must be a string"),
            (r#"{"policy": "fastest", "x": 1}"#, "unknown setting 'x'"),
        ]
        .into_iter()
        .enumerate()
        {
            let (outcome, _) = b.configure(settings, 2 + n as u64);
            assert_eq!(outcome, Outcome::Failed, "{settings} (dropped={dropped})");
            let warns = b.recorder.warns();
            assert!(
                warns
                    .iter()
                    .any(|w| w.contains("hook-ranking: configure() rejected:") && w.contains(why)),
                "{settings}: {warns:?}"
            );
        }
        assert_eq!(b.order(), vec![0, 1], "a NACK commits nothing");
    }
}
