# Harness-Agnostic Background Agent Definitions — Audit (2026-07-02)

Status: audit + build plan. No promise state flips; no public copy changes.

Companions: `2026-07-02-qa-swarm-product-plan.md` (same design partner),
`2026-07-01-episode-245-completion-and-multi-harness-orchestration.md`
(multi-harness delegation), `2026-07-01-artanis-fleet-administrator-audit.md`,
ADR `docs/adr/0012-adopt-khala-native-terminal-tool-runtime.md`.

## 1. What prompted this

Our QA-swarm design partner — the author of the `executor` project (local
read-only clone: `projects/repos/executor`) — posted a chain worth treating as
a signal, paraphrased:

> Setting up background agents for myself and becoming 100x more bullish on
> executor. The relationship with how agents work totally changes once
> they're no longer running in the foreground — **the permission model
> becomes so important**. A lot of work to make it good for this use case.
>
> Figuring that out now; I started by brain-dumping everything I want agents
> running for. I think this might turn into an Executor feature of
> **"Agents"** where you store a **name**, a **goal**, and an **allowed
> toolset**, and it just handles dispatching an API call to start whatever
> agent you want (hermes, openclaw, claude code, etc). The reason I'm tempted
> to put this in Executor is it **stays agnostic to whatever the driving
> agent is**, so you can swap that out without losing your workflows.

The accompanying brain-dump (screenshot, 2026-07-02, on file locally) lists
what he actually wants running:

1. An agent that watches the codex GitHub repo and reports how their
   code-mode implementation changes.
2. An agent that, on every Slack message, decides whether to page him or
   start investigating.
3. (Explicitly "not an agent") a **shared database** that ingests all
   Slack / GitHub / iMessage sources into one place and tracks
   "has this been handled / responded to".
4. An agent that every 4 hours figures out what he needs to reply to or
   follow up on.
5. An agent that watches every GitHub commit for output health — an
   exponential-backoff observability sweep (maybe at its own discretion)
   confirming releases go out OK, monitoring Cloudflare / Axiom / etc.
6. A growth agent running experiments and monitoring PostHog "like a growth
   guy".
7. An agent monitoring all production traffic "like an SRE".

For context, `executor` itself is "the integration layer for AI agents — one
catalog for every tool, shared across every agent you use": a local
background service + web UI + MCP server that turns OpenAPI / GraphQL / MCP /
Google Discovery sources into a shared tool catalog with shared auth and
policies across Cursor, Claude Code, OpenCode, etc. The proposed "Agents"
feature is the natural next rung: since executor already owns tools + auth +
policy, let it also own **named agent definitions** and dispatch any harness
against them.

## 2. Why this matters to us

Three separable insights, each of which we should act on:

1. **The durable object is the agent definition, not the harness.**
   `{name, goal, allowed toolset}` survives swapping Codex for Claude Code
   for hermes for openclaw. Whoever owns that record owns the user's
   workflows; the harnesses become interchangeable backends. This is
   precisely the position Khala already takes for coding capacity (the
   2026-06-25 capacity-routing spec routes onto the caller's own
   Codex/Claude capacity behind one typed router) — but we have never
   reified the *definition* as a first-class stored object.
2. **Background flips the permission model from advisory to load-bearing.**
   Foreground agents borrow the user's judgment turn-by-turn. Background
   agents have only their allowed toolset and an escalation path. "Allowed
   toolset" must be an *enforced authority boundary*, not a prompt — which
   is exactly the ADR-0012 stance (no smuggled owner-local danger authority)
   and the workroom `secret_policy`/`network_policy` stance.
3. **Half the brain-dump isn't agents — it's a triage substrate.** Items 2,
   3, and 4 all depend on the shared ingest database with handled-state.
   Items 1, 5, 6, 7 are watchers: trigger + read tools + judgment + a
   bounded write/escalate set. A product that ships *definitions + triggers
   + inbox + enforcement* covers all seven wants with one architecture.

## 3. What we already have (inventory)

