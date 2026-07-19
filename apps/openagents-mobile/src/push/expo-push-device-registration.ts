// Mobile Expo push-token device registration (SARAH-PUSH-1 #9062).
//
// Closes the gap documented in
// `docs/sarah/2026-07-19-sarah-activation-gap-analysis.md` §5 Gap A: no
// mobile code ever called `Notifications.getExpoPushTokenAsync()` or
// `POST /api/mobile/push-tokens`, so the server's device-token registry was
// always empty and no push could ever be delivered.
//
// Split the same way `../auth/native-session-vault.ts` splits secure-store
// custody: every exported function below takes its native dependencies
// (`store`, `randomId`, `getExpoPushTokenAsync`, permission state) as
// explicit parameters with no default, so the core registration/rotation/
// removal logic is fully unit-testable with fakes and never touches
// `expo-notifications`, `expo-secure-store`, `expo-crypto`, or `react-native`
// directly. `openExpoPushDeviceRegistration()` at the bottom is the thin
// native shell — it lazily `require()`s those native modules (never at
// module load time) and is the only part of this file real app code should
// call.

import { mapNotificationPermission } from "../settings/mobile-settings"
import { loadNativeSessionCredential } from "../auth/native-session-vault"
import {
  registerPushDeviceTokenRemote,
  unregisterPushDeviceTokenRemote,
  type PushPlatform,
} from "./push-token-client"

declare const require: (id: string) => unknown

/** No EAS project has been linked yet for `apps/openagents-mobile` — there is
 * no `eas.json` and no `expo.extra.eas.projectId` in `app.json` (verified
 * 2026-07-19). This is the SAME gap `docs/khala-mobile/2026-07-06-app-store-
 * submission-pack.md` already flagged: "Currently non-functional in practice
 * because the Expo project id is unset ... registration no-ops with
 * `project_id_missing` until that is linked." Once an owner links a real EAS
 * project (and this app takes on an `expo-constants` dependency to read
 * `Constants.expoConfig?.extra?.eas?.projectId`, or the id is inlined here
 * directly), replace this literal. Until then, every sync call below
 * deliberately returns `{ state: "skipped", reason: "project_id_missing" }`
 * instead of guessing an id or letting `getExpoPushTokenAsync` throw. */
export const OPENAGENTS_MOBILE_EAS_PROJECT_ID: string | null = null

export const OPENAGENTS_MOBILE_PUSH_BASE_URL = "https://openagents.com"

const PUSH_DEVICE_ID_KEY = "openagents.mobile.push.device-id.v1"
const PUSH_REGISTRATION_RECORD_KEY = "openagents.mobile.push.last-registered.v1"

export type SecureStoreLike = Readonly<{
  getItemAsync: (key: string) => Promise<string | null>
  setItemAsync: (key: string, value: string) => Promise<void>
  deleteItemAsync: (key: string) => Promise<void>
}>

/** Read-only subset `readPushDeviceRegistrationRecord` needs, so callers that
 * only ever read (like the notification-settings health snapshot) don't have
 * to satisfy the full read/write/delete `SecureStoreLike` shape. */
export type PushRegistrationReadStore = Readonly<{
  getItemAsync: (key: string) => Promise<string | null>
}>

export type PushDeviceRegistrationRecord = Readonly<{
  deviceId: string
  expoPushToken: string
}>

export type PushRegistrationOutcome = Readonly<
  | { state: "registered"; rotated: boolean }
  | { state: "skipped"; reason: "permission_not_granted" | "project_id_missing" | "signed_out" }
  | { state: "failed"; reason: "unauthorized" | "unavailable" }
>

export type PushRegistrationRemovalOutcome = Readonly<
  | { state: "removed" | "not_registered" }
  | { state: "skipped"; reason: "signed_out" }
  | { state: "failed"; reason: "unauthorized" | "unavailable" }
>

/** Stable per-installation device id, created once and persisted forever
 * (independent of sign-in/out — it names the installation, not the session).
 * Creates one on first read so registration always has something stable to
 * upsert against; never call this from a path that should NOT create a
 * device identity as a side effect (use `peekPushDeviceId` there instead). */
