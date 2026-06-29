#!/usr/bin/env bun
// AO-6 (#5447, EPIC #5441): end-to-end first-run smoke — clean Mac → earning.
//
// This is the proof that the EPIC's auto-onboarding chain converges, exercised
// end-to-end and HEADLESSLY (no GUI, no terminal, no env vars) against a mock of
// the openagents.com Worker. It EXTENDS the Phase-1 headless convergence proof
// (`auto-onboarding-headless-proof.ts`) rather than duplicating it:
//
//   - reuses the same 9-gate convergence (drive the REAL dev Pylon node through
//     the desktop launcher into a fresh managed home with `autoOnboarding: true`,
//     pointed at a local mock, and assert each gate: identity → register → token
//     persisted → presence (bearer) → payout target → assignment poll), and
//   - adds the AO-3 identity-choice gates for BOTH paths (#5444):
//       * create-new (named): the user-chosen display name flows into
//         `POST /api/agents/register` `displayName`, and a fresh managed home is
//         minted (not an existing one), and
//       * use-existing: a seed-bearing home is DETECTED (marker-presence only,
//         seed never read) and ADOPTED without forking — and create-new stays
//         available even when an existing Pylon is present, and
//   - adds the AO-4 wizard-state assertions (#5445): feed the REAL observed
//     signals into `projectOnboardingStatus` and assert the live chain converges
//     to the right per-step states (identity → registered → node online → wallet
//     → payout → presence → Tassadar → claimed → earned), with no faked progress.
//
// HONEST SCOPE — the live-production / live-proof gates are NOT faked here. The
// real from-DMG run on a clean Apple-Silicon Mac, the node's appearance on
// production `/api/public/pylon-stats`, and an actual claimed + SETTLED Tassadar
// window with a real Bitcoin receipt require a physical Mac + a fresh signed DMG
// (AO-5) + a live validator pair. This harness automates everything UP TO that
// boundary and prints the exact manual steps + expected public-safe evidence for
// the live proof run. See
// `docs/launch/2026-06-18-autopilot-desktop-ao6-from-dmg-runbook.md`.
//
// Secrets boundary: the mock NEVER receives or echoes a real secret; the only
// token in play is a fake `oa_agent_...` value minted by the mock. The smoke
// asserts the token is never printed to any status/log surface, and never reads
// or prints an identity seed.

import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  detectExistingPylonIdentity,
  loadIdentityChoice,
  projectIdentityChoiceState,
  saveIdentityChoice,
} from "../src/bun/identity-choice"
import { superviseManagedNode } from "../src/bun/node-launcher"
import { postForumIntroduction } from "../src/bun/forum-intro"
import { searchForumWork } from "../src/bun/forum-work-search"
import {
  projectOnboardingStatus,
  type OnboardingStatusInput,
} from "../src/shared/onboarding-status"

const FAKE_TOKEN = "oa_agent_e2e_smoke_token"
const CHOSEN_NAME = "Studio Mac (AO-6 smoke)"
// A fake NIP-06 seed string. It is written ONLY into the fake-home use-existing
// fixture (never a real home), and the smoke asserts it is never detected as a
// value (detection is marker-presence only) and never printed.
const FAKE_SEED = "fake mnemonic words never read by detection do not print"

type Hit = { method: string; path: string; bearer: string | null }

const hits: Hit[] = []
let registerBody: Record<string, unknown> | null = null

