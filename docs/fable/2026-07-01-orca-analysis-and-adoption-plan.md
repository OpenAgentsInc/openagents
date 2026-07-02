# Orca — Analysis, Audit, And Prioritized Adoption Plan

Date: 2026-07-01
Status: analysis + adoption plan. Reviews everything this codebase has
written and built about Orca (`stablyai/orca`, vendored read-only at
`projects/repos/orca`), audits where we actually stand against it, and lays
out what to take or adapt from it in what order — including the mobile
companion direction (Khala iOS reaching rough feature parity with Khala Code
Desktop, minus a few things). Companion to the fleet fan-out instructions,
the QA framework design, and the Effect integration audit in this folder.
Documentation-only; flips no promise state.
Execution: Priorities 1–5 in §3 are scheduled in the unified
[`ROADMAP.md`](./ROADMAP.md) — Priority 1 is folded into the fan-out
Lane A/B foundation (one state store), Priority 2 into the status-spine
workstream, Priority 3 is the mobile-companion workstream, and the Artanis
audit's Priority 3 administers the same spine.

Reference-repo policy (already established in
`docs/ade/2026-06-27-orca-orchestrator-adaptation-report.md` and unchanged
here): Orca is MIT-licensed reference material — **adapt patterns, never
vendor code, never use the name** in our product surfaces.

## 1. How To Think About Orca

Orca ("The AI Orchestrator for 100x builders" — run Codex, Claude Code,
OpenCode, or Pi side-by-side, each in its own worktree, tracked in one
place, steerable from a phone) is three things to us at once:

1. **Validation.** A ~3.5-month-old Electron app with 5,770 commits (~57/day
   in the last two weeks), a shipped iOS/Android companion, and real
   adoption is proof that "orchestrate a bunch of different harnesses from
   one desktop app with a mobile companion" is a product category, not a
   hunch. It is almost line-for-line the locally-installed version of what
   we are building as a distributed, Cloudflare-mediated service.
2. **A pattern library.** Its orchestration data model, its
   handoff-vs-supervised-dispatch taxonomy, its mobile pairing/security
   model, and several UX loops are clean, proven, and portable to our
   Effect/Foldkit idiom. We already ported some of them.
3. **A foil.** Understanding *how* Orca achieves its breadth tells us
   exactly where not to follow it — and where our depth is the moat.

