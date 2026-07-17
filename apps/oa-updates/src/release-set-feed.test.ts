import { readFile } from "node:fs/promises"

import { describe, expect, test } from "vite-plus/test"

import {
  canonicalizeReleaseSet,
  decodeReleaseSet,
  type ReleaseSet,
} from "../../openagents-desktop/src/release-set-contract.ts"
import {
  deriveReleaseKeyPin,
  signReleasePayload,
  type ReleaseSigningKey,
} from "../../openagents-desktop/src/release-publish.ts"
import {
  createInMemoryReleaseSetFeedStore,
  createReleaseSetFeed,
  RELEASE_SET_PAYLOAD_LIMIT,
  type ReleaseSetFeed,
  type ReleaseSetFeedStore,
} from "./release-set-feed.ts"
import { createUpdatesServer } from "./server.ts"

const seed = "Nx09GNy4f2Z4wWbHSOz97m9qoO5zD66nvDVUnQ3QWmo"
const signingKey: ReleaseSigningKey = { d: seed, kid: "fixture-release-set-v2" }
const pin = deriveReleaseKeyPin(signingKey)

const fixture = async (): Promise<ReleaseSet> => {
  const raw = JSON.parse(await readFile(
    new URL("../../openagents-desktop/tests/fixtures/release-set-v2.json", import.meta.url),
    "utf8",
  )) as unknown
  const decoded = decodeReleaseSet(raw)
  if (!decoded.ok) throw new Error("fixture rejected")
  return decoded.releaseSet
}

const signed = (releaseSet: ReleaseSet): {
  payloadBytes: Uint8Array
  signatureBytes: Uint8Array
} => {
  const payloadBytes = canonicalizeReleaseSet(releaseSet)
  const result = signReleasePayload(payloadBytes, signingKey)
  return {
    payloadBytes,
    signatureBytes: new TextEncoder().encode(JSON.stringify(result.envelope)),
  }
}

const makeFeed = (overrides: Partial<Parameters<typeof createReleaseSetFeed>[0]> = {}): {
  feed: ReleaseSetFeed
  logs: unknown[]
} => {
  const logs: unknown[] = []
  return {
    logs,
    feed: createReleaseSetFeed({
      store: createInMemoryReleaseSetFeedStore(),
      pins: new Map([[pin.kid, pin]]),
      verifyArtifact: async (artifact) => ({
        byteLength: artifact.byteLength,
        sha256: artifact.sha256,
      }),
      now: () => "2026-07-16T18:00:00.000Z",
      log: (entry) => logs.push(entry),
      ...overrides,
    }),
  }
}

