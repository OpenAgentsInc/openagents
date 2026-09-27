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
| At most `max_running` auto-started tasks run at once, and every eligible, started, skipped, and refused decision is appended to the host's `autostart.jsonl` before the next. | New on 2026-09-27. | `a_policy_starts_admitted_tasks_within_its_bounds_and_records_each`, `turning_the_policy_off_stops_new_starts_and_cancelled_tasks_are_skipped` |

See [the auto-start guide](docs/coder/runtime/host-autostart.md).

## Linking devices

| Invariant | Status | Checked by |
| --- | --- | --- |
| Tailscale, SSH, and a relay only introduce devices. Rights come only from a host-signed NIP-HOST grant, and every invitation's rights are chosen explicitly. | New on 2026-09-27 for `coder link` ([#9731](https://github.com/OpenAgentsInc/openagents/issues/9731)). | `invite_needs_explicit_rights`; `join_check_every_route_list_the_host_and_lose_it_on_revocation` in `crates/coder-setup/tests/link.rs` |
| The owner secret key stays in one private file on the owner's computer; `coder link` never puts it, or an invitation, in an argument or a log line. | New on 2026-09-27. | `an_open_or_malformed_key_refuses_without_echoing_it`, `remote_commands_quote_every_word_and_expand_only_home` |
