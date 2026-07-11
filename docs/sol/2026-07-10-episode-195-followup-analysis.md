# Episode 195 follow-up — the missing product layer above reliable Desktop/mobile

- Date: 2026-07-10
- Source: [`docs/transcripts/195.md`](../transcripts/195.md)
- Source recording date: 2025-11-11
- Compared against: [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md), Revision 28
- Status: recommendation analysis; not roadmap or issue authority until
  reconciled into the master roadmap

## Executive conclusion

Episode 195 anticipated the current product direction unusually well. Its core
claim was not “add more agent features.” It was that coding-agent software
should become a coherent application: conversation-first Desktop, the same
work available from mobile, durable unattended work, several coding agents
composed as workers inside one flow, discoverable history, low-friction
integrations, open-source extensibility, and explicit choice among local,
community, and cloud compute.

Revision 28 has already absorbed the foundation:

- a real Desktop application instead of a TUI-first product;
- mobile continuity over the same identity, authority, state, and receipts;
- Codex and Claude as named Fleet workers rather than isolated products;
- lossless parent/subagent/tool history as a D1 requirement;
- a host-owned runtime gateway, bounded terminal, and complete workbench;
- owner-local Pylon and managed Agent Computer execution;
- an isolated signed extension catalog; and
- open-source implementation in the public monorepo.

The follow-up does identify four material omissions from the current master
roadmap and one architectural horizon worth preserving:

1. **Conversation-native delegation is not yet an explicit product exit.** The
   roadmap requires historical subagent inspection and Fleet controls, but it
   does not quite require a user to delegate from the current conversation to
   Codex or Claude with an inspectable context envelope and see that child work
   return to the same conversation graph.
2. **Scheduled and overnight work is already substantial code-landed
   substrate, but it is missing from the active direct-software product plan.**
   The right move is not a new scheduler. It is a bounded Desktop/mobile
   Automations surface over the existing `agent_definition.v1`, trigger,
   budget, run-history, Pylon, Fleet, and receipt paths after R4 recovery is
   proven.
3. **History completeness is specified, but history discovery and governed
   memory are not.** Search, explicit cross-conversation references, and
   provenance-bearing retrieval should follow #8674. Ambient surveillance or
   silent self-editing memory should remain excluded.
4. **MCP appears in D4 as state/auth/enablement, while the episode's actual
   product requirement is a complete integration lifecycle.** Discovery,
   provenance, install, capability review, auth, per-session enablement,
   health, update, rollback, and removal should be named in D4.
5. **Local inference should remain a typed future execution target.** It should
   not interrupt R0–R7, but Revision 28 should stop treating all local compute
   as equivalent. Running a cloud-backed Codex account from a local Pylon is
   not the same as executing a model on the device. The target and evidence
   contracts should preserve that distinction now.

Compute resale, plugin revenue sharing, and an embedded wallet remain aligned
long-term directions, but they should not enter the active R0–R7 issue set.
They require the reliable client, isolation, metering, settlement, and
extension-authority floors first.

## Source handling

The transcript is machine-generated and explicitly marked as unsuitable for
quote-grade use without review. This analysis therefore treats it as an intent
record and paraphrases its product claims. It does not treat dates, market
percentages, product names, or individual words in the transcript as factual
authority.

The recording also contains two connected sections:

1. an edge-inference thesis centered on Apple Silicon, Foundation Models, and
   moving appropriate orchestration/summarization work off cloud APIs; and
2. a “10x better” coding-agent product thesis.

The best reconstruction of the ten product dimensions is:

1. replace the fake-terminal-first experience with a normal Desktop app;
2. make the same work controllable from mobile;
3. make overnight and scheduled work first-class;
4. compose CLI coding agents as subagents in one conversation;
5. make history and memory discoverable;
6. make integrations easy instead of exposing MCP plumbing to users;
7. build in public and embrace open source;
8. mix device-local, owner-local, network, and cloud inference intentionally;
9. turn unused compute into opt-in economic capacity; and
10. share revenue with extension/tool/compute contributors through a built-in
    payment path.

## Detailed fit against Revision 28

