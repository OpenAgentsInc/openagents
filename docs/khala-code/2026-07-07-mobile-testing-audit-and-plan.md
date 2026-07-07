# Khala Code Mobile — Testing Audit and the Thorough Programmatic QA Plan

Date: 2026-07-07
Status: owner-directed audit + plan. The owner mandate: **very thorough
programmatic testing of the mobile app for current and planned features**;
a disciplined development system where generators emit test artifacts with
the code; agent-captured screenshots for visual regression; everything
testable programmatically, tested programmatically. This doc audits what
exists (grounded in three parallel explorations run 2026-07-07: the mobile
test inventory, the QA Swarm corpus, and the Blueprint/Arbiter evaluation
model), then specifies the system. It extends — does not replace —
`docs/khala-mobile/2026-07-05-qa-swarm-mobile-adaptation.md` and
`2026-07-05-mobile-qa-swarm-audit.md`, and applies the seam-testing
lessons of `docs/fable/2026-07-06-seam-testing-audit-qa-swarm-gaps.md`.
Terminology: per the owner rule recorded in
`docs/fable/2026-07-07-palantir-institutional-sovereignty-smb-analysis.md`,
the testing process is modeled in **Blueprint** vocabulary (Eval Suites,
oracles, Release Gates, receipts) — our own terms throughout. The QAM
lanes are sequenced as **Phase P0** of
`docs/fable/MASTER_ROADMAP.md` (fully-tested MVP first, then Codex).

## 0. The verdict in one paragraph

Mobile testing today is a good *logic* net with almost no *surface* net:
68 `bun test` files cover cores, policies, and sync (against an in-process
fake), ~29 enforced behavior contracts guard stated expectations, and
three Maestro flows exist — but only 7 components are actually mounted
(thread-list, thread-messages, and credits-history screens have **no
mount test**), there is **zero visual regression**, the one seam-crossing
Maestro flow has effectively never run unattended, Android has no
automation at all, and the biggest planned features (IAP, agent-computer
streaming into the thread UI) have no test story. Meanwhile the desktop
QA Swarm machine — scenario DSL with mandatory oracles, seeded monkeys
with explore→distill→regress, coverage ledger + frontier steering, a
hand-rolled pixel-exact visual-baseline engine, an owned-runner nightly
matrix that auto-files strict issues — is running green every night *and
none of it points at mobile*. The plan is therefore not to invent a
testing system; it is to **extend the proven machine to mobile, model the
process as Blueprint Eval Suites and Release Gates, and make the
generators emit the test artifacts so coverage is a property of how code
is created, not a chore after.**

## 1. Audit — what exists today

### 1.1 Mobile (clients/khala-mobile) inventory

