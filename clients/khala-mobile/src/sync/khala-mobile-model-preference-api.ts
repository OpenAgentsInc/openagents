/**
 * MM-F1 (#8484, merged 8f38922fc4) landed while this lane was mid-flight on
 * Settings (#8487/#8480) — client for the real, shipped
 * `GET/PUT /api/mobile/model-preference` route
 * (`apps/openagents.com/workers/api/src/inference/model-preference-store.ts`).
 * Unlike the credits balance/history endpoints (still proposed, not built),
 * this one exists today, so the Models section wires against it directly
 * rather than stubbing.
 */

import { demoModelPreference, isDemoToken } from "../demo/demo-fixtures"

export type KhalaModelPreferenceFallback =
  | "none"
  | "no_preference_set"
  | "preference_unavailable"
  | "default_unavailable"

/** CX-4 (#8548): a connected Codex/Claude account, projected down to exactly
 * what the picker needs (never a secret). `accountRefHash` is the opaque,
 * non-secret ref the server already uses to build `codex:<accountRefHash>` /
 * `claude:<accountRefHash>` execution-target ids. */
export type KhalaModelPreferenceAccountSummary = Readonly<{
  accountRefHash: string
  label: string
  ready: boolean
  reason?: KhalaAutoExecutionTargetFallbackReason | undefined
}>

export type KhalaAutoExecutionTargetFallbackReason =
  | "account_exhausted"
  | "account_rate_limited"
  | "account_requires_reauth"
  | "account_unavailable"

export type KhalaAutoExecutionTargetFallbackEvent = Readonly<{
  type: KhalaAutoExecutionTargetFallbackReason
  targetId: string
  nextTargetId: string | null
}>

/** The typed, never-silent answer to "what would `auto` do right now" —
 * mirrors the server's `AutoExecutionTargetResolution`
 * (`inference/model-preference-store.ts`). `events` is non-empty whenever
 * `auto` skipped a connected account; the UI renders it, never swaps quietly. */
export type KhalaAutoExecutionTargetResolution = Readonly<{
  effectiveTargetId: string | null
  usedFallback: boolean
  events: ReadonlyArray<KhalaAutoExecutionTargetFallbackEvent>
}>

export type KhalaModelPreference = Readonly<{
  availableModelIds: ReadonlyArray<string>
  availableTargetIds: ReadonlyArray<string>
  // Optional (CX-4, #8548, added after MM-F1 shipped): absent/undefined on
  // older cached shapes or hand-built test fixtures means "not computed",
  // treated the same as `null`/empty by every consumer
  // (`buildExecutionTargetOptions`, `autoResolutionNoticeMessage`) —
  // never a silent crash on a pre-CX-4 shape.
  autoResolution?: KhalaAutoExecutionTargetResolution | null | undefined
  claudeAccounts?: ReadonlyArray<KhalaModelPreferenceAccountSummary> | undefined
  codexAccounts?: ReadonlyArray<KhalaModelPreferenceAccountSummary> | undefined
  effectiveModelId: string | null
  effectiveTargetId: string | null
  fallback: KhalaModelPreferenceFallback
  preferredModelId: string | null
  preferredTargetId: string | null
  updatedAt: string | null
  usedPreference: boolean
}>

export type KhalaModelPreferenceFetchLike = (
  url: string,
  init: { body?: string; headers: Record<string, string>; method: string },
) => Promise<{ json: () => Promise<unknown>; ok: boolean; status?: number }>

export type KhalaModelPreferenceResult =
  | Readonly<{ ok: true; value: KhalaModelPreference }>
  | Readonly<{
      availableModelIds: ReadonlyArray<string>
      availableTargetIds: ReadonlyArray<string>
      kind: "target_unavailable"
      ok: false
    }>
  | Readonly<{ kind: "unauthorized" | "bad_request" | "unavailable" | "unknown"; ok: false }>

const FALLBACK_VALUES: ReadonlySet<string> = new Set([
  "none",
  "no_preference_set",
  "preference_unavailable",
  "default_unavailable",
])

const FALLBACK_REASON_VALUES: ReadonlySet<string> = new Set([
  "account_exhausted",
  "account_rate_limited",
  "account_requires_reauth",
  "account_unavailable",
])

const parseAccountSummary = (value: unknown): KhalaModelPreferenceAccountSummary | null => {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  if (
    typeof record.accountRefHash !== "string" ||
    typeof record.label !== "string" ||
    typeof record.ready !== "boolean"
  ) {
    return null
  }
  const reason = typeof record.reason === "string" && FALLBACK_REASON_VALUES.has(record.reason) ? record.reason : undefined
  return {
    accountRefHash: record.accountRefHash,
    label: record.label,
    ready: record.ready,
    reason: reason as KhalaAutoExecutionTargetFallbackReason | undefined,
  }
}

const parseAccountSummaries = (value: unknown): ReadonlyArray<KhalaModelPreferenceAccountSummary> => {
  if (!Array.isArray(value)) return []
  const parsed = value.map(parseAccountSummary)
  return parsed.every((account): account is KhalaModelPreferenceAccountSummary => account !== null) ? parsed : []
}