| Episode 195 dimension | Revision 28 status | Follow-up verdict |
| --- | --- | --- |
| Desktop app over TUI | **Absorbed.** D1–D6 define a conversation-first Electron/Effect Native workbench with native navigation, editor, review, bounded terminal, settings, diagnostics, and Fleet cockpit. | No new lane. Preserve the bounded terminal for real shell work; “ditch the TUI” means the TUI is not the product shell, not that terminals disappear. |
| Mobile with the same work | **Absorbed and strengthened.** R1–R7 require the same refs, versions, policy, commands, outcomes, and receipts across Desktop and mobile. | Keep the roadmap's current interpretation: same task/authority semantics, phone-native information architecture. Literal pixel/component symmetry would be a regression. |
| Overnight/scheduled work | **Engine largely exists; active product surface absent.** The current master roadmap even lists arbitrary scheduling as a pre-F7 non-goal in the Desktop architecture. | Add a bounded post-R4, pre-R7 Automations slice over existing definitions/triggers. Do not build another scheduler or reopen named-assistant standing-role work. |
| Coding agents as subagents | **Partial.** Fleet supports named Codex/Claude workers, and #8674 requires lossless historical subagent graphs. The live conversation-to-child-delegation/context-return contract is not explicit. | Add conversation-native delegation and context provenance to D1/D5. Historical rendering alone does not prove a unified multi-agent work flow. |
| History and memory | **History strong; discovery/memory weak.** #8674 measures complete rendering, and D2 mentions search/archive. Blueprint is named as provenance-bearing memory, but no current gate defines cross-thread retrieval or memory writes. | Add owner-private history discovery, explicit cross-thread references, and staged memory writes with provenance. Keep ambient capture and inferred personal memory out. |
| Hassle-free integrations | **Architecture present; product lifecycle underspecified.** The Desktop architecture's A10 defines an isolated signed catalog, while D4 only names MCP state/auth. | Expand D4 acceptance to the full integration lifecycle. Users should choose capabilities and policy, not manually reason about transports and config files. |
| Open source | **Structurally absorbed.** The implementation, contracts, tests, and roadmaps live in the public monorepo. | No feature issue. Treat public schemas, fixtures, contribution docs, and stable extension boundaries as product requirements when the extension slice opens. |
| Local plus cloud inference | **Partially absorbed, with an important naming collision.** `owner_local` currently means execution through the owner's Pylon and may still call a cloud provider. Apple/local-model routing is paused in the mobile port plan. | Preserve a separate `device_local`/local-model execution location for a post-R7 bounded lane. Never silently call Pylon-hosted cloud inference “local inference.” |
| Idle-compute marketplace | **Substrate-adjacent, not an R0–R7 client requirement.** Pylon, NIP-90, receipts, capacity, and payout systems exist, but broad capacity resale is not the current product exit. | Defer. Revisit only after owner opt-in, isolation, honest availability, marginal-cost accounting, verification, and settlement are live-proven. |
| Revenue sharing and wallet | **Payment rails exist; extension economics do not have a current product contract.** | Defer behind the safe extension lifecycle. Do not make payment a prerequisite for installing useful local integrations. Later attach economic receipts to immutable extension/tool versions and actual usage. |

## The most important discovery: scheduled work is not greenfield

The master roadmap currently closes “named-assistant standing
responsibilities using `agent_definition.v1`” as a speculative direction. That
closure is correct for persona-owned responsibilities. It should not obscure
the direct-software capability already present in the repository.

The earlier
[`Background-Agents Roadmap`](../fable/ROADMAP_BACKGROUND_AGENTS.md) records
the code-landed substrate, and current source confirms it:

- [`packages/agent-runtime-schema/src/index.ts`](../../packages/agent-runtime-schema/src/index.ts)
  defines `openagents.agent_definition.v1`, cron/webhook/inbox/manual triggers,
  lane, budget, escalation, owner scope, and deny-by-default tool policy;
- [`agent-definition-trigger-store.ts`](../../apps/openagents.com/workers/api/src/agent-definition-trigger-store.ts)
  persists owner-scoped trigger state, next-run time, failure count, and pause
  state;
- [`agent-definition-scheduler.ts`](../../apps/openagents.com/workers/api/src/agent-definition-scheduler.ts)
  processes due triggers with a bounded tick and the shared dispatch path;
- [`agent-definition-run-routes.ts`](../../apps/openagents.com/workers/api/src/agent-definition-run-routes.ts)
  exposes run history and manual `run-now` over the same dispatch/budget path;
- Pylon already has Codex and Claude harness adapters, while the tool-authority
  compiler, budget gates, failure auto-pause, Forge work refs, event ledger,
  and closeout/evidence paths already exist.

The narrow truthful claim is **code-landed and fixture-proven substrate**. This
analysis does not claim it is a current Desktop/mobile product, deployed in the
required configuration, live-proven for owner overnight coding, or
owner-accepted.

The roadmap amendment should therefore be “surface and prove scheduled work,”
not “invent automations.” It also should not create a second run universe:

