# Surface-Vision Gap Analysis and Delivery Roadmap

Date: 2026-07-17
Class: proposal / dispatch-candidate source — **not** sequencing authority.
`docs/sol/MASTER_ROADMAP.md` remains the canonical roadmap; work below enters
execution only through the normal admission paths (owner-accepted plans, work
packets, or strict-bug issues per repository policy). Spec authority: the
three surface-vision ProductSpecs at `spec_revision: 3`:

- `specs/desktop/desktop-trust-complete-workbench.product-spec.md`
- `specs/mobile/mobile-any-host-fleet-controller.product-spec.md`
- `specs/web/openagents-com-trust-surface.product-spec.md`

Evidence sources: repository state on `main` at 2026-07-17 (audited this
session: MASTER_ROADMAP rev 117, Desktop `GUARANTEES.md`,
`docs/fable/2026-07-17-full-auto-implementation-audit.md`, the open GitHub
issue set, and file-level surveys of `apps/openagents-desktop`,
`apps/openagents-mobile`, `apps/openagents.com`, `apps/pylon`, and
`packages/*`), the teardown catalog, and transcripts 200–256.

A claim of "Implemented" below means code and tests exist on `main` for the
substance of the item; it is not a release claim, a promise-registry state,
or an assurance verdict. "Partial" and "Unverified" are load-bearing words.

---

## 1. Current-state snapshot

