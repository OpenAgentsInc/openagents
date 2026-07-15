import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, test } from "vite-plus/test"

import type { CodexAppServerLease, CodexAppServerNotification } from "./codex-app-server-supervisor.ts"
import { makeCodexControlPlane } from "./codex-control-plane.ts"

const roots: string[] = []
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const fakeLease = (optionalFailures: ReadonlySet<string> = new Set()) => {
  let generation = 1
  let configVersion = 1
  let account: unknown = { type: "chatgpt", email: "owner@example.com", planType: "pro" }
  const listeners = new Set<(notification: CodexAppServerNotification) => void>()
  const requests: Array<{ method: string; params: unknown }> = []
  let released = false
  const response = (method: string, params: unknown): unknown => {
    switch (method) {
      case "account/read": return { account, requiresOpenaiAuth: true }
      case "account/rateLimits/read": return { rateLimits: { primary: { usedPercent: 12 } } }
      case "account/usage/read": return { summary: { totalTokens: 42 } }
      case "account/workspaceMessages/read": return { featureEnabled: true, messages: [{ messageId: "message-1", messageType: "warning", messageBody: "private workspace body" }] }
      case "model/list": return { data: [
        { id: "gpt-5.6-sol", model: "gpt-5.6-sol", displayName: "GPT 5.6", hidden: false, isDefault: true, supportedReasoningEfforts: [{ reasoningEffort: "medium" }] },
        { id: "hidden", model: "hidden", displayName: "Hidden", hidden: true, isDefault: false, supportedReasoningEfforts: [] },
      ] }
      case "modelProvider/capabilities/read": return { imageGeneration: false, namespaceTools: true, webSearch: false }
      case "config/read": return { config: { model: "gpt-5.6-sol", version: configVersion }, origins: {}, layers: [] }
      case "configRequirements/read": return { requirements: {
        allowRemoteControl: false,
        allowedApprovalPolicies: ["never"],
        allowedSandboxModes: ["read-only"],
        allowedPermissionProfiles: { safe: true, unsafe: false },
        allowedWebSearchModes: ["disabled"],
        featureRequirements: { betaFeature: true, forbiddenFeature: false },
      } }
      case "permissionProfile/list": return { data: [
        { id: "safe", allowed: true, description: "Safe" },
        { id: "unsafe", allowed: true, description: "Unsafe" },
      ] }
      case "experimentalFeature/list": return { data: [
        { name: "betaFeature", enabled: true, stage: "beta" },
        { name: "forbiddenFeature", enabled: true, stage: "beta" },
      ] }
      case "collaborationMode/list": return { data: [{ name: "default", model: "gpt-5.6-sol" }] }
      case "account/login/start": {
        const type = (params as { type?: string }).type
        return type === "chatgptDeviceCode"
          ? { type, loginId: "login-device", userCode: "PRIVATE-CODE", verificationUrl: "https://private/login" }
          : { type: "chatgpt", loginId: "login-browser", authUrl: "https://private/auth" }
      }
      case "account/logout": account = null; return {}
      case "config/value/write": configVersion += 1; return {}
      case "config/batchWrite": configVersion += 1; return {}
      default: return {}
    }
  }
  const lease = {
    key: "control-lease",
    identity: { binary: "/codex", binarySha256: "hash", codexHome: null, accountRef: "account", hostTarget: "desktop" },
    state: () => ({ status: "ready", generation }) as const,
    request: async (method: string, params: unknown) => {
      requests.push({ method, params })
      if (optionalFailures.has(method)) throw new Error(`${method} requires authentication`)
      return response(method, params)
    },
    notify: async () => undefined,
    subscribe: (listener: (notification: CodexAppServerNotification) => void) => { listeners.add(listener); return () => listeners.delete(listener) },
    subscribeCompatibility: () => () => undefined,
    nativeEnvelopes: () => [],
    compatibilityReceipts: () => [],
    nativeJournal: () => [],
    registerVisibleThread: () => () => undefined,
    registerReverseHandler: () => () => undefined,
    release: () => { released = true },
  } as unknown as CodexAppServerLease
  return {
    lease,
    requests,
    released: () => released,
    generation: (next: number) => { generation = next },
    emit: (method: string, params: unknown, eventGeneration = generation) => {
      for (const listener of listeners) listener({ generation: eventGeneration, message: { method, params } })
    },
  }
}