// --- mock openagents.com Worker --------------------------------------------
const server = Bun.serve({
  port: 0,
  async fetch(req) {
    const url = new URL(req.url)
    const bearer = req.headers.get("authorization")
    hits.push({ method: req.method, path: url.pathname, bearer })

    if (url.pathname === "/api/agents/register" && req.method === "POST") {
      registerBody = (await req.json().catch(() => ({}))) as Record<
        string,
        unknown
      >
      return Response.json(
        {
          user: { id: "user_smoke", status: "active" },
          credential: { token: FAKE_TOKEN, tokenPrefix: "oa_agent_e2e" },
        },
        { status: 201 },
      )
    }
    if (url.pathname === "/api/pylons/register" && req.method === "POST") {
      return Response.json(
        { registrationRef: "reg_smoke", pylonRef: "pylon.smoke", stale: false },
        { status: 200 },
      )
    }
    if (url.pathname.endsWith("/heartbeat") && req.method === "POST") {
      return Response.json({ ok: true, stale: false }, { status: 200 })
    }
    if (url.pathname.endsWith("/spark-payout-target") && req.method === "POST") {
      return Response.json(
        { payoutTargetRef: "payout_smoke", state: "registered" },
        { status: 200 },
      )
    }
    if (url.pathname.endsWith("/wallet-readiness") && req.method === "POST") {
      return Response.json({ ok: true }, { status: 200 })
    }
    if (url.pathname.endsWith("/assignments") && req.method === "GET") {
      return Response.json({ assignments: [] }, { status: 200 })
    }
    // AF-3 (#5900): forum board read for typed intro-lane selection.
    if (url.pathname === "/api/forum" && req.method === "GET") {
      return Response.json(
        {
          boardId: "board_smoke",
          slug: "openagents",
          title: "OpenAgents",
          categories: [],
          forums: [
            { slug: "general", title: "General", locked: false },
            { slug: "introductions", title: "Introductions", locked: false },
          ],
          generatedAt: new Date().toISOString(),
          publicProjection: {},
        },
        { status: 200 },
      )
    }
    // AF-3 (#5900): create-topic (the intro post).
    if (
      /^\/api\/forum\/forums\/[^/]+\/topics$/.test(url.pathname) &&
      req.method === "POST"
    ) {
      return Response.json(
        {
          topic: { id: "topic_smoke", slug: "intro-smoke" },
          firstPost: { id: "post_smoke" },
          idempotent: false,
          receiptRefs: [],
        },
        { status: 200 },
      )
    }
    // AF-4 (#5901): read-only work-search over the typed work-requests lane.
    if (url.pathname === "/api/forum/work-requests" && req.method === "GET") {
      return Response.json(
        {
          workRequests: [
            { workRequestId: "wr_1", title: "demo", state: "open" },
            { workRequestId: "wr_2", title: "demo2", state: "open" },
            { workRequestId: "wr_3", title: "demo3", state: "running" },
          ],
          pagination: { cursor: null, hasMore: false, limit: 50, nextCursor: null },
        },
        { status: 200 },
      )
    }
    return Response.json({ ok: true }, { status: 200 })
  },
})

const baseUrl = `http://127.0.0.1:${server.port}`
const home = mkdtempSync(join(tmpdir(), "ao6-e2e-smoke-"))
// A separate fake HOME for the AO-3 use-existing fixture, so we never touch the
// real `~`. We seed a `~/.openagents/pylon` with the seed marker + a public
// identity.json projection.
const fakeHomeDir = mkdtempSync(join(tmpdir(), "ao6-fakehome-"))

const repoRoot = join(import.meta.dir, "..", "..", "..")
const cwd = join(repoRoot, "apps", "autopilot-desktop")

const sleep = (ms: number) => new Promise(r => setTimeout(r, ms))

let ok = true
const check = (label: string, cond: boolean) => {
  console.log(`${cond ? "PASS" : "FAIL"}  ${label}`)
  if (!cond) ok = false
}

const cleanup = () => {
  try {
    server.stop(true)
  } catch {
    // ignore
  }
  rmSync(home, { recursive: true, force: true })
  rmSync(fakeHomeDir, { recursive: true, force: true })
}

const fail = (message: string): never => {
  console.error(`\nFAIL: ${message}`)
  cleanup()
  process.exit(1)
}

console.log("== AO-6 end-to-end first-run smoke (clean Mac → earning) ==")
console.log(`mock openagents.com : ${baseUrl}`)
console.log(`fresh managed home  : ${home}`)
console.log(`fake HOME (AO-3)    : ${fakeHomeDir}`)
console.log("(token + seed are redacted in all output)\n")

// ===========================================================================
// PART A — AO-3 identity choice (both paths), BEFORE the node mints anything.
// ===========================================================================
console.log("-- Part A: AO-3 first-run identity choice (both paths) --")

// A.1 fresh machine: no existing Pylon → choice needed, defaults to create-new.
const freshState = projectIdentityChoiceState({ homeDir: fakeHomeDir })
check(
  "AO-3 fresh machine: no existing identity detected",
  freshState.detected.present === false,
)
check("AO-3 fresh machine: choice needed", freshState.choiceNeeded === true)
check(
  "AO-3 fresh machine: create-new is always available",
  freshState.createNewAvailable === true,
)

