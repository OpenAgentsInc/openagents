import { describe, expect, test } from "bun:test"

import {
  KHALA_CODE_TAILNET_HEALTH_PORT,
  candidateTargets,
  connectionProfileFromStatus,
  normalizeSyncBaseUrl,
  resolveKhalaCodeConnectionProfile,
  resolveKhalaCodeConnectivity,
  type FetchLike
} from "../src/status/khala-code-connectivity-core"

const fakeFetch = (
  responses: Record<string, { ok: boolean; hostname?: string }>
): FetchLike => async url => {
  const response = responses[url]
  if (response === undefined) throw new Error(`unexpected fetch: ${url}`)
  return {
    json: async () => ({ hostname: response.hostname }),
    ok: response.ok
  }
}

describe("Khala mobile connectivity targets", () => {
  test("simulator (not a device) probes localhost only", () => {
    expect(candidateTargets(false)).toEqual([
      `http://127.0.0.1:${KHALA_CODE_TAILNET_HEALTH_PORT}/health`
    ])
  })

  test("physical device probes the configured Tailnet hosts", () => {
    expect(candidateTargets(true, 50099, ["host-a", "host-b"])).toEqual([
      "http://host-a:50099/health",
      "http://host-b:50099/health"
    ])
  })
})

describe("Khala mobile connectivity resolution", () => {
  test("reports reachable on the first responding target", async () => {
    const result = await resolveKhalaCodeConnectivity(
      ["http://host-a/health", "http://host-b/health"],
      fakeFetch({
        "http://host-a/health": { hostname: "bertha", ok: true }
      })
    )
    expect(result.reachable).toBe(true)
    expect(result.hostname).toBe("bertha")
    expect(result.target).toBe("http://host-a/health")
  })

  test("falls through to the next candidate on failure", async () => {
    const result = await resolveKhalaCodeConnectivity(
      ["http://host-a/health", "http://host-b/health"],
      fakeFetch({
        "http://host-a/health": { ok: false },
        "http://host-b/health": { hostname: "m2", ok: true }
      })
    )
    expect(result.reachable).toBe(true)
    expect(result.target).toBe("http://host-b/health")
  })

  test("reports unreachable when every candidate fails", async () => {
    const result = await resolveKhalaCodeConnectivity(
      ["http://host-a/health"],
      async () => {
        throw new Error("network unreachable")
      }
    )
    expect(result.reachable).toBe(false)
    expect(result.target).toBeNull()
    expect(result.hostname).toBeNull()
  })

  test("reports unreachable on abort/timeout", async () => {
    const result = await resolveKhalaCodeConnectivity(
      ["http://host-a/health"],
      (_url, init) =>
        new Promise((_resolve, reject) => {
          init.signal.addEventListener("abort", () => reject(new Error("aborted")))
        }),
      5
    )
    expect(result.reachable).toBe(false)
  })
})

describe("Khala mobile connection profile", () => {
  test("normalizes the sync base url separately from the health beacon", () => {
    expect(normalizeSyncBaseUrl(" http://127.0.0.1:8787/ ")).toBe(
      "http://127.0.0.1:8787"
    )
    expect(normalizeSyncBaseUrl("")).toBe("https://openagents.com")
  })

  test("turns a simulator health check into a loopback sync profile", async () => {
    const profile = await resolveKhalaCodeConnectionProfile({
      fetchImpl: fakeFetch({
        "http://127.0.0.1:50099/health": { hostname: "local-mac", ok: true }
      }),
      isDevice: false,
      syncBaseUrl: "http://127.0.0.1:8787/"
    })

    expect(profile).toMatchObject({
      healthTarget: "http://127.0.0.1:50099/health",
      hostname: "local-mac",
      reachable: true,
      syncBaseUrl: "http://127.0.0.1:8787",
      targetKind: "simulator_loopback"
    })
  })

  test("marks physical-device profiles as Tailnet even when unreachable", () => {
    const profile = connectionProfileFromStatus(
      {
        checkedAt: "2026-07-04T20:00:00.000Z",
        hostname: null,
        reachable: false,
        target: null
      },
      { isDevice: true, syncBaseUrl: "https://openagents.com" }
    )

    expect(profile.targetKind).toBe("tailnet")
    expect(profile.healthTarget).toBeNull()
    expect(profile.reachable).toBe(false)
  })
})