describe("ReleaseSet v2 candidate admission", () => {
  test("re-verifies the signed complete set and every artifact before admission", async () => {
    const releaseSet = await fixture()
    const checked: string[] = []
    const { feed, logs } = makeFeed({
      verifyArtifact: async (artifact) => {
        checked.push(artifact.objectIdentity)
        return { byteLength: artifact.byteLength, sha256: artifact.sha256 }
      },
    })
    const candidate = await feed.admitCandidate({ channel: "rc", ...signed(releaseSet) })
    expect(candidate.generation).toMatch(/^[0-9a-f]{64}$/)
    expect(checked).toHaveLength(releaseSet.targets.flatMap((row) => row.artifacts).length)
    expect(logs).toContainEqual(expect.objectContaining({
      event: "candidate_admitted",
      channel: "rc",
      generation: candidate.generation,
    }))
    expect(feed.metrics()).toMatchObject({
      "candidate_admitted.rc": 1,
      "target_count.rc": 5,
    })
  })

  test("rejects malformed, oversized, wrong-key, wrong-channel, and bad artifact candidates", async () => {
    const releaseSet = await fixture()
    const valid = signed(releaseSet)
    const { feed } = makeFeed()
    await expect(feed.admitCandidate({
      channel: "rc",
      payloadBytes: new Uint8Array(RELEASE_SET_PAYLOAD_LIMIT + 1),
      signatureBytes: valid.signatureBytes,
    })).rejects.toThrow("payload_size_invalid")
    await expect(feed.admitCandidate({
      channel: "rc",
      payloadBytes: valid.payloadBytes,
      signatureBytes: new TextEncoder().encode("not-json"),
    })).rejects.toThrow("signature_json_invalid")
    await expect(feed.admitCandidate({ channel: "stable", ...valid }))
      .rejects.toThrow("release_set_channel_mismatch")

    const otherKey: ReleaseSigningKey = {
      d: "FY9SPznnUKhwoYL8vW6TR7HjYWoFxx0-RU_vKcj5odE",
      kid: pin.kid,
    }
    const forged = signReleasePayload(valid.payloadBytes, otherKey)
    await expect(feed.admitCandidate({
      channel: "rc",
      payloadBytes: valid.payloadBytes,
      signatureBytes: new TextEncoder().encode(JSON.stringify(forged.envelope)),
    })).rejects.toThrow("release_set_signature_invalid")

    const mismatch = makeFeed({
      verifyArtifact: async (artifact) => ({
        byteLength: artifact.byteLength + 1,
        sha256: artifact.sha256,
      }),
    }).feed
    await expect(mismatch.admitCandidate({ channel: "rc", ...valid }))
      .rejects.toThrow("artifact_observation_mismatch")
  })
})