export const resolvePushDeviceId = async (
  store: SecureStoreLike,
  randomId: () => string,
): Promise<string> => {
  const existing = (await store.getItemAsync(PUSH_DEVICE_ID_KEY))?.trim() ?? ""
  if (existing !== "") return existing
  const created = randomId()
  await store.setItemAsync(PUSH_DEVICE_ID_KEY, created)
  return created
}

const peekPushDeviceId = async (store: SecureStoreLike): Promise<string | null> => {
  const raw = (await store.getItemAsync(PUSH_DEVICE_ID_KEY))?.trim() ?? ""
  return raw === "" ? null : raw
}

/** The last `{ deviceId, expoPushToken }` pair this installation successfully
 * registered with the server, or `null` if it never has (or the record was
 * cleared after a sign-out removal). Also used by
 * `../settings/expo-mobile-notification-settings.ts`'s health snapshot to
 * report the ACTUAL server-side registration state, not merely whether the OS
 * handed the app a native token. */
export const readPushDeviceRegistrationRecord = async (
  store: PushRegistrationReadStore,
): Promise<PushDeviceRegistrationRecord | null> => {
  try {
    const raw = await store.getItemAsync(PUSH_REGISTRATION_RECORD_KEY)
    if (raw === null) return null
    const value = JSON.parse(raw) as Record<string, unknown>
    return typeof value.deviceId === "string" && value.deviceId.trim() !== "" &&
      typeof value.expoPushToken === "string" && value.expoPushToken.trim() !== ""
      ? { deviceId: value.deviceId, expoPushToken: value.expoPushToken }
      : null
  } catch {
    return null
  }
}

const writePushDeviceRegistrationRecord = async (
  store: SecureStoreLike,
  record: PushDeviceRegistrationRecord,
): Promise<void> => {
  await store.setItemAsync(PUSH_REGISTRATION_RECORD_KEY, JSON.stringify(record))
}

const clearPushDeviceRegistrationRecord = async (store: SecureStoreLike): Promise<void> => {
  await store.deleteItemAsync(PUSH_REGISTRATION_RECORD_KEY)
}

export type SyncPushDeviceRegistrationInput = Readonly<{
  baseUrl: string
  accessToken: string
  /** `null` when no EAS project is linked yet — see
   * `OPENAGENTS_MOBILE_EAS_PROJECT_ID` above. */
  projectId: string | null
  platform: PushPlatform
  permission: Readonly<{ granted: boolean; canAskAgain: boolean }>
  store: SecureStoreLike
  randomId: () => string
  getExpoPushTokenAsync: (
    options: Readonly<{ projectId: string }>,
  ) => Promise<Readonly<{ data: string }>>
  fetch?: typeof fetch
}>

/** Core registration + rotation logic, fully injectable and native-free. Never
 * throws: every failure mode is a typed `PushRegistrationOutcome`.
 *
 * - No-ops (never calls `getExpoPushTokenAsync` or the network) unless
 *   notification permission is currently granted AND an EAS project id is
 *   configured.
 * - Skips the network call entirely when the freshly-fetched Expo push token
 *   for this device id already matches the last successfully registered
 *   pair (no server round trip needed).
 * - Registers (or re-registers, on rotation) otherwise, and persists the new
 *   pair locally only after the server confirms it. */
export const syncPushDeviceRegistration = async (
  input: SyncPushDeviceRegistrationInput,
): Promise<PushRegistrationOutcome> => {
  if (mapNotificationPermission(input.permission) !== "granted") {
    return { state: "skipped", reason: "permission_not_granted" }
  }

  const projectId = input.projectId?.trim() ?? ""
  if (projectId === "") {
    return { state: "skipped", reason: "project_id_missing" }
  }

  let expoPushToken: string
  try {
    const token = await input.getExpoPushTokenAsync({ projectId })
    expoPushToken = token.data.trim()
    if (expoPushToken === "") return { state: "failed", reason: "unavailable" }
  } catch {
    return { state: "failed", reason: "unavailable" }
  }

  const deviceId = await resolvePushDeviceId(input.store, input.randomId)
  const existing = await readPushDeviceRegistrationRecord(input.store)
  if (existing !== null && existing.deviceId === deviceId && existing.expoPushToken === expoPushToken) {
    return { state: "registered", rotated: false }
  }

  const result = await registerPushDeviceTokenRemote({
    baseUrl: input.baseUrl,
    accessToken: input.accessToken,
    deviceId,
    expoPushToken,
    platform: input.platform,
    fetch: input.fetch,
  })

  if (result.state === "registered") {
    await writePushDeviceRegistrationRecord(input.store, { deviceId, expoPushToken })
    return { state: "registered", rotated: existing !== null }
  }
  if (result.state === "unauthorized") return { state: "failed", reason: "unauthorized" }
  return { state: "failed", reason: "unavailable" }
}

