import { describe, expect, test } from "vite-plus/test"
import { readFile } from "node:fs/promises"
import { Effect, Stream } from "@effect-native/core/effect"
import { IntentRef, StaticPayload } from "@effect-native/core"

import {
  createAuthenticatedMobileRepositoryEnvironment,
  MOBILE_ENVIRONMENT_DIRECTORY_ENDPOINT,
  MOBILE_ENVIRONMENT_PAIR_ENDPOINT,
  MOBILE_ENVIRONMENT_RECONNECT_ENDPOINT,
} from "../src/coding/mobile-repository-environment-client"
import {
  decodeMobileEnvironmentDirectory,
  decodeMobileEnvironmentReceipt,
  decodeMobileShareUrl,
  mobileShareComposerText,
  type MobileEnvironmentDirectory,
  type MobileNotificationSnapshot,
} from "../src/settings/mobile-settings"
import { buildHomeProgram, renderContentView } from "../src/screens/home-core"

const directory: MobileEnvironmentDirectory = {
  directoryRef: "directory.mobile.1",
  environments: [{
    environmentRef: "environment.owner.1",
    label: "Studio Mac",
    kind: "owner_local",
    health: "connected",
    paired: true,
    capabilities: ["coding", "git", "terminal", "notifications"],
    lastSeenAt: "2026-07-18T00:30:00.000Z",
    detail: "Exact worktree authority is reachable.",
  }],
  truncated: false,
}

const receipt = (operation: "pair" | "reconnect") => ({
  ok: true,
  receiptRef: `receipt.${operation}.1`,
  operation,
  environmentRef: "environment.owner.1",
  recordedAt: "2026-07-18T00:31:00.000Z",
  summary: operation === "pair" ? "Studio Mac paired." : "Studio Mac reconnected.",
  directory,
})

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) => Effect.map(Stream.runHead(program.stateChanges), option => {
  if (option._tag !== "Some") throw new Error("expected state")
  return option.value
})

const dispatch = (program: ReturnType<typeof buildHomeProgram>, name: string, payload: unknown = {}) =>
  Effect.runPromise(program.report(IntentRef(name, StaticPayload(payload as never))) as Effect.Effect<unknown>)

