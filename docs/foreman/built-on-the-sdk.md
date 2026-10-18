# Foreman is built from SDK primitives

Foreman is not a special part of the OpenAgents app. It is a custom agent,
like any other, defined with the same OpenAgents SDK primitives that anyone
on the platform can use. The owner's rule (2026-10-17): Foreman must be
definable through the SDK alone, and anyone can fork it, as with every
custom agent, to make their own version with its own spin.

This page says which primitives Foreman is made of, which parts still break
that rule today, and what changes so that the official Foreman and anyone's
fork have exactly the same powers.

## The rule

1. **No Foreman-only code.** The engine, the app, the server, and the SDK
   have no code path that checks for "Foreman" by name or by a Foreman-only
   type. Whatever Foreman can do, any custom agent can be given.
2. **Foreman is data.** The official Foreman is a published agent
   definition: instructions, runtime and model, skills, an Agent Lab test
   suite, a wake schedule, and the grants it asks for. Nothing else.
3. **Forking is copying the data.** A fork starts from that definition and
   changes any part of it: the instructions, the model, the skills, the
   repositories it watches, the stall rule, the tests. A fork runs on the
   same primitives with the same limits.
4. **Powers come from grants, not names.** Steering other chats, posting
   check-ins, and writing a decision log are capabilities the owner grants
   to an agent. The official Foreman gets no grant a fork can't ask for.

## What Foreman is made of

| Foreman needs to | SDK primitive | Where it lives today | Open to any agent today? |
| --- | --- | --- | --- |
| Exist as an agent: name, instructions, runtime, model, access, fallbacks | `AgentSpec` and the agent catalog: `Agents::create`, `update`, `list`, `get` | [`crates/agent-contract/src/lib.rs`](../../crates/agent-contract/src/lib.rs), [`crates/openagents-sdk/src/agents.rs`](../../crates/openagents-sdk/src/agents.rs) | Yes |
| Start from a template, be tested, then be saved as an agent | Agent Lab: `AgentLabClient::save`, `start`, `runs`, `promote`, and `Experiment`, `Draft`, `Suite` | [`crates/openagents-sdk/src/agent_lab.rs`](../../crates/openagents-sdk/src/agent_lab.rs), [`crates/agent-lab/src/lib.rs`](../../crates/agent-lab/src/lib.rs) | Yes |
| Carry its playbook | Agent Lab `Component::Skill` and `Component::Memory` in a `Draft` | [`crates/agent-lab/src/lib.rs`](../../crates/agent-lab/src/lib.rs) | Yes. The Foreman template doesn't use them yet: its whole playbook is one instruction string, `FOREMAN_INSTRUCTIONS`. |
| Run on a computer, in a project | Assignments and placement: `agents::place`, the host's runtimes | [`crates/openagents-sdk/src/agents/place.rs`](../../crates/openagents-sdk/src/agents/place.rs), [`assignments.rs`](../../crates/openagents-sdk/src/agents/assignments.rs) | Yes |
| Be asked a question from the CLI, the web, and phone chats | The saved-agent door: `openagents agents ask NAME` | [`crates/openagents-cli/src/agents.rs`](../../crates/openagents-cli/src/agents.rs) | Yes. It looks agents up by name. |
| Wake on a timer | Account schedules: `Schedules::list`, `sync`, with `chat` set to the agent's chat | [`crates/openagents-sdk/src/schedules.rs`](../../crates/openagents-sdk/src/schedules.rs) | Yes for a timed prompt. The event wakes (a chat it manages finished, an issue opened, a claim went stale) and the coalescing are Foreman-only (below). |
| See every chat on every device, with live status | App agent tools: `list_chats` with `harnessSessionId` | [`apps/openagents/crates/mcp/src/tools.rs`](../../apps/openagents/crates/mcp/src/tools.rs) | Yes, to an agent running in the app. Not through the SDK (see [sdk-additions.md](sdk-additions.md) S1). |
| Message, start, steer, and stop other chats | App agent tools, gated by standing authority | [`tools.rs`](../../apps/openagents/crates/mcp/src/tools.rs), [`proto/src/foreman.rs`](../../apps/openagents/crates/proto/src/foreman.rs) | **No.** The standing authority is Foreman's state. |
| Keep a decision log | The `record_decision` app tool and `DecisionEntry` | [`tools.rs`](../../apps/openagents/crates/mcp/src/tools.rs) (its description begins "Foreman only:"), [`engine/src/foreman.rs`](../../apps/openagents/crates/engine/src/foreman.rs) | **No.** The tool refuses unless it is called from "Foreman's own chat". |
| Read GitHub issues, claims, and boards | The host's `gh` | The computer, not the SDK | Yes, to any agent with host access. Not an SDK primitive yet (S6). |
| Be set up by the owner | Settings → General → Foreman | [`apps/openagents/crates/ui/src/settings/foreman.rs`](../../apps/openagents/crates/ui/src/settings/foreman.rs) | **No.** The card is for Foreman only. |

The SDK example [`crates/openagents-sdk/examples/foreman.rs`](../../crates/openagents-sdk/examples/foreman.rs)
already makes Foreman with SDK calls only: it loads
`agent_lab::templates::foreman()`, picks a computer and a project, saves the
experiment, runs its tests, and with `--promote` saves the agent. So the
definition is on the SDK today. The autonomy is not.