The single most important architectural fact: **Orca is not an
SDK-orchestration engine. It is a terminal multiplexer with git-worktree
management and a SQLite message bus.** Agents run as their real TUIs inside
node-pty PTYs rendered by xterm.js; Orca learns their status by parsing
terminal OSC escape codes and title glyphs (Claude idle `✳`, Gemini
`✦/◇/✋`) and by injecting managed hook scripts into each vendor's own
config files; agents participate in coordination by shelling out to an
`orca orchestration` CLI that pokes a SQLite message queue. That is how it
supports ~30 harnesses cheaply — and why it has no structured tool-call
data, no real token accounting (it scrapes vendor usage files), no
verification (a worker is "done" when it self-reports a three-sentence
summary the coordinator trusts), no claim/dedup for parallel work, and no
sandboxing (agents are full-credential local processes; approvals are
delegated to each TUI's own prompt).

We are the inverse shape: deep typed integration (Codex app-server as the
harness kernel, Claude Agent SDK executor), exact token accounting,
verification gates and closeout evidence, deterministic delegation with a
recovery ladder, isolated per-account homes, and a claim registry on the
way — but weaker at the visibility/control UX layer Orca polishes daily,
and with **zero** mobile companion. The strategy writes itself: **keep our
spine, adopt their surfaces.**

## 2. Where We Are: The Five-Port Scoreboard

The 2026-06-27 adaptation report extracted five ports. Honest status as of
today (issues #6404–#6407, #7808, #7809 — all closed; the closing ≠ the
goal in three cases):

| Port | What it was | Status |
| --- | --- | --- |
| 1. AgentRunner registry | Unify the ~80%-identical Claude/Codex executors behind one interface + declarative registry | **Shipped and live.** `apps/pylon/src/agent-runner-registry.ts` (PR #6505) is in the real assignment path (`assignment.ts` → `executeRegisteredAgentRunner`). |
| 2. Typed coordinator + task DAG | Replace bash-supervisor ad-hoc state with a persisted task/dispatch/message model | **Built, tested, DORMANT.** `apps/pylon/src/orchestration/` (1,132 lines + 501 test lines; PRs #6588/#7813/#7818) is a faithful port — task DAG with dependency promotion, dispatch contexts with 3-strike circuit breaker and 5-min-fresh/10-min-hung liveness, base-drift guard (refuse >20 commits behind), group addressing (`@all`/`@idle`/`@runner:<kind>`), plus our own `VirtualHead` reservations. **A repo-wide grep finds zero runtime consumers.** The live fleet still runs on bash supervisor process-state. |
| 3. Operator dashboard | Live/retained agent-status store + annotate-diff loop on the web | **Mock-data shell.** `/pro` page (PR #6596) renders the right shapes (`stateStartedAt` vs `updatedAt`, state history, diff comments) from hardcoded sample state; live ingest was an explicit follow-up that never followed. |
| 4. Mobile companion | E2EE-paired phone: status subscription, finish notifications, steer | **Never filed, zero code.** Deliberately deferred on 06-28 ("mobile is secondary"); no issue exists. |
| 5. Artanis action surface | Uniform status ingest + gated orchestration verbs | **Partial.** Approval-gate verbs shipped (`khala artanis approve/reject`); the orchestration verbs (task-create/list/dispatch) did not. Status ingest is Codex-lane-only. |

Two adjacent pieces matter: `agent-status-reporter.ts` defines the
runner-neutral status/control contract
(`openagents.pylon.agent_runner_status_event.v1`, neutral states, control
verbs, `stateStartedAt`/`stateHistory`, hashed public-safe refs) and is
consumed by the live Codex turn reporter — while the Khala iOS Fleet
Inspector polls a *bespoke* `/api/operator/fleet/status` snapshot instead.
The two halves of our own status story have not been introduced to each
other, and the missing middle is exactly Port 2's unwired store.

Also confirmed: **we deliberately do not have Orca's compare-N-merge-winner
flow**, and every layer of our system enforces the opposite invariant (one
worker per distinct work unit; the fan-out instructions make the claim
registry structural precisely because the June 29 burn produced duplicate
PRs on 59 issues, up to 7 per issue). That is the right default for backlog
burn-down. A bounded best-of-N mode could exist someday as an *explicit*
run mode with one claim covering N candidate attempts — but it is not a
gap, it is a policy difference.

### Our mobile app today (`clients/khala-ios/Khala`)

A clean native SwiftUI chat client (25 files, 4,232 lines, zero third-party
deps, TestFlight-shipped): streaming chat to Khala, the owner-only Artanis
channel, and local Apple FM — plus a **read-only Fleet Inspector** polling
the operator snapshot with heavy client-side redaction. A `codex_agent_task`
delegation client method exists but no UI calls it. There is no WebSocket,
no push notifications, no pairing, no steering. (Side note the mobile docs
should reconcile: the "voice app spec" describes voice features the code
does not contain; the app matches the ChatGPT-style spec.)

## 3. What To Take From Orca, In Order

The ordering principle: each step must pay for itself in the fleet-fan-out
and episode-245 storylines already in flight, and the mobile companion
arrives as a *projection* of state the desktop work creates anyway — not as
a parallel system.

### Priority 1 — Wire the dormant orchestration store into the live loop

The adaptation report's own 2026-07-01 note names this as "Next", and the
fleet fan-out instructions independently specify a `FleetRunSupervisor`
that needs persisted state. **These are the same work item — do not build
two state stores.** The FleetRun record and claim registry from the fan-out
doc should be implemented *on* `apps/pylon/src/orchestration/` (tasks =
work units with deps; dispatch contexts = workers with heartbeats, circuit
breakers, and slots; messages = the lifecycle bus; virtual-head
reservations = the merge-queue base pinning). The bash supervisors demote
to process launchers driven by store state: live desired-slot, pause,
dispatch-attempt, completion, and work-claim state is store-backed via
`apps/pylon/src/orchestration/supervisor-state.ts`, while shell keeps only
process PID/log/cache files and the launchd wedge telemetry consumed by the
fleet-liveness gate. Adopt Orca's **handoff vs supervised-dispatch taxonomy**
verbatim in the FleetRun model: `codex_spawn` one-shots are handoffs (no
tracking state), fleet runs are supervised dispatches (DAG-tracked). This
one step converts Port 2 from dormant library to production spine and
de-duplicates the fan-out plan's Lane A/B.

### Priority 2 — One status spine, end to end

Make `agent_runner_status_event.v1` the *only* status vocabulary from
runner to glass: orchestration store → status-control projection → the
desktop Fleet cockpit (fan-out Lane C consumes this instead of bespoke
`codexFleetStatus` shapes) → the Worker → mobile. Adopt the Orca dashboard
state-model details we already validated: live vs retained entries,
`stateStartedAt` distinct from `updatedAt` (unread/attention tracking),
rolling `stateHistory`, decay-to-idle. This also un-mocks the `/pro` page
(Port 3) for free, since its shapes were built to match. The 06-29
after-action's diagnosis ("No active Codex sessions" while `CODING: 20`
was live) is the bug class this kills.

### Priority 3 — The mobile companion (file Port 4 now)

This is the piece with zero code and the one the owner has now explicitly
prioritized. Adopt Orca's security shape wholesale and its transport shape
selectively:

- **Steal wholesale**: QR pairing (`orca://pair`-style offer with endpoint
  + per-device token + public key), per-device bearer tokens in the
  keychain, NaCl box E2EE at the app layer, and — critically — an
  **allowlisted RPC surface with a test that enforces every
  mobile-callable method is explicitly registered** (Orca's
  `mobile-rpc-allowlist.test.ts` is a discipline worth copying exactly).
- **Adapt the transport**: Orca is LAN-direct (desktop is the WebSocket
  server; remote reach requires Tailscale/VPN). Per the adaptation report,
  ours should be **Durable-Object-relay-mediated** so it works for remote
  fleets and multiple devices, with APNs layered for background delivery
  (Orca's socket-push only works foregrounded; our native app has no OTA
  constraint but also currently zero push capability). A LAN-direct fast
  path can come later; the DO relay is the correct first transport for a
  fleet that already reports to the Worker.
- **Feature parity target** ("more or less the same feature set, minus a
  few things"): the mobile app becomes a projection of the desktop's Fleet
  cockpit and Inbox — fleet run status and worker cards, account
  rate-limit meters, throughput gauges, **notify-on-finish/blocked**,
  **approve/reject from the phone** (the Inbox's typed responses; note
  `khala artanis approve <gateRef>` already exists in the CLI but not the
  app), **steer** (send a follow-up/objective to a run or worker), and
  bounded diff/PR review. **Minus**: full terminal emulation/splits,
  Design Mode/computer-use, worktree file management, and local execution
  — the phone observes, decides, and steers; it does not host work.
- Sequencing inside this priority: (a) pairing + DO relay + read-only
  status subscription (replaces the bespoke poll), (b) push notifications
  on finish/blocked/approval-needed, (c) approvals + steer, (d) diff
  review. The existing Fleet Inspector and the dormant `requestCodexTask`
  client method are the seeds of (a) and (c).

### Priority 4 — The review loop and cockpit polish

- **Annotate-AI-diff-and-ship-back**: comment on diff lines and return the
  comments to the agent as steering input. Orca's strongest UX loop; lands
  in the desktop diff renderer first, mobile second, and pairs naturally
  with our verification gates (review what the verifier already blessed).
- Source-control AI actions (commit-message/PR-body/fix-checks prompts) —
  cheap adjacent wins in the same surface.

### Priority 5 — Later, if/when the shape calls for it

- **Terminal scrollback checkpointing** (`daemon-checkpoint-file.ts`
  pattern: checkpoint + incremental log with generation pairing) — only if
  the desktop grows live terminal panes; our transcript-first UI may never
  need it.
- SSH/remote worker hosts (Orca's relay) — our answer is Pylon nodes on
  other machines, which already exists conceptually; revisit when a second
  machine matters.
- Automations/schedules — we have Pylon scheduling primitives; expose them
  through the same FleetRun surface rather than porting Orca's.
- Agent-facing SKILL.md capability docs for the orchestration CLI — nice
  discipline, low cost, adopt opportunistically.

### Do not copy

- **PTY + OSC-glyph status detection and vendor-config hook injection.**
  Our Codex app-server and Claude SDK adapters give structured events Orca
  can only guess at. Breadth-per-effort is their trade; depth is ours.
- **The 30-harness breadth** as a goal. Our target set is Codex, Claude
  Code, and Khala-routed (`codex | claude | auto`), per the multi-harness
  doc.
- **Trust-the-summary `worker_done`.** Ours stays verification-gated
  (verify command green + closeout evidence) — the single clearest
  differentiation opportunity their model leaves open.
- **Unclaimed parallel work.** The claim registry stays structural.
- **Full-credential local execution as default.** Isolated homes and
  sandbox containment stay.
- The Electron shell, embedded Chromium/Design Mode, computer-use native
  modules, emulator bridge, and `orca.yaml` (which is just a dogfood setup
  script, not a config system).

## 4. Risks And Watch Items

- **Their velocity.** ~57 commits/day means any feature-race on surface
  breadth is unwinnable; the plan above deliberately races on spine
  (verification, accounting, determinism, claims) and adopts only their
  stable, settled patterns (the orchestration schema and mobile security
  model have been stable while their recent churn is terminal/SSH/i18n
  hardening).
- **Port-2 rot.** Dormant code drifts; wiring the store (Priority 1) is
  urgent precisely because the fan-out epic would otherwise build a
  competing state model and strand 1,600 lines of tested work.
- **Two status vocabularies.** Until Priority 2 lands, the desktop, the
  operator snapshot, and the runner-neutral contract are three dialects;
  every new consumer added before unification increases migration cost.
- **Mobile scope creep.** The companion succeeds as a projection with four
  verbs (observe, get notified, approve, steer); resisting terminal
  emulation and file management on the phone is what keeps it shippable on
  the native-Swift, no-OTA policy.
- **Naming/attribution hygiene.** Keep "Orca" out of product copy,
  commits, and public projections; patterns only, per the standing policy
  (and note for grep-archaeologists: most "orca" hits in older docs are
  `…ORCA…` substring false positives like `resolveValidatORCAndidates`).

## 5. Bottom Line

Orca proved the product category and gifted us — under MIT — a tested
orchestration data model, a taxonomy that prevents phantom coordination
state, and a complete mobile-companion security design. We already ported
the data model; it is sitting unwired while the live fleet still runs on
bash process-state, and the mobile port was never even filed. The plan is
therefore less "adopt more of Orca" than "finish adopting what we chose,
in the order that serves the work already in flight": wire the
orchestration store as the FleetRun spine, unify on the runner-neutral
status contract from runner to cockpit, then build the mobile companion as
an E2EE-paired, DO-relayed, allowlisted projection of that same state with
notify/approve/steer as its verbs. Everywhere Orca is shallow —
verification, accounting, determinism, claims, isolation — we are already
deep, and that spine, surfaced through their grade of UX, is the product.