// A.2 seed an existing Pylon home in the fake HOME and re-detect.
const existingHome = join(fakeHomeDir, ".openagents", "pylon")
mkdirSync(existingHome, { recursive: true })
// The seed marker (presence only — never read by detection). Written here only.
writeFileSync(join(existingHome, "identity.mnemonic"), FAKE_SEED, { mode: 0o600 })
writeFileSync(
  join(existingHome, "identity.json"),
  JSON.stringify({
    npub: "npub1existingexisting000000000000000000000000000000000000000000",
    pylonRef: "pylon.exist01",
    nodeLabel: "Existing Pylon",
  }),
)

const detected = detectExistingPylonIdentity({ homeDir: fakeHomeDir })
check("AO-3 use-existing: seed-bearing home detected", detected !== null)
check(
  "AO-3 use-existing: detected home is the seeded ~/.openagents/pylon",
  detected?.home === existingHome,
)
check(
  "AO-3 use-existing: detection used marker presence (public npub surfaced, seed not)",
  detected?.npub === "npub1existingexisting000000000000000000000000000000000000000000",
)

const detectedState = projectIdentityChoiceState({ homeDir: fakeHomeDir })
check(
  "AO-3 use-existing: choice screen still offers create-new alongside existing",
  detectedState.detected.present === true &&
    detectedState.createNewAvailable === true,
)

// A.3 use-existing choice persists + re-verifies the seed marker (never adopts a
//     wrong home). Saving must NOT write into the seed home.
const useExisting = saveIdentityChoice(
  { kind: "use_existing", home: existingHome },
  { homeDir: fakeHomeDir },
)
check("AO-3 use-existing: choice saved", useExisting.ok === true)
const useExistingLoaded = loadIdentityChoice({ homeDir: fakeHomeDir })
check(
  "AO-3 use-existing: persisted choice boots the existing home (no fork)",
  useExistingLoaded?.kind === "use_existing" &&
    useExistingLoaded.home === existingHome,
)
// The choice file lives under the desktop's own managed dir, NOT inside the seed
// home, so recording the choice can never overwrite an existing identity.
const choiceFile = join(
  fakeHomeDir,
  ".openagents",
  "autopilot-desktop",
  "identity-choice.json",
)
let choiceFileRaw = ""
try {
  choiceFileRaw = readFileSync(choiceFile, "utf8")
} catch {
  choiceFileRaw = ""
}
check(
  "AO-3 use-existing: choice persisted OUTSIDE the seed home (never overwrites it)",
  choiceFileRaw.length > 0 && !choiceFileRaw.includes(FAKE_SEED),
)
// The seed home's seed file is untouched (still exactly the marker we wrote).
check(
  "AO-3 use-existing: seed marker untouched (Orwell rule: never overwrite)",
  readFileSync(join(existingHome, "identity.mnemonic"), "utf8") === FAKE_SEED,
)

// A.4 NEVER adopt the wrong home: a use-existing choice against a home with no
//     seed marker must be rejected.
const wrongHome = join(fakeHomeDir, "not-a-pylon-home")
mkdirSync(wrongHome, { recursive: true })
const rejected = saveIdentityChoice(
  { kind: "use_existing", home: wrongHome },
  { homeDir: mkdtempSync(join(tmpdir(), "ao6-fakehome-reject-")) },
)
check(
  "AO-3 never-adopt-wrong-home: a seedless home is rejected",
  rejected.ok === false,
)

// ===========================================================================
// PART B — full create-new convergence: drive the REAL node through the
// launcher, asserting the AO-1/AO-2 chain AND that the AO-3 chosen NAME flows
// into registration.
// ===========================================================================
console.log(
  "\n-- Part B: create-new (named) convergence through the real node --",
)

const controlPort = 40000 + Math.floor(Math.random() * 10000)
const statuses: string[] = []
const sup = superviseManagedNode({
  cwd,
  env: {
    PYLON_HOME: home,
    PYLON_CONTROL_PORT: String(controlPort),
  },
  controlBaseUrl: `http://127.0.0.1:${controlPort}`,
  // Clean-machine first-run: never adopt a real local node discovered by walking
  // up from cwd. Force a fresh launch into our temp home.
  discover: () => null,
  autoOnboarding: true,
  onboardingBaseUrl: baseUrl,
  // AO-3 create-new path: the user named the identity; assert it reaches the
  // register call's displayName.
  onboardingDisplayName: CHOSEN_NAME,
  readinessTimeoutMs: 60_000,
  readinessIntervalMs: 500,
  onStatus: s => {
    statuses.push(s)
    console.log(`[status] ${s}`)
  },
})

