// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! THE BOTH-WAYS CONFORMANCE of `busbar-hook-ranking`: busbar's PUBLISHED suite
//! (`busbar_plugin_loader::conformance_suite!`, at the busbar commit this repo pins in `.busbar-ref`),
//! run by this repo. It loads the LINKED door (the logic crate's `linked::door`, the one a busbar
//! build names) and the BUILT cdylib (`busbar-hook-ranking-plugin`) through busbar's real plugin
//! loader, drives both with the hook kind's script over `conformance.json` (rank, abstain, transform,
//! an order that overflows the host's first buffer and takes the ONE re-call, a refresh to another
//! strategy), and requires every step's crossings exactly at the script's pin and the two folds
//! equal. The suite's RED arms are kept beside it. `plugin-ci.yml` runs it under `--release`.

busbar_plugin_loader::conformance_suite! {
    door: busbar_hook_ranking::linked::door,
    cdylib: "busbar_hook_ranking_plugin",
    inputs: include_str!("conformance.json"),
}
