// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! **ONE RANKING DOOR, BOTH WAYS IN**: the linked door (`busbar_hook_ranking::door::door`) and this
//! crate's built cdylib (the same door behind the one `export_door!`), each admitted through the
//! loader's ONE door validation and driven through its real dispatch, answer alike for every
//! strategy word, and as the logic ranks. Run against the busbar rev this repo pins
//! (`.busbar-ref`).
//!
//! THE RED ARMS, same file: settings the ranking refuses never open (linked and dropped in), the
//! door asked for as another kind is refused (linked by the door's own kind, dropped in by the
//! stated kind before `dlopen`), and the Statement claims exactly the four hook words. A missing
//! cdylib PANICS: this test IS the dropped-in door's proof, and never skips.

use std::path::PathBuf;
use std::sync::Arc;

use busbar_contract::abi::hook::{slot, DecideOut, VERB_ABSTAIN, VERB_PREFER};
use busbar_contract::abi::host::hook::{Caps, DecideFrame, DecideView};
use busbar_contract::abi::mechanism::call::{Blob, Outcome, BLOB_JSON};
use busbar_contract::abi::mechanism::door::MARK_WORD_HOOK;
use busbar_contract::abi::mechanism::lifecycle::{slot as life, OpenIn, OpenOut};
use busbar_contract::abi::mechanism::rendering;
use busbar_contract::abi::mechanism::KindCode;
use busbar_contract::abi::sdk::door::blank_out;
use busbar_contract::hooks::{Candidate, RoutingContext, RoutingRequest};
use busbar_contract::SignalBag;
use busbar_plugin_loader::dispatch::kinds::hook::Hook;
use busbar_plugin_loader::dispatch::kinds::transport::Transport;
use busbar_plugin_loader::dispatch::{
    in_head, load_dropped, load_linked, out_head, rendering_of, Bind, DispatchConfig, Dispatcher,
    Frame, LinkedRow, LoadError, NoSink, Plugin,
};

/// This crate's built cdylib (uplifted or under `deps`, newest wins). A missing artifact is a
/// failure, never a skip: this test IS the dropped-in door's proof.
fn cdylib() -> PathBuf {
    let exe = std::env::current_exe().expect("the test binary has a path");
    let profile = exe
        .parent()
        .and_then(|d| d.parent())
        .expect("target/<profile>");
    let file = busbar_plugin_loader::plugin_library_filename("busbar_hook_ranking_plugin");
    [profile.join(&file), profile.join("deps").join(&file)]
        .into_iter()
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .max()
        .map(|(_, p)| p)
        .unwrap_or_else(|| panic!("the busbar-hook-ranking-plugin cdylib ({file}) is not built"))
}

/// The door's Statement rendering, as the pack tool signs it into the manifest.
fn stated() -> Vec<u8> {
    rendering_of(busbar_hook_ranking::door::door).expect("the door renders its Statement")
}

/// The row a compiled-in build holds for this door.
fn row() -> LinkedRow {
    LinkedRow::of(busbar_hook_ranking::door::door).expect("the door states itself")
}

fn bind(d: &Dispatcher) -> Bind {
    Bind {
        instance: Arc::from("the-instance"),
        max_inflight_cap: 64,
        sink: Arc::new(NoSink),
        dispatcher: d.adopter(),
        conns: None,
    }
}

/// The two doors.
#[derive(Clone)]
enum Arm {
    Linked,
    Dropped(PathBuf),
}

impl Arm {
    fn load(&self, d: &Dispatcher) -> Plugin<Hook> {
        match self {
            Arm::Linked => load_linked::<Hook>(&row(), bind(d)),
            Arm::Dropped(path) => load_dropped::<Hook>(path, &stated(), bind(d)),
        }
        .expect("the door loads")
    }
}

/// `open` over `settings`: `Ok` on READY, else the text the plugin stated.
fn open(p: &Plugin<Hook>, settings: &str) -> Result<(), String> {
    let mut f = Frame::new(
        OpenIn {
            head: in_head(),
            host: std::ptr::null(),
            settings: Blob {
                ptr: settings.as_ptr(),
                len: settings.len(),
                fmt: BLOB_JSON,
                flags: 0,
            },
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
    let called = p.call(life::OPEN, &mut f);
    match called.outcome {
        Outcome::Ready => Ok(()),
        o => Err(format!(
            "{o:?}: {}",
            called
                .error
                .as_deref()
                .map(|e| String::from_utf8_lossy(e).into_owned())
                .unwrap_or_default()
        )),
    }
}

// ── the request a hook is handed ─────────────────────────────────────────────────────────────────

fn request() -> RoutingRequest<'static> {
    RoutingRequest {
        request_id: 42,
        pool: "pool-a",
        ingress_protocol: "openai",
        requested_model: None,
        message_count: 1,
        tool_count: 0,
        has_tools: false,
        total_chars: 5,
        system_chars: 0,
        max_tokens: Some(64),
        stream: false,
        prompt: None,
        identity: None,
        signals: SignalBag::new(),
    }
}

/// `(idx, cost, latency, concurrency, rate headroom)`: each strategy ranks these a different way.
type Row = (usize, Option<f64>, Option<f64>, usize, Option<f64>);

const ROWS: [Row; 3] = [
    (3, Some(5.0), Some(200.0), 2, Some(0.2)),
    (9, Some(2.0), Some(50.0), 1, None),
    (7, None, Some(90.0), 4, Some(0.8)),
];

fn candidates(rows: &[Row]) -> Vec<Candidate<'static>> {
    rows.iter()
        .map(|&(idx, cost, lat, conc, rate)| Candidate {
            idx,
            model: "m",
            provider: "p",
            weight: 1,
            context_max: None,
            tier: None,
            cost_per_mtok: cost,
            tags: &[],
            latency_ms: lat,
            available_concurrency: conc,
            budget_remaining: None,
            rate_headroom: rate,
            signals: SignalBag::new(),
        })
        .collect()
}