describe("Codex app-server control plane", () => {
  test("projects account/rate/model/config/policy truth without credential or message leakage", async () => {
    const fake = fakeLease()
    const plane = makeCodexControlPlane({ lease: fake.lease })
    const snapshot = await plane.initialize()

    expect(snapshot.account).toEqual({ signedIn: true, authMode: "chatgpt", planType: "pro" })
    expect(snapshot.workspaceMessages).toEqual([{ messageId: "message-1", messageType: "warning" }])
    expect(JSON.stringify(snapshot)).not.toContain("owner@example.com")
    expect(JSON.stringify(snapshot)).not.toContain("private workspace body")
    expect(snapshot.models.map(model => model.id)).toEqual(["gpt-5.6-sol", "hidden"])
    expect(snapshot.modelCapabilities).toEqual({ imageGeneration: false, namespaceTools: true, webSearch: false })
    expect(fake.requests.map(request => request.method)).toEqual([
      "account/read", "account/rateLimits/read", "account/usage/read", "account/workspaceMessages/read",
      "model/list", "modelProvider/capabilities/read", "config/read", "configRequirements/read",
      "permissionProfile/list", "experimentalFeature/list", "collaborationMode/list",
    ])
    plane.close()
    expect(fake.released()).toBe(true)
  })

  test("applies one managed-policy gate across model and turn controls", async () => {
    const fake = fakeLease()
    const plane = makeCodexControlPlane({ lease: fake.lease })
    await plane.initialize()

    expect(plane.gate({ type: "model", value: "gpt-5.6-sol" }).allowed).toBe(true)
    expect(plane.gate({ type: "model", value: "hidden" }).allowed).toBe(false)
    expect(plane.gate({ type: "approvalPolicy", value: "never" }).allowed).toBe(true)
    expect(plane.gate({ type: "approvalPolicy", value: "on-request" }).allowed).toBe(false)
    expect(plane.gate({ type: "sandboxMode", value: "workspace-write" }).allowed).toBe(false)
    expect(plane.gate({ type: "permissionProfile", value: "safe" }).allowed).toBe(true)
    expect(plane.gate({ type: "permissionProfile", value: "unsafe" }).allowed).toBe(false)
    expect(plane.gate({ type: "feature", value: "betaFeature" }).allowed).toBe(true)
    expect(plane.gate({ type: "feature", value: "forbiddenFeature" }).allowed).toBe(false)
    expect(plane.gate({ type: "webSearch", value: "disabled" }).allowed).toBe(true)
    expect(plane.gate({ type: "webSearch", value: "enabled" }).allowed).toBe(false)
    expect(plane.gate({ type: "namespaceTools", value: true }).allowed).toBe(true)
    expect(plane.gate({ type: "imageGeneration", value: true }).allowed).toBe(false)
    expect(plane.gate({ type: "remoteControl", value: true }).allowed).toBe(false)
    plane.close()
  })

  test("requires one-shot owner intent and stale-state protection for every mutation", async () => {
    const root = mkdtempSync(join(tmpdir(), "oa-control-receipts-"))
    roots.push(root)
    const receiptPath = join(root, "receipts.json")
    const fake = fakeLease()
    const plane = makeCodexControlPlane({ lease: fake.lease, receiptPath })
    await plane.initialize()
    const revision = plane.snapshot().revision
    const intent = plane.authorizeOwnerIntent("config_write", revision)
    await plane.writeConfig({ keyPath: "model", value: "PRIVATE-VALUE", mergeStrategy: "replace" }, intent)
    await expect(plane.writeConfig({ keyPath: "model", value: "again", mergeStrategy: "replace" }, intent))
      .rejects.toMatchObject({ reason: "intent_reused" })
    expect(plane.snapshot().revision).toBeGreaterThan(revision)

    const stale = plane.authorizeOwnerIntent("logout", plane.snapshot().revision)
    fake.emit("configWarning", { summary: "changed", details: null, path: "/private/config.toml" })
    await expect(plane.logout(stale)).rejects.toMatchObject({ reason: "stale_state" })
    const disk = readFileSync(receiptPath, "utf8")
    expect(disk).not.toContain("PRIVATE-VALUE")
    expect(disk).not.toContain("/private/config.toml")
    expect(plane.receipts().map(receipt => receipt.outcome)).toEqual(["accepted", "stale"])
    plane.close()
  })

  test("supports browser/device login, cancel/logout, notifications, and restart reconciliation", async () => {
    const root = mkdtempSync(join(tmpdir(), "oa-control-login-"))
    roots.push(root)
    const receiptPath = join(root, "receipts.json")
    const fake = fakeLease()
    const plane = makeCodexControlPlane({ lease: fake.lease, receiptPath })
    await plane.initialize()

    const deviceIntent = plane.authorizeOwnerIntent("login", plane.snapshot().revision)
    await plane.startLogin({ type: "chatgptDeviceCode" }, deviceIntent)
    expect(plane.snapshot().loginFlow).toMatchObject({ type: "chatgptDeviceCode", loginId: "login-device" })
    fake.emit("account/login/completed", { loginId: "login-device", success: true, error: null })
    expect(plane.snapshot().loginCompletion).toEqual({ loginId: "login-device", success: true, error: null })
    expect(plane.snapshot().loginFlow).toBeNull()

    const cancelIntent = plane.authorizeOwnerIntent("login_cancel", plane.snapshot().revision)
    await plane.cancelLogin("login-device", cancelIntent)
    const logoutIntent = plane.authorizeOwnerIntent("logout", plane.snapshot().revision)
    await plane.logout(logoutIntent)
    expect(plane.snapshot().account.signedIn).toBe(false)
    plane.close()

    const restartedFake = fakeLease()
    const restarted = makeCodexControlPlane({ lease: restartedFake.lease, receiptPath })
    await restarted.initialize()
    expect(restarted.snapshot().loginFlow).toBeNull()
    expect(restarted.receipts()).toHaveLength(3)
    expect(readFileSync(receiptPath, "utf8")).not.toContain("PRIVATE-CODE")
    expect(readFileSync(receiptPath, "utf8")).not.toContain("https://private")
    restarted.close()
  })

  test("reconciles config mutations and surfaces exact private warnings while fencing stale generations", async () => {
    const fake = fakeLease()
    const plane = makeCodexControlPlane({ lease: fake.lease })
    await plane.initialize()
    const before = JSON.stringify(plane.snapshot().config)
    const intent = plane.authorizeOwnerIntent("config_batch_write", plane.snapshot().revision)
    await plane.batchWriteConfig({ edits: [], expectedVersion: "v1" }, intent)
    expect(JSON.stringify(plane.snapshot().config)).not.toBe(before)

    fake.generation(2)
    fake.emit("configWarning", { summary: "current warning", details: "exact details", path: "/private/current" }, 2)
    fake.emit("configWarning", { summary: "stale warning", details: null, path: null }, 1)
    expect(plane.snapshot().warnings).toEqual([{
      summary: "current warning",
      details: "exact details",
      path: "/private/current",
    }])
    plane.close()
  })

  test("degrades account-scoped optional reads without hiding exact method failures", async () => {
    const failures = new Set(["account/rateLimits/read", "account/usage/read", "account/workspaceMessages/read", "collaborationMode/list"])
    const fake = fakeLease(failures)
    const plane = makeCodexControlPlane({ lease: fake.lease })
    const snapshot = await plane.initialize()
    expect(snapshot.models).toHaveLength(2)
    expect(snapshot.errors.map(error => error.method)).toEqual([...failures])
    plane.close()
  })

  test("sends generated parameter shapes for every remaining owner mutation", async () => {
    const fake = fakeLease()
    const plane = makeCodexControlPlane({ lease: fake.lease })
    await plane.initialize()
    const perform = async (kind: Parameters<typeof plane.authorizeOwnerIntent>[0], action: (intent: ReturnType<typeof plane.authorizeOwnerIntent>) => Promise<unknown>) => {
      const intent = plane.authorizeOwnerIntent(kind, plane.snapshot().revision)
      await action(intent)
    }
    await perform("reset_credit", intent => plane.consumeResetCredit({ idempotencyKey: "reset-1", creditId: "credit-1" }, intent))
    await perform("workspace_nudge", intent => plane.sendWorkspaceNudge("usage_limit", intent))
    await perform("config_write", intent => plane.writeConfig({ keyPath: "model", value: "gpt-5.6-sol", mergeStrategy: "replace" }, intent))
    await perform("config_batch_write", intent => plane.batchWriteConfig({ edits: [], expectedVersion: "v1" }, intent))
    await perform("mcp_reload", intent => plane.reloadMcp(intent))
    await perform("feature_enablement", intent => plane.setFeatureEnablement({ betaFeature: true }, intent))
    expect(fake.requests.filter(request => [
      "account/rateLimitResetCredit/consume", "account/sendAddCreditsNudgeEmail", "config/value/write",
      "config/batchWrite", "config/mcpServer/reload", "experimentalFeature/enablement/set",
    ].includes(request.method))).toEqual([
      { method: "account/rateLimitResetCredit/consume", params: { idempotencyKey: "reset-1", creditId: "credit-1" } },
      { method: "account/sendAddCreditsNudgeEmail", params: { creditType: "usage_limit" } },
      { method: "config/value/write", params: { keyPath: "model", value: "gpt-5.6-sol", mergeStrategy: "replace" } },
      { method: "config/batchWrite", params: { edits: [], expectedVersion: "v1" } },
      { method: "config/mcpServer/reload", params: {} },
      { method: "experimentalFeature/enablement/set", params: { enablement: { betaFeature: true } } },
    ])
    expect(plane.receipts()).toHaveLength(6)
    plane.close()
  })
})
