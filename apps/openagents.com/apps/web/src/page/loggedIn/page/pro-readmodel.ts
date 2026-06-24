// /pro read model (issue 6184): the public-safe projection the runs/evals pages
// render.
//
// EVIDENCE-ONLY + PUBLIC-SAFE. These types mirror the qa-runner's persisted
// artifacts — `openagents.qa_runner.result.v1` (a run) and
// `openagents.qa_runner.eval.v1` (a chill-eval comparison) — reduced to exactly
// what an operator/PR-reviewer needs to SEE: status, target, brain, the video
// ref, the step/variant table, and the deltas. They carry NO prompt, token,
// account, secret, price, or margin (the qa-runner tripwire already strips those
// at the source; this read model never reintroduces them).
//
// For this build the index + detail data is served from a committed, public-safe
// FIXTURE set (below) so the pages render deterministically + headless and the
// URLs are stable/shareable today. The live read path (reading the qa-runner
// artifact store) is a drop-in follow-up: it returns the SAME shapes, so the
// view code does not change. Keeping the seam explicit (a `resolve*` lookup)
// avoids ad-hoc data wiring and keeps the projection honest.

// A measured latency value OR the honest "not measured" marker, mirroring the
// qa-runner eval schema. Rendered literally as "not_measured" — never a fake 0.
export type MeasuredMs = number | 'not_measured'

// ---------------------------------------------------------------------------
// Run read model (openagents.qa_runner.result.v1 projection)
// ---------------------------------------------------------------------------

export type ProRunStepStatus = 'ok' | 'failed'

export type ProRunStep = Readonly<{
  index: number
  kind: string
  label: string
  status: ProRunStepStatus
}>

export type ProRunVideo = Readonly<{
  // Dereferenceable, public-safe URL to the playable video. In the fixture set
  // this points at a small committed sample asset under /pro-assets so the page
  // renders a real <video> headless.
  src: string
  format: 'mp4' | 'webm'
}>

// The verify investigator verdict (#6192), projected from the run's additive
// `verify` field. CONFIRMED/REFUTED/INCONCLUSIVE, with per-commitment findings
// carrying the OBSERVED evidence summary so a reviewer sees WHY the verdict
// landed — never inflated (uncertain stays INCONCLUSIVE; a false claim is
// REFUTED, never a fake CONFIRMED).
export type ProVerdict = 'CONFIRMED' | 'REFUTED' | 'INCONCLUSIVE'

export type ProVerifyFinding = Readonly<{
  id: string
  claim: string
  verdict: ProVerdict
  evidenceSummary: string
}>

export type ProVerify = Readonly<{
  verdict: ProVerdict
  findings: ReadonlyArray<ProVerifyFinding>
  observed: boolean
}>

// One target's row in a multi-target matrix (#6190): the SAME scenario run
// against this target, with its own pass/fail + video + verdict. `readOnly`
// flags a restricted target (prod by default); a read-only target that a
// scenario tried to mutate fails HONESTLY with the reason in `failure` (never a
// silent skip). Mirrors the qa-runner `openagents.qa_runner.target_matrix.v1`
// per-target ref, reduced to the public-safe projection the page renders.
export type ProTargetRun = Readonly<{
  targetName: string
  targetBaseUrl: string
  readOnly: boolean
  status: 'pass' | 'fail'
  durationMs: number
  video?: ProRunVideo
  failure?: string
  verdict?: ProVerdict
}>

export type ProRun = Readonly<{
  id: string
  title: string
  status: 'pass' | 'fail'
  targetName: string
  targetBaseUrl: string
  brain: string
  backend: string
  startedAt: string
  durationMs: number
  steps: ReadonlyArray<ProRunStep>
  video?: ProRunVideo
  // The committed distilled-test reference (path in the repo), if any.
  distilledTestPath?: string
  failure?: string
  // The verify investigator verdict (#6192), when the run declared commitments.
  verify?: ProVerify
  // The per-target matrix (#6190): when this run is the SAME scenario run across
  // N registry targets (dev/staging/prod/selfhost) from a single definition. The
  // run-detail page renders a per-target table (status + video + restriction +
  // verdict) so a reviewer sees "test the dev server / test production" results
  // side by side with no rewrite.
  targets?: ReadonlyArray<ProTargetRun>
}>