describe("T3M-F1 mobile settings, connections, notifications, and share intake", () => {
  test("production composition routes initial/live shares and keeps notification registration in the native host", async () => {
    const app = await readFile(new URL("../src/app.tsx", import.meta.url), "utf8")
    const notificationHost = await readFile(new URL("../src/settings/expo-mobile-notification-settings.ts", import.meta.url), "utf8")
    expect(app).toContain("decodeMobileShareUrl")
    expect(app).toContain("deliverUrl(event.url)")
    expect(app).toContain("deliverUrl(url)")
    expect(app).toContain("notificationSettings={notificationSettings}")
    expect(notificationHost).toContain("getDevicePushTokenAsync")
    expect(notificationHost).not.toContain("accessToken")
    expect(notificationHost).not.toContain("devicePushToken:")
  })

  test("decodes bounded public-safe environment health and exact mutation receipts", () => {
    expect(decodeMobileEnvironmentDirectory(directory)).toEqual(directory)
    expect(decodeMobileEnvironmentDirectory({ ...directory, environments: [...directory.environments, directory.environments[0]] })).toBeNull()
    expect(decodeMobileEnvironmentDirectory({ ...directory, environments: [{ ...directory.environments[0], environmentRef: "https://secret.example/token" }] })).toBeNull()
    expect(decodeMobileEnvironmentReceipt(receipt("pair"), "pair")?.receiptRef).toBe("receipt.pair.1")
    expect(decodeMobileEnvironmentReceipt(receipt("pair"), "reconnect")).toBeNull()
  })

  test("uses authenticated fixed endpoints for directory, pair, and reconnect", async () => {
    const calls: Array<Readonly<{ url: string; authorization: string | null; body: unknown }>> = []
    const environment = createAuthenticatedMobileRepositoryEnvironment({
      baseUrl: "https://openagents.com",
      accessToken: "verified-token-1234567890",
      fetch: async (input, init) => {
        calls.push({
          url: String(input),
          authorization: new Headers(init?.headers).get("authorization"),
          body: JSON.parse(String(init?.body)),
        })
        return new Response(JSON.stringify(directory), { status: 200, headers: { "content-type": "application/json" } })
      },
    })
    await environment.environmentDirectory()
    await environment.pairEnvironment({ pairingCode: "PAIR-123", idempotencyRef: "pair.1" })
    await environment.reconnectEnvironment({ environmentRef: "environment.owner.1", directoryRef: directory.directoryRef, idempotencyRef: "reconnect.1" })
    expect(calls.map(call => new URL(call.url).pathname)).toEqual([
      MOBILE_ENVIRONMENT_DIRECTORY_ENDPOINT,
      MOBILE_ENVIRONMENT_PAIR_ENDPOINT,
      MOBILE_ENVIRONMENT_RECONNECT_ENDPOINT,
    ])
    expect(calls.every(call => call.authorization === "Bearer verified-token-1234567890")).toBe(true)
  })

  test("loads hierarchy and health, pairs and reconnects with receipts, and requests permission only after the explicit tap", async () => {
    let permissionRequests = 0
    let preferences: MobileNotificationSnapshot["preferences"] = { attention: true, completion: true, approvals: true }
    const notification = (): MobileNotificationSnapshot => ({ permission: permissionRequests === 0 ? "undetermined" : "granted", registration: permissionRequests === 0 ? "unregistered" : "registered", preferences, detail: "Public-safe native health." })
    const program = buildHomeProgram({ settings: {
      environments: {
        environmentDirectory: async () => directory,
        pairEnvironment: async () => receipt("pair"),
        reconnectEnvironment: async () => receipt("reconnect"),
      },
      notifications: {
        snapshot: async () => notification(),
        requestPermission: async () => { permissionRequests += 1; return notification() },
        setPreferences: async next => { preferences = next; return notification() },
      },
    } })
    program.chrome.pressSettings()
    await Effect.runPromise(settle)
    expect(permissionRequests).toBe(0)
    let state = await Effect.runPromise(lastState(program))
    expect(state.workbenchRoute).toBe("settings")
    expect(state.settings.environments?.environments[0]?.health).toBe("connected")
    expect(JSON.stringify(renderContentView(state))).toContain("Storage & cache")
    await dispatch(program, "SettingsSectionSelected", { section: "environments" })
    await dispatch(program, "EnvironmentPairingCodeChanged", "PAIR-123")
    await dispatch(program, "EnvironmentPairRequested")
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(state.settings.environmentReceipt?.operation).toBe("pair")
    await dispatch(program, "EnvironmentReconnectRequested", { environmentRef: "environment.owner.1" })
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(state.settings.environmentReceipt?.operation).toBe("reconnect")
    await dispatch(program, "NotificationPermissionRequested")
    await Effect.runPromise(settle)
    expect(permissionRequests).toBe(1)
    await dispatch(program, "NotificationPreferenceToggled", { preference: "completion" })
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(state.settings.notification.preferences.completion).toBe(false)
  })

  test("accepts only bounded safe share links and inserts after review without replacing transcript state", async () => {
    const share = decodeMobileShareUrl("openagents://share?title=Bug%20report&text=Please%20inspect&url=https%3A%2F%2Fexample.com%2Ftrace")
    expect(share).not.toBeNull()
    expect(mobileShareComposerText(share!)).toContain("https://example.com/trace")
    expect(decodeMobileShareUrl("openagents://share?url=file%3A%2F%2F%2Fetc%2Fpasswd")).toBeNull()
    let consumed = 0
    const program = buildHomeProgram({ settings: { incomingShare: share, onShareConsumed: () => { consumed += 1 } } })
    const transcript = program.initialState.khala.entries
    program.chrome.pressSettings()
    await Effect.runPromise(settle)
    await dispatch(program, "SettingsSectionSelected", { section: "share" })
    await dispatch(program, "IncomingShareInserted")
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    expect(state.workbenchRoute).toBe("conversation")
    expect(state.khala.draft).toContain("Please inspect")
    expect(state.khala.entries).toEqual(transcript)
    expect(state.settings.incomingShare).toBeNull()
    expect(consumed).toBe(1)
  })
})
