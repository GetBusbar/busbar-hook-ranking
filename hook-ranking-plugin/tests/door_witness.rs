// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! THE RANKING HOOK'S CONFORMANCE: its door, BOTH WAYS (#2 rule (1): compiled in or dropped in,
//! one contract, one loading path), and RANKING NEVER PENDS (ARCHITECT Q-SO9).
//!
//! * [`the_linked_and_the_dropped_in_door_rank_every_shape_the_same`]: the linked door
//!   (`busbar_hook_ranking::door`) and this crate's dropped-in image (its built cdylib, the same door behind
//!   `export_door!`), each admitted through the one loader, run ONE script — a refused open,
//!   then every strategy word opened and asked to `decide` over every candidate shape the ranking
//!   parity cases use. The two transcripts (outcome, verbs, the order written) must be identical.
//! * RED ARMS: [`red_a_door_with_a_real_ranking_bug_answers_differently`] runs the same script over
//!   a rival door that ranks by its own comparator (it can only name the bare `door`, so it cannot
//!   wrap the real one). With no bug it must answer exactly as the real door; with a WRONG TIE-BREAK
//!   (equal keys ordered by the largest index) or a NaN KEY DEMOTED to the end (1.5.5 ties it with
//!   every number), its transcript must differ, at the tie shape and the NaN shape and nowhere
//!   else. A door that mis-ranks a tie or a NaN cannot pass for this one.
//! * [`every_strategy_word_answers_decide_ready_on_the_first_poll_never_pending`]: through both
//!   doors, `decide` answers READY on the FIRST poll for every word and shape. Ranking is pure
//!   compute: an answer of PENDING would hand the call to the dispatcher's deadline, where 1.5.5's
//!   in-process ranking could never time out. RED by making the door's `decide` answer
//!   `Poll::Pending`: this test fails on the first word.

use std::sync::Arc;
use std::task::Poll;

use busbar_contract::abi::hook::{slot, DecideOut};
use busbar_contract::abi::host::hook::{DecideFrame, DecideView};
use busbar_contract::abi::mechanism::call::{Blob, Outcome, BLOB_JSON};
use busbar_contract::abi::mechanism::lifecycle::{slot as life, OpenIn, OpenOut};
use busbar_contract::abi::sdk::door::blank_out;
use busbar_contract::abi::sdk::exchange::Op;
use busbar_contract::abi::sdk::hook::{
    Decoded, DecodedCandidate, Hook as HookImpl, HookOpen, Verdict,
};
use busbar_contract::hooks::{Candidate, RoutingContext, RoutingRequest};
use busbar_contract::SignalBag;
use busbar_plugin_loader::dispatch::kinds::hook::Hook;
use busbar_plugin_loader::dispatch::{
    in_head, load_dropped, load_linked, out_head, rendering_of_loaded, Bind, ConnTable,
    DispatchConfig, Dispatcher, Frame, LinkedRow, LoadedDoor, NoSink, Plugin,
};

/// The four strategy words the door's Statement claims.
const WORDS: [&str; 4] = ["cheapest", "fastest", "least_busy", "usage"];

fn bind(d: &Dispatcher) -> Bind {
    Bind {
        instance: Arc::from("ranking"),
        max_inflight_cap: 64,
        sink: Arc::new(NoSink),
        dispatcher: d.adopter(),
        conns: ConnTable::NoNeeds,
    }
}

/// This crate's own bare `door`, as the loader names a compiled-in one.
#[allow(unused_unsafe)]
fn loaded() -> LoadedDoor {
    // SAFETY: `door` is this crate's own `plugin_door!` entry.
    unsafe { LoadedDoor::from_raw(busbar_hook_ranking::door) }
}

/// `row`, linked and bound on `d`.
fn linked_row(row: LinkedRow, d: &Dispatcher) -> Plugin<Hook> {
    load_linked::<Hook>(&row, bind(d)).expect("the door loads")
}

/// The linked door, bound on `d`.
fn linked(d: &Dispatcher) -> Plugin<Hook> {
    linked_row(
        LinkedRow::of_loaded(loaded()).expect("the door states itself"),
        d,
    )
}

