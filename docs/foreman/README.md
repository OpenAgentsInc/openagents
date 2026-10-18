# Foreman

Foreman is the OpenAgents agent that keeps the whole picture of the work in
`OpenAgentsInc/openagents`: every open issue, every claim, and every agent
chat. It reads GitHub through the host's `gh` and reads every chat on every
device through the app's agent tools. It delegates the work to agent chats
and, with standing authority, messages, starts, steers, and stops them.

This folder reviews how Foreman did on its first two days (2026-10-16 and
2026-10-17), records what went wrong, and lays out what to build next.

## What Foreman is today

| Part | Where | State on 2026-10-17 |
| --- | --- | --- |
| Template and instructions | [`crates/agent-lab/src/templates.rs`](../../crates/agent-lab/src/templates.rs), [`templates/foreman.md`](../../crates/agent-lab/src/templates/foreman.md) | Claude Code on Opus 5.5, host-default access, five Agent Lab tests (#11926). |
| Claim-to-chat join | `harnessSessionId` on `list_chats` in [`apps/openagents/crates/mcp/src/tools.rs`](../../apps/openagents/crates/mcp/src/tools.rs) | On `main` (#11927). A claim whose session matches no chat is reported as "outside the app". |
| Ask from the CLI, web, and phone chats | `openagents agents ask Foreman …`, the chat agents door | On `main` (#11934). |
| Check-ins, standing authority, decision log | [`apps/openagents/crates/engine/src/foreman.rs`](../../apps/openagents/crates/engine/src/foreman.rs), [`doc_host/foreman.rs`](../../apps/openagents/crates/engine/src/doc_host/foreman.rs), Settings → General → Foreman | On `main` (#11965, `96876f7206`, `a33cdd25ba`). Off until the owner picks Foreman's chat in Settings. Never turned on on the owner's Mac (see [chat-review.md](chat-review.md#what-the-autonomy-code-did-and-didnt-do)). |
| Push order by file area | `changed_paths` tool, [`changed_paths.rs`](../../apps/openagents/crates/mcp/src/changed_paths.rs) | On `main` (#11972). |
| Two-minute cap on blocking waits | `wait_for_turn` in `tools.rs` (`MAX_WAIT_SECS`) | On `main` (#11969). |
| Saved-agent identity and runtime setting | Custom agents (#11966) | On `main`. |

The user guide is [`docs/agent-lab/foreman.md`](../agent-lab/foreman.md).

## In this folder

- [chat-review.md](chat-review.md): a review of the two app chats titled
  "Foreman", with timestamps, tool counts, and quoted evidence.
- [lessons-learned.md](lessons-learned.md): each failure, what should have
  caught it, and what is missing.
- [roadmap.md](roadmap.md): an ordered plan, in three phases, to make
  Foreman the default way issue work runs.
- [sdk-additions.md](sdk-additions.md): the `openagents-sdk`, HTTP, and CLI
  surface that Foreman and outside harnesses need.

Related records:

- [`docs/audits/2026-10-17-nostr-identity-close-timeline.md`](../audits/2026-10-17-nostr-identity-close-timeline.md):
  the 4 h 45 min gap between "code complete" and "closed" for 11 issues.
- [`docs/audits/2026-10-16-master-delegation.md`](../audits/2026-10-16-master-delegation.md):
  the Foreman rows (#11926, #11927, #11934, #11940, #11965, #11966) and
  their After-close checks.

## The plan in one paragraph

Foreman works well as a dispatcher when it can see and reach the agents. It
started six chats in parallel at 19:58 CDT on 2026-10-17, and three of the
six issues closed within 35 minutes. It fails when the work runs where it
can't see it: a terminal Claude Code session coordinated the Nostr identity
issues, and Foreman could only offer help. It also fails when its own turn
hangs: one shell call with no timeout froze it from 14:11 to 18:24 CDT, and
nothing restarted it. The plan is, in order: make every coordinating
session an app chat; give the host, not the model, a watchdog over
Foreman's own turn and over every chat it manages, with a stall rule that
messages, then restarts, then tells the owner on the phone; send all issue
work through `openagents chat work --on gce` and every landing through
`openagents land submit`, so nothing builds or hand-merges on the owner's
Mac; ask System One for the typed judgments (stalled or not, on `main` too
or not, overlapping or not); and measure Foreman by the minutes from code
complete to close and the minutes any chat sat stalled.
