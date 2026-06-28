/**
 * Blueprint Signature 6 — `operator-grounded-assertion`
 *
 * No runnable artifact reaches the owner unless it has been verified to exist.
 *
 * Pure, ordered-predicate state machine implementing the gate from
 * `docs/artanis/2026-06-28-blueprint-signature-governed-autonomous-ops.md`:
 *
 *   UNGROUNDED → REFERENCED → LOOKED_UP → GROUNDED
 *
 * Only the terminal state `GROUNDED` unlocks presenting the artifact as
 * runnable/real. Any operator output that references a runnable COMMAND, FILE
 * PATH, SCRIPT, or API ENDPOINT must reach `GROUNDED` (a positive existence
 * lookup THIS turn) or be explicitly labeled SPECULATIVE / omitted.
 *
 * Ordered predicates (terminal-last):
 *  1. referenced — an artifact ref is in hand.
 *  2. looked-up  — the matching grounding tool was actually called this turn
 *                  (`repo_path_exists` / `repo_grep` for a file/script/command
 *                  source; `route_exists` for an api_endpoint). A reference with
 *                  no lookup stays below GROUNDED, so a path/endpoint invented
 *                  from memory cannot be asserted.
 *  3. grounded   — the lookup returned a POSITIVE existence result (EXISTS / is
 *                  a registered route / pattern matched). A negative lookup
 *                  (does-not-exist / not-in-registry / no-match / a read failure)
 *                  holds the gate at LOOKED_UP and the artifact stays UNGROUNDED.
 *
 * This makes the MirrorCode / `distill_traces` / admin-endpoint fabrication
 * class structurally impossible: the terminal state that unlocks "assert as
 * real" is unreachable without a positive lookup of the real repo / real
 * OpenAPI registry.
 *
 * SINGLE AUTHORITY: this is the one home for the S6 gate so the openagents.com
 * Worker operator loop (which produces the S6 evidence via its grounding tools)
 * and Pylon both import + apply THIS function rather than re-describing it.
 */

export const OPERATOR_GROUNDED_ASSERTION_STATES = [
  "UNGROUNDED",
  "REFERENCED",
  "LOOKED_UP",
  "GROUNDED",
] as const

export type OperatorGroundedAssertionState =
  (typeof OPERATOR_GROUNDED_ASSERTION_STATES)[number]

export const OPERATOR_GROUNDED_ASSERTION_EVIDENCE = {
  /** A file/script/command source confirmed to exist via repo_path_exists. */
  pathExists: "evidence://grounding/path-exists",
  /** A flag/symbol confirmed present in a real file via repo_grep. */
  contentMatch: "evidence://grounding/content-match",
  /** An API endpoint confirmed registered via route_exists. */
  routeRegistered: "evidence://grounding/route-registered",
} as const

export type OperatorGroundedAssertionEvidenceRef =
  (typeof OPERATOR_GROUNDED_ASSERTION_EVIDENCE)[keyof typeof OPERATOR_GROUNDED_ASSERTION_EVIDENCE]

/**
 * The kind of runnable artifact being asserted. `command`, `file_path`, and
 * `script` all ground through the path/content evidence; `api_endpoint` grounds
 * through the route-registry evidence.
 */
export const OPERATOR_GROUNDED_ARTIFACT_KINDS = [
  "command",
  "file_path",
  "script",
  "api_endpoint",
] as const

export type OperatorGroundedArtifactKind =
  (typeof OPERATOR_GROUNDED_ARTIFACT_KINDS)[number]

/**
 * The outcome of the matching grounding lookup for the artifact:
 *  - `positive`      — the tool confirmed existence (EXISTS / registered route /
 *                      pattern matched).
 *  - `negative`      — the tool ran and reported non-existence / not-registered /
 *                      no-match / an unrecoverable read failure.
 *  - `not_looked_up` — no matching grounding lookup was performed this turn.
 */
export type OperatorGroundedLookupResult =
  | "positive"
  | "negative"
  | "not_looked_up"

