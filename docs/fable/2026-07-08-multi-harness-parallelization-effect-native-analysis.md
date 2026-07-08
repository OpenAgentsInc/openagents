# Multi-harness parallelization on Effect Native — analysis and suggestions

Date: 2026-07-08
Status: Fable analysis (flips no promise state). Responds to the owner
priority — **coding-agent parallelization across Claude Code / Codex /
Grok, steered from Khala Code mobile + desktop, ASAP, all via Effect
Native** — and to the Grok analysis pair
(`docs/grok/parallel-multi-harness-asap.md`,
`docs/grok/grok-cli-as-third-harness.md`). Sequencing authority remains
`MASTER_ROADMAP.md`; this doc proposes the rev 6.4 changes that would
make the priority official.

## 1. Verdict on the Grok analysis

**Endorse the spine.** Five things in those docs are exactly right and
should be treated as law for this program:

1. **Axis A (chat harness) ≠ Axis B (worker kind)** — keep them
   independent; one enum trying to mean both recreates the pre-pivot
   mess.
2. **Grok enters through ACP (`grok agent stdio`), never TUI scraping**
   — the client owns FS/terminal/MCP; the model is the brain.
3. **Contract-first waves** — schema literals land first (a ~1–3 day
   serial critical path), then adapters/fleet/Sync/UI parallelize.
4. **The June 29 laws hold at 3× engines** — typed claims, sustained
   refill concurrency, typed verify gates, product-visible control.
   Three engines without claims is duplicate-PR collapse at scale.
5. **Don't starve CX-3** — Codex-on-agent-computers is the cloud
   substrate every harness's workers will eventually share.

The rest of this doc is where I sharpen, correct, or go further.

## 2. Sharpening #1: Effect Native is the architecture, not the chrome

