import * as Crypto from "expo-crypto"
import Constants from "expo-constants"
import * as Notifications from "expo-notifications"
import { Platform } from "react-native"

import { mobileProblemMessageSafe, readOkMobileJsonResponse } from "../network/mobile-problem"
import {
  clearPushDeviceId,
  loadHasEverPromptedForPush,
  loadOrCreatePushDeviceId,
  readPushDeviceIdIfPresent,
  saveHasEverPromptedForPush
} from "./push-device-store"
import {
  buildRegisterPushDeviceTokenBody,
  permissionOutcomeIsRegisterable,
  shouldPromptForPushPermission,
  type PushDispatchEvent,
  type PushPlatform
} from "./push-registration-core"

const PUSH_DEVICE_TOKENS_PATH = "/api/mobile/push-tokens"

const platform = (): PushPlatform => (Platform.OS === "android" ? "android" : "ios")

/** The Expo project id push tokens are minted against. Requires an Expo
 * account project (`expo.extra.eas.projectId` in app.json) even though this
 * app builds locally (no `eas build`) — Expo's push notification service
 * still routes by project id. See NEEDS_OWNER.md: this is currently unset,
 * so `registerForPushNotificationsAsync` no-ops with
 * `{ ok: false, reason: "project_id_missing" }` until an owner links one. */
const expoProjectId = (): string | undefined => {
  const extra = Constants.expoConfig?.extra as
    | { eas?: { projectId?: unknown } }
    | undefined
  const projectId = extra?.eas?.projectId
  return typeof projectId === "string" && projectId.trim().length > 0
    ? projectId.trim()
    : undefined
}

export type RegisterPushOutcome =
  | Readonly<{ ok: true; deviceId: string }>
  | Readonly<{
      ok: false
      reason: "permission_denied" | "project_id_missing" | "request_failed"
      messageSafe?: string
    }>

/**
 * Called from the composer's "start a brand-new turn" path (the "first task
 * dispatched" moment, see `khala_mobile.push.permission_prompt_on_first_task_dispatch.v1`).
 * Fire-and-forget from the caller's perspective: never throws, always
 * resolves to a typed outcome. Only ever prompts for OS permission once per
 * device (see `push-device-store.ts`); after that it silently re-registers
 * (refreshing the token server-side) on every dispatch if already granted,
 * and no-ops if previously denied.
 */
export const registerForPushNotificationsAsync = async (input: {
  apiBaseUrl: string
  bearerToken: string
  event: PushDispatchEvent
}): Promise<RegisterPushOutcome> => {
  try {
    const hasEverPrompted = await loadHasEverPromptedForPush()
    const willPrompt = shouldPromptForPushPermission({ hasEverPrompted }, input.event)

    const existing = await Notifications.getPermissionsAsync()
    let granted = existing.granted

    if (!granted && willPrompt) {
      const requested = await Notifications.requestPermissionsAsync()
      granted = requested.granted
      await saveHasEverPromptedForPush()
    } else if (!granted && !hasEverPrompted) {
      // Not our moment to prompt (e.g. an app-launch-triggered refresh call)
      // and never asked before — leave the OS prompt for the next real task
      // dispatch instead of asking here.
      return { ok: false, reason: "permission_denied" }
    }

    if (!permissionOutcomeIsRegisterable(granted ? "granted" : "denied")) {
      return { ok: false, reason: "permission_denied" }
    }

    const projectId = expoProjectId()
    if (projectId === undefined) {
      return { ok: false, reason: "project_id_missing" }
    }

    const deviceId = await loadOrCreatePushDeviceId({
      makeDeviceId: () => Crypto.randomUUID()
    })
    const { data: expoPushToken } = await Notifications.getExpoPushTokenAsync({ projectId })

    const base = input.apiBaseUrl.replace(/\/$/, "")
    const response = await fetch(`${base}${PUSH_DEVICE_TOKENS_PATH}`, {
      body: JSON.stringify(
        buildRegisterPushDeviceTokenBody({ deviceId, expoPushToken, platform: platform() })
      ),
      headers: {
        authorization: `Bearer ${input.bearerToken}`,
        "content-type": "application/json"
      },
      method: "POST"
    })
    await readOkMobileJsonResponse(response, "push token registration")

    return { ok: true, deviceId }
  } catch (error) {
    return {
      messageSafe: mobileProblemMessageSafe(error, "push token registration"),
      ok: false,
      reason: "request_failed"
    }
  }
}

/**
 * Sign-out cleanup: best-effort unregister of this device's token (never
 * throws — mirrors `deleteMobileOpenAuthSession`'s "local sign-out must
 * complete even if the network call fails" posture in
 * `../auth/khala-auth-context.tsx`), then forgets the local device id so a
 * fresh sign-in mints a new one.
 */
export const unregisterPushNotificationsAsync = async (input: {
  apiBaseUrl: string
  bearerToken: string
}): Promise<void> => {
  try {
    const deviceId = await readPushDeviceIdIfPresent()
    if (deviceId !== null && input.bearerToken.trim().length > 0) {
      const base = input.apiBaseUrl.replace(/\/$/, "")
      const response = await fetch(
        `${base}${PUSH_DEVICE_TOKENS_PATH}?deviceId=${encodeURIComponent(deviceId)}`,
        {
          headers: { authorization: `Bearer ${input.bearerToken}` },
          method: "DELETE"
        }
      )
      await readOkMobileJsonResponse(response, "push token unregistration")
    }
  } catch {
    // Best-effort; the server-side revocation-key prune (workers/api
    // `push/push-device-tokens.ts`) is the fallback if this network call
    // fails or never fires.
  } finally {
    await clearPushDeviceId()
  }
}
