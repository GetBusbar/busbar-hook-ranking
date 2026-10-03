// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Busbar Inc and contributors

//! **THE PUBLISHED CONFORMANCE SUITE, RUN BY THIS PLUGIN** (busbar TODO ABI-b4; OWNER 2026-10-03:
//! plugins test themselves against busbar). busbar's suite, at the commit this repo pins
//! (`.busbar-ref`), drives the ranking hook two ways through the one loader: LINKED (the logic
//! crate's `door::door`) and DROPPED IN (this crate's built cdylib), over the hook kind's script
//! with the request set in `conformance.json` (rank, abstain, transform, the order that overflows
//! the host's first buffer and takes the ONE re-call, a refresh to another strategy); every step's
//! crossings exactly at the script's pin, the two folds equal, and the suite's RED arms kept.
//! `plugin-ci.yml` runs it under `--release`.

busbar_plugin_loader::conformance_suite! {
    door: busbar_hook_ranking::door::door,
    cdylib: "busbar_hook_ranking_plugin",
    inputs: include_str!("conformance.json"),
}