const CONTEXT: RoutingContext<'static> = RoutingContext {
    pool: "pool-a",
    budget_remaining: None,
    budget: &[],
};

/// `decide` over `rows`: the order the door answered (`None` = abstained).
fn decide(p: &Plugin<Hook>, rows: &[Row]) -> Option<Vec<usize>> {
    let frame = DecideFrame::new(
        DecideView::build(&request(), &candidates(rows), &CONTEXT),
        Caps {
            order: 8,
            reject_message: 4096,
            restrict_tags: 4096,
            rewrite: 64 * 1024,
        },
    );
    let mut f = Frame::new(frame.input(), blank_out::<DecideOut>());
    assert_eq!(p.call(slot::DECIDE, &mut f).outcome, Outcome::Ready);
    match f.out.verbs {
        v if v == VERB_PREFER => Some(frame.order(f.out.order_written)),
        v if v == VERB_ABSTAIN => None,
        v => panic!("neither prefer nor abstain: {v:#x}"),
    }
}

/// What the logic ranks `rows` to, per word.
const EXPECT: [(&str, [usize; 3]); 4] = [
    ("cheapest", [9, 3, 7]),
    ("fastest", [9, 7, 3]),
    ("least_busy", [7, 3, 9]),
    ("usage", [7, 3, 9]),
];

/// Drive one arm through every word; its transcript.
fn script(arm: &Arm) -> Vec<String> {
    let d = Dispatcher::new(DispatchConfig::default());
    let mut t = Vec::new();
    for (word, order) in EXPECT {
        let p = arm.load(&d);
        open(&p, &format!(r#"{{"policy": "{word}"}}"#)).expect("opens");
        let got = decide(&p, &ROWS).expect("a ranking");
        assert_eq!(got, order, "{word} ranks as the logic does");
        t.push(format!("{word}: {got:?}"));
        // No signal on any member: no opinion, the default SWRR.
        let none: Vec<Row> = ROWS.iter().map(|r| (r.0, None, None, r.3, None)).collect();
        match word {
            "least_busy" => assert!(decide(&p, &none).is_some(), "least_busy always has data"),
            _ => assert_eq!(decide(&p, &none), None, "{word} abstains with no signal"),
        }
        t.push(format!("{word}: abstains without its signal"));
    }
    t
}

#[test]
fn the_linked_and_the_dropped_in_door_rank_alike() {
    assert_eq!(script(&Arm::Linked), script(&Arm::Dropped(cdylib())));
}

/// RED: settings the ranking refuses never open, in its words, both ways.
#[test]
fn red_refused_settings_never_open_both_ways() {
    let d = Dispatcher::new(DispatchConfig::default());
    for arm in [Arm::Linked, Arm::Dropped(cdylib())] {
        for (bad, words) in [
            ("{}", "settings.policy is required"),
            (r#"{"policy": 5}"#, "settings.policy is required"),
            (r#"{"policy": "bogus"}"#, "no strategy 'bogus'"),
            (r#"{"policy": "weighted"}"#, "no strategy 'weighted'"),
            ("not json", "the settings are not JSON"),
        ] {
            let refused = open(&arm.load(&d), bad).expect_err("refused");
            assert!(refused.contains(words), "{bad}: {refused}");
        }
    }
}

/// RED: the Statement claims exactly the four hook words, and the dropped-in image states the
/// same Statement as the linked door.
#[test]
fn red_the_statement_claims_the_four_hook_words() {
    let read = rendering::read(&stated()).expect("the rendering reads");
    let want: Vec<(u32, String)> = ["cheapest", "fastest", "least_busy", "usage"]
        .iter()
        .map(|w| (MARK_WORD_HOOK, (*w).to_string()))
        .collect();
    assert_eq!(read.mark_words, want);
    let dropped = busbar_plugin_loader::dispatch::rendering_of_library(&cdylib())
        .expect("the cdylib opens")
        .expect("the cdylib exports the door");
    assert_eq!(dropped, stated());
}

/// RED: the door asked for as another kind is refused, linked (by the door's own kind) and
/// dropped in (by the stated kind, before `dlopen`).
#[test]
fn red_the_door_asked_for_as_another_kind_is_refused_both_ways() {
    let d = Dispatcher::new(DispatchConfig::default());
    let want = (KindCode::Hook, KindCode::Transport);
    match load_linked::<Transport>(&row(), bind(&d)) {
        Err(LoadError::WrongKind { door, want: asked }) => assert_eq!((door, asked), want),
        other => panic!("the linked door loaded as a transport: {:?}", other.err()),
    }
    match load_dropped::<Transport>(&cdylib(), &stated(), bind(&d)) {
        Err(LoadError::ManifestKind {
            stated,
            want: asked,
        }) => assert_eq!((stated, asked), want),
        other => panic!(
            "the dropped-in door loaded as a transport: {:?}",
            other.err()
        ),
    }
}
