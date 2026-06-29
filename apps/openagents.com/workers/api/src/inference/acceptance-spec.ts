// Intent -> acceptance spec seam for the Khala verified-work lane (EPIC #6017).
//
// THE PRINCIPLE (docs/inference/2026-06-22-verified-work-must-execute-the-artifact.md):
// `verified:true` must mean "we ran it and it did what the user asked." A verifier
// that only pattern-matches source is a keyword filter, not a verifier. This module
// turns a request's INTENT into a structured, deterministic AcceptanceSpec — a list
// of checkable tests, not prose — that an out-of-Worker execution runner runs against
// the produced artifact. The runner's per-test pass/fail and the fraction passing are
// the honest `verified` / `scalarReward` signal.
//
// This module is pure and Worker-safe (no browser, no I/O). The runner
// (`acceptance-runner/`) consumes the spec; the route consumes the runner verdict.

// The deterministic check ids for the crossy-road / khala-code lane. Each id is a
// PROGRAM the runner executes against a live page, NOT a regex over source. The ids
// map 1:1 to the four real defects the regex verifier missed plus the obvious
// must-haves (see the spec doc "The failure, with the receipt").
export type CrossyRoadAcceptanceCheckId =
  // Loads with ZERO console/page errors (catches "crashed on load").
  | 'loads_without_errors'
  // PLAY starts the game: start/overlay screen hides AND an update loop advances
  // (catches the dead PLAY button intercepted by a full-screen .hidden overlay).
  | 'play_starts_game'
  // A forward input advances the player ~one tile (catches "PLAY did nothing").
  | 'forward_input_advances_player'
  // The camera follow delta per move is BOUNDED (catches the 100x camera bug).
  | 'camera_follow_delta_bounded'
  // The world keeps generating ahead of the player for N moves (catches blue sky).
  | 'world_keeps_generating_ahead'
  // Restart resets the player position and progress to the start.
  | 'restart_resets_state'

// A typed acceptance spec: the structured, executable contract derived from intent.
// `kind` discriminates the lane so we can generalize to other prompts later while the
// runner stays deterministic per lane. The crossy-road lane carries the concrete,
// bounded thresholds the runner asserts against the page's exposed state hooks.
export type CrossyRoadAcceptanceSpec = Readonly<{
  kind: 'crossy_road_single_html'
  rubricRef: string
  // The ordered checks to run. The runner runs ALL of them and reports each.
  checks: ReadonlyArray<CrossyRoadAcceptanceCheckId>
  // Bounded, deterministic parameters the runner uses. Concrete numbers, never prose.
  params: Readonly<{
    // Number of forward moves to drive when checking world generation + advance.
    forwardMoves: number
    // Max allowed camera-position delta magnitude per single move (world units).
    // The known-good game moves the camera ~1 unit per hop; the 100x bug moves it
    // ~TILE_SIZE*1 -> far over this bound. Pick a bound that passes good, fails 100x.
    maxCameraDeltaPerMove: number
    // Expected per-forward-move player advance (tiles). Tolerance is +/- this value's
    // own slack window so a clean 1-tile hop passes and a no-op (0) fails.
    expectedForwardAdvance: number
    // Min number of distinct world rows/tiles that must exist ahead of the player
    // after `forwardMoves` forward moves (catches "stopped generating after ~10").
    minWorldRowsAhead: number
  }>
}>

// The generalization seam: other lanes get their own spec variants. Today only the
// crossy-road lane is concrete; the union makes the runner + route forward-compatible.
export type AcceptanceSpec = CrossyRoadAcceptanceSpec

// A minimal shape of the inference request the seam reads. Kept structural so callers
// (the route) can pass their own `InferenceRequest` without a hard import cycle.
export type AcceptanceIntentRequest = Readonly<{
  model?: string | undefined
  messages?:
    | ReadonlyArray<Readonly<{ role: string; content: string }>>
    | undefined
}>

export const CROSSY_ROAD_ACCEPTANCE_RUBRIC_REF =
  'rubric.khala_code.crossy_road.executed_acceptance.v1'

// The full ordered crossy-road check list (the four caught bugs + must-haves).
export const CROSSY_ROAD_ACCEPTANCE_CHECKS: ReadonlyArray<CrossyRoadAcceptanceCheckId> =
  [
    'loads_without_errors',
    'play_starts_game',
    'forward_input_advances_player',
    'camera_follow_delta_bounded',
    'world_keeps_generating_ahead',
    'restart_resets_state',
  ]

// The default, bounded crossy-road spec. The thresholds are chosen so the known-good
// fixture passes every check and each of the four real defects fails its check.
export const crossyRoadAcceptanceSpec = (
  overrides?: Partial<CrossyRoadAcceptanceSpec['params']>,
): CrossyRoadAcceptanceSpec => ({
  checks: CROSSY_ROAD_ACCEPTANCE_CHECKS,
  kind: 'crossy_road_single_html',
  params: {
    expectedForwardAdvance: 1,
    forwardMoves: 12,
    maxCameraDeltaPerMove: 5,
    minWorldRowsAhead: 12,
    ...overrides,
  },
  rubricRef: CROSSY_ROAD_ACCEPTANCE_RUBRIC_REF,
})