| Needed piece | What exists today | Where |
| --- | --- | --- |
| Multi-harness dispatch | `khala.fleet.delegate` deterministic delegation; Pylon runs `@openai/codex-sdk` + `@anthropic-ai/claude-agent-sdk` in bounded workspaces via `assignment_lease.v0.3`/`codingAssignment`; fleet hotbar in Khala Code | openagents monorepo; Episode-245 doc §1 |
| Unattended cloud execution | `oa-codex-control` on GCE (per-run ephemeral VMs, durable queue off-by-default); workrooms via `oa-node`/`oa-workroomd`. Standing policy: background/unattended runs belong on **our** GCE, not in-session or third-party runners | private `cloud/` repo; 2026-06-14 infra-mismatch audit |
| Tool-authority boundary | ADR-0012 native Khala terminal tool runtime — typed tool policy shared across Khala Code desktop, CLI, Pylon lanes; capability gateways + sandbox profiles (`network_policy`, `secret_policy`, `execution_class`) on the cloud side | ADR-0012; sandbox platform audit |
| Named agents w/ identity | Forum agent registration (slug/externalId, optional owner claim), agent personas | openagents.com agent surfaces |
| Typed semantic routing | Workspace-wide invariant: no keyword routing; central typed semantic selector / classifier (capacity-routing spec commits to a full typed coding-workflow classifier) | root `CLAUDE.md`; khala routing spec |
| Metering/receipts for runs | `resource_usage_receipt.v1`, credit ledger seams, quotas (`compute_quota_routing.v1`) | cloud metering docs |
| Event stream to clients | Resumable-SSE Durable Streams; `codex_workroom_event.v1` → `SessionEvent` lane-transparent mapping | Pylon `openagents-cloud` provider |
| Preferred substrate | Cloudflare primitives (DO, D1, Queues, Analytics Engine) for anything stateful/event-shaped — no third-party deps | standing policy |

Verdict: we have the **execution** and **enforcement** layers. We are missing
the **definition**, **trigger**, and **inbox** layers — the three things that
make agents *background* rather than *dispatched*.

## 4. What we need to build

### 4.1 `openagents.agent_definition.v1` (the core record)

A typed, stored, versioned object — deliberately a superset of
`{name, goal, allowed toolset}`:

```
agent_definition.v1 {
  id, ownerRef, name, slug
  goal: string                      // the standing objective, verbatim
  harness: { kind: codex | claude_code | khala | opencode | custom,
             modelHint?, versionPin? }        // swappable, never load-bearing
  toolset: { allow: [toolRef...], deny: [toolRef...],
             ask: [toolRef...],               // escalate instead of act
             networkPolicy, secretPolicy }    // enforced, not prompted
  triggers: [ cron(expr) | webhook(source) | inboxMatch(classifierRef) ]
  lane: own_pylon | cloud_workroom | worker_only
  budget: { maxRunSeconds, maxRunsPerDay, maxCreditsPerDay }
  escalation: { channel: forum|push|email, askPolicy }
}
```

Design rules:

- **Harness is a field, not a foreign key into behavior.** Everything a run
  needs (goal, toolset, triggers, budget) lives in the definition, so the
  harness can be swapped without touching workflows — the exact property the
  executor author identified. We validate this from day one by shipping ≥2
  adapters (Codex + Claude Code, both already live in Pylon).
- **Toolset is enforced at the ADR-0012 tool-runtime boundary** (local lane)
  and the workroom capability gateways (cloud lane). The definition compiles
  to the same policy object both enforce. `ask` entries route to escalation
  instead of failing — this is the background permission model: deny by
  default outward, allow read widely, ask on the boundary.
- Storage: D1 via the openagents.com Worker (owner-scoped, same auth as
  agent registration). CRUD: `POST/GET/PATCH /v1/agent-definitions`.

### 4.2 Dispatch: `POST /v1/agent-definitions/:id/runs`

One API call that turns a definition + trigger payload into a run:

- `lane=own_pylon` → existing `assignment_lease.v0.3` path to the owner's
  linked Pylon (the capacity-routing spec's single-user invariant holds:
  background agents run on *your* capacity or *your* metered workroom, never
  pooled).
- `lane=cloud_workroom` → the `cloud/` `POST /v1/placement` path via the
  same adapter seam as `/v1/cloud-coding-sessions`; unattended by
  definition, so this is where the durable queue (currently off-by-default,
  concurrency 1) must actually get turned on and sized — a known RED item.
- Every run emits `SessionEvent`s on a Durable Stream and settles a
  `resource_usage_receipt.v1`; budget caps enforce runaway protection
  (a background watcher with a bug is a money pump — budgets are not
  optional).

A thin **harness adapter contract** (`agent_harness_adapter.v1`:
`start(definition, triggerPayload) → sessionRef`, normalize events, report
terminal state) is the only per-harness code. Pylon's existing
`openagents-cloud` provider event mapping is the template.

### 4.3 Triggers (the part nothing owns today)

- **Cron**: schedule table + a dispatcher on our GCE control plane
  (`oa-codex-control` already has per-run VM + queue machinery; the
  scheduler is a small standing loop or a Worker cron trigger that calls
  the dispatch API). Covers brain-dump items 1, 4, 5 (with backoff state
  kept per-definition so "exponential backoff at its own discretion" is a
  stored knob the agent may adjust within bounds).
- **Webhooks**: GitHub (repo watch, commit/release events) and Slack
  (message events) receivers on the Worker → Queues → trigger evaluation.
  Covers items 1, 2, 5.