export interface OperatorGroundedAssertionInputs {
  readonly artifactKind: OperatorGroundedArtifactKind
  readonly artifactRef: string
  /** The grounding tool that produced `lookupResult`, or null if none ran. */
  readonly lookupTool: string | null
  readonly lookupResult: OperatorGroundedLookupResult
}

export interface OperatorGroundedAssertionResult {
  readonly state: OperatorGroundedAssertionState
  /** True only when the terminal state GROUNDED is reached. */
  readonly canAssert: boolean
  readonly artifactKind: OperatorGroundedArtifactKind
  readonly artifactRef: string
  readonly lookupTool: string | null
  readonly lookupResult: OperatorGroundedLookupResult
  readonly satisfiedEvidence: ReadonlyArray<OperatorGroundedAssertionEvidenceRef>
  readonly missingEvidence: ReadonlyArray<OperatorGroundedAssertionEvidenceRef>
  readonly locked: boolean
  readonly lockedAt: OperatorGroundedAssertionState | null
  readonly blockedReason: string | null
}

/** The evidence ref a given artifact kind must satisfy to reach GROUNDED. */
function requiredEvidenceFor(
  kind: OperatorGroundedArtifactKind,
): OperatorGroundedAssertionEvidenceRef {
  return kind === "api_endpoint"
    ? OPERATOR_GROUNDED_ASSERTION_EVIDENCE.routeRegistered
    : OPERATOR_GROUNDED_ASSERTION_EVIDENCE.pathExists
}

/**
 * Evaluate the `operator-grounded-assertion` gate. Pure function.
 */
export function evaluateOperatorGroundedAssertion(
  inputs: OperatorGroundedAssertionInputs,
): OperatorGroundedAssertionResult {
  const requiredEvidence = requiredEvidenceFor(inputs.artifactKind)

  const lock = (
    state: OperatorGroundedAssertionState,
    lockedAt: OperatorGroundedAssertionState,
    blockedReason: string,
  ): OperatorGroundedAssertionResult => ({
    state,
    canAssert: false,
    artifactKind: inputs.artifactKind,
    artifactRef: inputs.artifactRef,
    lookupTool: inputs.lookupTool,
    lookupResult: inputs.lookupResult,
    satisfiedEvidence: [],
    missingEvidence: [requiredEvidence],
    locked: true,
    lockedAt,
    blockedReason,
  })

  // Predicate 1 — referenced. An empty ref is not an assertion at all.
  if (
    typeof inputs.artifactRef !== "string" ||
    inputs.artifactRef.trim().length === 0
  ) {
    return lock(
      "UNGROUNDED",
      "REFERENCED",
      "no artifact ref was supplied",
    )
  }

  // Predicate 2 — looked up. A reference with no matching grounding lookup this
  // turn stays UNGROUNDED: a path/endpoint recalled from memory cannot reach
  // GROUNDED without actually checking the real repo / OpenAPI registry.
  if (inputs.lookupTool === null || inputs.lookupResult === "not_looked_up") {
    return lock(
      "REFERENCED",
      "LOOKED_UP",
      `"${inputs.artifactRef}" was referenced but no grounding lookup was run for it this turn — UNGROUNDED, label it SPECULATIVE`,
    )
  }

  // Predicate 3 — grounded. The lookup ran but did not confirm existence.
  if (inputs.lookupResult === "negative") {
    return lock(
      "LOOKED_UP",
      "GROUNDED",
      `"${inputs.artifactRef}" was looked up via ${inputs.lookupTool} but the lookup did not confirm it exists — UNGROUNDED, label it SPECULATIVE`,
    )
  }

  return {
    state: "GROUNDED",
    canAssert: true,
    artifactKind: inputs.artifactKind,
    artifactRef: inputs.artifactRef,
    lookupTool: inputs.lookupTool,
    lookupResult: inputs.lookupResult,
    satisfiedEvidence: [requiredEvidence],
    missingEvidence: [],
    locked: false,
    lockedAt: null,
    blockedReason: null,
  }
}