// Lightweight intent signal: is this a crossy-road-shaped coding ask? Kept as a
// bounded keyword check ONLY for lane selection (NOT for the verification verdict) —
// once a lane is selected the runner does the real, executed verification. A broader
// semantic selector replaces this when the lane set grows.
const looksLikeCrossyRoad = (text: string): boolean => {
  const lower = text.toLowerCase()
  return (
    lower.includes('crossy') ||
    (lower.includes('road') &&
      (lower.includes('game') || lower.includes('frogger'))) ||
    lower.includes('frogger')
  )
}

// ---------------------------------------------------------------------------
// ACCEPTANCE-CONTRACT GUIDANCE (EPIC #6017 — "turn intent into tests it must
// pass"). A generated game is only execution-VERIFIABLE if it EXPOSES the state
// hooks the headless runner drives. Left unsaid, a model writes a self-contained
// game that plays fine for a human but exposes nothing the runner can read, so
// every executed check fails honestly and the artifact never reaches an accepted
// outcome. So we tell the model the contract up front, gateway-side, as guidance
// — additive to (never clobbering) the identity prompt. This is the SAME `window`
// state contract the runner asserts (`acceptance-runner/runner.ts`).
//
// SCOPED + EXTENSIBLE: the contract text is keyed on the spec `kind`, so the
// crossy-road target gets the crossy-road hooks and a future lane adds its own
// branch here without touching the route. A request with no executable lane gets
// no contract guidance (returns undefined).
// ---------------------------------------------------------------------------

// The crossy-road `window` state contract, mirrored from the runner's documented
// hooks. Concrete and copy-pasteable so the model wires it deterministically.
const CROSSY_ROAD_ACCEPTANCE_CONTRACT_GUIDANCE = [
  'ACCEPTANCE CONTRACT (REQUIRED so the game is automatically verifiable):',
  'In addition to building a great playable game, you MUST expose these hooks on `window` so an automated runner can drive and check it:',
  '- `window.__openagentsCrossyRoadState()` returns an object: { player: { x, y, z }, camera: { position: { x, y, z } }, progress: number, worldRowsAhead?: number, started?: boolean, loopTicks?: number }. `player` is the player world-unit position; `camera.position` is the camera world position; `progress` is forward tiles travelled; `worldRowsAhead` is distinct rows generated ahead of the player; `started` is true once play begins; `loopTicks` increments each update tick.',
  '- A PLAY control: either `window.__openagentsCrossyRoadStart()` OR a clickable element with id `start-btn` or `play` that hides the start overlay and begins the update loop.',
  '- `window.__openagentsCrossyRoadRestart()` resets the player position and progress back to the start.',
  'Keep these hooks accurate every frame (e.g. update `progress`, `loopTicks`, and the player/camera positions as the game runs). They are read-only probes for verification and must not change the gameplay a human sees.',
].join('\n')

// Map a selected acceptance spec to its model-facing contract guidance. Keyed on
// `kind` so each lane owns its contract; exhaustive so a new lane must add text.
export const acceptanceContractGuidanceForSpec = (
  spec: AcceptanceSpec,
): string => {
  switch (spec.kind) {
    case 'crossy_road_single_html':
      return CROSSY_ROAD_ACCEPTANCE_CONTRACT_GUIDANCE
  }
}

// Resolve the acceptance-contract guidance for a request, or undefined when the
// request has no executable acceptance lane (so no contract is injected). Pure +
// Worker-safe; the route injects the returned text as a leading system message.
export const acceptanceContractGuidanceForRequest = (
  request: AcceptanceIntentRequest,
): string | undefined => {
  const spec = intentToAcceptanceSpec(request)
  return spec === undefined ? undefined : acceptanceContractGuidanceForSpec(spec)
}

// Intent -> AcceptanceSpec. For the khala-code crossy-road lane this returns the
// concrete bounded spec. For other prompts in the khala-code lane today it falls back
// to the crossy-road spec (the only executable lane built so far); a future
// coordinator/verifier generates lane-appropriate specs here. Returns `undefined`
// when there is no executable acceptance lane for the request (the route then must
// NOT claim execution-backed verification).
export const intentToAcceptanceSpec = (
  request: AcceptanceIntentRequest,
): AcceptanceSpec | undefined => {
  const text = (request.messages ?? [])
    .map(message => message.content)
    .join('\n')
  if (looksLikeCrossyRoad(text)) {
    return crossyRoadAcceptanceSpec()
  }
  // No prose match — but the khala-code lane today only ships the crossy-road
  // executable suite. Default to it so the lane's artifacts are executed rather than
  // regex-stamped. (Generalize per-lane here later.)
  return crossyRoadAcceptanceSpec()
}