const parseAutoResolution = (value: unknown): KhalaAutoExecutionTargetResolution | null => {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  if (typeof record.usedFallback !== "boolean" || !Array.isArray(record.events)) return null
  const events: Array<KhalaAutoExecutionTargetFallbackEvent> = []
  for (const raw of record.events) {
    if (raw === null || typeof raw !== "object") return null
    const event = raw as Record<string, unknown>
    if (
      typeof event.type !== "string" ||
      !FALLBACK_REASON_VALUES.has(event.type) ||
      typeof event.targetId !== "string" ||
      (typeof event.nextTargetId !== "string" && event.nextTargetId !== null)
    ) {
      return null
    }
    events.push({
      nextTargetId: event.nextTargetId,
      targetId: event.targetId,
      type: event.type as KhalaAutoExecutionTargetFallbackReason,
    })
  }
  return {
    effectiveTargetId: typeof record.effectiveTargetId === "string" ? record.effectiveTargetId : null,
    events,
    usedFallback: record.usedFallback,
  }
}

const parsePreference = (body: unknown): KhalaModelPreference | null => {
  if (body === null || typeof body !== "object") return null
  const record = body as Record<string, unknown>
  if (
    !Array.isArray(record.availableModelIds) ||
    !record.availableModelIds.every(id => typeof id === "string") ||
    typeof record.fallback !== "string" ||
    !FALLBACK_VALUES.has(record.fallback) ||
    typeof record.usedPreference !== "boolean"
  ) {
    return null
  }
  const availableModelIds = record.availableModelIds as ReadonlyArray<string>
  const effectiveModelId = typeof record.effectiveModelId === "string" ? record.effectiveModelId : null
  const preferredModelId = typeof record.preferredModelId === "string" ? record.preferredModelId : null
  const availableTargetIds =
    Array.isArray(record.availableTargetIds) && record.availableTargetIds.every(id => typeof id === "string")
      ? (record.availableTargetIds as ReadonlyArray<string>)
      : availableModelIds
  return {
    availableModelIds,
    availableTargetIds,
    autoResolution: "autoResolution" in record ? parseAutoResolution(record.autoResolution) : null,
    claudeAccounts: parseAccountSummaries(record.claudeAccounts),
    codexAccounts: parseAccountSummaries(record.codexAccounts),
    effectiveModelId,
    effectiveTargetId: typeof record.effectiveTargetId === "string" ? record.effectiveTargetId : effectiveModelId,
    fallback: record.fallback as KhalaModelPreferenceFallback,
    preferredModelId,
    preferredTargetId: typeof record.preferredTargetId === "string" ? record.preferredTargetId : preferredModelId,
    updatedAt: typeof record.updatedAt === "string" ? record.updatedAt : null,
    usedPreference: record.usedPreference,
  }
}

export const fetchKhalaMobileModelPreference = async (
  apiBaseUrl: string,
  token: string,
  fetchImpl: KhalaModelPreferenceFetchLike = fetch,
): Promise<KhalaModelPreferenceResult> => {
  if (isDemoToken(token)) return { ok: true, value: demoModelPreference() }
  try {
    const response = await fetchImpl(`${apiBaseUrl.replace(/\/$/, "")}/api/mobile/model-preference`, {
      headers: { authorization: `Bearer ${token}` },
      method: "GET",
    })
    if (response.status === 401) return { kind: "unauthorized", ok: false }
    if (!response.ok) return { kind: "unknown", ok: false }
    const parsed = parsePreference(await response.json())
    if (parsed === null) return { kind: "unknown", ok: false }
    return { ok: true, value: parsed }
  } catch {
    return { kind: "unavailable", ok: false }
  }
}

export const putKhalaMobileModelPreference = async (
  apiBaseUrl: string,
  token: string,
  targetId: string,
  fetchImpl: KhalaModelPreferenceFetchLike = fetch,
): Promise<KhalaModelPreferenceResult> => {
  if (isDemoToken(token)) return { ok: true, value: demoModelPreference(targetId) }
  try {
    const response = await fetchImpl(`${apiBaseUrl.replace(/\/$/, "")}/api/mobile/model-preference`, {
      body: JSON.stringify({ targetId }),
      headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
      method: "PUT",
    })
    const body = await response.json()
    if (response.status === 409) {
      const record = body as { availableModelIds?: unknown; availableTargetIds?: unknown }
      const availableModelIds = Array.isArray(record.availableModelIds)
        ? record.availableModelIds.filter((id): id is string => typeof id === "string")
        : []
      const availableTargetIds = Array.isArray(record.availableTargetIds)
        ? record.availableTargetIds.filter((id): id is string => typeof id === "string")
        : availableModelIds
      return { availableModelIds, availableTargetIds, kind: "target_unavailable", ok: false }
    }
    if (response.status === 400) return { kind: "bad_request", ok: false }
    if (response.status === 401) return { kind: "unauthorized", ok: false }
    if (!response.ok) return { kind: "unknown", ok: false }
    const parsed = parsePreference(body)
    if (parsed === null) return { kind: "unknown", ok: false }
    return { ok: true, value: parsed }
  } catch {
    return { kind: "unavailable", ok: false }
  }
}