describe("ReleaseSet v2 atomic promotion and routes", () => {
  test("serves immutable candidates and bounded-cache current pointer truth", async () => {
    const releaseSet = await fixture()
    const { feed } = makeFeed()
    const candidate = await feed.admitCandidate({ channel: "rc", ...signed(releaseSet) })
    const pointer = await feed.promote("rc", candidate.generation, null)

    const pointerResponse = await feed.fetch(new Request(
      "https://updates.openagents.com/desktop/openagents/rc/v2/pointer.json",
    ))
    expect(pointerResponse?.status).toBe(200)
    expect(pointerResponse?.headers.get("cache-control")).toContain("max-age=15")
    expect(await pointerResponse?.json()).toEqual(pointer)

    for (const leaf of ["release-set", "release-set.sig"] as const) {
      const response = await feed.fetch(new Request(
        `https://updates.openagents.com/desktop/openagents/rc/${leaf}.json`,
      ))
      expect(response?.status).toBe(200)
      expect(response?.headers.get("cache-control")).toBe("no-store")
      expect(response?.headers.get("access-control-allow-origin")).toBe("*")
      expect(response?.headers.get("x-openagents-release-generation"))
        .toBe(candidate.generation)
    }

    const immutable = await feed.fetch(new Request(
      `https://updates.openagents.com/desktop/openagents/rc/candidates/${candidate.generation}/release-set.json`,
    ))
    expect(new Uint8Array(await immutable!.arrayBuffer())).toEqual(candidate.payloadBytes)
    expect(immutable?.headers.get("cache-control")).toBe(
      "public, max-age=31536000, immutable",
    )

    const head = await feed.fetch(new Request(
      `https://updates.openagents.com/desktop/openagents/rc/candidates/${candidate.generation}/release-set.sig.json`,
      { method: "HEAD" },
    ))
    expect(await head?.text()).toBe("")
    expect(Number(head?.headers.get("content-length"))).toBe(candidate.signatureBytes.byteLength)
    expect(feed.metrics()).toMatchObject({
      "route_resolved.rc.payload.immutable.success": 1,
      "route_resolved.rc.signature.immutable.success": 1,
      "route_resolved.rc.pointer.bounded.success": 1,
    })

    const cors = await feed.fetch(new Request(
      "https://updates.openagents.com/desktop/openagents/rc/v2/pointer.json",
      { method: "OPTIONS" },
    ))
    expect(cors?.status).toBe(204)
    expect(cors?.headers.get("access-control-allow-methods")).toBe("GET, HEAD, OPTIONS")
  })

  test("CAS rejects promotion races and rollback only swaps with the retained slot", async () => {
    const firstSet = await fixture()
    const secondSet = {
      ...firstSet,
      version: "2.4.0-rc.4",
      publishedAt: "2026-07-16T18:01:00.000Z",
      targets: firstSet.targets.map((row) => ({
        ...row,
        artifacts: row.artifacts.map((artifact) => ({
          ...artifact,
          version: "2.4.0-rc.4",
          name: artifact.name.replaceAll("2.4.0-rc.3", "2.4.0-rc.4"),
          url: artifact.url.replaceAll("2.4.0-rc.3", "2.4.0-rc.4"),
          objectIdentity: artifact.objectIdentity.replaceAll("2.4.0-rc.3", "2.4.0-rc.4"),
        })),
      })),
    } as ReleaseSet
    const { feed } = makeFeed()
    const first = await feed.admitCandidate({ channel: "rc", ...signed(firstSet) })
    const second = await feed.admitCandidate({ channel: "rc", ...signed(secondSet) })
    await feed.promote("rc", first.generation, null)
    await expect(feed.promote("rc", second.generation, null))
      .rejects.toThrow("pointer_revision_conflict")
    await expect(feed.promote("rc", first.generation, 1))
      .rejects.toThrow("candidate_already_current")
    const promoted = await feed.promote("rc", second.generation, 1)
    expect(promoted.previousGeneration).toBe(first.generation)
    const rolledBack = await feed.rollback("rc", 2)
    expect(rolledBack).toMatchObject({
      revision: 3,
      generation: first.generation,
      previousGeneration: second.generation,
    })
    await expect(feed.rollback("rc", 2)).rejects.toThrow("pointer_revision_conflict")
    expect(await feed.listGarbageCandidates("rc")).toEqual([])
  })

  test("rollback re-authenticates the exact retained object and never changes current on failure", async () => {
    const firstSet = await fixture()
    const secondSet = {
      ...firstSet,
      version: "2.4.0-rc.4",
      publishedAt: "2026-07-16T18:01:00.000Z",
      targets: firstSet.targets.map((row) => ({
        ...row,
        artifacts: row.artifacts.map((artifact) => ({
          ...artifact,
          version: "2.4.0-rc.4",
          name: artifact.name.replaceAll("2.4.0-rc.3", "2.4.0-rc.4"),
          url: artifact.url.replaceAll("2.4.0-rc.3", "2.4.0-rc.4"),
          objectIdentity: artifact.objectIdentity.replaceAll("2.4.0-rc.3", "2.4.0-rc.4"),
        })),
      })),
    } as ReleaseSet
    const store = createInMemoryReleaseSetFeedStore()
    const healthy = makeFeed({ store }).feed
    const first = await healthy.admitCandidate({ channel: "rc", ...signed(firstSet) })
    const second = await healthy.admitCandidate({ channel: "rc", ...signed(secondSet) })
    await healthy.promote("rc", first.generation, null)
    await healthy.promote("rc", second.generation, 1)

    const retained = await store.readCandidate("rc", first.generation)
    if (retained === null) throw new Error("fixture retained candidate missing")
    const corruptions = [
      null,
      { ...retained, generation: "e".repeat(64) },
      { ...retained, channel: "stable" as const },
      { ...retained, signatureBytes: new Uint8Array() },
      { ...retained, payloadBytes: Uint8Array.from([...retained.payloadBytes, 0]) },
    ] as const
    for (const corrupted of corruptions) {
      const malicious: ReleaseSetFeedStore = {
        ...store,
        readCandidate: async (channel, generation) =>
          generation === first.generation ? corrupted : store.readCandidate(channel, generation),
      }
      const repairing = makeFeed({ store: malicious }).feed
      await expect(repairing.rollback("rc", 2)).rejects.toThrow(
        corrupted === null ? "rollback_candidate_missing" : "rollback_candidate_invalid",
      )
      expect(await store.readPointer("rc")).toMatchObject({
        revision: 2,
        generation: second.generation,
        previousGeneration: first.generation,
      })
    }
  })

  test("fails closed on pointer/object mismatch", async () => {
    const releaseSet = await fixture()
    const store = createInMemoryReleaseSetFeedStore()
    const { feed } = makeFeed({ store })
    const candidate = await feed.admitCandidate({ channel: "rc", ...signed(releaseSet) })
    await feed.promote("rc", candidate.generation, null)
    const brokenStore = createInMemoryReleaseSetFeedStore()
    await brokenStore.compareAndSwapPointer("rc", null, {
      schema: "openagents.desktop.release_pointer.v2",
      channel: "rc",
      revision: 1,
      generation: candidate.generation,
      previousGeneration: null,
      payloadSha256: candidate.generation,
      signatureSha256: "b".repeat(64),
      publishedAt: "2026-07-16T18:00:00.000Z",
    })
    const brokenFeed = makeFeed({ store: brokenStore }).feed
    const response = await brokenFeed.fetch(new Request(
      "https://updates.openagents.com/desktop/openagents/rc/release-set.json",
    ))
    expect(response?.status).toBe(503)
    expect(response?.headers.get("cache-control")).toBe("no-store")
  })

  test("normalizes storage errors into bounded redacted public failures and exports metrics", async () => {
    const failingStore: ReleaseSetFeedStore = {
      ...createInMemoryReleaseSetFeedStore(),
      readPointer: async () => { throw new Error("private bucket topology") },
    }
    const { feed } = makeFeed({ store: failingStore })
    const response = await feed.fetch(new Request(
      "https://updates.openagents.com/desktop/openagents/rc/v2/pointer.json",
    ))
    expect(response?.status).toBe(503)
    expect(response?.headers.get("access-control-allow-origin")).toBe("*")
    expect(response?.headers.get("cache-control")).toBe("no-store")
    expect(await response?.json()).toEqual({ error: "feed_unavailable" })

    const metrics = await feed.fetch(new Request(
      "https://updates.openagents.com/metrics/release-set.json",
    ))
    expect(metrics?.status).toBe(200)
    expect(metrics?.headers.get("cache-control")).toBe("no-store")
    const body = await metrics?.json() as { counters: Record<string, number> }
    expect(body.counters["route_failed.rc.pointer.bounded.failure"]).toBe(1)
    expect(JSON.stringify(body)).not.toContain("bucket topology")
  })
})