| Layer | State | Key refs |
|---|---|---|
| Runner | `bun test` (bun:test), preloaded RN environment; no Jest, no @testing-library/react-native | `bunfig.toml`, `tests/support/rn-test-environment.ts` |
| Component mounts | **7 real mounts** via react-test-renderer (full mount, no network/native): chat-composer, repo-picker, thread-header, ui-primitives, sign-out button, onboarding CTA, crash-reporting | `tests/*.test.tsx` |
| Logic cores | Broad: auth (8 files), sync runtime (9, incl. SQLite persistence, fake transport server), runtime compose/transcript, credits/model-pref/repos/push/onboarding/voice/OTA cores | `tests/*-core.test.ts` etc. |
| Architecture guards | Strong: dependency-cruiser rules asserted, native-modules-through-adapter, navigation hardening, i18n copy, theme colors, asset policy, plus **policy tests that forbid `.github/workflows` and `eas`** | `tests/architecture-guardrails.test.ts`, `maestro-policy.test.ts`, `storybook-setup.test.ts` |
| Behavior contracts | Registry v`2026-07-07.3`: ~29 enforced / ~11 pending; oracle sweep in normal tests | `src/contracts/ux-contracts.ts`, `tests/ux-contracts.test.ts`, `docs/khala-mobile/khala-mobile-ux-contract.md` |
| Maestro E2E | 3 flows (LaunchFallback, LaunchGitHubSignInInteraction, SignedInThreadSmoke) + clean-launch hook; run scripts for iOS sim with secret-gated seeded creds; **manual, macOS-local, iOS-only**; the seeded smoke is the pending half of `khala_mobile.platform.launched_app_interaction_smoke.v1` | `.maestro/`, `scripts/emulator-test-run.sh`, `scripts/signed-in-thread-smoke-run.sh` |
| Storybook | On-device (@storybook/react-native), 4 primitive stories, **no snapshot/screenshot testing** | `.rnstorybook/`, `src/components/*.stories.tsx` |
| Visual regression | **None** for mobile | — |
| Codegen | None for app code; Ignite-style EJS scaffolds exist (screen/component/navigator/**ux-contract-oracle** templates) but are dev conveniences, not an enforced path | `templates/` |
| CI | **No hosted CI by policy** (deliberate: owned runners only). The enforced gate is the monorepo `test` chain; typecheck + depcruise are convention-run | root `package.json` |
| Android | Green Gradle assemble; **zero emulator automation, zero boot-proof scripts** | — |

### 1.2 The desktop QA machine (what we get to reuse)

The QA Swarm engine (`docs/fable/ROADMAP_QA.md` #8051 closed;
productization #8071) is live nightly for desktop:

- **Scenario DSL with mandatory oracles** — typed Effect Schema documents;
  a phase without an oracle is rejected at load. Oracle catalog: schema,
  consistency (cross-mode agreement), invariant, public_safe, **visual**,
  perf (named budgets), a11y, event ordering, crash.
- **Four driver modes** over one scenario (programmatic RPC / DOM / vision
  / headless JSONL) — mode disagreement is itself a bug.
- **Seeded monkey** (deterministic PRNG walker, seed+log replay) and LLM
  explorer, both steered by a **coverage ledger + frontier** (a coverage
  class at zero for a week auto-files an issue); **explore → distill →
  regress** (`khala_code_qa_distilled_scenario.v1` → committed
  `*.e2e.test.ts`).
- **Visual baselines** — a dependency-free pixel-exact PNG diff engine
  (`packages/khala-qa-harness/src/visual-baseline.ts`, schema
  `openagents.khala_visual_baselines.v1`): own PNG decoder, magenta delta
  images, baselines keyed by `{id, harness, viewport, colorScheme,
  reducedMotion}`, sha256 fingerprints, **public-safety tripwires enforced
  on every write**. No third-party visual SaaS, by policy.
- **Nightly matrix on an owned runner** (`scripts/qa-nightly-matrix.ts`,
  systemd timer): harness tests, contracts, real-bridge smoke, visual
  smokes, monkey night (1024 actions), model-based tier → writes
  `qa-nightly-report`, `qa-status-surface`, contract receipts, coverage
  union/frontier/steering, flake-quarantine ledger, and **auto-files
  strict-form issues** on failure/regression/quarantine/zero-coverage.
- **Verdict discipline**: CONFIRMED / REFUTED / INCONCLUSIVE; CONFIRMED
  requires observed evidence from *this* run; exact-only accounting;
  receipts everywhere; the swarm-board projection
  (`@openagentsinc/arbiter-effect`) lights an edge **only** when a real
  receipt dereferences.

### 1.3 The seam lesson (why unit green ≠ working app)

Four TestFlight builds shipped an infinite "Loading threads" spinner
because `/api/sync/connect` never read the `?token=` query bearer — both
sides' tests were green against their own doubles; *every layer stopped
exactly at the seam*. The audit's R1–R7 remedies are the checklist; R6
(the `khala-sync-transport` qa-runner backend driving the **real**
transport headless against a live deployment, classifying
`live / connect_unauthenticated / connect_denied / silent_retry_loop /
never_live`) has landed. R4 (the seeded signed-in Maestro smoke) remains
blocked on a seeded public-safe GitHub test account — owner-gated.

### 1.4 The honest gap list (audit output)

1. **No hosted or scheduled gate for mobile at all** — everything beyond
   `bun test` is manual and macOS-local; mobile is not a row in the
   nightly matrix.
2. **Screens under-mounted** — thread-list, thread-messages,
   credits-history have no mount tests; settings is source-string only.
3. **Zero visual regression**, mobile — despite the engine existing in
   the harness package.
4. **Maestro seam flow never runs unattended**; no Android automation;
   no device monkey/explorer; no on-device perf budgets (cold launch,
   thread switch, OTA check).
5. **No mobile-owned seam contract** — sync wire-compat rides the shared
   packages' conformance suites; `transport`-class files can be imported
   by zero tests without any guard noticing.
6. **Planned features without a test story**: IAP (zero code), live
   agent-computer streaming into the thread UI, credits balance UX, model
   picker UI, delivered-push→deep-link navigation; every post-MVP lane
   (minerals IAP, Codex connect CX-2, Agents panel AE-2) currently
   inherits this default.
7. **Generators don't enforce anything** — templates exist, but a screen
   can be created with no mount test, no story, no contract, no flow, and
   nothing fails.

## 2. The design decision: model the testing process as Blueprint

Blueprint already contains a complete, typed model of evaluation and
release — built for governing agent behavior, and exactly right for
governing an app's quality process
(`autopilot4-deprecated/blueprint/docs/security-evals-and-release-gates.md`,
`programs-optimization-and-rlm.md`):

- **Eval Suite / Eval Case**: versioned typed records; each case is an
  `input fixture + expected_* outcome + severity` — **the expected-***
  **fixture IS the oracle**. Severities `blocking | warning |
  informational | regression_only`; blocking failures prevent release.
- **Release Gate**: an ordered validation checkpoint — every gate records
  checks, evidence refs, a **rollback posture**, a drafted release
  receipt, and captured operator approval.
- **Autonomy promotion ladder**: capabilities earn wider authority only
  by passing gates (`read_only → recommend → propose → execute_low_risk →
  execute_bounded`), each hop receipted.
- **Trust/Failure Receipts**: the plain-language closeout of what was
  proved and *what could not be proved*.

Mapping to mobile QA (concept-for-concept, reusing shipped schemas rather
than minting parallel ones — the scenario DSL, behavior contracts, visual
baselines, and QA run reports are the existing embodiments):

| Blueprint concept | Mobile QA embodiment |
|---|---|
| Eval Suite | A named scenario/test bundle per feature (the "feature ladder" of §4) — scenario DSL + contract oracles + visual baselines + Maestro flows for one feature |
| Eval Case / expected-* fixture | Scenario phase oracles; contract oracle tests; visual baseline images; Maestro assertions; seam-outcome classifications |
| Severity | `blocking` = ship gate; `warning` = nightly issue-filed; `regression_only` = distilled monkeys/explorer finds |
| Release Gate | The **mobile release gate** (§3): the ordered check sequence a build must pass before TestFlight/OTA, each check emitting a receipt |
| Rollback posture | Named per gate: OTA channel rollback (`apps/oa-updates` republish), store-build hold, contract retirement with owner sign-off |
| Autonomy promotion | The **feature test ladder** (§4): a feature climbs rungs — logic core → mount → contract → device flow → visual → seam — and marketing/promise copy for it is capped by its rung |
| Trust/Failure Receipt | `qa-nightly-report` + behavior-contract receipts + the QA Swarm run projection — including the *could-not-prove* list (pending contracts with named blockers) |
| Simulation Branch | The fixture tier (fake transport, fixture intents, StoreKitTest sandboxes) — isolated state where destructive/expensive cases run first |

**Arbiter and "Unit."** The visualization/control layer for this process
is `@openagentsinc/arbiter-effect` — our evidence-bound dataflow graph
(typed-pin nodes, links that light **only** when a receipt dereferences),
already rendering the desktop swarm board. "Unit" is the external
reference repo whose primitives Arbiter deliberately reimplemented under
our own name (the audit records that the name "Unit" was rejected for
collision and brand reasons — same discipline as the ontology/Blueprint
terminology rule). The mobile QA pipeline becomes nodes on the same swarm
board: suites → oracles → verdicts, edges lit by run receipts. No new
visualization work is required beyond adding mobile nodes to the existing
QA Swarm projection (`openagents.qa_swarm.run_projection.v1`).

## 3. The mobile release gate (the ordered checks)

One typed gate, run as a single command (`qa:mobile:gate`), every check
emitting a receipt; blocking severity fails the gate. Order is
cheapest-first:

1. **Static**: `tsc --noEmit` + dependency-cruiser architecture check —
   *promoted from convention to gate* (today they run by hand).
2. **Unit + logic cores**: the existing `bun test` sweep.
3. **Component mounts**: the §4 mount tier — every screen in
   `src/screens/` must have a mount test or a typed waiver naming the
   blocking mock (the settings-screen expo-notifications problem becomes
   an explicit waiver with an issue ref, not silence).
4. **Behavior-contract sweep**: the existing oracle run; pending
   contracts listed in the receipt as could-not-prove.
5. **Generator conformance** (§5): every screen/component has its
   generated test bundle; a missing artifact fails the gate.
6. **Sync/runtime fixture tier**: fake-transport suite + fixture intent
   streams (agent-computer streaming cases, §7).
7. **Seam smokes** (staging-gated): the R6 `khala-sync-transport` backend
   classification must be `live`; the mobile-session auth probe
   (bearer, no cookies) must reach `live`.
8. **Device tier** (nightly + pre-release, not per-push): Maestro flows
   on iOS sim (and Android emulator once QAM-6 lands), monkey run,
   screenshot capture for the visual tier.
9. **Visual tier**: captured screenshots vs blessed baselines
   (`openagents.khala_visual_baselines.v1`); `changed` without a blessing
   receipt is blocking.
10. **Perf budgets** (device tier): named budgets
    (`budget.khala_mobile.cold_launch.v1`, `thread_switch.v1`,
    `sync_bootstrap_to_live.v1`, `ota_check_overhead.v1`) with p95
    assertions — the desktop `qa_metrics` pattern applied to mobile.

Rollback posture recorded per release: OTA channel republish for JS
regressions; store-build hold for native; contract/waiver changes need
owner sign-off (existing behavior-contract law).

**Where it runs.** Hosted CI stays out by standing policy (the policy
tests enforcing no `.github/workflows`/EAS are respected, not fought).
The gate runs: (a) locally pre-push (checks 1–6 are fast), and (b) as a
**mobile row in the owned-runner nightly matrix** — checks 7–10 need a
Mac for the iOS simulator, so the nightly mobile tier runs on an owned
Tailnet Mac (launchd timer, same report/issue-filing discipline as
`qa-nightly-matrix.ts`; the Linux runner keeps the non-device rows).
Android device rows join when QAM-6 lands.

## 4. The feature test ladder (Blueprint promotion applied to features)

Every feature — current and planned — is assigned a ladder position, and
**the ladder caps what we may claim about it** (promise-registry
discipline at feature granularity):

| Rung | Proof | Exists today for |
|---|---|---|
| L0 logic core | Pure-function/core tests | Most features |
| L1 mount | Real component mount incl. loading/error/empty states | 7 components only |
| L2 contract | Enforced behavior-contract oracle | ~29 contracts |
| L3 fixture flow | Scenario over fake transport/fixture backends (streaming, degradation, interrupt cases) | Sync runtime partially |
| L4 device flow | Maestro on sim/emulator, unattended | Launch + sign-in-tap only |
| L5 visual | Blessed baselines per {screen/story × device × colorScheme} | Nothing |
| L6 seam | Real-transport probe against staging/prod classification `live` | R6 backend (not yet mobile-scheduled) |

Current-feature ladder debt (from §1.4): thread-list, thread-messages,
credits-history, settings → L1; model picker + credits UX → L1/L2; push
deep-link → L4 (delivered notification → navigation); everything → L5.

**Planned features get their Eval Suite before their code** (the
fixture-first rule). The named suites to author now:

- **IAP/minerals** (#8481/#8482 reopen): server rail replay fixtures
  (receipt validation, clawback, restore), StoreKitTest-based purchase
  flows on the device tier, Apple 3.1.1 copy assertions as contract
  oracles, ledger-fulfillment idempotency cases. Zero code exists — the
  suite defines the acceptance before RevenueCat lands.
- **Agent-computer streaming** (#8503/#8477 UX): fixture
  `khala_runtime_control_intent`/`runtime_event` streams rendered into a
  mounted thread-messages screen (ordering, interruption, typed-refusal
  rendering: `insufficient_credit`, `rate_limited`,
  `org_capacity_unavailable`; writeback link card). This is L1+L3 work
  that needs no live cloud.
- **Codex connect (CX-2)**: device-auth state machine cores, account list
  mounts with readiness/quota states, typed-failure rendering
  (`account_exhausted`, `account_rate_limited`).
- **Agents panel (AE-2)**: the pending
  `agents_panel.run_status_indicators_truthful.v1` contract is already
  the oracle spec — the panel cannot ship below L2 by existing law.
- **Push end-to-end**: sim-delivered notification (xcrun simctl push) →
  deep-link navigation assertion at L4.

## 5. Generators: coverage as a property of creation

The owner directive "lean more on generators that include test stuff" has
a head start: `clients/khala-mobile/templates/` already holds Ignite-style
EJS scaffolds *including a ux-contract-oracle template*. Upgrade them from
convenience to **the enforced path**:

- `generate screen <Name>` emits: the screen; a **mount test** (loading /
  empty / error / populated states against typed fixtures); a **story**
  per meaningful state; a **contract stub** (pending, with the statement
  slot empty — forcing the expectation to be written down); a **Maestro
  flow stub** tagged to the screen; and a **visual-baseline
  registration** (ids keyed for the capture matrix).
- `generate component <Name>` emits component + mount test + story.
- `generate api-core <Name>` emits the fetch core + fixture-server test
  (the `khala-mobile-*-api` pattern that already works well).
- **Conformance is a policy test** (gate check 5): a screen present in
  `src/screens/` without its bundle members fails — the same
  source-assertion technique the architecture guards already use, so this
  is cheap to enforce.
- Fixtures are typed and shared: one `tests/fixtures/` module per domain
  (threads, credits, repos, runtime events) so mounts, stories, scenario
  phases, and Maestro seeds all draw the same states — a story and its
  visual baseline render exactly what the mount test asserted.

This is also the agent-development contract: fleet workers building
mobile features run the generator first, so their diffs arrive with the
test skeleton filled in, and the gate refuses the lazy path.

## 6. Visual regression with agent-captured screenshots

Reuse the shipped engine; build only capture:

- **Tier V1 — story screenshots (the workhorse).** A capture script
  drives the on-device Storybook build on the iOS simulator, walks the
  story list (Maestro or deep-link story selection), snaps
  `xcrun simctl io screenshot` per story, and feeds
  `visual-baseline.ts` keyed `{storyId, device, colorScheme}`. Because
  stories render typed fixtures, diffs are deterministic. Android later
  via `adb exec-out screencap`. The generator's story requirement (§5) is
  what makes this tier's coverage grow automatically.
- **Tier V2 — screen checkpoints.** Named `takeScreenshot` checkpoints in
  Maestro flows (post-launch, thread list, opened thread, composer open,
  settings, credits) compared against blessed baselines with the same
  engine. Fewer, chunkier, catches integration-level regressions stories
  can't.
- **Blessing is a receipted act**: `changed` deltas (magenta diff
  artifacts) land in the nightly report; an agent or owner blesses with a
  recorded reason; unexplained `changed` is blocking. Baselines are
  public-safe by construction (the engine's tripwires already refuse
  `/Users/`, bearer material, raw prompts — seeded test data must be
  public-safe fixture content).
- **No third-party visual SaaS** (standing policy) — the hand-rolled
  engine plus owned-runner storage is the whole stack. Theme note: one
  Protoss-blue theme means one colorScheme axis value in practice; keep
  the key dimension anyway for the OLED/reduced-motion matrix.

## 7. Monkeys, explorers, and the seam — extending the swarm to mobile

- **M1 mount monkey (CI-speed, now):** a seeded PRNG walker over mounted
  screens' action space (press/scroll/type from the fuzz corpus) using
  the existing rn-test-environment — the `monkey-explorer.ts` pattern
  with a mobile action space; crash/console/invariant oracles; seed+log
  replay; distill survivors into mount regression tests.
- **M2 device monkey (nightly):** Maestro-driven random-walk over the
  view hierarchy on the sim, seeded, with screenshot-on-crash and the
  memory/zombie oracle pattern; coverage ledger counts screens visited,
  routes navigated, contract surfaces touched, deep links fired, sync
  mutator kinds exercised — frontier steering + zero-for-a-week issue
  filing exactly as desktop.
- **M3 LLM explorer (later):** the qa-runner vision brain driving
  simulator screenshots + hierarchy dumps, goal-directed ("sign in, bind
  a repo, dispatch a turn, find anything dishonest"), discoveries
  distilled or they didn't happen.
- **Seam tier (R1–R7 applied to mobile, the non-negotiables):**
  scheduled R6 `khala-sync-transport` probes against staging + prod as
  the mobile nightly's first row; a **zero-test-imports guard** for
  `transport`-class files (R2); a two-sided **seam contract** kind
  binding "cookie-less bearer client reaches `live`" (R5) into the mobile
  registry; the silent-retry tripwire (R7) asserted by a contract; and
  the seeded-account unblock for `SignedInThreadSmoke` (R4) — owner-gated,
  goes to NEEDS_OWNER with the launch items.

## 8. Lanes (QAM-*) with exit receipts

Dependency spine: QAM-1 → QAM-2/3/4 parallel → QAM-5 → QAM-6/7.

- **QAM-1 The gate.** `qa:mobile:gate` running checks 1–6 locally +
  pre-push; typecheck/depcruise promoted to blocking; generator
  conformance policy test. *Exit: gate refuses a screen added without its
  bundle; receipt in repo.*
- **QAM-2 Mount debt + fixture suites.** Thread-list, thread-messages,
  credits-history, settings (with mock waivers resolved or typed);
  agent-computer streaming fixture suite; shared typed fixtures module.
  *Exit: every screen at L1+, streaming suite green against fixture
  intents.*
- **QAM-3 Generators.** Template bundles upgraded per §5.
  *Exit: `generate screen` output passes the gate with zero manual test
  authoring; one real feature shipped through it.*
- **QAM-4 Visual tier.** Story capture harness + baseline store + V2
  checkpoints; blessing workflow in the nightly report.
  *Exit: first full blessed baseline set; one intentional UI change
  caught as `changed` and blessed with a reason; one unintentional
  regression caught (seeded test acceptable).*
- **QAM-5 Nightly mobile row.** Owned-Mac launchd tier running checks
  7–10 (Maestro flows, M2 monkey, visual capture, perf budgets, seam
  probes), reporting into the same `qa-nightly-report`/status-surface/
  strict-issue discipline; mobile nodes on the QA Swarm board.
  *Exit: 7 consecutive nightly receipts; one auto-filed strict issue
  proving the failure path.*
- **QAM-6 Android lane.** Emulator boot + Maestro flows + screencap
  capture parity; the Android boot-proof gap closed.
  *Exit: the launch flows green on emulator in the nightly row.*
- **QAM-7 Planned-feature suites.** The §4 fixture-first Eval Suites for
  IAP/minerals, push E2E, CX-2, AE-2 — authored and red/waived *before*
  their implementation lanes start.
  *Exit: each suite exists with named blocking cases; the first
  implementation PR turns cases green rather than writing them.*

## 9. What we deliberately do not do

- **No GitHub-hosted CI, no EAS** — the standing policy tests stay; the
  system is owned runners + pre-push gates.
- **No third-party visual/testing SaaS** (Percy, Applitools, device
  farms) — the owned engine + Tailnet Macs + emulators.
- **No parallel schema inventions** — scenario DSL, behavior contracts,
  visual baselines, QA run reports, coverage ledgers are the existing
  typed records; mobile adds instances, not new record kinds (a new kind
  requires the same INVARIANTS discipline as any contract change).
- **No green theater**: pending contracts stay pending with named
  blockers; device-only truths (STT capture, Apple FM, real push) are
  never simulated into fake receipts; INCONCLUSIVE is a real verdict.

## 10. Open questions

1. **Which owned Mac hosts the nightly device tier** — and its
   launchd/Tailnet wiring vs extending the existing runner host. Decide
   at QAM-5 start; both Tailnet Macs are candidates.
2. **StoreKitTest scope** — how much IAP flow is honestly testable in the
   simulator sandbox vs deferred to TestFlight receipts; the suite should
   mark the boundary explicitly.
3. **Baseline churn budget** — story-level visual baselines on a fast-
   moving UI can drown the blessing queue; start V1 with primitives +
   stable screens, expand with the generator's growth.
4. **Seeded test account** (R4) — remains the single owner action gating
   the unattended seam smoke; it also unblocks the straight-line E2E
   contract. Re-flag in NEEDS_OWNER.
5. **Should the feature ladder positions be rendered in Aiur** (ops view
   of ladder debt per feature) — cheap once receipts exist; decide after
   QAM-5.
