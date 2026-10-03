// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! THE DOOR: the ranking hook on the hook kind's memory ABI (`plugin_door!` over the hook table),
//! on the SDK's safe surface: the generic lifecycle (`lifecycle: life(Ranking)`) and safe slots.
//! No `unsafe` here.
//!
//! * `validate` / `open` / `refresh` read the strategy from the settings document,
//!   `{"policy": "<word>"}`, one of [`crate::WORDS`]; anything else refuses in words. A `refresh`
//!   switches the open instance to the word it names.
//! * `decide` ranks the view's candidates ([`crate::rank`]) into the host's order buffer. An order
//!   longer than that buffer answers FAILED naming the slots it needs (the short-buffer rule), and
//!   the host's one re-call carries a buffer that holds it. It reads no prompt and no caller (the
//!   Statement's tail asks for none) and never pends.
//! * `transform` abstains, `notify` and `configure` acknowledge; `status`, `describe` and `serve`
//!   are not served (REFUSED).
//!
//! The Statement claims the four strategy words as hook word marks.

use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};

use busbar_contract::abi::hook::{
    cancel, CandidateDynamic, CandidateStatic, ConfigureIn, ConfigureOut, DecideIn, DecideOut,
    Tail, TransformOut, CANDIDATE_HAS_BUDGET_REMAINING, CANDIDATE_HAS_COST_PER_MTOK,
    CANDIDATE_HAS_LATENCY_MS, CANDIDATE_HAS_RATE_HEADROOM, CLASS_GATE, PROMPT_NO, USER_NO,
    VERB_ABSTAIN, VERB_PREFER,
};
use busbar_contract::abi::mechanism::call::Outcome;
use busbar_contract::abi::mechanism::door::{KindTailHead, MarkWord, Statement, MARK_WORD_HOOK};
use busbar_contract::abi::sdk::door::{abi_str, statement, AbiIn, AbiOut};
use busbar_contract::abi::sdk::life::{Held, Life, Refreshed, Refusal};
use busbar_contract::abi::sdk::{Instance, Lent, Out, SafeSlot};

use crate::{rank, Candidate, Verdict, NAME, WORDS};

/// How many ops one instance holds in flight; the host clamps. Ranking is CPU-only.
pub const MAX_INFLIGHT: u32 = 64;

/// An order gate that reads neither the prompt nor the caller, wants no signal and serves no route.
const TAIL: &Tail = &Tail {
    head: KindTailHead {
        size: std::mem::size_of::<Tail>() as u32,
        _reserved: 0,
    },
    kind_class: CLASS_GATE,
    prompt_access: PROMPT_NO,
    user_access: USER_NO,
    infallible: 0,
    _reserved: [0; 3],
    requested_signals: std::ptr::null(),
    requested_signals_len: 0,
    routes: std::ptr::null(),
    routes_len: 0,
};

const fn word(w: &'static str) -> MarkWord {
    MarkWord {
        class: MARK_WORD_HOOK,
        _reserved: 0,
        word: abi_str(w),
    }
}

/// The hook words, one mark each.
const MARKS: &[MarkWord] = &[
    word(WORDS[0]),
    word(WORDS[1]),
    word(WORDS[2]),
    word(WORDS[3]),
];

/// THE STATEMENT: name, version, the hook tail and the four claimed words.
pub const STATEMENT: Statement = Statement {
    kind_tail: std::ptr::from_ref(TAIL).cast::<KindTailHead>(),
    mark_words: MARKS.as_ptr(),
    mark_words_len: MARKS.len(),
    ..statement(NAME, env!("CARGO_PKG_VERSION"), MAX_INFLIGHT)
};

/// One opened ranking: the strategy its settings named (an index into [`WORDS`]), switched by a
/// `refresh`.
#[derive(Debug)]
pub struct Ranking(AtomicUsize);

/// The strategy `settings` name, as an index into [`WORDS`]; anything else refuses in words.
fn policy(settings: &[u8]) -> Result<usize, Refusal> {
    let doc: serde_json::Value = serde_json::from_slice(settings)
        .map_err(|e| Refusal::failed(format!("hook-ranking: the settings are not JSON: {e}")))?;
    let word = doc
        .get("policy")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Refusal::failed("hook-ranking: settings.policy is required"))?;
    WORDS.iter().position(|w| *w == word).ok_or_else(|| {
        Refusal::failed(format!(
            "hook-ranking: no strategy '{word}' (one of {WORDS:?})"
        ))
    })
}

impl Life for Ranking {
    const CANCEL: u32 = cancel::ABORTED;

    fn validate(settings: &[u8]) -> Result<(), Refusal> {
        policy(settings).map(|_| ())
    }

    fn open(settings: &[u8], _: &[&[u8]], _: u64) -> Result<Self, Refusal> {
        policy(settings).map(|w| Self(AtomicUsize::new(w)))
    }

    fn refresh(&self, settings: &[u8], _: &[&[u8]], _: u64) -> Result<Refreshed, Refusal> {
        self.0.store(policy(settings)?, Ordering::Release);
        Ok(Refreshed::default())
    }
}

impl Ranking {
    fn word(&self) -> &'static str {
        WORDS[self.0.load(Ordering::Acquire)]
    }
}

