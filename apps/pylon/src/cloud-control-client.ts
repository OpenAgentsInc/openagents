// Cloud control-plane client for the Pylon `openagents-cloud` execution
// provider backend (#4997).
//
// This is the HTTP seam that turns the already-merged lane plumbing (#4998,
// #4999) into a live loop. When a control session is spawned with a cloud lane
// (`cloud-gcp`, `cloud-shc`, or `auto` resolving to cloud), Pylon calls the
// private OpenAgents Cloud control plane (`oa-codex-control`) instead of
// running the agent locally:
//
//   1. POST /v1/placement  — lane-agnostic placement
//      (`openagents.codex_placement_assignment.v1`) -> RunnerBinding +
//      externalRunId. Lane maps GCE primary / SHC secondary per owner policy.
//   2. GET  /v1/codex-runs/{externalRunId}/events?cursor=N — poll the cloud
//      run's `openagents.codex_workroom_event.v1` events.
//   3. POST /v1/codex-runs/{externalRunId}/cancel — propagate cancellation.
//
// This module is transport-only: it knows the cloud HTTP contract and maps
// cloud events to a neutral shape. The mapping into the Pylon `SessionEvent`
// stream lives in `openagents-cloud-provider.ts` so the existing
// `/sessions/:ref/events` stream is lane-transparent.
//
// Scope deferral: live VM provisioning, warm pools, and receipt cost-comparison
// are the cloud repo's job. Pylon targets the documented HTTP contract only
// (docs/control/CODEX_CONTROL_API.md,
// docs/contracts/openagents.codex_placement_assignment.v1.md).

export const CLOUD_CONTROL_URL_ENV = "OA_CLOUD_CONTROL_URL" as const
export const CLOUD_CONTROL_TOKEN_ENV = "OA_CLOUD_CONTROL_TOKEN" as const

// The cloud control plane's documented placement contract version.
export const CLOUD_PLACEMENT_CONTRACT_VERSION =
  "openagents.codex_placement_assignment.v1" as const

// The lane-agnostic compute lane understood by the cloud placement endpoint.
// `local` is rejected by the cloud endpoint (it is resolved by the caller's own
// Pylon), so the cloud client only ever sends a cloud or `auto` lane.
export type CloudComputeLane = "auto" | "cloud-gcp" | "cloud-shc"

export type CloudControlConfig = {
  baseUrl: string
  bearerToken: string
}

export type ResolvedCloudControlConfig =
  | { configured: true; config: CloudControlConfig }
  | { configured: false }

function isNonEmpty(value: string | undefined): value is string {
  return typeof value === "string" && value.trim().length > 0
}

// Resolve the cloud control-plane endpoint from neutral env. When either var is
// missing, the provider is *not* configured and Pylon falls back to running the
// session locally exactly as before. This is the capability/env gate that keeps
// a no-cloud-config Pylon working unchanged.
export function resolveCloudControlConfig(
  env: Readonly<Record<string, string | undefined>> = {},
): ResolvedCloudControlConfig {
  const baseUrl = env[CLOUD_CONTROL_URL_ENV]
  const bearerToken = env[CLOUD_CONTROL_TOKEN_ENV]
  if (isNonEmpty(baseUrl) && isNonEmpty(bearerToken)) {
    return {
      configured: true,
      config: { baseUrl: baseUrl.replace(/\/+$/, ""), bearerToken },
    }
  }
  return { configured: false }
}

// The lane-agnostic placement assignment body
// (`openagents.codex_placement_assignment.v1`). Refs only — no raw secrets.
export type CloudPlacementAssignment = {
  contract_version: typeof CLOUD_PLACEMENT_CONTRACT_VERSION
  run_id: string
  owner_ref: string
  provider_account_ref: string
  auth_grant_ref: string
  goal: string
  lane: CloudComputeLane
  repository?: string
  sandbox_mode?: string
  wallet_authority: false
  created_at_ms: number
}