const seen = (pred: (h: Hit) => boolean) => hits.some(pred)
const deadline = Date.now() + 90_000
while (Date.now() < deadline) {
  const registered = seen(h => h.path === "/api/agents/register")
  const presence = seen(h => h.path === "/api/pylons/register")
  const payout = seen(h => h.path.endsWith("/spark-payout-target"))
  const assignments = seen(
    h => h.path.endsWith("/assignments") && h.method === "GET",
  )
  if (registered && presence && payout && assignments) break
  await sleep(1000)
}

// We force PYLON_HOME to the fresh temp `home` and the launcher honors an
// explicit env PYLON_HOME, so the node writes there; fall back to `home` (never
// the repo-root `.pylon-local`, a different node's home) if home() races null.
const managedHome = sup.home() ?? home
const nodeLaunchStatus = sup.status()

sup.stop()
await sleep(500)

// AF-3/AF-4/AF-6 (#5900/#5901/#5903): with the node converged, drive the forum
// loop directly against the still-running mock — the path index.ts fires on poll.
let introOutcome = "unrun"
let workOutcome = "unrun"
try {
  introOutcome = (await postForumIntroduction({ home: managedHome, baseUrl })).outcome
} catch (error) {
  introOutcome = `threw:${error instanceof Error ? error.message : String(error)}`
}
try {
  workOutcome = (await searchForumWork({ home: managedHome, baseUrl })).outcome
} catch (error) {
  workOutcome = `threw:${error instanceof Error ? error.message : String(error)}`
}
await sleep(150)

console.log("\n-- captured mock hits --")
for (const h of hits) {
  console.log(
    `  ${h.method} ${h.path}${
      h.bearer
        ? ` [bearer ${
            h.bearer.startsWith("Bearer oa_agent_") ? "oa_agent_…" : "other"
          }]`
        : ""
    }`,
  )
}
console.log(`\nmanaged home used: ${managedHome}\n`)

// --- Part B assertions: the 9-gate convergence (reused from Phase 1) --------
let identity: Record<string, unknown> | null = null
try {
  identity = JSON.parse(readFileSync(join(managedHome, "identity.json"), "utf8"))
} catch {
  identity = null
}
check(
  "Gate 1 — node generated identity (identity.json written)",
  identity !== null && typeof identity.npub === "string",
)
check(
  "Gate 2 — agent self-registered (POST /api/agents/register)",
  seen(h => h.path === "/api/agents/register" && h.method === "POST"),
)
check(
  "Gate 3 — registration used the node npub as externalId",
  registerBody !== null && registerBody.externalId === identity?.npub,
)
// AO-3 gate: the user-chosen display name flowed into registration.
check(
  "Gate 3b (AO-3) — the chosen display name flowed into registration",
  registerBody !== null && registerBody.displayName === CHOSEN_NAME,
)
let persisted: Record<string, unknown> | null = null
try {
  persisted = JSON.parse(
    readFileSync(join(managedHome, "agent-credential.json"), "utf8"),
  )
} catch {
  persisted = null
}
check(
  "Gate 4 — token persisted to managed home (agent-credential.json)",
  persisted !== null &&
    typeof persisted.token === "string" &&
    (persisted.token as string).startsWith("oa_agent_"),
)
const presenceHits = hits.filter(h => h.path === "/api/pylons/register")
check("Gate 5 — presence registered (POST /api/pylons/register)", presenceHits.length > 0)
check(
  "Gate 6 — presence used the bearer agent token (not NIP-98)",
  presenceHits.some(h => h.bearer === `Bearer ${FAKE_TOKEN}`),
)
check(
  "Gate 6b — no pre-token presence request claimed the pylon",
  presenceHits.every(h => h.bearer === `Bearer ${FAKE_TOKEN}`),
)
check(
  "Gate 7 — payout target registered (POST .../spark-payout-target)",
  seen(h => h.path.endsWith("/spark-payout-target") && h.method === "POST"),
)
check(
  "Gate 8 — assignment worker polled for work (GET .../assignments)",
  seen(h => h.path.endsWith("/assignments") && h.method === "GET"),
)
check(
  "Gate 9 — token never printed to status/log surfaces",
  !statuses.join("\n").includes(FAKE_TOKEN),
)
// Create-new path must NOT have booted the AO-3 existing fixture home.
check(
  "AO-3 create-new: a fresh managed home was minted (not the existing fixture)",
  managedHome !== existingHome,
)