// ---------------------------------------------------------------------------
// Eval read model (openagents.qa_runner.eval.v1 projection)
// ---------------------------------------------------------------------------

export type ProEvalVariant = Readonly<{
  variantId: string
  label: string
  note?: string
  passRate: number
  passCount: number
  runCount: number
  latencyP50Ms: MeasuredMs
  latencyP90Ms: MeasuredMs
  video?: ProRunVideo
  // Delta vs the baseline variant (the first one). The baseline's deltas are 0.
  passRateDelta: number
  latencyP50DeltaMs: MeasuredMs
}>

export type ProEval = Readonly<{
  id: string
  title: string
  scenarioLabel: string
  scenarioId: string
  targetName: string
  repetitions: number
  baselineVariantId: string
  decisionGrade: boolean
  variants: ReadonlyArray<ProEvalVariant>
  // The verify investigator verdict for the comparison's candidate scenario
  // (#6192), when commitments were declared. Surfaced on the eval detail page.
  verify?: ProVerify
}>

// ---------------------------------------------------------------------------
// Public-safe FIXTURE data (committed). Deterministic for headless render.
// ---------------------------------------------------------------------------

// A small committed sample video so the detail pages render a real, playable
// <video> with NO backend. Public-safe (a neutral UI capture, not a customer
// session). Lives under the web app's public assets.
const SAMPLE_VIDEO: ProRunVideo = {
  src: '/pro-assets/sample-session.webm',
  format: 'webm',
}

// Forward declarations for the multi-target fixture runs are defined below
// (after FIXTURE_RUNS' simpler entries) and appended via FIXTURE_RUNS_ALL.

const FIXTURE_RUNS_BASE: ReadonlyArray<ProRun> = [
  {
    id: 'login-regression-prod',
    title: '/login renders the sign-in form (prod)',
    status: 'pass',
    targetName: 'openagents.com-prod',
    targetBaseUrl: 'https://openagents.com',
    brain: 'scripted',
    backend: 'local',
    startedAt: '2026-06-24T00:00:00.000Z',
    durationMs: 2140,
    steps: [
      { index: 0, kind: 'navigate', label: 'open /login', status: 'ok' },
      {
        index: 1,
        kind: 'wait-for',
        label: 'sign-in form renders',
        status: 'ok',
      },
      { index: 2, kind: 'screenshot', label: 'screenshot login-page', status: 'ok' },
      {
        index: 3,
        kind: 'assert',
        label: 'stays at /login (no redirect to home)',
        status: 'ok',
      },
      {
        index: 4,
        kind: 'assert',
        label: 'body contains "Log in to OpenAgents"',
        status: 'ok',
      },
    ],
    video: SAMPLE_VIDEO,
    distilledTestPath: 'apps/qa-runner/generated/login-verify.e2e.test.ts',
    // #6192: the run declared commitments; every one is backed by an observed
    // ok step -> CONFIRMED (not inflated — each finding cites the observed step).
    verify: {
      verdict: 'CONFIRMED',
      observed: true,
      findings: [
        {
          id: 'no-redirect',
          claim: '/login does NOT redirect to home when logged out',
          verdict: 'CONFIRMED',
          evidenceSummary:
            'observed step "stays at /login (no redirect to home)" = ok',
        },
        {
          id: 'renders-signin',
          claim: '/login renders "Log in to OpenAgents"',
          verdict: 'CONFIRMED',
          evidenceSummary:
            'observed step "body contains \\"Log in to OpenAgents\\"" = ok',
        },
      ],
    },
  },
  {
    // A REFUTED run: the agent CLAIMED /login redirects away (it does not). The
    // claim is FALSE, so the verdict is REFUTED with the contradicting evidence
    // inline — a false claim is a valid finding, never a fake pass (#6192).
    id: 'login-redirect-claim-refuted',
    title: '/login redirect claim (FALSE) — refuted',
    status: 'fail',
    targetName: 'openagents.com-prod',
    targetBaseUrl: 'https://openagents.com',
    brain: 'scripted',
    backend: 'local',
    startedAt: '2026-06-24T00:05:00.000Z',
    durationMs: 1980,
    steps: [
      { index: 0, kind: 'navigate', label: 'open /login', status: 'ok' },
      { index: 1, kind: 'wait-for', label: 'sign-in form renders', status: 'ok' },
      { index: 2, kind: 'screenshot', label: 'screenshot login-page', status: 'ok' },
      {
        index: 3,
        kind: 'assert',
        label: 'redirects away from /login (intentionally wrong)',
        status: 'failed',
      },
    ],
    video: SAMPLE_VIDEO,
    failure:
      'redirects away from /login (intentionally wrong): expected url NOT to include "/login"',
    verify: {
      verdict: 'REFUTED',
      observed: true,
      findings: [
        {
          id: 'claims-redirect',
          claim: '/login redirects away from /login (FALSE claim under test)',
          verdict: 'REFUTED',
          evidenceSummary:
            'observed step "redirects away from /login (intentionally wrong)" = failed (contradicting evidence)',
        },
      ],
    },
  },
]