```text
conversation or automation definition
  -> typed trigger + budget + policy
  -> existing definition dispatch
  -> existing Fleet/Pylon/workroom execution
  -> existing run events + outcome + receipt
  -> Khala Sync
  -> Desktop/mobile upcoming, active, attention, and history views
```

### Minimum Automations product slice

After R4 is green, one bounded slice should provide:

- create “run later” and recurring work from a conversation or pinned
  repository/work context;
- inspect the exact objective, selected context refs, target policy, tools,
  budget, timezone, next run, and stop conditions before enabling it;
- list upcoming, active, paused, auto-paused, exhausted, blocked, and completed
  automations on both clients;
- run now, pause, resume, edit future runs, or disable without mutating an
  already-started run's evidence;
- show every trigger attempt, including skipped, policy-refused,
  quota-unavailable, expired, failed, and completed outcomes;
- route approval/escalation to the shared Inbox and exact mobile deep link;
- produce the same Fleet/workroom/account/usage/verification receipts as a
  manual run; and
- survive scheduler, Pylon, device, and network restart without duplicate work.

“In three hours” can be a typed one-shot time trigger. “After the account limit
resets” should be a typed capacity/health condition with a deadline and budget,
not a model repeatedly guessing whether it is safe to retry. Model discretion
may propose the next bounded action; it does not own the clock, budget, account
health, or dispatch decision.

For the fastest path, the first account-linked version should use the existing
Worker scheduler. A later local-first version may run from a host/Pylon clock,
but it must consume the same definition, trigger, idempotency, run, and receipt
contracts. Local scheduling must not fork into a private Desktop run database.

## Conversation-native delegation is the missing connective tissue

Episode 195's strongest multi-agent idea is not merely that Codex can spawn a
child or that a Fleet board can show several workers. It is that a user can
stay in one OpenAgents conversation, delegate a bounded task to a named coding
agent, and inspect the returned work in context.

Revision 28 currently proves two adjacent but different things:

- #8674 will make provider-native historical parent/child/tool activity
  complete and inspectable; and
- D5 will expose server-authoritative Fleet state and controls.

The missing product contract is the edge between them. A conversation action
should be able to create a child execution with an explicit context envelope,
and the resulting run should attach back to the same graph without copying raw
private context into a public projection.

### Required context envelope

The delegation command should bind at least:

- parent conversation/thread ref and initiating message ref;
- explicitly selected source message, file, diff, artifact, Blueprint, or
  receipt refs;
- repository, workspace/workroom, branch/baseline, and verification refs;
- objective and expected deliverable;
- named worker kind, account/capacity policy, and execution target policy;
- tool/capability grant, approval posture, budget, deadline, and cancellation
  policy;
- context redaction/provenance policy and an explicit “not shared” set;
- stable delegation/run/attempt/idempotency refs.

“All context available to agents who want it” must not become ambient context
inheritance. The safe interpretation is **selectable, provenance-bearing
context under the same owner and capability scope**. Every child can report
which refs it consumed. A child requesting more context creates a typed request
or approval; it does not silently read every prior conversation, filesystem,
or private provider event.

### Required return path

The parent conversation should receive bounded typed facts for child created,
claimed, running, blocked, asking, completed, failed, cancelled, and receipt
ready. The full child transcript remains selectable in the Agents inspector.
The parent receives a concise result plus safe artifact/verification/receipt
refs, not a pasted lossy summary presented as authority.

This should amend D1 and D5, not create a new orchestration service. The
Runtime Gateway command plane, `agent_run`/`agent_run_event`, FleetRun, work
unit, attempt, and command-outcome records remain the implementation
substrate.

## History needs discovery; memory needs governance

The episode correctly diagnosed provider conversation archives as poor
discovery systems. Revision 28 fixes loss before discovery: #8674 removes
silent time, item, and descendant caps and makes completeness measurable. That
ordering is right. An index over lossy input would only make omissions harder
to notice.

The next product slice should distinguish three concepts:

1. **History** — immutable or provider-owned conversation/run evidence.
2. **Discovery index** — rebuildable local metadata/embedding/search state over
   authorized history.
3. **Memory** — an explicit, provenance-bearing durable assertion selected for
   future use.

Recommended requirements:

- local owner-private indexing by default for provider-native history;
- semantic retrieval through a central typed selector/embedding index, not ad
  hoc intent keyword matching;
- filters for repository, worker, account, branch, status, date, tool, and
  receipt state;
- every result links to the exact source thread/item and discloses unloaded,
  redacted, missing, or stale state;
- deletion/tombstones remove or rebuild derived index entries;
- cross-device Sync includes only canonical OpenAgents conversations and
  explicitly promoted safe memory, not raw local provider archives by default;
