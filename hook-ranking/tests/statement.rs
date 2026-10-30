// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! The Statement the ranking door states, read back field by field: the four hook word marks and
//! the hook tail (class, prompt grant, user grant), as the plugin states them.

use busbar_contract::abi::hook::{Tail, CLASS_GATE, PROMPT_NO, USER_NO};
use busbar_contract::abi::mechanism::door::MARK_WORD_HOOK;
use busbar_hook_ranking::door::STATEMENT;
use busbar_hook_ranking::{NAME, WORDS};

/// Every claimed word is one hook-class word mark, in `WORDS`' order, and no other mark is stated.
#[test]
fn the_statement_marks_the_four_words_as_hook_words() {
    assert_eq!(WORDS, ["cheapest", "fastest", "least_busy", "usage"]);
    assert_eq!(STATEMENT.mark_words_len, 4);
    // SAFETY: `mark_words` is the door's `'static` mark table of `mark_words_len` entries.
    let marks =
        unsafe { std::slice::from_raw_parts(STATEMENT.mark_words, STATEMENT.mark_words_len) };
    let stated: Vec<(u32, String)> = marks
        .iter()
        .map(|m| {
            // SAFETY: each mark's word is a `'static` UTF-8 string of its stated length.
            let bytes = unsafe { std::slice::from_raw_parts(m.word.ptr, m.word.len) };
            (
                m.class,
                String::from_utf8(bytes.to_vec()).expect("a UTF-8 word"),
            )
        })
        .collect();
    let want: Vec<(u32, String)> = WORDS
        .iter()
        .map(|w| (MARK_WORD_HOOK, (*w).to_owned()))
        .collect();
    assert_eq!(stated, want);
}

/// The tail: an order gate (not a tap) that asks for no prompt and no user view.
#[test]
fn the_statement_tail_is_a_gate_with_no_prompt_and_no_user_grant() {
    assert!(!STATEMENT.kind_tail.is_null(), "the hook tail is stated");
    // SAFETY: the hook door's `kind_tail` points at its `'static` hook `Tail`.
    let tail: &Tail = unsafe { &*STATEMENT.kind_tail.cast::<Tail>() };
    assert_eq!(tail.kind_class, CLASS_GATE);
    assert_eq!(tail.prompt_access, PROMPT_NO);
    assert_eq!(tail.user_access, USER_NO);
    assert_eq!(tail.infallible, 0);
    assert_eq!(tail.requested_signals_len, 0);
    assert_eq!(tail.routes_len, 0);
    // SAFETY: `name` is a `'static` UTF-8 string of its stated length.
    let name = unsafe { std::slice::from_raw_parts(STATEMENT.name.ptr, STATEMENT.name.len) };
    assert_eq!(name, NAME.as_bytes());
}