// ---------------------------------------------------------------------------
// Multi-target fixture runs (#6190) — appended to FIXTURE_RUNS below.
// ---------------------------------------------------------------------------

// A MULTI-TARGET run (#6190): the SAME /login scenario run across dev + staging
// + prod from a single definition (no rewrite). dev/staging are writable and
// pass; prod is read-only — the scenario stays observe-only there and passes
// too. The per-target matrix renders side by side on the run-detail page.
const MULTI_TARGET_FIXTURE_RUN: ProRun = {
  id: 'login-multi-target',
  title: '/login renders the sign-in form (dev + staging + prod)',
  status: 'pass',
  targetName: 'dev, staging, prod',
  targetBaseUrl: 'http://localhost:5173',
  brain: 'scripted',
  backend: 'local',
  startedAt: '2026-06-24T00:10:00.000Z',
  durationMs: 6300,
  steps: [
    { index: 0, kind: 'navigate', label: 'open /login', status: 'ok' },
    { index: 1, kind: 'wait-for', label: 'sign-in form renders', status: 'ok' },
    { index: 2, kind: 'screenshot', label: 'screenshot login-page', status: 'ok' },
    { index: 3, kind: 'assert', label: 'stays at /login (no redirect to home)', status: 'ok' },
    { index: 4, kind: 'assert', label: 'body contains "Log in to OpenAgents"', status: 'ok' },
  ],
  video: SAMPLE_VIDEO,
  distilledTestPath: 'apps/qa-runner/generated/login-verify.e2e.test.ts',
  // The same definition run against each registry target. prod is read-only
  // (never create data) — the scenario is observe-only, so it passes there too.
  targets: [
    {
      targetName: 'dev',
      targetBaseUrl: 'http://localhost:5173',
      readOnly: false,
      status: 'pass',
      durationMs: 2140,
      video: SAMPLE_VIDEO,
      verdict: 'CONFIRMED',
    },
    {
      targetName: 'staging',
      targetBaseUrl: 'https://staging.openagents.com',
      readOnly: false,
      status: 'pass',
      durationMs: 2090,
      video: SAMPLE_VIDEO,
      verdict: 'CONFIRMED',
    },
    {
      targetName: 'prod',
      targetBaseUrl: 'https://openagents.com',
      readOnly: true,
      status: 'pass',
      durationMs: 2070,
      video: SAMPLE_VIDEO,
      verdict: 'CONFIRMED',
    },
  ],
}

