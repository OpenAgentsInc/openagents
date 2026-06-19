import { describe, expect, test } from "bun:test"

import { GotInferenceGatewayReadiness } from "../src/ui/message"
import {
  initialModel,
  Model,
  modelInferenceDecision,
  modelInferenceGatewayReadiness,
} from "../src/ui/model"
import { update } from "../src/ui/update"
import type {
  AccountRow,
  InferenceGatewayReadinessResponse,
  NodeStateMessage,
} from "../src/shared/rpc"

// #5485 (EPIC #5474): the model-level glue — the GotInferenceGatewayReadiness
// reducer + the derived own-auth-vs-gateway decision off the live model.

const account = (
  provider: string,
  ready: boolean,
): AccountRow => ({
  provider,
  homeState: ready ? "present" : "missing",
  ready,
  accountRef: `${provider}-acct`,
  accountRefHash: "hash",
  selector: "registry_ref",
  blockerRefs: ready ? [] : ["blocker.account.login_required"],
  priority: null,
})

const nodeWith = (accounts: AccountRow[]): NodeStateMessage => ({
  ok: true,
  schema: "test",
  sessions: [],
  accounts,
})

const gatewayReadiness = (
  overrides: Partial<InferenceGatewayReadinessResponse> = {},
): InferenceGatewayReadinessResponse => ({
  ok: true,
  fetchedAt: "2026-06-19T00:00:00.000Z",
  sourceUrl: "https://openagents.com/v1/credits",
  enabled: true,
  apiKeyPresent: true,
  model: "oa-default",
  creditBalance: 10,
  lowBalanceThreshold: 1,
  blockerRefs: [],
  ...overrides,
})

describe("GotInferenceGatewayReadiness reducer", () => {
  test("stores the public-safe projection on the model, no further command", () => {
    const projection = gatewayReadiness()
    const [next, commands] = update(
      initialModel,
      GotInferenceGatewayReadiness({ projection }),
    )
    expect(modelInferenceGatewayReadiness(next)).toEqual(projection)
    expect(commands).toEqual([])
  })
})

describe("modelInferenceDecision — derived off the live model", () => {
  test("fresh user (no accounts) with gateway ready + auto → gateway fallback", () => {
    const model = Model.make({
      ...initialModel,
      spawnAdapter: "codex",
      gatewayInferenceFallback: "auto",
      node: nodeWith([]),
      inferenceGatewayReadiness: gatewayReadiness(),
    })
    const d = modelInferenceDecision(model)
    expect(d.route).toBe("gateway")
    expect(d.usedFallback).toBe(true)
  })

  test("user with ready codex auth → own_auth (no credit spend)", () => {
    const model = Model.make({
      ...initialModel,
      spawnAdapter: "codex",
      gatewayInferenceFallback: "auto",
      node: nodeWith([account("codex", true)]),
      inferenceGatewayReadiness: gatewayReadiness(),
    })
    expect(modelInferenceDecision(model).route).toBe("own_auth")
  })

  test("gateway not yet fetched (null readiness) + no own auth → blocked (disabled)", () => {
    const model = Model.make({
      ...initialModel,
      spawnAdapter: "codex",
      gatewayInferenceFallback: "auto",
      node: nodeWith([]),
      inferenceGatewayReadiness: null,
    })
    const d = modelInferenceDecision(model)
    expect(d.route).toBe("blocked")
    expect(d.reason).toBe("blocker.inference.gateway_disabled")
  })

  test("preference off + no own auth → blocked even with a ready gateway", () => {
    const model = Model.make({
      ...initialModel,
      spawnAdapter: "claude_agent",
      gatewayInferenceFallback: "off",
      node: nodeWith([]),
      inferenceGatewayReadiness: gatewayReadiness(),
    })
    expect(modelInferenceDecision(model).route).toBe("blocked")
  })
})