- **Inbox match**: see 4.4 — a classifier decides which definitions care
  about an ingested event. Per workspace invariant this is a typed semantic
  classifier (embeddings/structured planner), *not* keyword rules.

### 4.4 The unified inbox (item 3 — "not an agent")

The design partner is right that this is infrastructure, not an agent. It is
also the piece with the widest blast radius, because items 2 and 4 are
queries over it:

- **`event_ledger.v1`** on Cloudflare primitives: Queues ingest → D1 rows
  (source, externalRef, actor, content ref, timestamps) + a DO per owner for
  ordering/dedup. Sources v1: GitHub + Slack (webhook-friendly); iMessage
  later via a local Pylon collector (it only exists on the owner's Mac —
  Pylon is already our on-your-machine daemon, so this is a natural Pylon
  capability, not a cloud one).
- **Handled-state as first-class**: `open | handled | responded | ignored`,
  with which run/agent touched it. The 4-hourly follow-up agent (item 4) is
  then a trivial definition: cron trigger + read-ledger tool + escalate.
- **Privacy edge**: this ledger is the owner's private communications.
  It lives owner-scoped, is never training data, never leaves the account
  boundary, and background agents get it read-only through a gateway tool
  that redacts per `secretPolicy`.

### 4.5 Khala Code surface

Khala Code is where definitions live for the user — the same pattern as the
Fleet panel (Episode-245 work), one rung up:

- An **Agents panel**: list of definitions (name, goal, harness badge, lane,
  last run, next trigger), create/edit as a typed form, per-agent run
  history rendered off the same Durable Streams as foreground sessions.
- An **escalations/inbox view**: `ask`-policy hits and pages land here (and
  push/forum per the escalation channel). This is the foreground half of the
  background permission model — approvals must be one keystroke or people
  will widen allowlists instead.
- CLI parity: `khala agents list|create|run|logs` for the terminal-first
  crowd.

## 5. Sequencing (proposed)

1. **Definition record + CRUD + one adapter** (Codex via own-Pylon lane) +
   cron trigger. Ship item-4 ("what do I need to follow up on") against
   GitHub notifications only — smallest end-to-end proof.
2. **Second adapter (Claude Code)** to prove harness-swap on an unchanged
   definition — the headline property; demo it.
3. **Webhook triggers + event ledger v1** (GitHub, Slack) with
   handled-state; items 1, 2, 5 become definable.
4. **Cloud lane**: dispatch into workrooms; requires turning on/sizing the
   durable queue and confirming git-writeback where a watcher writes back
   (both pre-existing RED items — this feature is a forcing function).
5. **Khala Code Agents panel + escalation UX**; then the observability-
   flavored definitions (items 5–7) which mostly need more read-tool
   gateways (Cloudflare analytics, PostHog, logs), not new architecture.

## 6. Strategic notes and risks

- **Validation, and a wedge.** An independent, credible builder converged on
  the same architecture we've been assembling (typed definitions, enforced
  tool policy, harness-agnostic dispatch) from the tool-catalog side. We come
  at it from the execution side — we already own leases, isolation, metering,
  and payments, which a local-first tool catalog does not. The moat claim is:
  definitions + *enforcement* + *settlement*, not definitions alone.
- **Interop over rivalry.** `executor` speaks MCP both directions. Their tool
  catalog can be a `toolset` *source* for our definitions (an MCP toolRef is
  already in the schema shape), and our dispatch API can be a target their
  "Agents" feature calls. The same person is our QA-swarm design partner and
  first-customer candidate (QS7, #8067) — build the owned thing, keep the
  seam open, don't contribute our architecture upstream.
- **Permission-model honesty.** The whole feature is only as good as
  enforcement. If any lane lets a background agent reach tools outside its
  compiled policy (e.g. a harness's own shell tool bypassing the runtime
  boundary), the product claim is false. This must land as an INVARIANTS
  entry with tests in the tool runtime and the workroom gateway before any
  public promise is made.
- **Read-only reference discipline.** `projects/repos/executor` stays
  read-only; ideas port, code does not.

## 7. Cross-references

- Brain-dump screenshot: owner's desktop, 2026-07-02 (not committed).
- `projects/repos/executor` — README ("integration layer for AI agents"),
  `vision.md`, e2e harness previously audited in
  `docs/feature-requests/2026-06-24-autonomous-qa-e2e-from-computer-use.md`.
- `docs/khala/2026-06-25-pylon-linked-coding-capacity-routing-spec.md` —
  own-capacity invariant, typed classifier, Durable Streams.
- `docs/adr/0012-adopt-khala-native-terminal-tool-runtime.md` — the
  enforcement boundary the toolset compiles into.
- `docs/fable/2026-07-02-qa-swarm-product-plan.md` — QS7 sales motion with
  the same design partner.