// The runner binding the placement endpoint returns. Refs-and-limits only; per
// the contract it carries no raw owner identity, cost, GCP project id, instance
// name, IP, credentials, or topology.
export type CloudRunnerBinding = {
  contractVersion: string
  runId: string
  externalRunId: string
  lane: "cloud-gcp" | "cloud-shc"
  providerLane: "gcp" | "shc"
  runnerId: string
  capacityClassId: string | null
  sandboxMode: string
  reason: string
  costDriven: boolean
  caps?: Record<string, unknown>
}

// A normalized terminal status from a cloud run event kind.
export type CloudTerminalKind = "completed" | "failed" | "timeout" | "cancelled"

// `openagents.codex_workroom_event.v1` event kinds emitted by the cloud control
// plane and mirrored through its events feed.
export type CloudWorkroomEventKind =
  | "queued"
  | "started"
  | "log"
  | "redacted"
  | "artifact"
  | "receipt"
  | "cleanup"
  | "completed"
  | "failed"
  | "timeout"
  | "cancelled"
  // `placement.bound` is emitted by the placement endpoint ack.
  | "placement.bound"
  // ---------------------------------------------------------------------------
  // #5005 — cloud GCE per-session lease lifecycle
  // (`openagents.gce_capacity_class.v1`, cloud commit fbd62cf).
  //
  // The `cloud-gcp` lane drives a live ephemeral-per-session VM lease and emits
  // four additional discriminators. On the cloud side these are carried by the
  // `JobEvent.type` field (`cloud.gce.*`) while the broad workroom `kind` is one
  // of the existing kinds (`started`/`receipt`/`cleanup`/`log`). Pylon reads the
  // `cloud.gce.*` discriminator from `type` when present (and tolerates `kind`
  // carrying it directly), so VM provenance and the resource_usage_receipt ref
  // round-trip to the desktop/phone instead of being silently dropped.
  | "cloud.gce.provisioned"
  | "cloud.gce.cleanup"
  | "cloud.gce.degraded"
  // The resource event carries the refs-only
  // `openagents.resource_usage_receipt.v1` digest. The cloud `JobEvent.type` is
  // `cloud.gce.resource_usage_receipt`; the issue refers to it as
  // `cloud.gce.resource`. Pylon accepts either spelling and normalizes to this
  // canonical kind.
  | "cloud.gce.resource_usage_receipt"

export const CLOUD_WORKROOM_EVENT_KINDS: readonly CloudWorkroomEventKind[] = [
  "queued",
  "started",
  "log",
  "redacted",
  "artifact",
  "receipt",
  "cleanup",
  "completed",
  "failed",
  "timeout",
  "cancelled",
  "placement.bound",
  "cloud.gce.provisioned",
  "cloud.gce.cleanup",
  "cloud.gce.degraded",
  "cloud.gce.resource_usage_receipt",
]

export type CloudWorkroomEvent = {
  kind: CloudWorkroomEventKind
  // Optional cloud-native discriminator. GCE lifecycle events arrive here while
  // `kind` carries the broader workroom bucket.
  type?: string
  // Bounded human-readable summary (the cloud side redacts token-like content).
  summary?: string
  // Artifact refs surfaced by `artifact` events.
  artifactRefs?: string[]
  // Receipt refs surfaced by `receipt` events (the
  // `openagents.resource_usage_receipt.v1` digest ref).
  receiptRefs?: string[]
  // True when the cloud side redacted the event payload.
  redacted?: boolean
}

export type CloudPlacementAck = {
  binding: CloudRunnerBinding
  externalRunId: string
  status: string
  events: CloudWorkroomEvent[]
}

export type CloudRunEventsPage = {
  status: string
  events: CloudWorkroomEvent[]
  cursor: number
}

export const CLOUD_TERMINAL_EVENT_KINDS: readonly CloudWorkroomEventKind[] = [
  "completed",
  "failed",
  "timeout",
  "cancelled",
]

export function isCloudTerminalEventKind(
  kind: CloudWorkroomEventKind,
): kind is CloudTerminalKind {
  return (CLOUD_TERMINAL_EVENT_KINDS as readonly string[]).includes(kind)
}

