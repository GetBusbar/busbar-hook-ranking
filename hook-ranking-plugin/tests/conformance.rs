// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! THE BOTH-WAYS CONFORMANCE of `busbar-hook-ranking`: busbar's PUBLISHED suite
//! (`busbar_plugin_loader::conformance_suite!`, at the busbar commit this repo pins in `.busbar-ref`),
//! run by this repo. It loads the LINKED door (the logic crate's `door`, the one a busbar
//! build names) and the BUILT cdylib (`busbar-hook-ranking-plugin`) through busbar's real plugin
//! loader, drives both with the hook kind's script over `conformance.json` (rank, abstain, transform,
//! an order that overflows the host's first buffer and takes the ONE re-call, a refresh to another
//! strategy), and requires every step's crossings exactly at the script's pin and the two folds
//! equal. The suite's RED arms are kept beside it. `plugin-ci.yml` runs it under `--release`.

/// This crate's own door, as the loader names a compiled-in one.
#[allow(unused_unsafe)]
fn loaded() -> busbar_plugin_loader::dispatch::LoadedDoor {
    // SAFETY: `door` is this crate's own `plugin_door!` entry.
    unsafe { busbar_plugin_loader::dispatch::LoadedDoor::from_raw(busbar_hook_ranking::door) }
}

busbar_plugin_loader::conformance_suite! {
    loaded: loaded(),
    cdylib: "busbar_hook_ranking_plugin",
    inputs: include_str!("conformance.json"),
}
