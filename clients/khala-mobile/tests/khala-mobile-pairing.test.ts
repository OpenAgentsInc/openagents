import { describe, expect, test } from "bun:test"

import {
  discoverKhalaMobilePairingCredentials,
  khalaMobilePairingTargets,
  KHALA_MOBILE_PAIRING_PATH,
  type PairingFetchLike
} from "../src/auth/khala-mobile-pairing-core"

type FakeResponse = { ok: boolean; body?: unknown }

const fakeFetch = (responses: Record<string, FakeResponse>): PairingFetchLike => async url => {
  const response = responses[url]
  if (response === undefined) throw new Error(`unexpected fetch: ${url}`)
  return {
    json: async () => response.body ?? {},
    ok: response.ok
  }
}

describe("khalaMobilePairingTargets", () => {
  test("simulator (not a device) probes localhost only", () => {
    expect(khalaMobilePairingTargets(false)).toEqual([
      `http://127.0.0.1:50099${KHALA_MOBILE_PAIRING_PATH}`
    ])
  })

  test("physical device probes localhost first, then the configured Tailnet hosts", () => {
    expect(khalaMobilePairingTargets(true, 50099, ["host-a", "host-b"])).toEqual([
      `http://127.0.0.1:50099${KHALA_MOBILE_PAIRING_PATH}`,
      `http://host-a:50099${KHALA_MOBILE_PAIRING_PATH}`,
      `http://host-b:50099${KHALA_MOBILE_PAIRING_PATH}`
    ])
  })
})

describe("discoverKhalaMobilePairingCredentials", () => {
  test("returns paired credentials from a signed-in desktop", async () => {
    const outcome = await discoverKhalaMobilePairingCredentials(
      ["http://host-a/khala-mobile-pairing"],
      fakeFetch({
        "http://host-a/khala-mobile-pairing": {
          body: { hostname: "bertha", ok: true, ownerUserId: "user_1", token: "oa_agent_1" },
          ok: true
        }
      })
    )
    expect(outcome).toEqual({
      credentials: { ownerUserId: "user_1", token: "oa_agent_1" },
      hostname: "bertha",
      state: "paired"
    })
  })

  test("prefers a paired host over one that is reachable but signed out", async () => {
    const outcome = await discoverKhalaMobilePairingCredentials(
      ["http://host-a/khala-mobile-pairing", "http://host-b/khala-mobile-pairing"],
      fakeFetch({
        "http://host-a/khala-mobile-pairing": {
          body: { hostname: "signed-out-host", ok: false, reason: "not_signed_in" },
          ok: true
        },
        "http://host-b/khala-mobile-pairing": {
          body: { hostname: "signed-in-host", ok: true, ownerUserId: "user_2", token: "oa_agent_2" },
          ok: true
        }
      })
    )
    expect(outcome).toEqual({
      credentials: { ownerUserId: "user_2", token: "oa_agent_2" },
      hostname: "signed-in-host",
      state: "paired"
    })
  })

  test("reports reachable_not_signed_in when the desktop answers but has no credentials", async () => {
    const outcome = await discoverKhalaMobilePairingCredentials(
      ["http://host-a/khala-mobile-pairing"],
      fakeFetch({
        "http://host-a/khala-mobile-pairing": {
          body: { hostname: "bertha", ok: false, reason: "not_signed_in" },
          ok: true
        }
      })
    )
    expect(outcome).toEqual({ hostname: "bertha", state: "reachable_not_signed_in" })
  })

  test("reports unreachable when every candidate fails or times out", async () => {
    const outcome = await discoverKhalaMobilePairingCredentials(
      ["http://host-a/khala-mobile-pairing"],
      async () => {
        throw new Error("network unreachable")
      }
    )
    expect(outcome).toEqual({ state: "unreachable" })
  })

  test("reports unreachable on abort/timeout", async () => {
    const outcome = await discoverKhalaMobilePairingCredentials(
      ["http://host-a/khala-mobile-pairing"],
      (_url, init) =>
        new Promise((_resolve, reject) => {
          init.signal.addEventListener("abort", () => reject(new Error("aborted")))
        }),
      5
    )
    expect(outcome).toEqual({ state: "unreachable" })
  })

  test("returns unreachable for an empty target list without calling fetch", async () => {
    const outcome = await discoverKhalaMobilePairingCredentials(
      [],
      async () => {
        throw new Error("should not be called")
      }
    )
    expect(outcome).toEqual({ state: "unreachable" })
  })

  test("probes candidates concurrently, not serially (total time ~= one timeout, not N)", async () => {
    const hosts = ["host-a", "host-b", "host-c"].map(h => `http://${h}${KHALA_MOBILE_PAIRING_PATH}`)
    const started = Date.now()
    const slowFetch: PairingFetchLike = (_url, init) =>
      new Promise((_resolve, reject) => {
        // Each candidate "hangs" until aborted by its own per-host timeout.
        init.signal.addEventListener("abort", () => reject(new Error("aborted")))
      })
    const outcome = await discoverKhalaMobilePairingCredentials(hosts, slowFetch, 30)
    const elapsedMs = Date.now() - started
    expect(outcome).toEqual({ state: "unreachable" })
    // Serial probing of 3 candidates at a 30ms timeout would take >= 90ms;
    // concurrent probing should land close to a single timeout window.
    expect(elapsedMs).toBeLessThan(80)
  })
})