// #5005 — the four cloud GCE lease-lifecycle discriminators. These are
// NON-terminal: a VM provisioned/released/degraded or a usage receipt does not
// end the session. The session terminal kinds stay
// completed/failed/timeout/cancelled.
export const CLOUD_GCE_EVENT_KINDS: readonly CloudWorkroomEventKind[] = [
  "cloud.gce.provisioned",
  "cloud.gce.cleanup",
  "cloud.gce.degraded",
  "cloud.gce.resource_usage_receipt",
]

export function isCloudGceEventKind(kind: string): kind is CloudWorkroomEventKind {
  return (CLOUD_GCE_EVENT_KINDS as readonly string[]).includes(kind)
}

// Normalize a raw cloud event discriminator (which may arrive on either the
// `type` field — the cloud `JobEvent.type` — or the broad workroom `kind`
// field) into a single canonical `CloudWorkroomEventKind`. The `cloud.gce.*`
// discriminator wins when present so VM lifecycle/receipt provenance is mapped
// even though the broad workroom `kind` reuses existing kinds
// (`started`/`receipt`/`cleanup`/`log`). The `cloud.gce.resource` spelling used
// by the issue is accepted as an alias for the canonical
// `cloud.gce.resource_usage_receipt`.
export function resolveCloudEventKind(
  kindField: unknown,
  typeField: unknown,
): CloudWorkroomEventKind | null {
  const candidates = [typeField, kindField]
  for (const raw of candidates) {
    if (typeof raw !== "string") continue
    if (raw === "cloud.gce.resource" || raw === "cloud.gce.resource_usage_receipt") {
      return "cloud.gce.resource_usage_receipt"
    }
    if (isCloudGceEventKind(raw)) return raw
  }
  // Fall back to the broad workroom kind when no GCE discriminator is present.
  return typeof kindField === "string" ? (kindField as CloudWorkroomEventKind) : null
}

// Map a Pylon control-session lane to the cloud compute lane. `local` never
// reaches the cloud client (the caller routes it locally). `auto` is sent as
// `auto` so the cloud applies its own GCE-primary / SHC-secondary policy.
export function cloudLaneForControlLane(
  lane: "auto" | "local" | "cloud-gcp" | "cloud-shc",
): CloudComputeLane {
  switch (lane) {
    case "cloud-gcp":
      return "cloud-gcp"
    case "cloud-shc":
      return "cloud-shc"
    case "auto":
    case "local":
    default:
      // `auto` defaults to the cloud policy (GCE primary). `local` should never
      // be dispatched to the cloud, but defaulting to `auto` here is safe — the
      // caller is responsible for not routing `local` to the cloud client.
      return "auto"
  }
}

export interface CloudControlClient {
  // Place a run and return the runner binding + initial ack events.
  placeRun: (assignment: CloudPlacementAssignment) => Promise<CloudPlacementAck>
  // Fetch events after `cursor` for an external run id.
  fetchEvents: (
    externalRunId: string,
    cursor: number,
  ) => Promise<CloudRunEventsPage>
  // Best-effort cancellation of an external run.
  cancelRun: (externalRunId: string) => Promise<void>
}

function authHeaders(config: CloudControlConfig): Record<string, string> {
  return {
    "Content-Type": "application/json",
    Authorization: `Bearer ${config.bearerToken}`,
  }
}

function normalizeBinding(raw: unknown, fallbackRunId: string): CloudRunnerBinding {
  const record = (raw ?? {}) as Record<string, unknown>
  const lane = record.lane === "cloud-shc" ? "cloud-shc" : "cloud-gcp"
  const providerLane = record.providerLane === "shc" ? "shc" : "gcp"
  return {
    contractVersion:
      typeof record.contractVersion === "string"
        ? record.contractVersion
        : CLOUD_PLACEMENT_CONTRACT_VERSION,
    runId: typeof record.runId === "string" ? record.runId : fallbackRunId,
    externalRunId:
      typeof record.externalRunId === "string" ? record.externalRunId : "",
    lane,
    providerLane,
    runnerId: typeof record.runnerId === "string" ? record.runnerId : "",
    capacityClassId:
      typeof record.capacityClassId === "string" ? record.capacityClassId : null,
    sandboxMode:
      typeof record.sandboxMode === "string"
        ? record.sandboxMode
        : "danger_full_access",
    reason: typeof record.reason === "string" ? record.reason : "",
    costDriven: record.costDriven === true,
    ...(record.caps && typeof record.caps === "object"
      ? { caps: record.caps as Record<string, unknown> }
      : {}),
  }
}