**Desktop.** The ProductSpec-native Codex Workroom MVP is complete and
owner-accepted (epic #8756 closed against signed/stapled RC9; AssuranceSpec
program #8767/#8770 fully CONFIRMED; public Observatory trace live). The app
has: a closed Runtime Gateway protocol (v8) with enforced UX contracts
(~35 enforced contracts in `src/contracts/ux-contracts.ts`); local-first
identity with optional account link; loss-accounted Codex history; a live
agent graph (enforced contract) with history agent-tree; turn checkpoints
with staged revert (host-side, no UI by MVP allowlist); a closed command
registry with fixed chords; per-thread composer ownership; image input;
Codex primary lane + Fable/Claude SDK lane + experimental ACP lanes (Grok,
Cursor) behind a pinned conformance matrix; a fail-closed signed/notarized
macOS release lane with launch receipts, auto-rollback, and a signed
ReleaseSet v2 contract (six targets defined, macOS applier only). Full Auto
is a durable, restart-survivable, exactly-once continuation loop (all 13
hardening children of #8873 merged, provider-lane generalization #8901, spec
projection #8902, stall/resume fixes verified on main) with a loopback
control API/CLI/MCP — but it is composer-toggle UX, single-lane per profile,
dev-build only (no release tag contains any Full Auto commit), and its
assurance program is unstarted (37/37 obligations `needs_design`).

**Mobile.** A strong but narrow foundation (~6.7k lines, 22 test files):
local-first identity + GitHub PKCE + SecureStore vault, Khala Sync over
expo-sqlite, a headless conversation core with question/approval/plan
interactions and cancel/interrupt, an execution-target catalog, fleet-run and
live-agent-graph projections, OTA updates via the owned server. Absent:
pairing/QR, any-host directory, attention inbox and push token registration
(server push infrastructure is built), files/changes/terminal/preview modes,
a named outbox module, widgets/Live Activities, voice, portable-session
client. The roadmap currently holds PORT-03–08 and mobile expansion lanes as
closed-not-planned tombstones pending new owner authority.

**Web.** Live: `/`, `/forum`, `/promises` + the programmatic promise
registry (states green/yellow/red/planned/degraded; 143 records),
`/api/public/khala-tokens-served` + history/mix routes, `/stats`, `/khala`
chat, `/AGENTS.md`, `/observer` + one public Observatory trace, the
OpenAI-compatible `/v1/chat/completions` API, referral payout-receipt routes
(RL-1 wired; no settled payout; promise yellow). Absent or partial: a
general `/trace/{uuid}` grammar, visibility receipts, a release-manifest
trust ledger + receipt-verification endpoint, data-flow matrix, zero-install
fragment-token onboarding, proof-first project boards, treasury page (stats
carries figures; no route).

**Cross-cutting.** `packages/behavior-contracts`, `product-spec`,
`assurance-spec` (most built-out), `environment-auth` (DPoP),
`portable-session-contract` + extensive server-side portable-session
modules, `khala-sync-*` stack with fleet cockpit/steering, `apps/oa-updates`
(Expo-compatible OTA + desktop-dist), `autopilot-control-protocol` (pairing
+ E2EE primitives, unwired). Pylon carries the account/quota/health/fleet
execution layer.

**Queue reality.** The active owner-approved program is the FULL HARVEST Amp
Fast Follow plan (thread fabric → routing/specialists → review/reader →
placement/remote control → generated clients/signed plugins). The
episode-256 direction ("build the whole short-term roadmap of the company
around" AFK-reliable Full Auto) is already partially reflected in the open
issue set (#8967 epic and children, #8913 DIST epic, #8980/#8982 mobile).
This document proposes how the spec-rev-3 vision decomposes into that
reality; the owner arbitrates priority between HARVEST and the Full Auto
program (they overlap heavily — see §5, Track FA notes).

---

## 2. Gap analysis — Desktop (spec rev 3, AC-1…AC-24)

| AC | Requirement (short) | Status | Evidence / gap |
| --- | --- | --- | --- |
| AC-1 | Queue/steer explicit; admission→promotion→execution→terminal as distinct facts | Partial | Composer queue/steer + per-thread ownership enforced; the four admission states are not all rendered as distinct transcript facts |
| AC-2 | Authority manifest + execution receipt pair per run | Absent | No authority-manifest artifact in desktop src; capability registry and receipts exist separately |
| AC-3 | Profiles compiled to OS enforcement, fail closed | Absent | Execution profiles exist only as Full Auto lane/account/model profiles; no OS-compiled profile set; sandbox stays `danger-full-access` |
| AC-4 | Complete agent tree, live children, gap nodes | Implemented (partial gaps) | `runtime_gateway_live_agent_graph.v1` enforced; history tree + inline cards; explicit orphan/gap-node rendering not proven |
| AC-5 | Staged conflict-aware rewind w/ irreversible disclosure | Partial | `workbench.turn_checkpoints.v1` host-side with staged revert; deliberately no renderer UI under MVP allowlist |
| AC-6 | Worktree lifecycle receipts; delivery states; confidence tiers | Partial | Worktree awareness + dev-preview isolation; no general lifecycle manager; delivery states and draft/verified/reviewed/bonded tiers not first-class |
| AC-7 | Fuse/IPC hardening oracles in release tests | Implemented | Hardened main, schema-decoded closed IPC, Gatekeeper oracles; fuse-specific oracle coverage should be confirmed once in CI |
| AC-8 | Transcript virtualization + frame baselines as gates | Partial | History catalog/browser virtualized (measured 25.6×); chat transcript anchored-scroll only; no checked-in chat frame baselines |
| AC-9 | Signed manifest updates, no downgrade, rollback, drain | Implemented (macOS) | ReleaseSet v2 signed contract, launch receipts, auto-rollback, update host; Windows/Linux appliers absent (DIST) |
| AC-10 | Hermetic profile w/ admitted-input manifest | Absent | No hermetic mode; nearest is isolated-app-proof env flag |
| AC-11 | One command registry, identical outcomes all paths | Implemented (partial) | Closed intent registry drives palette; model-proposed-action path parity unproven |
| AC-12 | Hotkeys never reshuffle; user-bindable | Partial | Stable fixed chords + ⌘1–9 history; no user keybinding editor |
| AC-13 | Per-thread composer/queue/attachment isolation; no replay | Implemented | `composer_thread_ownership.v1`, bug-bash fixes merged and contract-tested |
| AC-14 | Effective model/provider/account identity per message | Partial | Effective-identity doctrine landed for lanes; per-message requested-vs-effective metadata display not uniformly proven |
| AC-15 | Fleet lights only from decoded fresh receipts | Partial | Fleet substrate + arbiter-discipline design; desktop fleet page breadth limited; Pylon owns account health/quota ledgers |
| AC-16 | Reasoning expanded by default; inline usage; no UUIDs | Partial | Inline usage counter shipped; reasoning-expansion and UUID hygiene fixed in bug bash but not contract-pinned everywhere |
| AC-17 | ProductSpec workroom: packets retain revision/criterion/evidence; no false-green | Implemented | MVP closed with owner acceptance; SM-9 machinery via Assurance program |
| AC-18 | Restart recovery + running-build disclosure | Partial | Turn recovery + Full Auto restart e2e green; build/branch disclosure exists in dev-preview contract, not a general surface |
| AC-19 | Honest exhaustion errors + failover to next account | Partial | Pylon rotates accounts with typed health; desktop Full Auto pins one lane/account per profile (no in-run rotation) |
| AC-20 | Full Auto dedicated run mode (launcher, read-only, play/pause/stop) | Absent | Open issue #8974 (FA-UX-01); today a composer toggle |
| AC-21 | Never halt on routable limit; multi-model/account/provider rotation | Absent | Single-lane durable profile by design (rev 9); #8967 epic scope |
| AC-22 | Same-thread cross-provider handoff, proven end-to-end | Partial / Unverified | Manual lane switch worked live (Claude→Codex); bounded host-owned history exists; no A-writes-fact/B-uses-it acceptance test; #8976 |
| AC-23 | Active-run thread never evicted/unopenable | Implemented (unverified at pressure) | LRU retention fix + regression merged (`8cb900bbf9`); real thread-pressure e2e not re-run |
| AC-24 | Run report + replayable fixture per run | Absent | Metrics plumbing #8911 default-off; no bounded run report; #8973 |

Success metrics: SM-6 (15-min activation) effectively proven by the MVP
release journey; SM-4 partially evidenced; SM-9 machinery exists (target
zero incidents is ongoing); SM-10/SM-11 (typed termination rate, multi-day
AFK dogfood) unmeasured and unattempted.

## 3. Gap analysis — Mobile (spec rev 3, AC-1…AC-13)

| AC | Requirement (short) | Status | Evidence / gap |
| --- | --- | --- | --- |
| AC-1 | QR pairing, bootstrap→scoped credential, vault-only | Absent | Pairing primitives exist unwired in `packages/autopilot-control-protocol`; no mobile pairing UI |
| AC-2 | Durable outcome + receipt per remote action | Partial | Typed intents with durable outcomes via Sync; receipt rendering absent |
| AC-3 | Visible editable outbox; exactly-once replay | Partial | Offline queue/expiry semantics in conversation core; no named outbox module or UI |
| AC-4 | Portable session move w/ exclusive generations | Designed-only (server) | `portable-session-contract` + extensive khala-sync-server move/authority modules; no mobile client; PORT-03–08 tombstoned |
| AC-5 | Attention inbox; push revalidate-at-open | Partial | Server push routes/preferences complete; client handles notification taps; no token registration, no inbox UI |
| AC-6 | Safe writeback: no force push, post-image receipts | Absent | No changes/writeback surface on mobile |
| AC-7 | Complete agent-graph drill-down w/ gaps | Implemented | Live-agent-graph projection + bounded rows + tests |
| AC-8 | Fault injection: honest degraded states | Partial | Deterministic offline-expiry tests; no fault-injection harness (network proxy, token revoke, host restart) |
| AC-9 | Screenshot harness vs disposable real servers | Absent | 22 deterministic test files; no device/screenshot matrix |
| AC-10 | Capacity as quantities, receipt-gated readiness | Partial | Fleet-run + execution-target projections; quantities/receipt-gating not surfaced |
| AC-11 | Effective identity per message on mobile | Partial | Provider-neutral timeline; per-message effective-model metadata not surfaced |
| AC-12 | UI-first enrollment/visibility/policy | Partial | Auth/link flows are UI; pairing/policy/visibility screens absent |
| AC-13 | Full Auto runs listed w/ remote Play/Pause/Stop + run reports | Partial | Fleet-run rows render; no Full Auto run objects, no remote controls; open issues #8980/#8982 (near-duplicates — consolidate) |

## 4. Gap analysis — Web (spec rev 3, AC-1…AC-13)

| AC | Requirement (short) | Status | Evidence / gap |
| --- | --- | --- | --- |
| AC-1 | Thread URLs render typed projection + replay-to-live + gaps | Partial | `/khala` chat-sync exists; no general authorized thread-object URL surface |
| AC-2 | Receipted visibility transitions, named audiences | Absent | No visibility-transition flow or receipts |
| AC-3 | Per-call provider/model/cost; counters reconcile to rows | Partial | Exact token rows + public counters + mix routes; per-call routing receipts not user-surfaced |
| AC-4 | Trust ledger: signed manifests + receipt verification endpoint | Absent | ReleaseSet v2 exists in-app; nothing published; no verification endpoint (referral receipts are the one public receipt route) |
| AC-5 | Zero-install front door w/ fragment token | Absent | No `npx`-style pairing front door |
| AC-6 | Web supervision parity + continuation links | Partial | Khala chat + forum; no approvals/steer parity or cross-surface continuation links |
| AC-7 | Data-flow matrix per work-unit type | Absent | Not published |
| AC-8 | Counter monotonic, ledger-convergent, dogfood split | Implemented (partial attestation) | Counter + history + demand-mix routes with tests; formal reconciliation attestation not published |
| AC-9 | Routing disclosure per response | Partial | `/v1/quote`, models, gateway readiness exist; per-message disclosure (msginfo pattern) incomplete |
| AC-10 | Referral binding + accrual from receipted rows | Partial | RL-1 accrual hooks + public payout-receipt routes; no settled payout; promise yellow |
| AC-11 | QA run share views: verdicts, videos, accounting from receipts | Partial | `/observer` + one public Observatory trace page; no general run-view grammar |
| AC-12 | Proof-first boards from authority records | Absent | Designed in 253-notes only |
| AC-13 | Promise registry full state machine, agent + human parity | Implemented (vocab drift) | Registry live with green/yellow/red/planned/degraded; spec's episode-234 vocabulary adds RED-Elected and WITHDRAWN — reconcile (see §6.1) |

---

## 5. Roadmap — tracks, epics, and work items

Conventions: existing GitHub issues are cited by number. Proposed work items
carry packet IDs (`FA-…`, `TR-…`, `WB-…`, `REL-…`, `MOB-…`, `WEB-…`,
`XC-…`) and are **not** GitHub issues (repository policy: feature issues are
not created ad hoc; these enter execution as owner-accepted plans/packets or
join existing epics). Effort classes: S (≤1 agent-day), M (1–3), L (3+ or
multi-lane). Every epic's exit criterion includes: `pnpm run check` green,
behavior contracts for any owner-stated expectation, and evidence linked
from the owning spec's Related Artifacts at the next `spec_revision`.

### Track FA — Full Auto flagship (Desktop) — the proposed P0 successor

Owner signal: episode 256. Existing anchors: epic **#8967**, spec
`specs/desktop/full-auto.product-spec.md` (rev 9), the corrected audit, and
the DIST chain. This track absorbs, rather than competes with, the HARVEST
directives it overlaps (Day-4 placement/remote-control and Day-2 routing
map directly onto FA-E2/FA-E6).

**FA-E1 — Dedicated run mode (existing #8974, FA-UX-01).** Surface AC-20.
Items: FA-E1.1 run-object schema + durable play/pause states in the registry
(M); FA-E1.2 launcher beside New Session + one-time setup (objective,
workspace, routing policy) (M); FA-E1.3 read-only run view (no composer;
Play/Pause/Stop; live state, rotations, elapsed budget) (M); FA-E1.4
composer-toggle migration + tombstone (S); FA-E1.5 behavior contracts +
oracles for the run-mode journey (S). Depends: none. Exit: AC-20 oracle
green; toggle removed or aliased; CUT-DSK-06 honored.

**FA-E2 — Multi-lane routing policy and never-halt failover.** Surface
AC-19/AC-21. Items: FA-E2.1 routing-policy schema (ordered lane/account/
model preferences, per-provider terms guards, own-capacity-only invariant)
(M); FA-E2.2 registry migration from single bound profile to policy (M);
FA-E2.3 in-run rotation on limit/exhaustion/error with typed rotation
records (L); FA-E2.4 model-tier fallback within a lane (the Fable→Opus
case) (M); FA-E2.5 surfaced honest provider conditions + burn-down per
account (S); FA-E2.6 rotation receipts in run report (S). Depends: FA-E1.1
(run object), FA-E4.1 (report schema) soft. Exit: AC-21 oracle — a
simulated limit mid-run continues on the next admitted lane without human
input; SM-10 measurable.

**FA-E3 — Cross-provider handoff proof (existing #8976, FA-QA-01).**
Surface AC-22. Items: FA-E3.1 bounded host-owned-history handoff contract
hardening (M); FA-E3.2 the six named sidebar-visible tests, including
A-writes-fact/B-uses-it for Codex↔Claude and Codex↔ACP lanes (M); FA-E3.3
handoff receipts + honest per-pair support matrix (S). Depends: none
(parallel to FA-E2). Exit: AC-22 oracle green; support matrix published in
GUARANTEES.

**FA-E4 — Run evidence loop (existing #8973 + #8911).** Surface AC-24.
Items: FA-E4.1 bounded run-report schema + emission at every terminal (M);
FA-E4.2 metrics on by default (repo-grounded first actions, consecutive
turns, stop reliability) (S); FA-E4.3 transcript-analysis workflow from
private dogfood runs (existing #8973) (M); FA-E4.4 replayable fixture-run
harness — any failed run reproducible (L). Depends: FA-E1.1. Exit: AC-24
oracle; one failed-run fixture replayed in CI.

**FA-E5 — Durability hardening.** Surface AC-23 + residual bug-bash. Items:
FA-E5.1 thread-pressure e2e (many threads, cache churn, active run
survives) (M); FA-E5.2 packaged-restart resume observed on a signed build
(the rev-9 sole success metric, never recorded) (S, owner-observed);
FA-E5.3 long-window soak harness (6–48h synthetic) (M). Depends: REL-E1 for
5.2. Exit: AC-23 pressure test green; owner observation recorded.

**FA-E6 — Guidance layer.** Surface the Guidance-Module lineage. Items:
FA-E6.1 typed between-turn decision record (goal/state/budget/environment
snapshot per continuation) (M); FA-E6.2 confidence-gated continuation
(low-confidence → pause-with-reason instead of blind continue) (M); FA-E6.3
hard guardrails schema (deterministic constraints that override guidance;
non-overridable set) (M); FA-E6.4 budget dials (token/time caps per run)
(S). Depends: FA-E1, FA-E4.1. Exit: guardrail violation halts with typed
reason in run report; behavior contracts for the pause-with-reason journey.

**FA-E7 — Assurance + release admission (existing #8978 FA-AS-01, #8979
FA-REL-01).** Items: FA-E7.1 author/admit the Full Auto AssuranceSpec (37
obligations out of `needs_design`) (L); FA-E7.2 execute obligations to
receipts (L); FA-E7.3 release admission: first tag containing Full Auto,
packaged restart proof, dogfood + promise evidence (M; depends REL-E1);
FA-E7.4 flip `autopilot.desktop_full_auto_guidance.v1` from red only on
evidence (S). Exit: SM-11 (one 24–48h owner-AFK run with reviewable report)
recorded; promise state moves on evidence.

### Track REL — Release breadth and provenance (existing epic #8913)

**REL-E1 — First tag containing Full Auto (macOS).** Items: existing DIST
prerequisites already merged (#8915 ReleaseSet v2, #8918 update host,
#8931 preview); REL-E1.1 cut and promote the next signed macOS tag off
current main (S, owner ceremony); REL-E1.2 wire the live feed host
(automatic update delivery — "not guaranteed yet" in GUARANTEES) (M). Exit:
a public tag whose bits contain the Full Auto loop; update applied via
signed ReleaseSet with launch receipt.

**REL-E2 — Cross-platform matrix (existing #8917, #8919, #8920, #8921,
#8925, #8926).** Five-target owned-runner matrix → macOS pair → Windows
NSIS Authenticode → Linux AppImage/DEB/RPM → promote + `/download` →
one-command release. Exit: per existing issue acceptance criteria; spec
AC-9 across platforms.

**REL-E3 — Component compatibility ledger.** Surface the spec's ledger
item. Items: REL-E3.1 ledger schema (shell/engine/renderer/extension
versions, protocol min/max, hashes, last-known-good) (M); REL-E3.2
user-visible receipt surface (S); REL-E3.3 publish alongside ReleaseSet on
the web trust ledger (ties WEB-E3) (S). Depends: REL-E1.

### Track TR — Trust layer (Desktop engine)

**TR-E1 — Authority manifest + execution receipt pair.** Surface AC-2.
Items: TR-E1.1 manifest schema (admitted authority per run: workspace
grant, lane, profile, capabilities) (M); TR-E1.2 execution-receipt emission
(what enforcement actually ran; today honestly "danger-full-access,
uncontained") (M); TR-E1.3 run-detail view rendering the pair (S). Honest
note: until TR-E2, the receipt records the absence of containment — that
candor is the product. Exit: AC-2 oracle.

**TR-E2 — Named execution profiles compiled to OS enforcement.** Surface
AC-3. Items: TR-E2.1 profile vocabulary + schema (projection-only /
workspace-bounded / networked-build / isolated-guest / owner-local danger /
managed-cloud) (M); TR-E2.2 macOS Seatbelt compilation for
workspace-bounded (L); TR-E2.3 fail-closed refusal on unrepresentable
policy (S); TR-E2.4 visually persistent danger-mode treatment (S; owner
gate on design). Depends: TR-E1. Exit: AC-3 oracle on macOS; Linux/Windows
follow REL-E2 platforms.

**TR-E3 — Hermetic profile.** Surface AC-10. Items: TR-E3.1 admitted-input
manifest emission (S); TR-E3.2 ambient-input suppression (hooks, skills,
memory, spec projection) behind the profile (M). Exit: AC-10 oracle.

**TR-E4 — Delivery lifecycle + confidence tiers.** Surface AC-6 residue.
Items: TR-E4.1 delivery states on Work Units (produced→reviewed→committed→
pushed→merged→accepted) distinct from completion (M); TR-E4.2
draft/verified/reviewed/bonded tier rendering from assurance/receipt state
(M); TR-E4.3 general worktree lifecycle manager with cleanup receipts (M).
Exit: AC-6 oracle.

### Track WB — Workbench and operator UX depth (Desktop)

Note: several items require the owner to widen the MVP visible-surface
allowlist (`mvp.visible_surface_allowlist.v1`) — a deliberate gate, listed
in §7.

**WB-E1 — Transcript engine.** AC-8: WB-E1.1 chat-transcript
virtualization + turn navigator (M); WB-E1.2 checked-in p95 frame baselines
as merge gates (S). **WB-E2 — Right-panel surface manager.** AC-5/AC-6
surfaces: WB-E2.1 tabbed right panel (review/diff/files/file/terminal/plan)
over the existing WorkbenchItem contract (L); WB-E2.2 rewind UI over the
checkpoint host (staged restore, irreversible disclosure) (M); WB-E2.3
allowlist widening + contracts (S, owner gate). **WB-E3 — Hotkey editor.**
AC-12: user-rebindable bindings with stability law (M). **WB-E4 —
Identity/status chrome.** AC-14/AC-16/AC-18 residue: per-message
requested-vs-effective metadata everywhere (S); reasoning-expansion
contract (S); running-build/branch disclosure surface (S). **WB-E5 —
Fan-out UX.** best-of-N / plan-first as typed FleetRun fan-out with
comparison records (L; overlaps HARVEST Day-2/3). Exit per epic: matching
AC oracles.

### Track MOB — Mobile controller

Owner-authority note: mobile expansion lanes are currently tombstoned
(PORT-03–08 closed not-planned). Everything below needs a fresh owner
admission; the two open issues (#8980/#8982 — consolidate into one) signal
the first slice is already wanted.

**MOB-E1 — Full Auto supervision first screen (existing #8980/#8982).**
AC-13: MOB-E1.1 run-object projection over Sync (S; server work in FA-E1.1);
MOB-E1.2 live run state first on home (S); MOB-E1.3 remote Play/Pause/Stop
as typed durable commands with receipts (M); MOB-E1.4 run report in-app
(S). Depends: FA-E1, FA-E4.1. **MOB-E2 — Attention + push.** AC-5:
token registration (S), attention inbox UI with pinned actionables (M),
revalidate-at-open deep links (S). **MOB-E3 — Outbox.** AC-3: named
per-environment outbox module + UI (visible/editable/cancellable) over the
existing queue semantics (M). **MOB-E4 — Pairing + any-host directory.**
AC-1/AC-12: wire `autopilot-control-protocol` pairing + QR flow (L);
environment directory with cached truth + classed reachability (M);
UI-first enrollment/revocation screens (M). **MOB-E5 — Workbench modes.**
AC-6/AC-10/AC-11: Changes (read + safe writeback w/ post-image receipts)
(L); Files read-only (M); capacity-as-quantities fleet view (M);
per-message effective identity (S). **MOB-E6 — Portable session client.**
AC-4: mobile client for the server-side move/authority modules (L; owner
gate — revives PORT scope). **MOB-E7 — Verification harness.** AC-8/AC-9:
fault injection + screenshot matrix vs disposable servers (M). Exit per
epic: matching AC oracles + physical-device evidence recorded separately.

### Track WEB — Web trust surface

**WEB-E1 — Promise-registry reconciliation.** AC-13: WEB-E1.1 reconcile the
state vocabulary with the spec (add or formally map RED-Elected and
WITHDRAWN; document `degraded`) (S — spec change needs `spec_revision`
bump, registry change needs its own transition pass); WEB-E1.2 agent/human
parity test (S). **WEB-E2 — Counter attestation.** AC-8: periodic published
reconciliation attestation (counter value == exact-row sum, with dogfood
split) (M). **WEB-E3 — Trust ledger.** AC-4: publish ReleaseSet manifests +
pinned keys + component ledger (ties REL-E3) (M); receipt-verification
endpoint (mechanical pass/fail for presented receipts) (M). **WEB-E4 —
Routing disclosure.** AC-9: per-response routing receipt surface (msginfo
pattern) on the Khala API + chat UI (M). **WEB-E5 — Run/trace grammar.**
AC-11: generalize the Observatory trace page into `/trace/{ref}` +
shareable QA run views (verdict/videos/accounting from receipts) (L).
**WEB-E6 — Thread objects + visibility receipts.** AC-1/AC-2: authorized
thread URLs with replay-to-live markers (L); receipted visibility
transitions with named audiences (M). **WEB-E7 — Supervision parity +
onboarding.** AC-5/AC-6: approvals/steer parity + continuation links (L);
zero-install fragment-token front door (L; depends MOB-E4 pairing
substrate). **WEB-E8 — Referral settlement + data-flow matrix.**
AC-7/AC-10: first settled referral payout behind promise gates (M, owner
gate); published data-flow matrix (M, owner copy gate). Exit per epic:
matching AC oracles; all public copy behind promise-registry gates.

### Track XC — Cross-cutting

**XC-E1 — Behavior-contract backfill** for every owner-stated law in the
rev-3 specs not yet in a registry (hotkey stability, reasoning expansion,
run-mode journey, never-halt-on-limit) (M, continuous). **XC-E2 — Surface
AssuranceSpecs**: author companion assurance specs for the three surface
specs' committed SMs (SM-4/9/10 desktop; SM-4 mobile; SM-4 web) (M).
**XC-E3 — Spec upkeep**: reconcile spec↔implementation divergences found
here at the next `spec_revision` (promise vocab; MVP allowlist vs workbench;
PORT revival) (S). **XC-E4 — FastFollow integration**: keep the HARVEST
directives cross-referenced to FA/WB epics so the two programs share
packets instead of duplicating (S, continuous). Later contracts (voice,
preference plane, capability marketplace, telemetry firewall, Verse) stay
out until their own specs are admitted.

---

## 6. Reconciliations this analysis surfaced

1. **Promise-state vocabulary drift**: registry uses
   `green/yellow/red/planned/degraded`; the web spec (per episode 234) also
   names `RED-Elected` and `WITHDRAWN`. One of them moves (WEB-E1.1).
2. **MVP visible-surface allowlist vs workbench ambition**: Files/review are
   deliberately hidden today; WB-E2 requires an owner allowlist decision.
3. **PORT tombstones vs mobile spec**: portable-session and any-host mobile
   phases are closed-not-planned on the roadmap; MOB-E4/E6 require explicit
   owner revival. The mobile spec is vision; the tombstones are current law.
4. **Full Auto single-lane profile vs multi-lane routing**: rev-9 designed
   single-lane binding deliberately; FA-E2 is a spec-level capability
   change to the full-auto ProductSpec (needs its rev 10), not a bug.
5. **GUARANTEES lag**: three "honest limits" listed in GUARANTEES (#8874,
   #8876, #8877) are fixed per the audit; refresh GUARANTEES (S).
6. **Duplicate mobile issues**: #8980 and #8982 describe the same slice —
   consolidate before dispatch.
7. **Sweep red risk from this session's own docs**: the synthesis essay
   added to `docs/teardowns/` requires a `FASTFOLLOW.md` teardown_ref (the
   coverage test enumerates the directory); fixed in the same push as this
   document.

---

## 7. Parallelization plan

### 7.1 Track independence

Independent (can run simultaneously with no shared hot files):
**FA-E3 / FA-E4**, **REL-E2**, **TR-E1..E3**, **WB-E1**, **MOB-E2/E3/E7**,
**WEB-E1..E5**, **XC-E1/E2**. Coupled pairs to serialize or co-own:
FA-E1↔FA-E2 (both rewrite the Full Auto registry schema — one owner);
FA-E1↔WB-E2 (both touch shell/renderer layout); MOB-E1↔FA-E1/E4 (consumes
run-object + report schemas — schedule after those schemas freeze, or
co-design the contract first and build in parallel against fixtures);
WEB-E3↔REL-E3 (shared ledger schema); WEB-E7↔MOB-E4 (shared pairing
substrate).

### 7.2 Collision domains (hot files — one owner each, claim before touching)

- `apps/openagents-desktop/src/full-auto-*.ts` + `specs/desktop/full-auto.product-spec.md` (FA track integrator)
- `apps/openagents-desktop/src/renderer/shell.tsx` + composer files (FA-E1 vs WB epics)
- `apps/openagents-desktop/src/contracts/ux-contracts.ts` and `packages/behavior-contracts/*` (append-only discipline; XC-E1 integrator)
- `packages/khala-sync-server/src/fleet-*.ts` + portable-session modules (MOB/FA server seam)
- `apps/openagents.com/workers/api/src/index.ts` (monolith route table — serialize WEB epics' route registrations)
- `FASTFOLLOW.md`, `docs/sol/document-manifest.json` (program ledgers — currently under active mutation by the running Full Auto lane; coordinate, never stomp)
- Release scripts + `apps/oa-updates` (REL track integrator)

### 7.3 Suggested simultaneous allocation (8–10 lanes)

Per the repo's proactive-delegation mandate, with one coordinating agent
owning integration, claims, and pushes:

- Lane 1 (FA integrator): FA-E1 then FA-E2 (registry/schema owner).
- Lane 2: FA-E3 handoff tests (worktree-isolated; touches tests + a
  bounded contract file).
- Lane 3: FA-E4 evidence loop (report schema early so Lanes 1/6 consume).
- Lane 4: TR-E1 manifests/receipts (new modules; low collision).
- Lane 5: REL — REL-E1 ceremony prep, then REL-E2 platform work.
- Lane 6 (MOB): MOB-E1 against frozen run-object fixtures, then MOB-E2/E3.
- Lane 7 (WEB): WEB-E1/E2/E4 (registry, attestation, disclosure), then E3/E5.
- Lane 8: WB-E1 virtualization + baselines (renderer-perf specialist).
- Lane 9 (floater): XC-E1 contract backfill, GUARANTEES refresh (§6.5),
  issue consolidation (§6.6), doc upkeep.
- Lane 10 (verification): assurance authoring (FA-E7.1, XC-E2) and
  adversarial review of other lanes' "done" claims — never the implementer.

Rules of engagement: every implementation lane uses a clean worktree from
`origin/main` (note: on the owner's primary machine, fresh worktrees are
currently being contaminated by a live dev process — until diagnosed,
prefer plumbing-based commits or worktrees created outside the affected
window; see the incident note in this session's push history); claims per
`docs/sol/CLAIM_PROTOCOL.md`; hot files single-owner; the coordinator
reconciles every child result against `origin/main` before closure; a
passing child test is not the integration receipt.

### 7.4 Wave sequencing

- **Wave 0 (immediate, this week):** FA-E1, FA-E2, FA-E3, FA-E4, REL-E1,
  MOB-E1 (against fixtures), XC-E1, §6 reconciliations. Rationale: this is
  the episode-256 AFK need end-to-end — dedicated mode, never-halt routing,
  proof of handoff, run reports, a shippable tag, and the phone check-in.
- **Wave 1:** FA-E5, FA-E6, FA-E7 (assurance + release admission), TR-E1,
  WB-E1, MOB-E2/E3, WEB-E1/E2/E4, REL-E2 start.
- **Wave 2:** TR-E2/E3/E4, WB-E2/E3/E4, MOB-E4/E5/E7, WEB-E3/E5/E6,
  REL-E2 finish, REL-E3.
- **Wave 3 (owner-gated expansions):** MOB-E6 portable client (PORT
  revival), WEB-E7/E8, WB-E5 fan-out UX, then the deferred contracts
  (voice, provider breadth beyond the closure bar, preference plane,
  marketplace, Verse).

Wave-0 exit test (the only one that matters short-term): the owner enables
Full Auto from the dedicated launcher, closes the laptop for 24–48 hours,
and returns to a completed run report with typed termination, provider
rotations that never waited on a human, and reviewable accepted work — with
the phone able to show, pause, and stop the run in between.

---

## 8. Verification discipline

Every epic closes only with: (1) oracles for its ACs in the normal sweep
(behavior contracts where owner-stated); (2) evidence linked from the
owning surface spec at a bumped `spec_revision` (never silent spec edits);
(3) receipts for consequential runtime claims (rotation, handoff, release,
payout) — prose and passing-child-tests are not integration receipts;
(4) honest states preserved end-to-end (unknown ≠ green; partial ≠
complete; "plumbing present, experience unverified" stays written until the
experience test exists); (5) promise-registry consistency for anything
public-facing. Committed SMs to instrument first: SM-10 (typed termination
rate) and SM-9 (false-green incidents) on Desktop; SM-4 (outbox
exactly-once) on mobile; SM-4 (reconciliation disputes) on web.

## 9. Owner gates raised by this roadmap

1. Priority arbitration: Full Auto Wave 0 versus/within the FULL HARVEST
   queue (they overlap; a merged ordering is proposed in §5 Track FA).
2. FA-E1 UX sign-off (launcher placement, read-only run view) — #8974.
3. MVP visible-surface allowlist widening for WB-E2.
4. PORT/mobile-expansion revival for MOB-E4/E6 (tombstoned lanes).
5. REL-E1 release ceremony (clean-machine install/update/rollback remains
   owner-gated) and REL-E2 platform signing identities/runners.
6. Promise-vocabulary reconciliation direction (WEB-E1.1).
7. Referral settlement arming and data-flow matrix copy (WEB-E8).
8. In-app rate-limit reset stays withheld (standing).

## 10. Change log

- 2026-07-17: initial version, authored against surface-spec rev 3 and
  main-state audits from this session.
