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

See [the auto-start guide](docs/coder/runtime/host-autostart.md).

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
