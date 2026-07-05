import { describe, expect, test } from "bun:test"
import {
  decodeTokensServedChannelMixSnapshotEntity,
  decodeTokensServedDemandMixSnapshotEntity,
  decodeTokensServedHistorySnapshotEntity,
  decodeTokensServedModelMixSnapshotEntity,
  encodeTokensServedChannelMixSnapshotEntity,
  encodeTokensServedDemandMixSnapshotEntity,
  encodeTokensServedHistorySnapshotEntity,
  encodeTokensServedModelMixSnapshotEntity,
  TOKENS_SERVED_AGGREGATES_CHANNEL_ID,
  TOKENS_SERVED_CHANNEL_MIX_ENTITY_TYPE,
  TOKENS_SERVED_DEMAND_MIX_ENTITY_TYPE,
  TOKENS_SERVED_HISTORY_ENTITY_TYPE,
  TOKENS_SERVED_MODEL_MIX_ENTITY_TYPE,
  tokensServedHistorySnapshotEntityId,
  tokensServedMixSnapshotEntityId,
} from "./tokens-served-mix.js"

const nowIso = "2026-07-05T00:00:00.000Z"

const validModelMix = {
  generatedAt: nowIso,
  groups: [
    { family: "glm" as const, label: "GLM family", pct: 60, reqs: 6, tokens: 600 },
    { family: "other" as const, label: "Other", pct: 40, reqs: 4, tokens: 400 },
  ],
  totalTokens: 1000,
  window: "30d" as const,
}

const validDemandMix = {
  generatedAt: nowIso,
  groups: [
    {
      client: "khala-code",
      kind: "external" as const,
      pct: 100,
      reqs: 10,
      source: "chat",
      tokens: 1000,
    },
  ],
  totalTokens: 1000,
  window: "30d" as const,
}

const validChannelMix = {
  generatedAt: nowIso,
  groups: [
    {
      channel: "khala_api" as const,
      label: "Khala API",
      pct: 100,
      reqs: 10,
      tokens: 1000,
    },
  ],
  totalTokens: 1000,
  window: "30d" as const,
}

const validHistory = {
  bucket: "day" as const,
  generatedAt: nowIso,
  series: [
    { day: "2026-07-04", tokensServed: 500 },
    { day: "2026-07-05", tokensServed: 500 },
  ],
  timezone: "America/Chicago",
  window: "30d" as const,
}

describe("tokens-served aggregate snapshot entity contracts (KS-6.7)", () => {
  test("channel + entity type constants", () => {
    expect(TOKENS_SERVED_AGGREGATES_CHANNEL_ID).toBe("tokens-served-aggregates")
    expect(TOKENS_SERVED_MODEL_MIX_ENTITY_TYPE).toBe(
      "tokens_served_model_mix_snapshot",
    )
    expect(TOKENS_SERVED_DEMAND_MIX_ENTITY_TYPE).toBe(
      "tokens_served_demand_mix_snapshot",
    )
    expect(TOKENS_SERVED_CHANNEL_MIX_ENTITY_TYPE).toBe(
      "tokens_served_channel_mix_snapshot",
    )
    expect(TOKENS_SERVED_HISTORY_ENTITY_TYPE).toBe(
      "tokens_served_history_snapshot",
    )
  })

  test("mix snapshots are keyed by window alone", () => {
    expect(tokensServedMixSnapshotEntityId("30d")).toBe("30d")
    expect(tokensServedMixSnapshotEntityId("today")).toBe("today")
  })

  test("history snapshots are keyed by window + timezone", () => {
    expect(tokensServedHistorySnapshotEntityId("30d", "America/Chicago")).toBe(
      "30d:America/Chicago",
    )
  })

  test("model-mix round-trips a valid post-image", () => {
    const entity = decodeTokensServedModelMixSnapshotEntity(validModelMix)
    expect(entity.window).toBe("30d")
    expect(entity.totalTokens).toBe(1000)
    expect(encodeTokensServedModelMixSnapshotEntity(entity)).toEqual(
      validModelMix,
    )
  })

  test("model-mix family is bounded to the closed literal set", () => {
    expect(() =>
      decodeTokensServedModelMixSnapshotEntity({
        ...validModelMix,
        groups: [{ ...validModelMix.groups[0], family: "made_up" }],
      }),
    ).toThrow()
  })

  test("demand-mix round-trips a valid post-image", () => {
    const entity = decodeTokensServedDemandMixSnapshotEntity(validDemandMix)
    expect(entity.groups[0]?.kind).toBe("external")
    expect(encodeTokensServedDemandMixSnapshotEntity(entity)).toEqual(
      validDemandMix,
    )
  })

  test("demand-mix kind is bounded to the closed literal set", () => {
    expect(() =>
      decodeTokensServedDemandMixSnapshotEntity({
        ...validDemandMix,
        groups: [{ ...validDemandMix.groups[0], kind: "made_up" }],
      }),
    ).toThrow()
  })

  test("channel-mix round-trips a valid post-image", () => {
    const entity = decodeTokensServedChannelMixSnapshotEntity(validChannelMix)
    expect(entity.groups[0]?.channel).toBe("khala_api")
    expect(encodeTokensServedChannelMixSnapshotEntity(entity)).toEqual(
      validChannelMix,
    )
  })

  test("history round-trips a valid post-image", () => {
    const entity = decodeTokensServedHistorySnapshotEntity(validHistory)
    expect(entity.series).toHaveLength(2)
    expect(encodeTokensServedHistorySnapshotEntity(entity)).toEqual(
      validHistory,
    )
  })

  test("history day must be YYYY-MM-DD", () => {
    expect(() =>
      decodeTokensServedHistorySnapshotEntity({
        ...validHistory,
        series: [{ day: "not-a-day", tokensServed: 1 }],
      }),
    ).toThrow()
  })

  test("counts must be non-negative integers", () => {
    expect(() =>
      decodeTokensServedModelMixSnapshotEntity({
        ...validModelMix,
        totalTokens: -1,
      }),
    ).toThrow()
    expect(() =>
      decodeTokensServedModelMixSnapshotEntity({
        ...validModelMix,
        totalTokens: 1.5,
      }),
    ).toThrow()
  })

  test("generatedAt must be ISO-8601 UTC", () => {
    expect(() =>
      decodeTokensServedModelMixSnapshotEntity({
        ...validModelMix,
        generatedAt: "not-a-date",
      }),
    ).toThrow()
  })
})
