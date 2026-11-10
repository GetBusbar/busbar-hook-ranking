# Ordinary assertion migration

Historical suite: core a35493a5e74cc935ff7dcc15c1af869290da91cb `crates/busbar-test-kit/src/conformance/{mod.rs,hook.rs,red.rs}`.

The generated suite and kit dependencies are removed; ordinary local and acknowledged common-loader owners are listed below. Native qualification remains pending.

| Historical assertion | Destination |
|---|---|
| actual linked/dropped parity | `door_witness::the_linked_and_the_dropped_in_door_rank_every_shape_the_same` |
| absolute rank/verdict expectations | `door_witness::every_strategy_has_its_expected_orders_through_both_doors` |
| first-poll READY | `door_witness::every_strategy_word_answers_decide_ready_on_the_first_poll_never_pending` |
| configured ranking changes, errors preserve old state, malformed/overflow/unknown recipe costs | existing ordinary `door_witness.rs` and `configure_witness.rs` tests, unchanged |
| altered ranking RED | `door_witness::red_a_door_that_ranks_the_other_way_answers_differently`, unchanged |
| actual lifecycle/operation crossings | the `checked_call` helper (used by the ordinary `door_witness.rs` tests) reads actual public `Plugin.inner.crossings/resumes`, pins one entry for plugin calls and zero for host-refused unopened/closed requests; pure ranking resumes zero |
| lifecycle validate/open bad inputs and later valid open, ready, tick, refresh and close | `ordinary_lifecycle_and_management_preserve_the_published_ranking_contract`: all original bad settings (including the bogus-policy and unknown-key cases), exact outcomes and leases, each refusal's distinguishing reason text for validate and open, and Ready operations |
| every original request and refreshed policy | same ordinary test: cheapest three `[9,3,7]` twice, no-cost abstain, cheapest two `[9,3]`, transform abstain, fastest refreshed `[9,7,3]` |
| short order buffer with one grown recall, repeated-short Fault | the `ticketed_decide` helper, driven by the same ordinary test: actual `DecideFrame`/Lent, submit_watched on a ticket, same-ticket retry, exact needed/written, 2 physical entries; short-again same capacity faults at second entry |
| management configure version, notify, status/describe valid JSON leases and release | same ordinary test; configure ack2, ticketed notify Ready, status/describe Ready, each answering exactly `{}` while its lease is held, each lease released |
| declared needs and requested release profile | `declared_needs_and_requested_profile_match_the_actual_ranking_image`: `declares.json` and the linked Statement state no needs, and both images load as hooks (the dropped image is admitted only against the linked rendering). The profile check covers the test binary only (with `BUSBAR_EXPECT_RELEASE` set it must be a release build); the image's own build profile is not inspected |
| kind ABI ±1, layout, Statement mismatch, wrong kind, connection isolation, failing ready, no-table | local metadata tests and acknowledged common-loader owners below |
| moved count comparator RED | retired framework selfcheck; actual crossing checks preserved |
| canary | The original ranking fixture had no `credentials`; exact kit canary returns after asserting kind is not Secret and contributes no product/log scan. Existing configure witness Recorder preserves diagnostic behavior. No configured credential assertion retired. |
| plane-only class/layout, transport-only frame-piece, outbound-auth three cases, newer-schema store case | no ranking product operation; generic suite kind-conditional no-op |

All actual-image callers in `door_witness.rs` and `configure_witness.rs` now require BUSBAR_PLUGIN_CDYLIB absolute existing actual ranking artifact. CI must freshly build the real `busbar-hook-ranking-plugin` with dropped-in feature before testing. Missing image fails; no fallback.

Validation: rustfmt, git diff --check. Native testing has not run; centralized lane required.