// A MULTI-TARGET run where a MUTATING scenario hits a read-only prod target
// (#6190): dev (writable) passes the click; prod (read-only) REFUSES it with an
// honest recorded reason — proof a read-only restriction blocks a mutating step
// rather than silently creating data on prod (never a fake pass).
const MULTI_TARGET_READONLY_BLOCK_RUN: ProRun = {
  id: 'submit-login-multi-target',
  title: 'submit the login form (mutating) — blocked on read-only prod',
  status: 'fail',
  targetName: 'dev, prod',
  targetBaseUrl: 'http://localhost:5173',
  brain: 'scripted',
  backend: 'local',
  startedAt: '2026-06-24T00:15:00.000Z',
  durationMs: 3200,
  steps: [
    { index: 0, kind: 'navigate', label: 'open /login', status: 'ok' },
    { index: 1, kind: 'click', label: 'submit the form (mutates)', status: 'ok' },
  ],
  failure:
    'restriction violation: target "prod" is read-only (never create data) but step kind "click" mutates it',
  targets: [
    {
      targetName: 'dev',
      targetBaseUrl: 'http://localhost:5173',
      readOnly: false,
      status: 'pass',
      durationMs: 1600,
      video: SAMPLE_VIDEO,
    },
    {
      targetName: 'prod',
      targetBaseUrl: 'https://openagents.com',
      readOnly: true,
      status: 'fail',
      durationMs: 1600,
      failure:
        'restriction violation: target "prod" is read-only (never create data) but step kind "click" mutates it',
    },
  ],
}

// The full run set: the base runs plus the multi-target matrices (#6190).
const FIXTURE_RUNS: ReadonlyArray<ProRun> = [
  ...FIXTURE_RUNS_BASE,
  MULTI_TARGET_FIXTURE_RUN,
  MULTI_TARGET_READONLY_BLOCK_RUN,
]

const FIXTURE_EVALS: ReadonlyArray<ProEval> = [
  {
    id: 'login-mcp-compare',
    title: 'Login scenario: MCP on vs off',
    scenarioLabel: '/login renders sign-in',
    scenarioId: 'login-regression',
    targetName: 'openagents.com-prod',
    repetitions: 1,
    baselineVariantId: 'mcp-on',
    decisionGrade: false,
    variants: [
      {
        variantId: 'mcp-on',
        label: 'MCP on',
        note: 'filesystem + http MCP servers enabled',
        passRate: 1,
        passCount: 1,
        runCount: 1,
        latencyP50Ms: 2140,
        latencyP90Ms: 2140,
        video: SAMPLE_VIDEO,
        passRateDelta: 0,
        latencyP50DeltaMs: 0,
      },
      {
        variantId: 'mcp-off',
        label: 'MCP off',
        note: 'no MCP servers — agent regresses on the redirect assertion',
        passRate: 0,
        passCount: 0,
        runCount: 1,
        latencyP50Ms: 1980,
        latencyP90Ms: 1980,
        video: SAMPLE_VIDEO,
        passRateDelta: -1,
        latencyP50DeltaMs: -160,
      },
    ],
    // #6192: the candidate (MCP off) regressed on the redirect assertion. Its
    // commitment is REFUTED by observed evidence — surfaced on the eval page.
    verify: {
      verdict: 'REFUTED',
      observed: true,
      findings: [
        {
          id: 'no-redirect',
          claim: '/login does NOT redirect to home when logged out',
          verdict: 'REFUTED',
          evidenceSummary:
            'observed step "stays at /login (no redirect to home)" = failed (contradicting evidence)',
        },
      ],
    },
  },
]

// ---------------------------------------------------------------------------
// Resolution seam (fixture today; live artifact store later — same shapes).
// ---------------------------------------------------------------------------

export const listProRuns = (): ReadonlyArray<ProRun> => FIXTURE_RUNS

export const listProEvals = (): ReadonlyArray<ProEval> => FIXTURE_EVALS

export const resolveProRun = (id: string): ProRun | undefined =>
  FIXTURE_RUNS.find(r => r.id === id)

export const resolveProEval = (id: string): ProEval | undefined =>
  FIXTURE_EVALS.find(e => e.id === id)
