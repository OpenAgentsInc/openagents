import { describe, expect, test } from "vite-plus/test"

import {
  PUSH_DEVICE_TOKENS_PATH,
  registerPushDeviceTokenRemote,
  unregisterPushDeviceTokenRemote,
} from "../src/push/push-token-client"

const input = {
  baseUrl: "https://openagents.com",
  accessToken: "owner-token",
  deviceId: "device.fixture.1",
  expoPushToken: "ExponentPushToken[fixture]",
  platform: "ios" as const,
}

describe("push device-token mobile client (SARAH-PUSH-1 #9062)", () => {
  test("registers with the exact server contract shape", async () => {
    let url = ""
    let method = ""
    let authorization = ""
    let contentType = ""
    let body: Record<string, unknown> = {}
    const result = await registerPushDeviceTokenRemote({
      ...input,
      fetch: async (requestedUrl, init) => {
        url = String(requestedUrl)
        method = String(init?.method)
        const headers = new Headers(init?.headers)
        authorization = headers.get("authorization") ?? ""
        contentType = headers.get("content-type") ?? ""
        body = JSON.parse(String(init?.body)) as Record<string, unknown>
        return new Response(
          JSON.stringify({
            ok: true,
            registration: { deviceId: input.deviceId, platform: "ios", updatedAt: "2026-07-19T00:00:00.000Z" },
          }),
          { status: 200 },
        )
      },
    })

    expect(url).toBe(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}`)
    expect(method).toBe("POST")
    expect(authorization).toBe("Bearer owner-token")
    expect(contentType).toBe("application/json")
    expect(body).toEqual({
      deviceId: input.deviceId,
      expoPushToken: input.expoPushToken,
      platform: "ios",
    })
    expect(result).toEqual({
      state: "registered",
      platform: "ios",
      updatedAt: "2026-07-19T00:00:00.000Z",
    })
  })

  test("maps 401 to unauthorized and 400 to invalid_request without throwing", async () => {
    expect((await registerPushDeviceTokenRemote({
      ...input,
      fetch: async () => new Response(JSON.stringify({ error: "unauthorized" }), { status: 401 }),
    })).state).toBe("unauthorized")
    expect((await registerPushDeviceTokenRemote({
      ...input,
      fetch: async () => new Response(JSON.stringify({ error: "invalid_request" }), { status: 400 }),
    })).state).toBe("invalid_request")
  })

  test("treats a network failure, a non-2xx status, and a malformed body as unavailable", async () => {
    expect((await registerPushDeviceTokenRemote({
      ...input,
      fetch: async () => { throw new Error("offline") },
    })).state).toBe("unavailable")
    expect((await registerPushDeviceTokenRemote({
      ...input,
      fetch: async () => new Response(null, { status: 500 }),
    })).state).toBe("unavailable")
    expect((await registerPushDeviceTokenRemote({
      ...input,
      fetch: async () => new Response(JSON.stringify({ ok: true }), { status: 200 }),
    })).state).toBe("unavailable")
  })

  test("unregisters with the deviceId query param and the exact server contract shape", async () => {
    let url = ""
    let method = ""
    let authorization = ""
    const result = await unregisterPushDeviceTokenRemote({
      baseUrl: input.baseUrl,
      accessToken: input.accessToken,
      deviceId: input.deviceId,
      fetch: async (requestedUrl, init) => {
        url = String(requestedUrl)
        method = String(init?.method)
        authorization = new Headers(init?.headers).get("authorization") ?? ""
        return new Response(JSON.stringify({ ok: true, removed: true }), { status: 200 })
      },
    })

    expect(url).toBe(`https://openagents.com${PUSH_DEVICE_TOKENS_PATH}?deviceId=${input.deviceId}`)
    expect(method).toBe("DELETE")
    expect(authorization).toBe("Bearer owner-token")
    expect(result).toEqual({ state: "removed" })
  })

  test("distinguishes removed from not_found, and keeps auth/network failures typed", async () => {
    expect((await unregisterPushDeviceTokenRemote({
      baseUrl: input.baseUrl,
      accessToken: input.accessToken,
      deviceId: input.deviceId,
      fetch: async () => new Response(JSON.stringify({ ok: true, removed: false }), { status: 200 }),
    })).state).toBe("not_found")
    expect((await unregisterPushDeviceTokenRemote({
      baseUrl: input.baseUrl,
      accessToken: input.accessToken,
      deviceId: input.deviceId,
      fetch: async () => new Response(null, { status: 401 }),
    })).state).toBe("unauthorized")
    expect((await unregisterPushDeviceTokenRemote({
      baseUrl: input.baseUrl,
      accessToken: input.accessToken,
      deviceId: input.deviceId,
      fetch: async () => { throw new Error("offline") },
    })).state).toBe("unavailable")
  })
})