describe("Desktop v2 publication remains additive to mobile OTA", () => {
  test("registering and promoting Desktop metadata does not erase an Expo manifest", async () => {
    const releaseSet = await fixture()
    const { feed } = makeFeed()
    const server = createUpdatesServer({ releaseSetFeed: feed })
    server.registerUpdate({
      id: "mobile-update",
      branch: "production",
      runtimeVersion: "1.0.0",
      platform: "ios",
      createdAt: "2026-07-16T18:00:00.000Z",
      launchAsset: {
        hash: "launch",
        key: "launch",
        contentType: "application/javascript",
        url: "https://updates.openagents.com/assets/launch",
      },
      assets: [],
      metadata: {},
      extra: {},
    })
    const before = await server.fetch(new Request(
      "https://updates.openagents.com/production/manifest",
      { headers: { "expo-platform": "ios", "expo-runtime-version": "1.0.0" } },
    ))
    const candidate = await server.admitReleaseSetCandidate({
      channel: "rc",
      ...signed(releaseSet),
    })
    await server.promoteReleaseSet("rc", candidate.generation, null)
    const after = await server.fetch(new Request(
      "https://updates.openagents.com/production/manifest",
      { headers: { "expo-platform": "ios", "expo-runtime-version": "1.0.0" } },
    ))
    expect(before.status).toBe(200)
    expect(after.status).toBe(200)
    expect(await after.text()).toBe(await before.text())
  })
})