## Where today's code breaks the rule

These parts were written for Foreman alone in #11965 and must become
general agent capabilities:

| Foreman-only today | Becomes, for any agent |
| --- | --- |
| `engine/src/foreman.rs` and `doc_host/foreman.rs`: check-in coalescing, event wakes, the stale-claim timer, the check-in prompt | **Agent wakes.** An agent definition declares the events it wakes on (`chat_finished`, `chat_needs_answer`, `issue_opened`, `issue_closed`, `claim_stale`, a schedule tick) and a coalescing window. The host runs one wake engine for every agent that declares wakes. |
| `ForemanSettings` and its `Authority` (standing authority) in `proto/src/foreman.rs` | **Agent grants.** The owner grants an agent named scopes over named chats or projects: `chat.message`, `chat.start`, `chat.steer`, `chat.stop`, later `land.submit` and `work.dispatch`. The same never-list applies to every agent: money, keys, secrets, deploys, and closing other agents' issues. |
| `record_decision`, "Foreman only", stored in `foreman/decisions.jsonl` | **Agent decision log.** Any agent with the `log` capability writes entries. The log is per agent definition and lives on the account ([sdk-additions.md](sdk-additions.md) S7). |
| `RECORD_FOREMAN_DECISION` and the Foreman RPC methods | General RPC methods keyed by agent definition ID. |
| Settings → General → Foreman | **An agent's autonomy card.** The same card for any saved agent: its chat, its wakes, its grants, its log. The phone and the website show the same card. |

Until those changes land, a fork of Foreman can copy its instructions and
model, but it can't check in on its own, can't steer chats, and can't keep a
log. That is the gap to close.

## Forking Foreman

**Today.** In Agent Lab, **Start from Foreman** copies the template into a
new experiment that you own. You can edit it, test it, and save it as your
own agent. From code, the SDK example does the same with
`agent_lab::templates::foreman()`. Both copy a template that ships in
`crates/agent-lab`, not a published agent, so a fork can't see later
changes to the original, and nobody outside this repository can publish
their own Foreman for others to fork. Public catalogs are outside the
custom-agents delivery ([`docs/custom-agents.md`](../custom-agents.md)).
Private portable exports are specified in
[NIP-AGENT](../../nips/openagents/NIP-AGENT.md).

**Target.**

1. The official Foreman is a published agent definition with a stable ID
   and revisions, not a Rust constant. `templates::foreman()` becomes a
   fetch of that definition.
2. `fork` copies a published definition, including its skills, test suite,
   declared wakes, and requested grants, into the caller's catalog. It
   records `forked_from: {definition, revision}`, so a fork can see what
   changed upstream and pull it in.
3. Granted powers never copy. A fork asks its own owner for its grants.
4. Anyone can publish a definition, theirs or a fork, for others to fork.
   NIP-AGENT exports carry the same data.

Examples of a fork with its own spin:

- A Foreman that watches a different organization's repositories and
  boards.
- A Foreman that runs on Codex, or on a cheaper model for check-ins and a
  stronger one for decisions.
- A stricter stall rule: message after 10 minutes instead of 20.
- A Foreman that only reports and never steers: it asks for no grants.
- A Foreman with a different playbook skill for release days.

## A Foreman from SDK calls only

Once agent wakes, grants, and the decision log are general (roadmap items
19–21), anyone can build a Foreman like this. It's a sketch of the target;
the calls marked "proposed" don't exist yet.

```rust
use openagents_sdk::{agents::*, agent_lab::*, schedules::*};

// 1. Fork the official Foreman, or start from scratch with AgentSpec.
let mine = agents.fork(FOREMAN_ID, ForkOptions {          // proposed
    name: "Release Foreman".into(),
}, &cancel).await?;

// 2. Give it its own spin: instructions, model, a playbook skill.
let mut spec = mine.spec.clone();
spec.instructions.push_str("\nOn release days, hold every landing that touches crates/coder.");
spec.model = Some("claude-sonnet-5-5".into());
agents.update(agents.update_request(&mine, spec)?, &cancel).await?;

// 3. Declare when it wakes.
agents.set_wakes(&mine.id, Wakes {                         // proposed
    events: vec![Wake::ChatFinished, Wake::IssueOpened, Wake::ClaimStale { hours: 6 }],
    every: Some(Duration::from_secs(600)),
    coalesce: Duration::from_secs(60),
}, &cancel).await?;

// 4. The owner grants it powers over chats in one project.
agents.grant(&mine.id, Grant {                              // proposed
    scopes: vec![Scope::ChatMessage, Scope::ChatSteer],
    over: Over::Project("openagents".into()),
}, &cancel).await?;

// 5. Its decision log is readable from anywhere.
let page = agents.decisions(&mine.id, None, &cancel).await?; // proposed (S7)
```

The official Foreman would be made the same way, by the same calls.

## How we know the rule holds

- `git grep -i foreman -- apps/openagents/crates crates/openagents-web/src crates/openagents-sdk/src`
  finds only fixtures, docs, and the definition's data, never a branch on
  the name.
- An Agent Lab test forks Foreman under a new name, grants it the same
  scopes, and passes the same five Foreman tests plus a check-in, a steer,
  and a decision-log entry.
- The official Foreman and a fork produce the same tool list and the same
  wake events on the same host.