/// This crate's dropped-in image, the cdylib `cargo test` builds (`busbar_hook_ranking_plugin`),
/// admitted against the Statement the linked door renders. A missing artifact is a failure, never a
/// skip: this test IS the dropped-in door's proof.
fn dropped(d: &Dispatcher) -> Plugin<Hook> {
    let path = busbar_plugin_loader::conformance::cdylib_of("busbar_hook_ranking_plugin");
    let stated = rendering_of_loaded(loaded()).expect("the door renders its Statement");
    load_dropped::<Hook>(&path, &stated, bind(d)).expect("the dropped door loads")
}

/// `open` over `settings`: its outcome.
#[allow(unused_unsafe)]
fn open(p: &Plugin<Hook>, settings: &str) -> Outcome {
    let mut f = Frame::new(
        OpenIn {
            head: in_head(),
            host: std::ptr::null(),
            // SAFETY: `settings` is borrowed for this call, which reads the blob and returns.
            settings: unsafe {
                Blob::from_raw_parts(settings.as_ptr(), settings.len(), BLOB_JSON, 0)
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
    p.call(life::OPEN, &mut f).outcome
}

fn request() -> RoutingRequest<'static> {
    RoutingRequest {
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
    }
}

/// `(idx, cost, latency, concurrency, rate headroom)`.
type Row = (usize, Option<f64>, Option<f64>, usize, Option<f64>);

/// The candidate shapes the ranking parity cases rank: signals present, partly absent, all absent,
/// a non-finite key, all saturated, one member, none.
const SHAPES: &[&[Row]] = &[
    &[
        (0, Some(15.0), Some(120.0), 2, Some(0.10)),
        (1, Some(3.0), Some(40.0), 9, Some(0.90)),
        (2, None, None, 5, None),
    ],
    &[(0, None, None, 1, None), (1, None, None, 1, None)],
    &[
        (0, Some(f64::NAN), Some(f64::INFINITY), 0, Some(f64::NAN)),
        (1, Some(1.0), None, 0, None),
    ],
    &[(0, Some(5.0), Some(30.0), 3, Some(0.5))],
    &[],
    // Every key equal on every signal: only the tie-break orders it.
    &[
        (0, Some(3.0), Some(40.0), 4, Some(0.5)),
        (1, Some(3.0), Some(40.0), 4, Some(0.5)),
        (2, Some(3.0), Some(40.0), 4, Some(0.5)),
    ],
];

/// The shape holding a `NaN` key: 1.5.5 ties it with every number.
const NAN_SHAPE: usize = 2;
/// The shape whose keys all tie.
const TIE_SHAPE: usize = 5;

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

fn ctx() -> RoutingContext<'static> {
    RoutingContext {
        pool: "p",
        budget_remaining: None,
        budget: &[],
    }
}

/// One `decide` over `rows`: its outcome, verbs and the order it wrote.
fn decide(p: &Plugin<Hook>, rows: &[Row]) -> (Outcome, u32, Vec<usize>) {
    let frame = DecideFrame::first(DecideView::build(&request(), &candidates(rows), &ctx()));
    let mut f = Frame::new(frame.input(), blank_out::<DecideOut>());
    let outcome = p.call(slot::DECIDE, &mut f).outcome;
    (outcome, f.out.verbs, frame.order(f.out.order_written))
}

/// THE SCRIPT over the door `load` binds, one line per answer: an open that names no strategy,
/// then every strategy word opened and asked to rank every shape.
fn script(load: impl Fn(&Dispatcher) -> Plugin<Hook>) -> Vec<String> {
    let d = Dispatcher::new(DispatchConfig::default());
    let mut lines = vec![format!(
        "open nope: {:?}",
        open(&load(&d), r#"{"policy": "nope"}"#)
    )];
    for word in WORDS {
        let p = load(&d);
        lines.push(format!(
            "open {word}: {:?}",
            open(&p, &format!(r#"{{"policy": "{word}"}}"#))
        ));
        for (n, rows) in SHAPES.iter().enumerate() {
            let (outcome, verbs, order) = decide(&p, rows);
            lines.push(format!(
                "{word} shape {n}: {outcome:?} verbs={verbs:#x} order={order:?}"
            ));
        }
    }
    lines
}

#[test]
fn the_linked_and_the_dropped_in_door_rank_every_shape_the_same() {
    let linked = script(linked);
    assert_ne!(
        linked[0], "open nope: Ready",
        "a word no strategy spells opens nothing"
    );
    // The script reached real rankings: equal transcripts of abstains would prove nothing.
    assert!(
        linked
            .iter()
            .any(|l| l.contains("order=[1, 0") || l.contains("order=[0, 1")),
        "{linked:#?}"
    );
    assert_eq!(script(dropped), linked, "the dropped-in door");
}

/// Which bug a rival door ranks with.
#[derive(Clone, Copy)]
enum Bug {
    /// None: the comparator the real door uses (1.5.5's).
    Faithful,
    /// Equal keys ordered by the LARGEST index, not the smallest.
    TieByLargestIdx,
    /// A `NaN` key demoted behind every number, where 1.5.5 ties it with every number.
    NanLast,
}

/// The rival ranker: the four words over the same keys as the real door, ordered by its own
/// comparator, with the chosen bug.
#[allow(clippy::cast_precision_loss)]
fn rival_rank(word: &str, candidates: &[DecodedCandidate<'_>], bug: Bug) -> Verdict {
    use std::cmp::Ordering::{Equal, Greater, Less};
    let (descending, mut keyed): (bool, Vec<(usize, Option<f64>)>) = match word {
        "cheapest" => (
            false,
            candidates
                .iter()
                .map(|c| (c.idx, c.cost_per_mtok))
                .collect(),
        ),
        "fastest" => (
            false,
            candidates.iter().map(|c| (c.idx, c.latency_ms)).collect(),
        ),
        "least_busy" => (
            true,
            candidates
                .iter()
                .map(|c| (c.idx, Some(c.available_concurrency as f64)))
                .collect(),
        ),
        "usage" => (
            true,
            candidates
                .iter()
                .map(|c| (c.idx, c.rate_headroom))
                .collect(),
        ),
        _ => return Verdict::Abstain,
    };
    if keyed.iter().all(|(_, k)| k.is_none()) {
        return Verdict::Abstain;
    }
    keyed.sort_by(|(ia, ka), (ib, kb)| {
        let tie = if matches!(bug, Bug::TieByLargestIdx) {
            ib.cmp(ia)
        } else {
            ia.cmp(ib)
        };
        match (ka, kb) {
            (Some(a), Some(b)) => {
                if matches!(bug, Bug::NanLast) && a.is_nan() != b.is_nan() {
                    return if a.is_nan() { Greater } else { Less };
                }
                let (x, y) = if descending { (b, a) } else { (a, b) };
                x.partial_cmp(y).unwrap_or(Equal).then(tie)
            }
            (Some(_), None) => Less,
            (None, Some(_)) => Greater,
            (None, None) => tie,
        }
    });
    Verdict::Prefer(keyed.into_iter().map(|(idx, _)| idx).collect())
}

/// A rival ranking door named `$name`: the same name, hook tail and claimed words as the real
/// door, every strategy word opened, ranking by [`rival_rank`] with `$bug`.
macro_rules! rival_door {
    ($name:ident, $bug:expr) => {
        mod $name {
            use busbar_contract::abi::hook::{Tail, CLASS_GATE, PROMPT_NO, USER_NO};
            use busbar_contract::abi::mechanism::door::{MarkWord, Statement, MARK_WORD_HOOK};
            use busbar_contract::abi::sdk::door::{abi_str, statement};
            use busbar_contract::abi::sdk::hook::{statement_with_tail, tail};

            const TAIL: &Tail = &tail(CLASS_GATE, PROMPT_NO, USER_NO);

            const fn word(w: &'static str) -> MarkWord {
                MarkWord {
                    class: MARK_WORD_HOOK,
                    _reserved: 0,
                    word: abi_str(w),
                }
            }

            const MARKS: &[MarkWord] = &[
                word(crate::WORDS[0]),
                word(crate::WORDS[1]),
                word(crate::WORDS[2]),
                word(crate::WORDS[3]),
            ];

            const STATEMENT: Statement =
                statement_with_tail(statement("hooks-ranking", "1.6.0", 64), TAIL)
                    .with_mark_words(MARKS);

            struct Rival(&'static str);

            impl crate::HookImpl for Rival {
                fn decide(
                    &self,
                    view: &crate::Decoded<'_>,
                    _: &crate::Op<'_>,
                ) -> crate::Poll<crate::Verdict> {
                    crate::Poll::Ready(crate::rival_rank(self.0, &view.candidates, $bug))
                }
            }

            pub struct Open;

            impl crate::HookOpen for Open {
                fn open(settings: &str) -> Result<Box<dyn crate::HookImpl>, String> {
                    let doc: serde_json::Value =
                        serde_json::from_str(settings).map_err(|e| e.to_string())?;
                    let word = doc["policy"]
                        .as_str()
                        .ok_or("settings.policy is required")?;
                    let word: &'static str = crate::WORDS
                        .iter()
                        .copied()
                        .find(|w| *w == word)
                        .ok_or_else(|| format!("no strategy '{word}'"))?;
                    Ok(Box::new(Rival(word)))
                }
            }

            busbar_contract::hook_door! {
                open: Open,
                statement: STATEMENT,
            }
        }
    };
}

rival_door!(faithful, crate::Bug::Faithful);
rival_door!(wrong_tie, crate::Bug::TieByLargestIdx);
rival_door!(nan_last, crate::Bug::NanLast);

/// The script over the rival door `$door`.
macro_rules! rival_script {
    ($door:ident) => {
        script(|d| {
            linked_row(
                LinkedRow::of_macro($door::MACRO_DOOR).expect("the door states itself"),
                d,
            )
        })
    };
}

/// RED: a door that ranks by a comparator with a REAL bug answers differently. The rival with no
/// bug answers as the real door does, line for line (so any difference below is the bug); a wrong
/// tie-break differs where keys tie only; a NaN key demoted to the end differs at the NaN shape
/// of `cheapest` only.
#[test]
fn red_a_door_with_a_real_ranking_bug_answers_differently() {
    let honest = script(linked);
    assert_eq!(
        rival_script!(faithful),
        honest,
        "the rival with no bug ranks as the real door does"
    );

    let tie = rival_script!(wrong_tie);
    assert_eq!(tie.len(), honest.len());
    let differ: Vec<(&String, &String)> = honest.iter().zip(&tie).filter(|(a, b)| a != b).collect();
    assert!(
        !differ.is_empty(),
        "a wrong tie-break must answer differently"
    );
    // The tie shape, for every word; and `least_busy` over the NaN shape, whose two members both
    // hold zero permits and so tie too.
    assert!(
        differ
            .iter()
            .all(|(a, _)| a.contains(&format!(" shape {TIE_SHAPE}:"))
                || a.starts_with(&format!("least_busy shape {NAN_SHAPE}:"))),
        "the wrong tie-break differs where keys tie, nowhere else: {differ:#?}"
    );
    assert_eq!(
        differ
            .iter()
            .filter(|(a, _)| a.contains(&format!(" shape {TIE_SHAPE}:")))
            .count(),
        WORDS.len(),
        "every word mis-orders the all-tied shape: {differ:#?}"
    );

    let nan = rival_script!(nan_last);
    assert_eq!(nan.len(), honest.len());
    let differ: Vec<(&String, &String)> = honest.iter().zip(&nan).filter(|(a, b)| a != b).collect();
    assert_eq!(
        differ.len(),
        1,
        "a NaN demoted to the end must answer differently, once: {differ:#?}"
    );
    assert!(
        differ[0]
            .0
            .starts_with(&format!("cheapest shape {NAN_SHAPE}:")),
        "{differ:#?}"
    );
}

#[test]
fn every_strategy_word_answers_decide_ready_on_the_first_poll_never_pending() {
    for load in [linked as fn(&Dispatcher) -> Plugin<Hook>, dropped] {
        let d = Dispatcher::new(DispatchConfig::default());
        for word in WORDS {
            let p = load(&d);
            assert_eq!(
                open(&p, &format!(r#"{{"policy": "{word}"}}"#)),
                Outcome::Ready,
                "{word}"
            );
            for rows in SHAPES {
                let (outcome, ..) = decide(&p, rows);
                assert_eq!(
                    outcome,
                    Outcome::Ready,
                    "`{word}` must answer READY on its first poll (pure compute), never \
                     {outcome:?}, over {} candidates",
                    rows.len()
                );
            }
        }
    }
}

/// The door's Statement carries this plugin's whole `declares` document (BUSBAR-1.6.0.md l.246).
#[test]
fn the_statement_carries_the_declares_document() {
    let row = LinkedRow::of_loaded(loaded()).unwrap();
    let read = busbar_contract::abi::mechanism::rendering::read(&row.statement).unwrap();
    assert_eq!(
        busbar_contract::abi::mechanism::extensions::declares(&read.extensions.bytes),
        Some(&include_bytes!("../../hook-ranking/declares.json")[..]),
    );
}