All ordinary ticketed calls assert zero pending resumes and zero deadline completions without wake; actual ready crossings are exactly `has_ready` and facts cause zero entries. Local metadata and acknowledged common-loader tests now replace the original generic-negative suite; no SDK ref/transport pin/lock retarget occurs, and native compilation/execution remains unrun pending centralized build lane. The local ticketed helper drives only this real ranking product; it contains no copied hook implementation, suite registry, or general-purpose test framework.

## Monday-audit composition and ranking decision

Normal merge parents: structural source `dbdaab528518d5e01e3a153b4456a96c1301ddb7` and exact audit donor `97bb48f930061112f0a3e02be1bc5ade46fe9280`. Root decision: legacy 1.5.5 had a native policy registry, not configure/version semantics. Preserve later optional-policy/expected_units/complete-checkpoint APIs; validate present grammar before acknowledging a stale version. No claim that existing accepted ancestry establishes legacy authority.

Conflict dispositions:

- `door.rs` module prose and Ranking representation: preserve complete Configuration/version/revision/profile state and checkpoint API. Mandatory-policy configure prose is superseded by the explicit decision. OPEN still requires policy.
- `apply_configuration`: validate `configured(settings)` and `expected_units(settings)` before stale-version comparison. A valid stale push returns the existing complete checkpoint without changing strategy, version, revision or profile. Any invalid push is a NACK; diagnostic emission still occurs after releasing the mutex. Equal/newer valid partial pushes retain omitted fields and publish complete checkpoints atomically.
- `configured` and OPEN hunks: optional policy remains for CONFIGURE; unknown keys and invalid present policy refuse. OPEN continues requiring policy and accepts the independent expected_units configuration.
- `door_tests.rs`: retain checkpoint revision-zero/equal-version, replay, revision-exhaustion and size-refusal tests. Adapt donor policyless-NACK expectation to the decided valid partial-push behavior; retain donor malformed-stale NACK, bare-refusal, equal-version and initial-version-zero tests against the complete state representation. New stale checks assert all state fields remain unchanged.
- `configure_witness.rs`: retain both-door complete-checkpoint/profile/recovery tests; valid stale settings return the identical old checkpoint, malformed stale settings NACK with no checkpoint extension, subsequent stale-valid read proves no publication/state change. Keep donor stale-invalid logging checks, hostile diagnostic escaping, and all invalid-present-policy/unknown-key cases. Remove only mandatory-policy expectations explicitly contradicted by the decision; empty partial pushes remain tested as successful retention.
- `door_witness.rs`: preserve ordinary lifecycle/ticketed tests, generic-cost/profile tests and actual counters; retain donor pure ranking reference comparison, exact Failed invalid-open expectation and verb distinctions.
- `README.md` and `declares.json`: describe validation-before-stale with optional policy and complete checkpoints; retain generic-cost documentation.
- bad settings: preserve donor's additional bogus-policy and unknown-key cases (now inline in `door_witness.rs`; the `conformance.json` fixture, whose requests ranked by the unused scalar `cost_per_mtok`, is deleted).

Source formatting, TOML/JSON parsing and merge-marker checks only. No native execution, no peer-qualified claim, no framework removal and no SDK3/public-pin changes.

## Framework removal and loader-negative ownership

The framework macro and busbar-test-kit manifest edges are removed. Locked package records were reconciled by dependency reachability from the real workspace crates; retained versions and source URLs are unchanged. All three Busbar kit packages are absent from these manifests and locks. Native validation is pending the exclusive coordinated lane; source migration is not a GREEN/native/release claim.

