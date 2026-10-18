# Foreman

Voice relays to the saved agent chosen in Voice Settings, defaulting to the configured agent chat; [voice calls](../../apps/openagents/docs/research/codex-voice.md) keep spoken requests and full answers in that chat.

Foreman is the OpenAgents agent that keeps the whole picture of the work in
`OpenAgentsInc/openagents`: every open issue, every claim, and every agent
chat. It reads GitHub through the host's `gh` and reads every chat on every
device through the app's agent tools. It delegates the work to agent chats
and, with standing authority, messages, starts, steers, and stops them.

This folder reviews how Foreman did on its first two days (2026-10-16 and
2026-10-17), records what went wrong, and lays out what to build next.

## Foreman is a custom agent built from SDK primitives

Foreman is not a special part of the app. It is a custom agent defined with
the same OpenAgents SDK primitives anyone on the platform can use, and
anyone can fork it to make their own version, as with every custom agent.
Concretely, Foreman is:

1. **An agent definition**: an `AgentSpec` (name, instructions, runtime
   Claude Code, model Opus 5.5, host-default access) in the agent catalog,
   through `openagents_sdk::agents`.
2. **An Agent Lab experiment**: the template `agent_lab::templates::foreman()`
   with five tests, saved, run, and promoted to an agent through
   `openagents_sdk::agent_lab`. [`examples/foreman.rs`](../../crates/openagents-sdk/examples/foreman.rs)
   does all of this with SDK calls only.
3. **A placement and assignment**: a computer and a project picked with
   `agents::place`, running the agent's runtime there.
4. **A wake schedule**: an account schedule (`openagents_sdk::schedules`)
   that posts into the agent's chat.
5. **App agent tools** it calls while it runs: `list_chats`, messaging and
   steering chats, and `record_decision`.

Items 1 to 4 are open to anyone today. Item 5 and the autonomy around it
(event wakes, standing authority, the decision log, and the Settings card)
are still Foreman-only code from #11965. Until they become general agent
capabilities, a fork can copy Foreman's instructions and model but can't
check in, steer, or keep a log. [built-on-the-sdk.md](built-on-the-sdk.md)
maps every primitive, lists each Foreman-only part and what replaces it,
describes forking today and the target, and sketches a Foreman built from
SDK calls only. Roadmap items 19 to 22 do the work.

## What Foreman is today

| Part | Where | State on 2026-10-17 |
| --- | --- | --- |
| Template and instructions | [`crates/agent-lab/src/templates.rs`](../../crates/agent-lab/src/templates.rs), [`templates/foreman/`](../../crates/agent-lab/src/templates/foreman/) | Claude Code on Opus 5.5, host-default access, five Agent Lab tests (#11926). |
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
- [built-on-the-sdk.md](built-on-the-sdk.md): how Foreman is made of SDK
  primitives, what is still Foreman-only, and how anyone forks it.

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

Phase 1 of the [roadmap](roadmap.md) is held to six items that ship before
anything else: the watchdog, check-ins on by default, coordination in app
chats, the stall rule, machine actions as logged scopes, and a replay of
2026-10-17 as the acceptance test. That order follows Foreman's own review of
these docs. Making Foreman forkable doesn't delay it: Phase 1 is built keyed
by agent ID with no name checks, and roadmap items 19 to 22 then generalize
what proved itself.
