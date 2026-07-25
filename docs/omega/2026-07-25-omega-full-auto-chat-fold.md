# Folding Full Auto into the Omega chat UI

- Status: implementation record, 2026-07-25
- Packet: `OMEGA-AGENT-02` (OpenAgentsInc/omega#76)
- Product contract: [Omega Agent ProductSpec](../../specs/omega/omega-agent.product-spec.md) revision 1
- Shape record: [Omega Agent shape record](./2026-07-25-omega-agent-shape-record.md)
- Roadmap: [Omega Agent implementation roadmap](./2026-07-25-omega-agent-roadmap.md)
- Deltas: `OMEGA-DELTA-0019`, `OMEGA-DELTA-0020` in the omega repository
- Behavior contracts: `omega_agent.chat_first_front_door.v1`,
  `omega_agent.full_auto_folded_into_chat.v1`

## 1. The direction

The owner, 2026-07-25:

```text
I don't actually want a Full Auto panel, it should be folded into whatever the
chat UI for Omega is - you can decide how to handle this.
```

This sharpens the admitted Omega Agent shape rather than contradicting it.
Full Auto is one of the three admitted executor classes, so a user starting a
lane from chat is the router doing its job: the user asks for an outcome, and
the router dispatches to the `omega-effectd` engine lane.

Before the fold, Omega had two destinations for agent work. Both wore the same
Ω mark. A user had to choose a destination before knowing what they wanted to
do.

## 2. The shape chosen, and the one rejected

**Chosen: Full Auto is a surface of the agent panel.**

`agent_ui::AgentPanel` holds a retained `full_auto_ui::FullAutoPanel` entity
and renders it in place of its thread body when the Full Auto surface is
showing. The dock registration in `crates/zed/src/zed.rs` is gone. Both
`full_auto_panel::` actions are answered by the agent panel instead of by the
retired `full_auto_ui::init`.

Nothing about the Full Auto views changed. They render under a new parent.
That is the whole reason section 3 can claim no control was lost and prove it.

**Rejected: Full Auto as a composer mode flag.**

This is the obvious way to fold a surface into a composer, and it is the wrong
one. A mode flag is a boolean the send path reads, so anything able to set it
can start a run: a slash command, a restored draft, a model-authored composer
insertion. Owner gate 8, restated for this packet, is that *no model-initiated
path can start Full Auto authority. Only an explicit human action can, wherever
that action lives*.

So the fold moves **where the entry lives** and changes nothing about **what
counts as starting a run**:

1. A human opens the Full Auto surface. The draft arrives unsent.
2. A human presses "Start Full Auto".

Two gestures, both human, both on visible controls. `LaunchOrigin` in
`crates/omega_front_door` enumerates every way to reach step 1 and has no
variant for a tool call, slash command, restored draft, agent turn, or mode
flag. `origins_are_all_human_gestures` asserts the set against a written
allowlist, so adding one is a deliberate edit. `only_a_click_listener_starts_a_full_auto_run`
in `crates/omega_deltas` scans every crate and asserts step 2 is reachable from
exactly one place, and that the place is an `on_click` listener.

## 3. The affordance mapping

Every interactive control of the retired panel, and its home now. This table is
executable: `FULL_AUTO_AFFORDANCES` in `crates/omega_front_door` carries it,
and `every_full_auto_affordance_is_mapped` scans `crates/full_auto_ui/src` for
GPUI element ids and fails if one has no row, or if a row names a control that
no longer exists. A ledger that outlives its source stops being evidence, so
both directions are checked.

| Element id | What it does | Home after the fold |
| --- | --- | --- |
| `full-auto-panel` | Launch and run surface root, focus target | Agent panel Full Auto surface |
| `full-auto-openagents-connect` | Connect or reconnect the OpenAgents Sync account | Agent panel Full Auto surface |
| `full-auto-openagents-disconnect` | Revoke the OpenAgents Sync account credentials | Agent panel Full Auto surface |
| `full-auto-provider-account` | One connected provider account: readiness, quota, lane, ref | Agent panel Full Auto surface |
| `full-auto-advanced-toggle` | Reveal title, done condition, turn cap | Agent panel Full Auto surface |
| `full-auto-start` | Start the run — the only path to engine-lane run authority | Agent panel Full Auto surface |
| `full-auto-cancel` | Clear the draft without starting anything | Agent panel Full Auto surface |
| `full-auto-pause` | Pause a running run | Agent panel Full Auto surface |
| `full-auto-resume` | Resume a paused run | Agent panel Full Auto surface |
| `full-auto-handoff` | Hand a paused run to the other local lane | Agent panel Full Auto surface |
| `full-auto-retry` | Retry a stalled run whose recovery action is `retry_now` | Agent panel Full Auto surface |
| `full-auto-stop` | Stop a non-terminal run | Agent panel Full Auto surface |
| `full-auto-new` | Return to the launch surface from a run | Agent panel Full Auto surface |
| `full-auto-evidence-chain` | Host-verified evidence chain for the active run | Agent panel Full Auto surface |
| `full-auto-monitor` | Concurrent run monitor rail | Agent panel Full Auto surface |
| `full-auto-monitor-new` | Start a new draft from the monitor rail | Agent panel Full Auto surface |
| `full-auto-run-row` | Open one run from the monitor rail | Agent panel Full Auto surface |

Non-control capabilities of the retired panel:

| Capability | Home after the fold |
| --- | --- |
| `full_auto_panel::OpenLauncher` action | Answered by `AgentPanel::open_full_auto`. A user keymap naming it still works. |
| `full_auto_panel::ToggleFocus` action | Answered by `AgentPanel::toggle_full_auto`. Same reason. |
| "Full Auto" item in the agent panel's new-thread menu | Unchanged. It dispatched `OpenLauncher` before and still does, and the action now lands in the same panel. |
| Attention notifications for stalled and retrying runs, and `request_attention` | Unchanged. They live on the Full Auto view, which the agent panel hosts. |
| Three-second run refresh poll | Unchanged, same reason. |
| Draft text and selected run surviving a visit to a chat thread | Preserved deliberately: the entity is retained, not dropped, when the surface hides. Dropping it would have been a regression wearing the costume of a cleanup. |
| `Panel` trait implementation on `FullAutoPanel` | Kept. A re-dock is a registration line, not a rewrite. |

## 4. What the fold costs

Stated rather than discovered. `FOLD_COSTS` in `crates/omega_front_door` carries
these and a test requires them to stay written down.

1. **Independent dock placement.** Full Auto had `DockPosition::Right` and a
   520px default width of its own, so it could sit opposite the agent panel. It
   now inherits the agent panel's dock and size. This is the direct consequence
   of the owner's direction.

2. **Simultaneous full detail.** A separate dock panel could show a run's full
   detail while the agent panel showed a chat thread. One panel shows one
   surface at a time, so reading a run in full while typing in a thread is no
   longer possible. The monitor rail still lists active runs and the attention
   notifications still fire, so *noticing* a run is preserved. *Reading one in
   full alongside a thread* is not.

Neither is a control, which is why the mechanical ledger cannot see them and
why they are written here.

## 5. Disclosure is a record, not a label

The owner admitted the Omega Agent shape on 2026-07-25 on the recorded
condition that the first-party agent does not sign with its own principal *and*
that disclosure is stored as a typed record a label renders.

That condition is load-bearing, not decorative. It is the only reason the
identity choice stays cheap to reverse: moving to a signing principal later
then needs a signer, not a rewrite of every stored thread record. A label
string would silently convert a reversible decision into an irreversible one.

`ExecutorDisclosure` in `crates/omega_front_door` fixes that shape for
`OMEGA-AGENT-03` (omega#77) to populate:

- `class` is a closed enum over the three admitted executor classes, so a
  fourth needs a spec revision rather than a new string.
- `label()` is a function of the fields. There is no field to store a rendered
  line in, and `the_disclosure_record_holds_no_rendered_label` fails if one
  appears.
- `is_coherent()` rejects a run reference on a native or ACP turn, which is
  what a routed result mislabelled as first-party output would look like.

## 6. What is not delivered

`OMEGA-DELTA-0019` lands a window with nothing to restore on the agent instead
of on an empty untitled buffer. It does not land it on a focused composer.

`AgentPanel::activate_new_thread` returns early when no project is open — one
of seventeen `has_open_project` guards — and the no-restorable-session path is
by definition the no-project case. So a genuinely fresh install reaches the
agent panel's "Open Project / Clone Repository" state, not a composer. A window
that restores a project reaches the composer.

Making a thread start project-free and bind on its first workspace-touching
action is the remaining half of omega#76. It is not attempted here because the
guards also gate `ensure_native_agent_connection`, and removing them without
being able to observe the running application would be guessing.

`omega_agent.chat_first_front_door.v1` is therefore recorded as `pending` with
omega#76 as its blocker, and the second half carries a `planned` oracle. The
Full Auto fold is recorded as `enforced`.

## 7. Verification

In the omega repository:

```sh
cargo test -p omega_front_door   # 11 tests: launch rule, disclosure, origins, ledger
cargo test -p omega_deltas       # 33 tests, including 0019 and 0020
./script/clippy
```

Each new check was falsified before being trusted:

| Mutation | Check that failed |
| --- | --- |
| Restore `Editor::new_file` to one launch path | `a_fresh_window_opens_on_the_agent` |
| Re-register the Full Auto dock panel in `zed.rs` | `full_auto_is_folded_into_the_chat_panel` |
| Rename one Full Auto action handler | `full_auto_is_folded_into_the_chat_panel` |
| Add a second `start_run` caller in the agent panel | `only_a_click_listener_starts_a_full_auto_run` |
| Rename one Full Auto element id | `every_full_auto_affordance_is_mapped` |

In this repository:

```sh
pnpm run test:behavior-contracts
pnpm run typecheck:behavior-contracts
```

The Omega oracles are `kind: "script"` because they run as `cargo test` in a
different repository. The coverage checker skips that kind, so a green sweep
here is not evidence that they passed. Each contract's `verification` field
names the command and repository that does run them.

**No rendered proof.** Every oracle above is a source-level check. Nothing here
has observed the folded surface drawing on screen. Rendered confirmation of the
Full Auto surface inside the agent panel, and of the front door on a fresh
launch, is an owner observation step against a packaged build.