| Original generated loader assertion | Ordinary destination |
| --- | --- |
| ABI +/-1, pack error, linked admission, dropped manifest exact version refusal | Local `the_actual_kind_abi_is_checked_by_pack_linked_and_dropped_origins` |
| Kind layout xor1/zero, pack error and rebuild text, linked under honest and own rows | Local `the_actual_kind_layout_is_checked_under_honest_and_own_rows` |
| Changed row/manifest byte and restated actual version under honest row | Local `the_actual_statement_matches_each_row_manifest_and_restated_version` |
| Actual image requested as every other kind, exact linked/dropped refusal and honest twins | Local `the_actual_image_is_refused_as_every_other_kind_through_both_origins` and metadata tests' honest actual loads |
| Distinct physical linked/dropped door and operation-table addresses | Local `the_actual_dropped_image_is_distinct_from_the_linked_image` |
| Failed ready returns its reason and is entered once | Core `8227f0138ef2e44d4cefd25ce8fd3d51a9604b6a`, `crates/plugin-loader/src/tests/ordinary_load_negatives.rs::actual_transport_lifecycle_surrounds_one_failed_ready_without_retry`; common-loader SOURCE ownership ACK by doc_macro_bindings |
| Undeclared need and another instance's read/write/close, owner connection remains live | Actual GitHub `abd7663be59b670c8cceabaf30475334e771c566`, `auth-github-plugin/tests/support/remaining.rs::custody::github_connections_refuse_undeclared_needs_and_other_instances` |
| Named NoConnectionTable refusal and Probe success | Same actual GitHub source, `auth-github-plugin/tests/conformance.rs::github_serving_requires_its_connection_table_through_both_doors` |

The three connection predicates have exact SOURCE_EQUIVALENCE_ACK in `A/diet-loader-d306/transfer-packets/generic-connection-negative-transfer-ack.json`, SHA256 `ef00f652e3a7fa0f3f762e59f98e958fc586b225b3b7d1c1f351beaa75773573`; current composed GitHub `b6c823d90ef092920b74339dbe9a68f17b766a20` retains them. Their native qualification is separately pending the auth owner, not claimed here. These common mechanics are tested on an actual declared-needs product instead of grafting a synthetic network capability onto this no-needs product. The local metadata tests retain all per-product pack/origin oracles excluded from that ACK: immutable actual product tables are unchanged, only rejected numeric declarations/version differ. No shared replacement suite or fake product exists.

The moved-count RED comparator test is retired with its deleted comparator implementation; ordinary tests assert actual physical crossing/resume counts directly. Plane-only unreported classes/layout, transport-only frame-piece, outbound-auth-only three arms and store-only newer-schema branches executed no product assertions for this kind and are retired as such. No behavior failure or security oracle is silently retired.

The pinned reusable CI workflow still contains historical kit-name/comparator requirements; root renderer owns replacement and canonical fresh artifact wiring before eventual SDK qualification. No public pin is changed here.

Native diagnostic refinement: the historical last-byte Statement mutation corrupts the actual rendering's trailing claims count and gets Rendering refusal before equality comparison. The ordinary mismatch tests now change one same-length actual version-string byte, first proving the mutated rendering still parses, then retaining the exact linked/dropped StatementMismatch and separately restated-version refusal. This repairs the negative input to reach its original equality oracle; it does not weaken the expected refusal.


## Loader Hook ownership follow-up

Core's Hook loader checks now use its existing SDK Gate witness. The exact Ranking
inputs formerly embedded in `hook_door_conformance_tests.rs::ranking_decide` and
`hook_door_tests.rs::a_linked_and_a_dropped_in_hook_open_through_the_axis_and_decide_identically`
are preserved by `door_witness::migrated_loader_cost_answers_preserve_every_output_through_both_doors`.
Both actual origins see candidates with legacy scalar costs `[2, 9]` and, separately,
absent recipes or recipe costs `[9, 2]`; expected results remain abstain/empty order
and prefer/`[1, 0]`. All three output-buffer bounds, zero rejection/tag writes,
READY/no-error outcomes, and the complete result transcript remain checked. Missing,
numeric, and unknown policy validation reasons also remain checked through both doors.
Existing ordinary lifecycle/configure witnesses continue to own policy management;
core's pending-ticket, session forwarding, route metadata, aliases, and lease checks
retain their loader responsibilities using the neutral witness.