The Grok docs treat Effect Native as UI discipline ("don't build
throwaway React shells"). True but shallow. The deeper alignment:

**The EN intent algebra and the Khala Sync mutator vocabulary are the
same thing and should be one schema.** Effect Native's core commitment
is *interactions as named typed intents — serializable data, never
closures*. The multi-harness steering surface needs exactly three
mutators (`fleet_run_control`, `approval_decision`, `steer_message`)
flowing mobile → Sync → desktop/daemon authority. Those are not two
systems:

```text
EN view (mobile or desktop)
  └─ dispatches typed intent  ────────┐
                                      ▼
                    one Effect Schema intent/mutator package
                    (pause/resume/drain/stop, approve/deny,
                     steer text, harness/worker selection)
                                      │
        local runtime handles it  ────┤────  or Sync carries it to the
        (desktop dispatching to       │      authority (daemon/server)
         its own supervisor)          ▼
                          FleetRun supervisor / approval gate
```

Define the intents **once** (in `agent-runtime-schema` or a sibling
package), and let the EN runtime dispatch them locally on desktop and
over Sync from mobile. This gives cross-device steering *by
construction*: the approval card on the phone and the approval button on
the desktop dispatch the identical typed value; the event log that EN
already keeps (loggable/replayable intents) doubles as the audit trail
the receipts discipline wants. Do not let the Sync mutators and the EN
intents be designed by two lanes independently — that is the one place
this program could fork into two vocabularies that then need a bridge
forever.

**Corollary — the cockpit is already specified.** The multi-harness
fleet cockpit the Grok docs sketch (harness pill, worker cards,
approvals inbox, run timeline, fleet graph) is, nearly item for item,
the effect-native Phase 4 component set that is already filed:
Tabs/segmented control (#30), virtualized lists (#14), chips/badges/
meters (#39), toasts/status banners (#40), transcript (#35), the
GraphFigure fleet board (#37), and the desktop port proof (#42) whose
proof screen *is* a fleet cockpit. **Multi-harness UI demand and the EN
catalog build are the same work.** File the demand rows through EN-2
(#8572) naming the multi-harness screens as the demanding screens — this
gives the upstream catalog issues their "real screen" justification and
gives this program its UI without a single throwaway panel.

## 3. Sharpening #2: ride the Pylon fold, don't duplicate it

Rev 6.3 just decided PY-1 (#8578): extract `pylon-core` — typed Effect
services for custody/executor/presence behind a typed RPC contract, with
Khala Code desktop as the cockpit (PY-2 #8579). The Grok docs' file map
puts the Grok worker executor at `apps/pylon/src/grok-agent-executor.ts`
(mirroring `claude-agent-executor.ts`). **Don't.** That writes a new
module into exactly the surface PY-1 is about to carve up.

- **The Grok executor is born in `pylon-core`** (or lands in `apps/pylon`
  only if PY-1 has not started when Wave 1 begins — in which case it
  must be on PY-1's move list from day one).
- **PY-2's cockpit IS the multi-harness cockpit.** There is one Fleet
  pane. "Go online", account readiness, run supervision, and receipts
  are per-harness *rows* in that pane, not per-harness panels. PY-2 and
  the Grok docs' Wave 4 U2 lane should be merged into a single desktop
  cockpit lane or explicitly cross-linked with PY-2 as owner.
- **Capacity/readiness is one model.** `capacity.coding.grok.account.*`
  mirrors the Claude refs (good), and the account
  registry/quota/health ledgers in `pylon-core` P1 custody get a
  `grok` provider variant rather than a parallel Grok account store.

## 4. Sharpening #3: the cheapest big win is finishing Claude, not starting Grok

The Claude Pylon lane is ~80% parity and the Claude chat runtime is
partially landed. Finishing Claude Axis A/B parity is days of work
against known seams; Grok is greenfield against a third vendor's
protocol. Both should run in parallel (different people/agents, no
shared files beyond Wave 0 schemas), but if capacity forces a choice,
**Claude-parity-first delivers a real two-harness fleet sooner** — and a
proven two-harness FleetRun de-risks every three-harness assumption
(claims under mixed kinds, quota fallback classes, mixed closeouts)
before the Grok adapter even compiles. The demo everyone wants is three
logos; the engineering truth is that the second harness proves the
abstraction and the third one merely populates it.

## 5. Sharpening #4: an adapter conformance suite (steal EN's trick)

Effect Native enforces its catalog with a renderer conformance suite
driven by `componentTags`: a new tag fails until every renderer proves
support. Apply the identical mechanism to harnesses:

**A harness conformance suite driven by the harness-kind enum.** Adding
`grok_cli` to `AgentRuntimeAdapterKind` immediately *fails* CI until the
new kind has: (a) a chat-runtime fixture (startThread/startTurn/
interrupt/resume → neutral turn events), (b) a worker-executor fixture
(claim → pinned worktree → closeout with verify), (c) a capacity/
readiness probe fixture, (d) a metering-honesty fixture (exact fields
when present, `not_measured` otherwise — never synthesized), and (e) a
typed-failure-class fixture (`account_exhausted`, `account_rate_limited`
— never generic). This converts "three adapters stay consistent" from
review vigilance into mechanical enforcement, exactly like the June 29
claims law converted backlog-grepping into typed claims. It also makes
harness #4 (whatever it is) a fill-in-the-fixtures exercise.

Prerequisite: promote the neutral turn-event model
(`KhalaCodeDesktopChatTurnEvent`) from a desktop-local type to a
versioned schema (`khala.chat_turn_event.v1`) in
`agent-runtime-schema`, since mobile projections, Sync capture, and all
three adapters now depend on it. That is a Wave 0 item the Grok docs
imply (W0.4 "neutral ChatRuntime interface stabilized") but don't make
concrete.

## 6. Sharpening #5: `auto` is a typed policy, and v1 is dumb on purpose

The workspace semantic-routing rule applies: harness `auto` must never
be keyword/vibes routing. v1 `auto` should be embarrassingly simple and
fully typed: *first ready account with free quota, in a fixed preference
order, with a typed fallback event on every skip* (`account_exhausted` →
next). Cost/affinity/role scoring (the oh-my-pi matrix) comes only after
per-harness economics are measured — subscription-covered Codex vs
API-metered Grok have different marginal costs, and an `auto` that
optimizes before the ledger rows exist will optimize wrong. Put the
policy object in the same intent/mutator schema package so mobile can
*display and set* it as data.

## 7. What "ASAP" concretely means (the first 72 hours + the one owner gate)

Fixture-tier everything is reachable fast because contracts, not
integrations, are the critical path:

- **Day 0–1 (serial):** Wave 0 schemas — `grok_cli` literals,
  `workerKind: codex|claude|grok|auto`, `khala.chat_turn_event.v1`
  promotion, the shared intent/mutator package (§2), the conformance
  suite skeleton (§5, red for `grok_cli` by design).
- **Day 1–3 (parallel lanes, no shared files):** Grok ACP chat runtime
  against a **mock ACP process fixture**; Claude Axis A parity
  completion; mixed-kind FleetRun fixture (simulated workers, claim
  uniqueness under `targetConcurrency ≥ 3`); Sync fleet-run projection
  read path; EN-2 demand rows filed upstream.
- **The one hard owner gate:** X.ai auth for capacity hosts (API key or
  `grok login` device flow, per-owner, never shared) — file to
  NEEDS_OWNER immediately so the live-smoke tier isn't discovered
  blocked after the fixture tier is green. Everything else in the first
  two waves is fixture-provable without it.

Live-smoke and cloud tiers then follow the Grok docs' Waves 2–5
unchanged, with C1 (CX-3 Codex-in-VM) explicitly protected.

## 8. Proposed lane index (MH-*) and the rev 6.4 action

Consolidating the Grok docs' suggested board with this doc's additions —
file under a small **MH (multi-harness) lane family** so the priority is
legible in the tracker, cross-linked rather than duplicated where an
existing lane already owns the surface:

| Lane | Scope | Rides / depends |
| --- | --- | --- |
| MH-0 | Wave 0 schemas: `grok_cli` literals, workerKind enum, `khala.chat_turn_event.v1`, shared intent/mutator package | blocks all; 1–3 days |
| MH-1 | Harness conformance suite (enum-driven, red-until-proven) | MH-0 |
| MH-2 | Claude Axis A/B parity completion (chat runtime + worker lane to 100%) | MH-0; existing ~80% |
| MH-3 | `GrokAcpChatRuntime` + session store + projector (Axis A) | MH-0; mock-ACP fixture first |
| MH-4 | Grok worker executor **in pylon-core** + capacity/readiness + metering honesty (Axis B) | MH-0, PY-1 #8578 |
| MH-5 | Mixed-kind FleetRun: supervisor scheduling, claim uniqueness fixture, MCP verbs accept `grok` | MH-0; parallel with MH-2/3/4 |
| MH-6 | Sync projections + the three mutators (as §2 typed intents): mobile fleet peek, pause/approve/steer dogfood | MH-0, MH-5 |
| MH-7 | Cockpit: desktop multi-harness Fleet pane (merged with PY-2 #8579), mobile harness pill + fleet peek — EN-native, demand rows via EN-2 #8572 | EN-3 #8568, EN-5 #8574 |
| MH-8 | `auto` policy v1 (typed, dumb) + per-harness economics rows | MH-4/5 ledger data |
| MH-9 | Cloud parity: Grok/Claude workers on agent computers | CX-3 #8547 first — never before |

**Roadmap action:** record this as MASTER_ROADMAP **rev 6.4** — the MH
lane family inserted as a now-priority program running parallel to P1
sales (separate capacity, per the Grok doc's correct note that sales
agents never touch the coding claim registry), with CX-3 named as the
protected linchpin and the EN/PY cross-links above. Public
"multi-agent army" copy stays behind the promise registry until live
receipts exist.

## 9. Risks the Grok docs undercount

1. **EN catalog timing.** MH-7's components (virtualized lists, tabs,
   chips) sit in effect-native Phase 2/4 issues that are open, not
   landed. Mitigation is the rule the desktop already implies: **data
   first, chrome second** — adding `grok` to existing pills/enums in the
   current shell is a data change and allowed; *new panels* wait for EN
   or land as minimal RPC-first surfaces. Accept a short window where
   the mixed fleet is fully steerable but visually plain.
2. **Schema-package blast radius.** `agent-runtime-schema` is imported
   everywhere (desktop, pylon, mobile, sync). Wave 0 must be additive
   literals + new types only — no reshaping existing unions in the same
   change, or the "1–3 day critical path" becomes a week of fallout.
3. **Three session stores, three resume semantics.** Codex, Claude, and
   Grok all have different thread/session/resume models; the desktop
   session catalog is about to own three mappings. Get the mapping
   contract into MH-0's schema work (per-harness `sessionRef` as opaque
   data + capabilities flags for resume/fork) rather than three ad hoc
   JSON files.
4. **Approval-surface split-brain.** Grok has internal permission modes;
   Khala has product approvals. The Grok doc says approvals must surface
   in the Khala Inbox — make it stronger: harness-internal auto-approve
   is only ever enabled by the *product* approval posture (fleet
   unattended mode), so there is exactly one place a human grants
   authority.

## 10. Bottom line

The Grok analysis got the architecture right; the additions that make it
ASAP-able are (1) one typed intent/mutator vocabulary shared by Effect
Native UI and Khala Sync — steering as serializable data end to end;
(2) the multi-harness cockpit and the EN Phase-4 catalog recognized as
the same build, demand-registered through EN-2; (3) the Grok executor
born inside the rev 6.3 `pylon-core` extraction instead of the surface
being deleted around it; (4) Claude parity treated as the fastest real
two-harness fleet while Grok is built in parallel; (5) an enum-driven
harness conformance suite so consistency is mechanical; and (6) a
deliberately dumb, fully typed `auto` v1. File MH-0..9, record rev 6.4,
send the X.ai auth gate to NEEDS_OWNER today, and the fixture-tier
three-harness fleet is a same-week outcome.