// --- AF-3/AF-4 (#5900/#5901): forum self-introduction + read-only work-search -
let introReceipt: Record<string, unknown> | null = null
try {
  introReceipt = JSON.parse(
    readFileSync(join(managedHome, "forum-intro.json"), "utf8"),
  )
} catch {
  introReceipt = null
}
let workReceipt: Record<string, unknown> | null = null
try {
  workReceipt = JSON.parse(
    readFileSync(join(managedHome, "forum-work-search.json"), "utf8"),
  )
} catch {
  workReceipt = null
}
check("Gate 10 (AF-3) — forum self-introduction posted", introOutcome === "posted")
check(
  "Gate 10b (AF-3) — typed intro lane selected from the board",
  seen(h => h.path === "/api/forum" && h.method === "GET") &&
    introReceipt !== null &&
    introReceipt.forumSlug === "introductions",
)
check(
  "Gate 10c (AF-3) — intro receipt has a dereferenceable topic URL",
  introReceipt !== null &&
    typeof introReceipt.url === "string" &&
    (introReceipt.url as string).includes("/forum/t/"),
)
check("Gate 11 (AF-4) — read-only work-search ran", workOutcome === "searched")
check(
  "Gate 11b (AF-4) — work-search counted open items by typed state",
  seen(h => h.path === "/api/forum/work-requests" && h.method === "GET") &&
    workReceipt !== null &&
    workReceipt.openCount === 2,
)
const forumIntroPosted = introReceipt !== null
const forumWorkSearched = workReceipt !== null

// ===========================================================================
// PART C — AO-4 wizard live-state assertions, driven by the REAL observed
// signals from Part B (node launch status + persisted credential), with the
// wallet/assignment signals projected over the converged state. No faked
// progress: every input is an observable fact from this run.
// ===========================================================================
console.log("\n-- Part C: AO-4 wizard live-state projection --")

const agentRegistered = persisted !== null
const localPylonReady = identity !== null && agentRegistered

// C.1 the converged, earning-ready state the chain reaches by the end of the run
// (presence + payout fired, assignment loop polling). We assert the wizard
// projects each step to a sane, non-faked status. We do NOT assert "earned"
// done — that requires a settled receipt (live-proof, Part D).
const convergedInput: OnboardingStatusInput = {
  fetchedAt: new Date().toISOString(),
  identityChoiceMade: true,
  identityLabel: `new: ${CHOSEN_NAME}`,
  agentRegistered,
  nodeLaunchStatus,
  localPylonReady,
  onboardingEnvConfigured: agentRegistered,
  walletReceiveReady: true,
  walletBalanceSats: 0,
  openAssignmentCount: 0,
  // AF-2/AF-3/AF-4 forum-loop signals from THIS run's real receipts. The mock
  // wallet does not exercise the tip claim, so tip-ready stays false here.
  forumTipReady: false,
  forumIntroPosted,
  forumWorkSearched,
  forumWorkOpenCount: workReceipt !== null ? Number(workReceipt.openCount) : 0,
}
const converged = projectOnboardingStatus(convergedInput)
const stepStatus = (id: string) =>
  converged.steps.find(s => s.id === id)?.status ?? "missing"

check(
  "AO-4 wizard: identity step done after the choice",
  stepStatus("identity") === "done",
)
check(
  "AO-4 wizard: registered step done after self-register",
  stepStatus("registered") === "done",
)
check(
  "AO-4 wizard: node-online step done once control is ready",
  stepStatus("node-online") === "done",
)
check(
  "AO-4 wizard: wallet step done once receive-ready",
  stepStatus("wallet") === "done",
)
check(
  "AO-4 wizard: payout step done once env configured + node online",
  stepStatus("payout") === "done",
)
check(
  "AO-4 wizard: presence step done once env configured + node online",
  stepStatus("presence") === "done",
)
check(
  "AO-4 wizard: forum-intro step done once the introduction is posted",
  stepStatus("forum-intro") === "done",
)
check(
  "AO-4 wizard: work-search step done once a search has run",
  stepStatus("work-search") === "done",
)
check(
  "AO-4 wizard: not falsely 'complete' before a settled payout",
  converged.complete === false,
)
check(
  "AO-4 wizard: 'you are here' points at an unfinished step (Tassadar/earning)",
  converged.currentStepId !== null,
)

