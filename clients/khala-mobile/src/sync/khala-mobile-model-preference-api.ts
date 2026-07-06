/**
 * MM-F1 (#8484, merged 8f38922fc4) landed while this lane was mid-flight on
 * Settings (#8487/#8480) — client for the real, shipped
 * `GET/PUT /api/mobile/model-preference` route
 * (`apps/openagents.com/workers/api/src/inference/model-preference-store.ts`).
 * Unlike the credits balance/history endpoints (still proposed, not built),
 * this one exists today, so the Models section wires against it directly
 * rather than stubbing.
 */

export type KhalaModelPreferenceFallback =
  | "none"
  | "no_preference_set"
  | "preference_unavailable"
  | "default_unavailable"

export type KhalaModelPreference = Readonly<{
  availableModelIds: ReadonlyArray<string>
  effectiveModelId: string | null
  fallback: KhalaModelPreferenceFallback
  preferredModelId: string | null
  updatedAt: string | null
  usedPreference: boolean
}>

export type KhalaModelPreferenceFetchLike = (
  url: string,
  init: { body?: string; headers: Record<string, string>; method: string },
) => Promise<{ json: () => Promise<unknown>; ok: boolean; status?: number }>

export type KhalaModelPreferenceResult =
  | Readonly<{ ok: true; value: KhalaModelPreference }>
  | Readonly<{ availableModelIds: ReadonlyArray<string>; kind: "model_unavailable"; ok: false }>
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
  return {
    availableModelIds: record.availableModelIds as ReadonlyArray<string>,
    effectiveModelId: typeof record.effectiveModelId === "string" ? record.effectiveModelId : null,
    fallback: record.fallback as KhalaModelPreferenceFallback,
    preferredModelId: typeof record.preferredModelId === "string" ? record.preferredModelId : null,
    updatedAt: typeof record.updatedAt === "string" ? record.updatedAt : null,
    usedPreference: record.usedPreference,
  }
}

export const fetchKhalaMobileModelPreference = async (
  apiBaseUrl: string,
  token: string,
  fetchImpl: KhalaModelPreferenceFetchLike = fetch,
): Promise<KhalaModelPreferenceResult> => {
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
  modelId: string,
  fetchImpl: KhalaModelPreferenceFetchLike = fetch,
): Promise<KhalaModelPreferenceResult> => {
  try {
    const response = await fetchImpl(`${apiBaseUrl.replace(/\/$/, "")}/api/mobile/model-preference`, {
      body: JSON.stringify({ modelId }),
      headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
      method: "PUT",
    })
    const body = await response.json()
    if (response.status === 409) {
      const record = body as { availableModelIds?: unknown }
      const availableModelIds = Array.isArray(record.availableModelIds)
        ? record.availableModelIds.filter((id): id is string => typeof id === "string")
        : []
      return { availableModelIds, kind: "model_unavailable", ok: false }
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