- memory writes are staged with a visible diff, source refs, scope, retention,
  and approve/reject outcome; and
- automated memory edits remain deny/ask by default through the existing
  `memory_write` authority class.

This preserves the useful “three days ago we decided X” experience without
introducing ambient screen recording, unreviewable profile inference, or a
second source of truth.

## “Hassle-free integrations” means lifecycle, not hidden authority

The Desktop architecture already has the right security answer in A10:
extensions enter through an isolated signed catalog and declare provenance,
hash, runtime, capabilities, policy, update, rollback, and per-session
enablement. The master roadmap should promote that from architecture detail to
D4 product acceptance.

The user-facing flow should be:

```text
discover -> inspect publisher/version/capabilities -> install
  -> authenticate or bind scoped resources -> enable for selected contexts
  -> observe health and use -> update/rollback/disable/remove
```

MCP, MCPB, skills, plugins, and custom tools can be compatibility inputs behind
that flow. They do not need to be the primary vocabulary shown to a user.
Executable content stays outside the primary Desktop gateway, capabilities are
deny-by-default, and each run receipt identifies the immutable integration
version and effective grants. Open source improves inspection and contribution;
it does not make unsigned executable code safe.

## Local inference should return as a placement class, not a detour

The episode's edge-inference thesis remains strategically relevant, but the
current critical path should not be interrupted to revive a legacy Apple-only
surface. The mobile port plan correctly pauses Apple Foundation Models/local
model routing until it can re-enter through explicit target and evidence
contracts.

What Revision 28 should preserve now is a clean vocabulary:

| Execution location | Meaning |
| --- | --- |
| `device_local` | Model executes on the current Desktop/mobile device; no model prompt leaves it. |
| `owner_pylon` | Work executes on owner-controlled capacity; the selected model may still be local or provider-cloud and must say which. |
| `managed_agent_computer` | Work executes on OpenAgents-managed isolated capacity. |
| `provider_cloud` | A named provider owns model execution behind its account/API boundary. |

Location, model/provider, account, privacy posture, cost truth, and execution
profile are separate fields. `auto` may select only among owner-approved
eligible combinations and must report the effective choice. There is no silent
fallback from device-local to cloud.

Good first local-model tasks after R7 are bounded and independently evaluable:
title generation, short summarization, local history retrieval/reranking,
context compression, and typed routing proposals. Each needs latency, energy,
quality/fallback, privacy, and receipt tests. The product should measure local
share by task class and effective execution location; a single marketing
percentage that mixes tokens, calls, and heterogeneous tasks is not honest.

## Open source, compute markets, and revenue sharing

These themes reinforce the architecture but have different timing.

### Open source: a present constraint

The public repo already satisfies the basic claim. The next meaningful step is
not another roadmap epic labeled “open source.” It is to keep extension
manifests, action schemas, fixtures, policy decisions, compatibility tests, and
contributor setup public and stable enough that third parties can build without
joining the core runtime trust domain.

### Compute market: a post-reliability option

“Unused compute” becomes usable capacity only when all of the following are
true:

- the owner explicitly opts in and can cap, drain, or revoke capacity;
- local interactive work wins over market work;
- workload, identity, filesystem, network, and credential isolation are
  enforced;
- advertised hardware/capacity and measured execution match;
- verification and exact-or-explicitly-unmeasured resource receipts exist;
- requester charge, contributor earning, fees, disputes, and settlement are
  distinct durable states; and
- failure cannot consume unbounded power, bandwidth, subscription quota, or
  funds.

Pylon, Agent Computers, NIP-90, and payment/receipt systems are relevant
substrate. They do not make this a current R0–R7 requirement.

### Extension revenue: after safe usefulness

An extension registry should first prove useful, secure, updateable local and
remote integrations. Economic support can later bind an immutable
publisher/version/tool invocation to a metered usage receipt and payout policy.
Wallet custody remains outside the renderer and extension process. Free/local
extensions remain possible; payment is an economic option, not an authority
shortcut or installation gate.

## Recommended amendments to the master roadmap

This analysis recommends the following Revision 29 changes, in order. It does
not recommend opening issues until the owner accepts the amendment and the live
claim ledger is checked.

### Amendment A — add conversation-native delegation to D1/D5

Add an owner decision and implementation law stating:

> A user may delegate from the current conversation to a named Codex or Claude
> worker through one typed context envelope. The child run, requests, outcome,
> artifacts, verification, and receipt remain attached to the same inspectable
> conversation graph. Context sharing is explicit and provenance-bearing;
> history visibility is not execution authority.

