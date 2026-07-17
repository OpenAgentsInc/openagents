import { describe, expect, test } from "vite-plus/test"
import type { ConfirmedRuntimeAttentionSnapshot } from "@openagentsinc/khala-sync-client"

import {
  MobileAttentionTargetSchemaVersion,
  decodeMobileAttentionDeepLink,
  resolveMobileAttentionTarget,
} from "../src/attention/mobile-attention-target"

const item = {
  schema: "openagents.runtime_attention.v1" as const,
  attentionRef: "interaction.attention.mobile",
  ownerUserId: "owner.mobile",
  interactionRef: "interaction.attention.mobile",
  threadRef: "thread.attention.mobile",
  turnRef: "turn.attention.mobile",
  kind: "tool_approval" as const,
  status: "pending" as const,
  requestedAt: "2026-07-17T12:00:00.000Z",
  expiresAt: "2026-07-17T12:05:00.000Z",
  updatedAt: "2026-07-17T12:00:00.000Z",
}
const target = {
  schema: MobileAttentionTargetSchemaVersion,
  attentionRef: item.attentionRef,
  threadRef: item.threadRef,
  turnRef: item.turnRef,
} as const
const snapshot = (): ConfirmedRuntimeAttentionSnapshot => ({
  status: { phase: "live", cursor: 7 },
  pending: [item], terminal: [], issues: [],
})

describe("contract openagents_mobile.attention_target.v1", () => {
  test("resolves in-app, deep-link, and notification inputs to one exact target", () => {
    expect(resolveMobileAttentionTarget(snapshot(), { source: "in_app", target })).toEqual({
      state: "ready", source: "in_app", target,
    })
    const url = `openagents://attention/${item.attentionRef}?threadRef=${item.threadRef}&turnRef=${item.turnRef}`
    expect(decodeMobileAttentionDeepLink(url)).toEqual(target)
    expect(resolveMobileAttentionTarget(snapshot(), { source: "deep_link", url })).toEqual({
      state: "ready", source: "deep_link", target,
    })
    expect(resolveMobileAttentionTarget(snapshot(), { source: "notification", payload: target })).toEqual({
      state: "ready", source: "notification", target,
    })
  })

  test("rejects stale authority, terminal replay, mismatched identity, and malformed payloads", () => {
    expect(resolveMobileAttentionTarget({
      ...snapshot(), status: { phase: "must_refetch", cursor: null },
    }, { source: "in_app", target })).toMatchObject({ state: "rejected", reason: "authority_unavailable" })
    expect(resolveMobileAttentionTarget({
      ...snapshot(), pending: [], terminal: [{ ...item, status: "resolved" }],
    }, { source: "notification", payload: target })).toMatchObject({ state: "rejected", reason: "terminal_attention" })
    expect(resolveMobileAttentionTarget(snapshot(), {
      source: "notification", payload: { ...target, threadRef: "thread.other" },
    })).toMatchObject({ state: "rejected", reason: "target_mismatch" })
    expect(resolveMobileAttentionTarget(snapshot(), {
      source: "notification", payload: { ...target, prompt: "private" },
    })).toMatchObject({ state: "rejected", reason: "invalid_target" })
    expect(resolveMobileAttentionTarget(snapshot(), {
      source: "notification", payload: { threadRef: item.threadRef },
    })).toMatchObject({ state: "rejected", reason: "invalid_target" })
  })

  test("serialized target carries only stable refs", () => {
    const serialized = JSON.stringify(target)
    expect(Object.keys(target).sort()).toEqual(["attentionRef", "schema", "threadRef", "turnRef"])
    expect(serialized).not.toContain("prompt")
    expect(serialized).not.toContain("token")
    expect(serialized).not.toContain("/Users/")
  })
})
