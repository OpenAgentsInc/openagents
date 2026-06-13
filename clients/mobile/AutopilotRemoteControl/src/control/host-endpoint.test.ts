import { describe, expect, test } from "bun:test"

import {
  buildAuthedControlRequest,
  resolveHostControlServerBaseUrl,
} from "./host-endpoint"

describe("host endpoint", () => {
  test("resolves dev client loopback hosts by platform", () => {
    expect(resolveHostControlServerBaseUrl({ platform: "ios", port: 4317 })).toBe("http://127.0.0.1:4317")
    expect(resolveHostControlServerBaseUrl({ platform: "android", port: 4317 })).toBe("http://10.0.2.2:4317")
  })

  test("rejects invalid ports", () => {
    expect(() => resolveHostControlServerBaseUrl({ platform: "ios", port: 0 })).toThrow("Invalid control server port")
    expect(() => resolveHostControlServerBaseUrl({ platform: "android", port: 65_536 })).toThrow("Invalid control server port")
  })

  test("builds an authed request without performing I/O", async () => {
    const request = buildAuthedControlRequest({
      baseUrl: "http://127.0.0.1:4317/",
      path: "/command",
      devToken: "dev.fixture.token",
      body: { type: "session.list" },
    })

    expect(request.url).toBe("http://127.0.0.1:4317/command")
    expect(request.method).toBe("POST")
    expect(request.headers.get("Authorization")).toBe("Bearer dev.fixture.token")
    expect(request.headers.get("content-type")).toBe("application/json")
    expect(await request.json()).toEqual({ type: "session.list" })
  })
})
