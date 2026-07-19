// Fetch client for the server's mobile push device-token registry
// (SARAH-PUSH-1 #9062). Mirrors the pure fetch-client shape already
// established by `../sarah/sarah-client.ts` and
// `../sarah/sarah-speech-client.ts`: an explicit `{ baseUrl, accessToken,
// fetch? }` input, no ambient config, no native imports, fully injectable for
// tests. This module never touches `expo-notifications` or SecureStore — see
// `./expo-push-device-registration.ts` for the orchestration that calls it.
//
// The server contract lives in
// `apps/openagents.com/workers/api/src/push/push-device-token-routes.ts`
// (`PUSH_DEVICE_TOKENS_PATH`, `POST|DELETE /api/mobile/push-tokens`,
// mobile-bearer-authorized). This module intentionally re-declares the exact
// path/shape rather than importing the server route module (a different
// Worker package), matching how `SARAH_OWNER_MOBILE_PATH` and
// `SARAH_SPEECH_MOBILE_PATH` are re-declared client-side.

export const PUSH_DEVICE_TOKENS_PATH = "/api/mobile/push-tokens"

export type PushPlatform = "ios" | "android"

export type RegisterPushDeviceTokenInput = Readonly<{
  baseUrl: string
  accessToken: string
  deviceId: string
  expoPushToken: string
  platform: PushPlatform
  fetch?: typeof fetch
}>

export type PushDeviceTokenRegistrationResult = Readonly<
  | { state: "registered"; platform: PushPlatform; updatedAt: string }
  | { state: "unauthorized" | "invalid_request" | "unavailable" }
>

/** `POST /api/mobile/push-tokens` — registers (or upserts, per the server's
 * `(user_id, device_id)` conflict target) this device's current Expo push
 * token. Never throws: every failure mode is a typed result. */
export const registerPushDeviceTokenRemote = async (
  input: RegisterPushDeviceTokenInput,
): Promise<PushDeviceTokenRegistrationResult> => {
  try {
    const response = await (input.fetch ?? fetch)(
      new URL(PUSH_DEVICE_TOKENS_PATH, input.baseUrl),
      {
        method: "POST",
        headers: {
          authorization: `Bearer ${input.accessToken}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({
          deviceId: input.deviceId,
          expoPushToken: input.expoPushToken,
          platform: input.platform,
        }),
      },
    )
    if (response.status === 401) return { state: "unauthorized" }
    if (response.status === 400) return { state: "invalid_request" }
    if (!response.ok) return { state: "unavailable" }
    const body = (await response.json()) as {
      ok?: unknown
      registration?: { platform?: unknown; updatedAt?: unknown }
    }
    if (body.ok !== true || body.registration === undefined) return { state: "unavailable" }
    const platform: PushPlatform = body.registration.platform === "android" ? "android" : "ios"
    const updatedAt = typeof body.registration.updatedAt === "string" ? body.registration.updatedAt : ""
    return { state: "registered", platform, updatedAt }
  } catch {
    return { state: "unavailable" }
  }
}

export type UnregisterPushDeviceTokenInput = Readonly<{
  baseUrl: string
  accessToken: string
  deviceId: string
  fetch?: typeof fetch
}>

export type PushDeviceTokenRemovalResult = Readonly<
  | { state: "removed" | "not_found" }
  | { state: "unauthorized" | "unavailable" }
>

/** `DELETE /api/mobile/push-tokens?deviceId=...` — removes this device's
 * registration. Never throws: every failure mode is a typed result. */
export const unregisterPushDeviceTokenRemote = async (
  input: UnregisterPushDeviceTokenInput,
): Promise<PushDeviceTokenRemovalResult> => {
  try {
    const url = new URL(PUSH_DEVICE_TOKENS_PATH, input.baseUrl)
    url.searchParams.set("deviceId", input.deviceId)
    const response = await (input.fetch ?? fetch)(url, {
      method: "DELETE",
      headers: { authorization: `Bearer ${input.accessToken}` },
    })
    if (response.status === 401) return { state: "unauthorized" }
    if (!response.ok) return { state: "unavailable" }
    const body = (await response.json()) as { ok?: unknown; removed?: unknown }
    if (body.ok !== true) return { state: "unavailable" }
    return { state: body.removed === true ? "removed" : "not_found" }
  } catch {
    return { state: "unavailable" }
  }
}