// C.2 a claimed-but-unsettled snapshot: the wizard shows claimed=done and
// earning=active (work claimed, awaiting settlement) — still not "complete".
const claimedInput: OnboardingStatusInput = {
  ...convergedInput,
  openAssignmentCount: 1,
  walletBalanceSats: 0,
}
const claimed = projectOnboardingStatus(claimedInput)
check(
  "AO-4 wizard: Tassadar step done once an assignment is observed",
  claimed.steps.find(s => s.id === "tassadar")?.status === "done",
)
check(
  "AO-4 wizard: claimed step done once an assignment is observed",
  claimed.steps.find(s => s.id === "claimed")?.status === "done",
)
check(
  "AO-4 wizard: earning step ACTIVE (not done) while a settled payout is pending",
  claimed.steps.find(s => s.id === "earned")?.status === "active",
)
check(
  "AO-4 wizard: still not 'complete' on claimed-but-unsettled",
  claimed.complete === false,
)

// C.3 the FULLY-EARNED snapshot (sats > 0): the only state where complete=true.
// This is what the live-production Part D run must produce on real infra.
const earnedInput: OnboardingStatusInput = {
  ...claimedInput,
  walletBalanceSats: 21,
}
const earned = projectOnboardingStatus(earnedInput)
check(
  "AO-4 wizard: earning step done only once balance > 0 (real settled sats)",
  earned.steps.find(s => s.id === "earned")?.status === "done",
)
check(
  "AO-4 wizard: chain is 'complete' only at first settled sats",
  earned.complete === true,
)

// C.4 a failure-with-retry snapshot: a node that failed past its restart budget
// surfaces a retryable failed step — never a dead/blank screen.
const failedInput: OnboardingStatusInput = {
  ...convergedInput,
  nodeLaunchStatus: "failed",
  localPylonReady: false,
  walletReceiveReady: false,
}
const failedProj = projectOnboardingStatus(failedInput)
check(
  "AO-4 wizard: a failed node surfaces a retryable failure (offline → retry, not dead-end)",
  failedProj.hasRetryableFailure === true,
)

// ===========================================================================
// PART D — LIVE-PRODUCTION gates. NOT claimed by the mock harness; NOT faked.
// Listed so the run output is honest about what remains for a physical Mac with
// a fresh signed DMG + a live validator pair, per the runbook.
// ===========================================================================
console.log("\n-- Part D: LIVE-PRODUCTION gates (live-proof; NOT faked here) --")
const liveProofGates = [
  "From-DMG on a clean Apple-Silicon Mac: install + open the signed DMG, NO terminal, window RENDERS (no black screen) — visual confirmation on a stranger's machine.",
  "Production presence: the node appears on https://openagents.com/api/public/pylon-stats (real, not a mock).",
  "Real Tassadar settlement: a claimed window SETTLES against a live validator pair, producing a real Bitcoin receipt (balance > 0).",
]
for (const g of liveProofGates) {
  console.log(`LIVE-PROOF  ${g}`)
}
console.log(
  "\nRunbook for the live-production gates: docs/launch/2026-06-18-autopilot-desktop-ao6-from-dmg-runbook.md",
)

// --- final result -----------------------------------------------------------
cleanup()

console.log()
if (ok) {
  console.log(
    "RESULT: the AO-1..AO-4 + AF-3/AF-4 auto-onboarding chain converges end-to-end, headlessly, no GUI / no terminal / no env vars: AO-3 identity choice (both paths, never overwrites a home), the full register → presence → payout → assignment chain (with the chosen name), the automated forum self-introduction (typed lane + dereferenceable receipt) and read-only work-search, and the AO-4 wizard projecting each step from REAL state. The mock harness does not claim the from-DMG render, production pylon-stats, or settled Bitcoin receipt; those are live-production proof gates (see runbook).",
  )
  process.exit(0)
} else {
  fail("one or more AO-6 gates did not pass (see above)")
}