function normalizeEvent(raw: unknown): CloudWorkroomEvent | null {
  if (raw === null || typeof raw !== "object") return null
  const record = raw as Record<string, unknown>
  // #5005 — resolve the canonical kind from the `cloud.gce.*` `type`
  // discriminator when present, otherwise from the broad workroom `kind`.
  const kind = resolveCloudEventKind(record.kind, record.type)
  if (kind === null) return null
  const summary =
    typeof record.summary === "string" ? record.summary : undefined
  const artifactRefs = Array.isArray(record.artifactRefs)
    ? record.artifactRefs.filter((entry): entry is string => typeof entry === "string")
    : undefined
  const receiptRefs = Array.isArray(record.receiptRefs)
    ? record.receiptRefs.filter((entry): entry is string => typeof entry === "string")
    : undefined
  return {
    kind,
    ...(summary === undefined ? {} : { summary }),
    ...(artifactRefs === undefined ? {} : { artifactRefs }),
    ...(receiptRefs === undefined ? {} : { receiptRefs }),
    ...(record.redacted === true ? { redacted: true } : {}),
  }
}

function normalizeEvents(raw: unknown): CloudWorkroomEvent[] {
  if (!Array.isArray(raw)) return []
  const events: CloudWorkroomEvent[] = []
  for (const entry of raw) {
    const normalized = normalizeEvent(entry)
    if (normalized) events.push(normalized)
  }
  return events
}

export function makeCloudControlClient(
  config: CloudControlConfig,
  fetchImpl: typeof fetch = fetch,
): CloudControlClient {
  return {
    placeRun: async (assignment) => {
      const response = await fetchImpl(`${config.baseUrl}/v1/placement`, {
        method: "POST",
        headers: authHeaders(config),
        body: JSON.stringify(assignment),
      })
      if (!response.ok) {
        throw new Error(`cloud placement failed with HTTP ${response.status}`)
      }
      const payload = (await response.json()) as Record<string, unknown>
      const externalRunId =
        typeof payload.externalRunId === "string" ? payload.externalRunId : ""
      return {
        binding: normalizeBinding(payload.binding, assignment.run_id),
        externalRunId,
        status: typeof payload.status === "string" ? payload.status : "queued",
        events: normalizeEvents(payload.events),
      }
    },
    fetchEvents: async (externalRunId, cursor) => {
      const url = `${config.baseUrl}/v1/codex-runs/${encodeURIComponent(
        externalRunId,
      )}/events?cursor=${cursor}`
      const response = await fetchImpl(url, {
        method: "GET",
        headers: authHeaders(config),
      })
      if (!response.ok) {
        throw new Error(`cloud events fetch failed with HTTP ${response.status}`)
      }
      const payload = (await response.json()) as Record<string, unknown>
      const events = normalizeEvents(payload.events)
      const nextCursor =
        typeof payload.cursor === "number" ? payload.cursor : cursor + events.length
      return {
        status: typeof payload.status === "string" ? payload.status : "running",
        events,
        cursor: nextCursor,
      }
    },
    cancelRun: async (externalRunId) => {
      const url = `${config.baseUrl}/v1/codex-runs/${encodeURIComponent(
        externalRunId,
      )}/cancel`
      try {
        await fetchImpl(url, { method: "POST", headers: authHeaders(config) })
      } catch {
        // Best-effort: cancellation failures must not crash the local session.
      }
    },
  }
}
