import { beforeEach, describe, expect, test } from "vite-plus/test"

import {
  readPushDeviceRegistrationRecord,
  removePushDeviceRegistration,
  resolvePushDeviceId,
  syncPushDeviceRegistration,
  type SecureStoreLike,
  type SyncPushDeviceRegistrationInput,
} from "../src/push/expo-push-device-registration"

const values = new Map<string, string>()

const store: SecureStoreLike = {
  getItemAsync: async key => values.get(key) ?? null,
  setItemAsync: async (key, value) => {
    values.set(key, value)
  },
  deleteItemAsync: async key => {
    values.delete(key)
  },
}

const granted = { granted: true, canAskAgain: true }
const notGranted = { granted: false, canAskAgain: true }

const baseInput = {
  baseUrl: "https://openagents.com",
  accessToken: "owner-token",
  projectId: "eas.project.fixture",
  platform: "ios" as const,
  permission: granted,
  store,
  randomId: () => "device.fixture.1",
}

const jsonResponse = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status })

describe("Expo push device registration core (SARAH-PUSH-1 #9062)", () => {
  beforeEach(() => {
    values.clear()
  })

  test("resolvePushDeviceId creates one stable id and persists it across calls", async () => {
    let calls = 0
    const randomId = () => {
      calls += 1
      return `device.created.${calls}`
    }
    const first = await resolvePushDeviceId(store, randomId)
    const second = await resolvePushDeviceId(store, randomId)
    expect(first).toBe("device.created.1")
    expect(second).toBe("device.created.1")
    expect(calls).toBe(1)
  })

  test("readPushDeviceRegistrationRecord is null when unset or malformed, never throws", async () => {
    expect(await readPushDeviceRegistrationRecord(store)).toBeNull()
    await store.setItemAsync("openagents.mobile.push.last-registered.v1", "not json")
    expect(await readPushDeviceRegistrationRecord(store)).toBeNull()
    await store.setItemAsync("openagents.mobile.push.last-registered.v1", JSON.stringify({ deviceId: "" }))
    expect(await readPushDeviceRegistrationRecord(store)).toBeNull()
  })

  test("skips (never fetches a token) when permission is not granted", async () => {
    let tokenCalls = 0
    const outcome = await syncPushDeviceRegistration({
      ...baseInput,
      permission: notGranted,
      getExpoPushTokenAsync: async () => {
        tokenCalls += 1
        return { data: "ExponentPushToken[unused]" }
      },
      fetch: async () => jsonResponse({ ok: true }),
    })
    expect(outcome).toEqual({ state: "skipped", reason: "permission_not_granted" })
    expect(tokenCalls).toBe(0)
  })

  test("skips (never fetches a token) when no EAS project id is configured", async () => {
    let tokenCalls = 0
    const outcome = await syncPushDeviceRegistration({
      ...baseInput,
      projectId: null,
      getExpoPushTokenAsync: async () => {
        tokenCalls += 1
        return { data: "ExponentPushToken[unused]" }
      },
    })
    expect(outcome).toEqual({ state: "skipped", reason: "project_id_missing" })
    expect(tokenCalls).toBe(0)
  })

  test("reports failed/unavailable, never throws, when getExpoPushTokenAsync rejects", async () => {
    const outcome = await syncPushDeviceRegistration({
      ...baseInput,
      getExpoPushTokenAsync: async () => { throw new Error("offline") },
    })
    expect(outcome).toEqual({ state: "failed", reason: "unavailable" })
  })

  test("registers on first sync and persists the pair locally", async () => {
    let posted: Record<string, unknown> = {}
    const outcome = await syncPushDeviceRegistration({
      ...baseInput,
      getExpoPushTokenAsync: async () => ({ data: "ExponentPushToken[first]" }),
      fetch: async (_url, init) => {
        posted = JSON.parse(String(init?.body)) as Record<string, unknown>
        return jsonResponse({ ok: true, registration: { deviceId: "device.fixture.1", platform: "ios", updatedAt: "t" } })
      },
    })
    expect(outcome).toEqual({ state: "registered", rotated: false })
    expect(posted).toEqual({
      deviceId: "device.fixture.1",
      expoPushToken: "ExponentPushToken[first]",
      platform: "ios",
    })
    expect(await readPushDeviceRegistrationRecord(store)).toEqual({
      deviceId: "device.fixture.1",
      expoPushToken: "ExponentPushToken[first]",
    })
  })

  test("skips the network call on a repeat sync with the SAME token", async () => {
    let fetchCalls = 0
    const dependencies = {
      ...baseInput,
      getExpoPushTokenAsync: async () => ({ data: "ExponentPushToken[stable]" }),
      fetch: async () => {
        fetchCalls += 1
        return jsonResponse({ ok: true, registration: { deviceId: "device.fixture.1", platform: "ios", updatedAt: "t" } })
      },
    }
    const first = await syncPushDeviceRegistration(dependencies)
    const second = await syncPushDeviceRegistration(dependencies)
    expect(first).toEqual({ state: "registered", rotated: false })
    expect(second).toEqual({ state: "registered", rotated: false })
    expect(fetchCalls).toBe(1)
  })

  test("re-registers (rotated: true) when Expo returns a DIFFERENT token than last registered", async () => {
    let fetchCalls = 0
    let lastPostedToken = ""
    const makeDeps = (token: string): SyncPushDeviceRegistrationInput => ({
      ...baseInput,
      getExpoPushTokenAsync: async () => ({ data: token }),
      fetch: async (_url, init) => {
        fetchCalls += 1
        lastPostedToken = (JSON.parse(String(init?.body)) as { expoPushToken: string }).expoPushToken
        return jsonResponse({ ok: true, registration: { deviceId: "device.fixture.1", platform: "ios", updatedAt: "t" } })
      },
    })
    const first = await syncPushDeviceRegistration(makeDeps("ExponentPushToken[old]"))
    const second = await syncPushDeviceRegistration(makeDeps("ExponentPushToken[new]"))
    expect(first).toEqual({ state: "registered", rotated: false })
    expect(second).toEqual({ state: "registered", rotated: true })
    expect(fetchCalls).toBe(2)
    expect(lastPostedToken).toBe("ExponentPushToken[new]")
    expect(await readPushDeviceRegistrationRecord(store)).toEqual({
      deviceId: "device.fixture.1",
      expoPushToken: "ExponentPushToken[new]",
    })
  })

  test("surfaces unauthorized and unavailable from the register call as typed failures", async () => {
    const unauthorized = await syncPushDeviceRegistration({
      ...baseInput,
      getExpoPushTokenAsync: async () => ({ data: "ExponentPushToken[a]" }),
      fetch: async () => new Response(null, { status: 401 }),
    })
    expect(unauthorized).toEqual({ state: "failed", reason: "unauthorized" })
    // A failed registration must NOT persist a local record.
    expect(await readPushDeviceRegistrationRecord(store)).toBeNull()

    const unavailable = await syncPushDeviceRegistration({
      ...baseInput,
      getExpoPushTokenAsync: async () => ({ data: "ExponentPushToken[b]" }),
      fetch: async () => new Response(null, { status: 500 }),
    })
    expect(unavailable).toEqual({ state: "failed", reason: "unavailable" })
    expect(await readPushDeviceRegistrationRecord(store)).toBeNull()
  })

  test("removePushDeviceRegistration no-ops when this device never registered", async () => {
    let fetchCalls = 0
    const outcome = await removePushDeviceRegistration({
      baseUrl: baseInput.baseUrl,
      accessToken: baseInput.accessToken,
      store,
      fetch: async () => {
        fetchCalls += 1
        return jsonResponse({ ok: true, removed: true })
      },
    })
    expect(outcome).toEqual({ state: "not_registered" })
    expect(fetchCalls).toBe(0)
  })

  test("removePushDeviceRegistration removes remotely and clears the local record", async () => {
    await syncPushDeviceRegistration({
      ...baseInput,
      getExpoPushTokenAsync: async () => ({ data: "ExponentPushToken[to-remove]" }),
      fetch: async () => jsonResponse({ ok: true, registration: { deviceId: "device.fixture.1", platform: "ios", updatedAt: "t" } }),
    })
    expect(await readPushDeviceRegistrationRecord(store)).not.toBeNull()

    const outcome = await removePushDeviceRegistration({
      baseUrl: baseInput.baseUrl,
      accessToken: baseInput.accessToken,
      store,
      fetch: async () => jsonResponse({ ok: true, removed: true }),
    })
    expect(outcome).toEqual({ state: "removed" })
    expect(await readPushDeviceRegistrationRecord(store)).toBeNull()
  })

  test("removePushDeviceRegistration keeps the local record on a failed removal", async () => {
    await syncPushDeviceRegistration({
      ...baseInput,
      getExpoPushTokenAsync: async () => ({ data: "ExponentPushToken[keep]" }),
      fetch: async () => jsonResponse({ ok: true, registration: { deviceId: "device.fixture.1", platform: "ios", updatedAt: "t" } }),
    })

    const unauthorized = await removePushDeviceRegistration({
      baseUrl: baseInput.baseUrl,
      accessToken: baseInput.accessToken,
      store,
      fetch: async () => new Response(null, { status: 401 }),
    })
    expect(unauthorized).toEqual({ state: "failed", reason: "unauthorized" })
    expect(await readPushDeviceRegistrationRecord(store)).not.toBeNull()
  })
})
