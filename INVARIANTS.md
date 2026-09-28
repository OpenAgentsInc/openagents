# Invariants

This ledger records invariants of this repository whose change is a policy
change. The workspace-level guide is `INVARIANTS.md` at the workspace root.
When a change adds, removes, relaxes, or reinterprets an invariant here,
update this file in the same change and name the test that checks it.

## Remote task creation

| Invariant | Status | Checked by |
| --- | --- | --- |
| NIP-HOST `task.create` from an enrolled device is an inert inbox submission: it records intent and grants no execution authority. | Relaxed on 2026-09-27 by the owner's auto-start policy ([#9735](https://github.com/OpenAgentsInc/openagents/issues/9735)). Holds whenever the policy is absent, unreadable, or off. | `without_a_policy_creation_is_inert_and_unchanged` and `creation_is_inert_idempotent_and_bound_to_labels` in `crates/coder` |
| Only the host's owner, with a command on the host, turns auto-start on or widens it. A device sends only a workspace label, a title, and a prompt, and cannot choose the engine, model, or limits. | New on 2026-09-27. | `the_command_line_turns_the_policy_on_and_off`, `policies_outside_their_bounds_refuse` |
| An auto-started task runs only in a workspace the policy lists and the host admits, under a normal operator execution grant that the task owner admits with every usual check. | New on 2026-09-27. | `a_policy_starts_admitted_tasks_within_its_bounds_and_records_each` |
| At most `max_running` auto-started tasks run at once, and every eligible, started, skipped, refused, and no-capacity decision is appended to the host's `autostart.jsonl` before the next. | New on 2026-09-27; `no_capacity` added on 2026-09-28. | `a_policy_starts_admitted_tasks_within_its_bounds_and_records_each`, `turning_the_policy_off_stops_new_starts_and_cancelled_tasks_are_skipped`, `without_capacity_a_task_ends_as_no_capacity_with_the_reset` |
| An auto-started task generates only through a route (provider and model) the owner's policy admits. The host picks the first admitted route that is connected and has capacity; failover during a run picks only among the grant's admitted routes. Neither adds a model, widens a limit, or relaxes a spend bound. | New on 2026-09-28 ([#9831](https://github.com/OpenAgentsInc/openagents/issues/9831)). It reinterprets one admission check: the task's requested model must be one of the grant's admitted routes, not only its primary one. A grant without `fallbacks` admits exactly as before, and a policy without `routes` means exactly what it meant. | `a_task_starts_on_the_first_connected_route_with_capacity`, `an_existing_policy_file_keeps_its_meaning`, `admission_accepts_the_task_model_on_any_admitted_route`, `a_capacity_refusal_fails_over_to_the_next_admitted_route_and_is_recorded` |
| A task that no admitted, connected provider has capacity for does not start a run; it ends as `no_capacity` with the earliest recorded reset, and a device's summary headline says so from typed host state. | New on 2026-09-28. | `without_capacity_a_task_ends_as_no_capacity_with_the_reset`, `with_every_route_exhausted_the_run_ends_as_no_capacity_with_the_earliest_reset` |
| A usage probe reads a provider's OAuth access token only when the owner turned probes on (`coder host autostart on --probe-usage`), only in the `coder` host process, and sends it only to that provider's own usage endpoint (`api.anthropic.com/api/oauth/usage`, `chatgpt.com/backend-api/wham/usage`). The token is never written, logged, stored in `usage.json`, or sent elsewhere, and no credential store is modified. A probed reading is advisory: it only reorders the admitted, connected routes that have no holding refusal, never adds a route or overrides a refusal, and any probe failure falls back to refusal-only routing. | New on 2026-09-28 ([#9837](https://github.com/OpenAgentsInc/openagents/issues/9837)), owner-directed. It widens which process reads the Claude credential: before, only Claude Code read it; now `coder` reads it too, while probes are on. `microcoder` still never reads a credential file. Off by default; a policy without `usage_probe` means exactly what it meant. | `routing_passes_over_a_provider_at_its_probed_limit`, `with_usage_probes_a_start_avoids_a_provider_above_the_threshold`, `a_failing_usage_probe_falls_back_to_refusal_only_routing`, `a_claude_credential_is_read_typed_and_never_printed`, `malformed_answers_are_typed_failures`, `an_existing_policy_file_keeps_its_meaning` |
| Archiving removes a task only from device lists: an archived task's summary is never published again and its transcript lists as an archived chat, while its record, commands, evidence, and transcript stay in place. Only a finished or cancelled task archives, and only the owner restores one. | New on 2026-09-28. | `only_an_ended_task_is_archived_and_nothing_is_deleted`, `an_archived_task_leaves_the_current_list_and_only_an_ended_one_archives` in `crates/coder`; `a_task_the_owner_archived_lists_as_an_archived_chat` in `crates/coder-history`; `the_coder_list_leaves_out_archived_tasks` in `crates/openagents-mobile` |
| A live test or smoke that creates a task on a real host archives it when it ends, whatever its outcome. | New on 2026-09-28. | `archiving` in `crates/openagents-mobile/src/tests.rs`, used by every live test that sends a Coder chat |

See [the auto-start guide](docs/coder/runtime/host-autostart.md).

## Steering

| Invariant | Status | Checked by |
| --- | --- | --- |
| A host never treats a new turn, an enqueue, or a restart as native steering: native steering of a running turn is refused unless the engine steers mid-turn, and emulation (cancel and continue) runs only when the caller chose it. | New on 2026-09-28 ([#9836](https://github.com/OpenAgentsInc/openagents/issues/9836)); restates NIP-SESS for the adapters in code. | `native_steering_an_engine_lacks_is_refused_unless_emulation_was_chosen`, `each_adapter_reports_a_steering_mode_that_matches_what_it_demonstrated`, `microcoder_states_that_it_steers_only_at_a_turn_boundary` |
| An accepted steer is not a consumed one: consumption is recorded only from the adapter's stated acknowledgment, as its own trace step, and a correction stays in the unconsumed ledger until a run's admission reads it. | New on 2026-09-28. | `a_steer_is_consumed_when_the_next_turn_starts_and_recorded_as_its_own_step`, `only_engine_evidence_confirms_consumption` |

## Linking devices

| Invariant | Status | Checked by |
| --- | --- | --- |
| Tailscale, SSH, and a relay only introduce devices. Rights come only from a host-signed NIP-HOST grant, and every invitation's rights are chosen explicitly. | New on 2026-09-27 for `coder link` ([#9731](https://github.com/OpenAgentsInc/openagents/issues/9731)). | `invite_needs_explicit_rights`; `join_check_every_route_list_the_host_and_lose_it_on_revocation` in `crates/coder-setup/tests/link.rs` |
| A host with NIP-HOST tailnet admission on issues a single-use invitation, never a grant, only to a caller that its local `tailscale whois` names as the host machine's own untagged Tailscale user, with the rights the operator chose. Every other caller gets a refusal and no invitation. | New on 2026-09-27, owner-approved. It relaxes the row above only when the operator runs `coder host serve --tailnet-admission RIGHTS`: Tailscale identity then admits the device to an invitation, and the host still signs the grant on redemption. Off by default. | `refuses_malformed_and_non_tailnet_callers_before_whois` and `parses_status_and_whois` in `crates/coder-host`; `live_tailnet_admission_adds_the_computer_and_its_chats` (ignored, live) in `crates/openagents-mobile` |
| The owner secret key stays in one private file on the owner's computer; `coder link` never puts it, or an invitation, in an argument or a log line. | New on 2026-09-27. | `an_open_or_malformed_key_refuses_without_echoing_it`, `remote_commands_quote_every_word_and_expand_only_home` |

## Mobile terminal

| Invariant | Status | Checked by |
| --- | --- | --- |
| The mobile terminal screen opens only under the host grant's `terminal` right; the host checks it on every NIP-TERM request, and a missing right shows its reason rather than a terminal. | New on 2026-09-27 ([#9733](https://github.com/OpenAgentsInc/openagents/issues/9733)). | `a_device_without_the_terminal_right_is_refused_clearly`, `refusals_map_to_clear_phases` in `crates/coder-mobile` and `crates/coder-computers` |
| Output the host discarded is shown as a marked gap, never joined to the output around it, and frames apply once, in sequence order. | New on 2026-09-27. | `output_gaps_and_exit_reach_the_model_in_order`, `a_marker_starts_on_its_own_line_and_resets_the_parser` |
| Terminal input is live only: nothing typed while the screen is not attached is queued or sent later. | New on 2026-09-27. | `a_build_without_the_live_service_refuses_clearly`; `Session::send` refuses unless attached |
| Terminal output is untrusted data: it cannot read or write the phone's clipboard, and a paste cannot end a bracketed paste early. | New on 2026-09-27. | `title_bell_and_ignored_commands`, `a_paste_normalizes_newlines_and_cannot_close_the_bracket` in `crates/coder-vt` |