export type RemovePushDeviceRegistrationInput = Readonly<{
  baseUrl: string
  accessToken: string
  store: SecureStoreLike
  fetch?: typeof fetch
}>

/** Core removal logic, fully injectable and native-free. Safe to call even
 * when this device was never registered (no-ops, no network call). Clears the
 * local "last registered" record only once the server confirms removal, so a
 * failed removal leaves local state honestly pointing at what the server
 * still has. */
export const removePushDeviceRegistration = async (
  input: RemovePushDeviceRegistrationInput,
): Promise<PushRegistrationRemovalOutcome> => {
  const deviceId = await peekPushDeviceId(input.store)
  if (deviceId === null) return { state: "not_registered" }

  const result = await unregisterPushDeviceTokenRemote({
    baseUrl: input.baseUrl,
    accessToken: input.accessToken,
    deviceId,
    fetch: input.fetch,
  })

  if (result.state === "unauthorized") return { state: "failed", reason: "unauthorized" }
  if (result.state === "unavailable") return { state: "failed", reason: "unavailable" }
  await clearPushDeviceRegistrationRecord(input.store)
  return { state: "removed" }
}

export type PushDeviceRegistrationPort = Readonly<{
  /** Attempts registration if a session is present. Safe to call whether or
   * not permission is currently granted — no-ops typed `skipped` otherwise. */
  syncOnSignIn: () => Promise<PushRegistrationOutcome>
  /** Same attempt, for the moment permission is newly granted from the
   * settings flow (independent of sign-in timing). */
  syncOnPermissionGranted: () => Promise<PushRegistrationOutcome>
  /** Deregisters this device from the server. Call BEFORE clearing the local
   * session credential (e.g. before `signOutNativeSession()`) — it needs a
   * still-valid bearer token to authenticate the removal call. */
  removeOnSignOut: () => Promise<PushRegistrationRemovalOutcome>
}>

/** The native shell. Lazily `require()`s `expo-notifications`,
 * `expo-secure-store`, `expo-crypto`, and `react-native` only when its
 * returned methods are actually invoked — importing this module (as opposed
 * to calling this factory) never touches a native module, matching
 * `../settings/expo-mobile-notification-settings.ts`'s
 * `openExpoMobileNotificationSettings()` convention. */
export const openExpoPushDeviceRegistration = (
  baseUrl: string = OPENAGENTS_MOBILE_PUSH_BASE_URL,
): PushDeviceRegistrationPort => {
  const Notifications = require("expo-notifications") as typeof import("expo-notifications")
  const SecureStore = require("expo-secure-store") as SecureStoreLike
  const { randomUUID } = require("expo-crypto") as typeof import("expo-crypto")
  const { Platform } = require("react-native") as typeof import("react-native")
  const platform: PushPlatform = Platform.OS === "android" ? "android" : "ios"

  const sync = async (): Promise<PushRegistrationOutcome> => {
    const credential = await loadNativeSessionCredential()
    if (credential === null) return { state: "skipped", reason: "signed_out" }
    const permission = await Notifications.getPermissionsAsync()
    return syncPushDeviceRegistration({
      baseUrl,
      accessToken: credential.accessToken,
      projectId: OPENAGENTS_MOBILE_EAS_PROJECT_ID,
      platform,
      permission,
      store: SecureStore,
      randomId: randomUUID,
      getExpoPushTokenAsync: options => Notifications.getExpoPushTokenAsync(options),
    })
  }

  return {
    syncOnSignIn: sync,
    syncOnPermissionGranted: sync,
    removeOnSignOut: async () => {
      const credential = await loadNativeSessionCredential()
      if (credential === null) return { state: "skipped", reason: "signed_out" }
      return removePushDeviceRegistration({
        baseUrl,
        accessToken: credential.accessToken,
        store: SecureStore,
      })
    },
  }
}
