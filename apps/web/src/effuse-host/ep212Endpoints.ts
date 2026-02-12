import type { WorkerEnv } from "./env"

export type Ep212EndpointPreset = "A" | "B"

export const isEp212EndpointPreset = (value: unknown): value is Ep212EndpointPreset =>
  value === "A" || value === "B"

export type Ep212PresetResolution =
  | Readonly<{ readonly ok: true; readonly preset: Ep212EndpointPreset; readonly url: string }>
  | Readonly<{ readonly ok: false; readonly errorCode: "missing_env" | "invalid_env_url"; readonly message: string }>

const envVarForPreset = (preset: Ep212EndpointPreset): "OA_EP212_ENDPOINT_A_URL" | "OA_EP212_ENDPOINT_B_URL" =>
  preset === "A" ? "OA_EP212_ENDPOINT_A_URL" : "OA_EP212_ENDPOINT_B_URL"

export const resolveEp212PresetUrl = (preset: Ep212EndpointPreset, env: WorkerEnv): Ep212PresetResolution => {
  const envVar = envVarForPreset(preset)
  const raw = env[envVar]
  if (typeof raw !== "string" || raw.trim().length === 0) {
    return {
      ok: false,
      errorCode: "missing_env",
      message: `Missing env var ${envVar} for EP212 endpoint preset ${preset}`,
    }
  }

  try {
    const parsed = new URL(raw.trim())
    if (parsed.username || parsed.password) {
      return {
        ok: false,
        errorCode: "invalid_env_url",
        message: `EP212 endpoint URL for ${envVar} must not include userinfo`,
      }
    }
    return { ok: true, preset, url: parsed.toString() }
  } catch {
    return {
      ok: false,
      errorCode: "invalid_env_url",
      message: `Invalid URL in env var ${envVar} for EP212 endpoint preset ${preset}`,
    }
  }
}

export const sanitizeLightningHeadersForTask = (
  headers: Record<string, string> | undefined,
): Record<string, string> | undefined => {
  if (!headers) return undefined

  const allowlist = new Set(["accept", "content-type"])
  const next: Record<string, string> = {}

  for (const [k, v] of Object.entries(headers)) {
    const key = k.trim().toLowerCase()
    if (!allowlist.has(key)) continue
    const value = String(v ?? "").trim()
    if (!value) continue
    next[k] = value.slice(0, 1024)
  }

  return Object.keys(next).length > 0 ? next : undefined
}

