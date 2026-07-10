import { Schema as S } from "effect"

import { sarahOpenAgentsBaseUrl } from "./account-link.ts"

export const OPENAGENTS_SARAH_FLEET_RUNS_PATH = "/api/sarah/fleet-runs"

const MAX_RESPONSE_BYTES = 64 * 1024
const ROUTE_REF = "route.sarah.fleet_runs.authority.v1" as const

const SafeString = S.Trim.check(S.isMinLength(1), S.isMaxLength(1_000))
const SafeRef = S.Trim.check(
  S.isMinLength(1),
  S.isMaxLength(180),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:/#-]*$/u),
)
const IssueRef = S.String.check(S.isPattern(/^#[1-9]\d*$/u))
const Repository = S.Struct({
  owner: SafeRef,
  name: SafeRef,
  branch: SafeRef,
  commit: S.String.check(S.isPattern(/^[0-9a-f]{40}$/u)),
})
const Verifier = S.Struct({
  kind: S.Literal("command"),
  command: SafeString,
})
const WorkSource = S.Union([
  S.Struct({
    kind: S.Literal("issue_list"),
    issueRefs: S.Array(IssueRef),
  }),
  S.Struct({
    kind: S.Literal("plan_dag"),
    planRef: SafeRef,
    units: S.Array(
      S.Struct({
        unitRef: SafeRef,
        title: SafeString,
        dependsOn: S.Array(SafeRef),
      }),
    ),
  }),
])
const WorkerPolicy = S.Struct({
  workerKind: S.Literals(["codex", "claude", "grok", "auto"]),
  targetPreference: S.Literals(["owner_local", "managed_cloud", "auto"]),
})
const RelationshipPolicy = S.Struct({
  source: S.Literal("openagents_server_policy"),
  relationshipMode: S.Literals([
    "prospect",
    "customer",
    "operator",
    "administrator",
  ]),
  codingFleetStartAllowed: S.Boolean,
  fleetObservationAllowed: S.Boolean,
  retrievalScope: S.Literals(["public_only", "owner_fleet_runs"]),
  responsePosture: S.Literals(["guided", "state_oriented"]),
  uiDensity: S.Literals(["standard", "dense"]),
  administratorToolsAllowed: S.Boolean,
})
const ExecutionUsageEvidence = S.Union([
  S.Struct({
    truth: S.Literal("exact"),
    tokenUsageRefs: S.Array(SafeRef),
  }),
  S.Struct({
    truth: S.Literal("not_measured"),
    tokenUsageRefs: S.Array(SafeRef),
  }),
])
const ExecutionCloseoutBase = {
  unitRef: SafeRef,
  workClaimRef: SafeRef,
  workerKind: S.Literals(["codex", "claude", "grok"]),
  blockerRefs: S.Array(SafeRef),
  observedAt: SafeString,
  eventRef: SafeRef,
}
const ExecutionCloseoutCarrier = {
  assignmentRef: SafeRef,
  accountRefHash: S.String.check(
    S.isPattern(/^account\.pylon\.(?:codex|claude_agent|grok)\.[a-f0-9]{24}$/u),
  ),
  closeoutRef: SafeRef,
  usageEvidence: ExecutionUsageEvidence,
}
const ExecutionCloseout = S.Union([
  S.Struct({
    ...ExecutionCloseoutBase,
    ...ExecutionCloseoutCarrier,
    terminalState: S.Literal("accepted"),
  }),
  S.Struct({
    ...ExecutionCloseoutBase,
    ...ExecutionCloseoutCarrier,
    terminalState: S.Literals(["failed", "stale"]),
  }),
  S.Struct({
    ...ExecutionCloseoutBase,
    terminalState: S.Literals(["failed", "stale"]),
  }),
])
const ExecutionProjection = S.Struct({
  state: S.Literals(["pending", "running", "completed", "failed", "stopped"]),
  lastSequence: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  counters: S.Struct({
    workUnitsTotal: S.Int.check(S.isGreaterThanOrEqualTo(0)),
    activeAssignments: S.Int.check(S.isGreaterThanOrEqualTo(0)),
    acceptedAssignments: S.Int.check(S.isGreaterThanOrEqualTo(0)),
    failedAssignments: S.Int.check(S.isGreaterThanOrEqualTo(0)),
    staleAssignments: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  }),
  startedAt: S.NullOr(SafeString),
  updatedAt: S.NullOr(SafeString),
  closeouts: S.Array(ExecutionCloseout),
})
const PublicRun = S.Struct({
  runRef: S.String.check(S.isPattern(/^fleet_run\.sarah\.[0-9a-f]{20}$/u)),
  scope: S.String.check(
    S.isPattern(/^scope\.fleet_run\.fleet_run\.sarah\.[0-9a-f]{20}$/u),
  ),
  status: S.Literals(["pending_executor", "claimed_by_pylon"]),
  objective: SafeString,
  repository: Repository,
  verifier: Verifier,
  workSource: WorkSource,
  workerPolicy: WorkerPolicy,
  targetConcurrency: S.Int.check(
    S.isGreaterThanOrEqualTo(1),
    S.isLessThanOrEqualTo(8),
  ),
  execution: ExecutionProjection,
  createdAt: SafeString,
  updatedAt: SafeString,
  privateMaterialExcluded: S.Literal(true),
})
const AuthorityErrorCode = S.Literals([
  "unauthenticated",
  "authentication_unavailable",
  "relationship_not_authorized",
  "relationship_policy_unavailable",
  "invalid_request",
  "idempotency_conflict",
  "run_not_found",
  "pylon_not_authorized",
  "pylon_unavailable",
  "claim_conflict",
  "claim_not_found",
  "claim_expired",
  "storage_unavailable",
])
const StartSuccess = S.Struct({
  ok: S.Literal(true),
  duplicate: S.Boolean,
  policy: RelationshipPolicy,
  routeRef: S.Literal(ROUTE_REF),
  run: PublicRun,
})
const AuthorityFailure = S.Struct({
  ok: S.Literal(false),
  error: S.Struct({ code: AuthorityErrorCode, retryable: S.Boolean }),
  policy: S.optionalKey(RelationshipPolicy),
  routeRef: S.Literal(ROUTE_REF),
})
const StartEnvelope = S.Union([StartSuccess, AuthorityFailure])
type StartEnvelope = typeof StartEnvelope.Type

const ERROR_RETRYABILITY: Record<
  (typeof AuthorityErrorCode.Type),
  boolean
> = {
  unauthenticated: false,
  authentication_unavailable: true,
  relationship_not_authorized: false,
  relationship_policy_unavailable: true,
  invalid_request: false,
  idempotency_conflict: false,
  run_not_found: false,
  pylon_not_authorized: false,
  pylon_unavailable: true,
  claim_conflict: false,
  claim_not_found: false,
  claim_expired: true,
  storage_unavailable: true,
}

const PRIVATE_MATERIAL_PATTERN =
  /(?:^|[\s"'])\/(?:Users|private|home)\/|(?:^|[\s"'])~\/|OPENAGENTS_AGENT_TOKEN|(?:API|AUTH|SECRET|TOKEN|PASSWORD|PRIVATE)_?KEY|BEGIN [A-Z ]*PRIVATE KEY/iu

export type SarahFleetAuthorityFetch = (
  request: Request,
) => Promise<Response>

export type SarahFleetAuthorityToolResult = Readonly<{
  ok: boolean
  output: unknown
  refreshedSessionCookies: ReadonlyArray<string>
}>

const fixedFailure = (
  code: "invalid_response" | "store_unavailable",
): SarahFleetAuthorityToolResult => ({
  ok: false,
  output: {
    ok: false,
    error: { code, retryable: code === "store_unavailable" },
    routeRef: ROUTE_REF,
  },
  refreshedSessionCookies: [],
})

const collectResponseChunks = async (
  reader: ReadableStreamDefaultReader<Uint8Array>,
  chunks: ReadonlyArray<Uint8Array> = [],
  byteLength = 0,
): Promise<Readonly<{ byteLength: number; chunks: ReadonlyArray<Uint8Array> }>> => {
  const next = await reader.read()
  if (next.done) {
    return { byteLength, chunks }
  }
  const nextByteLength = byteLength + next.value.byteLength
  if (nextByteLength > MAX_RESPONSE_BYTES) {
    await reader.cancel().catch(() => undefined)
    throw new Error("fleet authority response exceeded its byte bound")
  }
  return collectResponseChunks(
    reader,
    [...chunks, next.value],
    nextByteLength,
  )
}

const readBoundedResponseJson = async (response: Response): Promise<unknown> => {
  const declaredRaw = response.headers.get("content-length")
  const declared = declaredRaw === null ? undefined : Number(declaredRaw)
  if (
    declared !== undefined &&
    (!Number.isSafeInteger(declared) ||
      declared < 0 ||
      declared > MAX_RESPONSE_BYTES)
  ) {
    throw new Error("fleet authority response exceeded its byte bound")
  }
  if (response.body === null) {
    throw new Error("fleet authority response body is missing")
  }
  const collected = await collectResponseChunks(response.body.getReader())
  const bytes = new Uint8Array(collected.byteLength)
  collected.chunks.reduce((offset, chunk) => {
    bytes.set(chunk, offset)
    return offset + chunk.byteLength
  }, 0)
  return JSON.parse(new TextDecoder().decode(bytes)) as unknown
}

const decodeStartEnvelope = (value: unknown): StartEnvelope => {
  const decoded = S.decodeUnknownSync(StartEnvelope)(value, {
    onExcessProperty: "error",
  })
  if (
    !decoded.ok &&
    decoded.error.retryable !== ERROR_RETRYABILITY[decoded.error.code]
  ) {
    throw new Error("fleet authority response carried invalid retryability")
  }
  if (PRIVATE_MATERIAL_PATTERN.test(JSON.stringify(decoded))) {
    throw new Error("fleet authority response carried private material")
  }
  return decoded
}

const refreshedSessionCookies = (response: Response): ReadonlyArray<string> => {
  const cookies = response.headers.getSetCookie()
  if (cookies.length > 0) {
    return cookies
  }
  const single = response.headers.get("set-cookie")
  return single === null ? [] : [single]
}

const defaultFetch: SarahFleetAuthorityFetch = request => fetch(request)

export const startSarahCodingFleetRunThroughAuthority = async (
  sourceRequest: Request,
  args: unknown,
  fetchAuthority: SarahFleetAuthorityFetch = defaultFetch,
): Promise<SarahFleetAuthorityToolResult> => {
  const headers = new Headers({
    accept: "application/json",
    "content-type": "application/json",
  })
  const cookie = sourceRequest.headers.get("cookie")
  const authorization = sourceRequest.headers.get("authorization")
  if (cookie !== null) {
    headers.set("cookie", cookie)
  }
  if (authorization !== null) {
    headers.set("authorization", authorization)
  }
  // Sarah is reverse-proxied through the openagents.com Worker into Cloud Run.
  // The incoming Cloud Run origin therefore is not the API authority origin;
  // use the same canonical OpenAgents resolver as account/session verification.
  const authorityRequest = (() => {
    try {
      return new Request(
        new URL(OPENAGENTS_SARAH_FLEET_RUNS_PATH, sarahOpenAgentsBaseUrl()),
        {
          method: "POST",
          headers,
          body: JSON.stringify(args),
          signal: AbortSignal.timeout(6_000),
        },
      )
    } catch {
      return undefined
    }
  })()
  if (authorityRequest === undefined) {
    return fixedFailure("store_unavailable")
  }

  const response = await fetchAuthority(authorityRequest).catch(() => undefined)
  if (response === undefined) {
    return fixedFailure("store_unavailable")
  }
  try {
    const cookies = refreshedSessionCookies(response)
    const output = decodeStartEnvelope(await readBoundedResponseJson(response))
    if (response.ok !== output.ok) {
      return fixedFailure("invalid_response")
    }
    return {
      ok: response.ok && output.ok === true,
      output,
      refreshedSessionCookies: cookies,
    }
  } catch {
    return fixedFailure("invalid_response")
  }
}