Extend the D1 exit with one live parent conversation that delegates to a child,
shows the child timeline/requests, and returns an exact outcome. Extend D5/R3
with two named workers delegated from that flow and controlled from both
clients without account or target substitution.

### Amendment B — add discovery and explicit memory to D2

After #8674 completeness, require owner-private paged search and semantic
retrieval across authorized conversations, runs, tools, and receipts, with
exact source navigation and counted gaps. Add staged, provenance-bearing
memory writes as an approval-gated capability; do not add ambient capture.

### Amendment C — expand D4 from MCP state to integration lifecycle

Replace “MCP auth/enable state” as the whole acceptance phrase with signed
catalog discovery, provenance/capability review, install, auth, scoped
enablement, health, immutable version evidence, update, rollback, disable, and
removal. Preserve A10 isolation as the binding implementation boundary.

### Amendment D — add a bounded Automations slice after R4 and before R7

Use the existing definition/trigger system. The first acceptance should be one
conversation-derived one-shot run and one recurring repository task, each
budgeted, pauseable, restart-safe, Inbox-visible, controllable on Desktop and
mobile, and closed by normal run/usage/verification receipts. Fold this into
the sole #8566 program as a bounded leaf, never a new epic.

R7 owner dogfood should include at least one scheduled overnight coding run
that encounters either an approval, quota delay, or restart and still reaches
one honest durable outcome without duplicate execution.

### Amendment E — preserve local-model placement without adding it to P0

Clarify that `owner_local`/Pylon placement does not imply device-local model
execution. Reserve an explicit local-model execution location and evidence
shape. Revisit a bounded local summarization/retrieval/router slice only after
R7 unless it becomes necessary to meet privacy, offline, latency, or cost gates
earlier.

### Amendment F — add explicit post-R7 revisit gates, not active issues

Record compute resale and extension revenue sharing as horizons whose entry
conditions are reliable releases, isolation, exact resource/usage evidence,
owner opt-in, and settled economic receipts. Do not restore a broad network or
marketplace epic.

## Sequencing impact

The recommendations do not change the first three current steps:

1. finish R1/R2 physical identity and Sync acceptance;
2. finish D1 real streaming/mobile continuation and #8674 lossless history;
3. deepen the Desktop and mobile workbenches over that shared seam.

They refine the later burn:

```text
R1/R2 identity + Sync
  -> D1 complete history + live conversation
  -> conversation-native delegation
  -> D2 history discovery/reference
  -> D3 workbench + R3 Fleet/workroom operations
  -> R4 fault/recovery proof
  -> D4 integration lifecycle
  -> bounded Automations surface over landed scheduler substrate
  -> D5/R5/R6 complete clients
  -> R7 release + mobile handoff + overnight scheduled dogfood
  -> local-model slice
  -> opt-in compute/extension economics only after their revisit gates
```

The key scheduling rule is that Automations waits for R4. Unattended work
multiplies every duplicate-dispatch, stale-lease, lost-acknowledgement, quota,
and recovery defect. Surfacing it earlier would turn known reliability gaps
into autonomous damage.

## What should not be added

Episode 195 should not be used to justify:

- removing the real bounded terminal from Desktop/mobile coding;
- copying Desktop layout literally onto a phone;
- sharing an entire conversation archive or filesystem with every child by
  default;
- treating local SQLite, a search index, or model summary as authority;
- ambient screen recording or inferred personal memory;
- another scheduler, run database, claim registry, or agent-definition schema;
- a model deciding account, spend, compute target, or retry timing without
  typed policy;
- silently substituting cloud inference when device-local execution fails;
- loading unsigned third-party code into the primary gateway;
- compute resale before isolation, owner opt-in, metering, verification, and
  settlement; or
- a wallet or payment requirement in front of basic local integrations.

## Final assessment

Episode 195 is best understood as an early product specification for the layer
Revision 28 is now building. The master roadmap already contains the difficult
foundation: a non-TUI Desktop application, useful mobile coding, one Sync and
authority reality, multi-harness Fleet execution, lossless agent history,
bounded workrooms, extension isolation, and receipts.

The useful follow-up is to make that foundation feel like the product described
in the episode:

- delegate to named coding agents without leaving the conversation;
- find and cite prior work without inventing memory;
- schedule bounded work and wake up to durable receipts;
- install integrations through a safe product flow rather than configuration
  plumbing; and
- later move suitable workloads to truly local models without confusing
  location, provider, cost, or proof.

Those amendments strengthen the present Desktop/mobile direction. They do not
reopen removed personas, broaden the web surface, or replace the current R0–R7
critical path.
