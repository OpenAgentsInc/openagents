/**
 * Pure decision logic for Khala Mobile push notification registration
 * (MM-G1, #8485). Kept native-import-free so it stays unit-testable under
 * `bun test` — the native-touching wiring (expo-notifications,
 * expo-secure-store, fetch) lives in `push-notifications-client.ts`.
 */

export type PushDispatchEvent = "task_dispatched" | "app_launch"

/**
 * The permission prompt fires the FIRST time the user dispatches a task
 * (starts a brand-new turn), never on app launch and never more than once
 * automatically — see `khala_mobile.push.permission_prompt_on_first_task_dispatch.v1`
 * in `../contracts/ux-contracts.ts`. Once `hasEverPrompted` is true, the app
 * never calls the OS permission prompt again on its own; the user's only path
 * back to "ask me" is the OS Settings app (standard platform behavior for a
 * previously-answered permission).
 */
export const shouldPromptForPushPermission = (
  state: Readonly<{ hasEverPrompted: boolean }>,
  event: PushDispatchEvent,
): boolean => event === "task_dispatched" && !state.hasEverPrompted

export type PushPlatform = "ios" | "android"

export type RegisterPushDeviceTokenBody = Readonly<{
  deviceId: string
  expoPushToken: string
  platform: PushPlatform
}>

export const buildRegisterPushDeviceTokenBody = (input: {
  deviceId: string
  expoPushToken: string
  platform: PushPlatform
}): RegisterPushDeviceTokenBody => ({
  deviceId: input.deviceId,
  expoPushToken: input.expoPushToken,
  platform: input.platform,
})

/** Whether a permission outcome should be persisted as "granted" for the
 * server registration (only `granted` — provisional/denied never register a
 * token, since there is nothing deliverable). */
export const permissionOutcomeIsRegisterable = (
  status: "granted" | "denied" | "undetermined",
): boolean => status === "granted"