/// The signals one candidate of the view carries; a signal the host does not have is `None`.
fn candidate(s: &CandidateStatic, d: Option<&CandidateDynamic>) -> Candidate {
    let has = |present: u32, bit: u32| present & bit != 0;
    let d = d.copied();
    Candidate {
        idx: s.idx as usize,
        weight: s.weight,
        cost_per_mtok: has(s.present, CANDIDATE_HAS_COST_PER_MTOK).then_some(s.cost_per_mtok),
        latency_ms: d
            .filter(|d| has(d.present, CANDIDATE_HAS_LATENCY_MS))
            .map(|d| d.latency_ms),
        available_concurrency: d.map_or(0, |d| {
            usize::try_from(d.available_concurrency).unwrap_or(usize::MAX)
        }),
        budget_remaining: d
            .filter(|d| has(d.present, CANDIDATE_HAS_BUDGET_REMAINING))
            .map(|d| d.budget_remaining),
        rate_headroom: d
            .filter(|d| has(d.present, CANDIDATE_HAS_RATE_HEADROOM))
            .map(|d| d.rate_headroom),
    }
}

/// `decide`: the ranking, into the host's order buffer.
pub struct Decide;

impl SafeSlot for Decide {
    type In = DecideIn;
    type Out = DecideOut;
    type State = Held<Ranking>;

    fn call(
        instance: Instance<'_, Held<Ranking>>,
        input: Lent<'_, DecideIn>,
        mut out: Out<'_, DecideOut>,
    ) -> Outcome {
        let Some(held) = instance.get() else {
            return Outcome::Refused;
        };
        let dynamics = input.candidate_dynamics();
        let candidates: Vec<Candidate> = input
            .candidates()
            .iter()
            .enumerate()
            .map(|(i, s)| candidate(s.get(), dynamics.get(i).map(Lent::get)))
            .collect();
        match rank(held.life().word(), &candidates) {
            Some(Verdict::Prefer(order)) => {
                let order: Vec<u32> = order
                    .into_iter()
                    .map(|i| u32::try_from(i).unwrap_or(u32::MAX))
                    .collect();
                let mut buf = input.order_buf();
                buf.extend(&order);
                if !buf.fits() {
                    out.set(|o| &o.order_needed, buf.needed());
                    return Outcome::Failed;
                }
                out.set(|o| &o.order_written, buf.written());
                out.set(|o| &o.verbs, VERB_PREFER);
            }
            Some(Verdict::Abstain) | None => out.set(|o| &o.verbs, VERB_ABSTAIN),
        }
        Outcome::Ready
    }
}

/// `transform`: the ranking rewrites nothing.
pub struct Transform;

impl SafeSlot for Transform {
    type In = DecideIn;
    type Out = TransformOut;
    type State = Held<Ranking>;

    fn call(
        instance: Instance<'_, Held<Ranking>>,
        _: Lent<'_, DecideIn>,
        mut out: Out<'_, TransformOut>,
    ) -> Outcome {
        if instance.get().is_none() {
            return Outcome::Refused;
        }
        out.set(|o| &o.verbs, VERB_ABSTAIN);
        Outcome::Ready
    }
}

/// `configure`: every pushed version is acknowledged (the ranking has nothing to configure).
pub struct Configure;

impl SafeSlot for Configure {
    type In = ConfigureIn;
    type Out = ConfigureOut;
    type State = Held<Ranking>;

    fn call(
        _: Instance<'_, Held<Ranking>>,
        input: Lent<'_, ConfigureIn>,
        mut out: Out<'_, ConfigureOut>,
    ) -> Outcome {
        out.set(|o| &o.acked_version, input.version);
        Outcome::Ready
    }
}

/// An op that answers READY with nothing to say (`notify`: a gate taps nothing).
pub struct Nothing<I, O>(PhantomData<(I, O)>);

impl<I: AbiIn, O: AbiOut> SafeSlot for Nothing<I, O> {
    type In = I;
    type Out = O;
    type State = Held<Ranking>;

    fn call(_: Instance<'_, Held<Ranking>>, _: Lent<'_, I>, _: Out<'_, O>) -> Outcome {
        Outcome::Ready
    }
}

/// An op the ranking does not serve: REFUSED.
pub struct Refuse<I, O>(PhantomData<(I, O)>);

impl<I: AbiIn, O: AbiOut> SafeSlot for Refuse<I, O> {
    type In = I;
    type Out = O;
    type State = Held<Ranking>;

    fn call(_: Instance<'_, Held<Ranking>>, _: Lent<'_, I>, _: Out<'_, O>) -> Outcome {
        Outcome::Refused
    }
}

busbar_contract::plugin_door! {
    ops: busbar_contract::abi::hook::Ops,
    statement: STATEMENT,
    lifecycle: life(Ranking),
    kind_ops: {
        decide: busbar_contract::abi::sdk::Safe<Decide>,
        transform: busbar_contract::abi::sdk::Safe<Transform>,
        notify: busbar_contract::abi::sdk::Safe<Nothing<
            busbar_contract::abi::hook::NotifyIn,
            busbar_contract::abi::mechanism::call::OutHead,
        >>,
        configure: busbar_contract::abi::sdk::Safe<Configure>,
        status: busbar_contract::abi::sdk::Safe<Refuse<
            busbar_contract::abi::mechanism::call::InHead,
            busbar_contract::abi::hook::StatusOut,
        >>,
        describe: busbar_contract::abi::sdk::Safe<Refuse<
            busbar_contract::abi::mechanism::call::InHead,
            busbar_contract::abi::hook::DescribeOut,
        >>,
        serve: busbar_contract::abi::sdk::Safe<Refuse<
            busbar_contract::abi::hook::ServeIn,
            busbar_contract::abi::hook::ServeOut,
        >>,
    },
}
