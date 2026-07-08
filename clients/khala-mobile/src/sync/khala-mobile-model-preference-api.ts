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

export type KhalaModelPreference = Readonly<{
  availableModelIds: ReadonlyArray<string>
  availableTargetIds: ReadonlyArray<string>
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
