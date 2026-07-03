import { describe, expect, test } from "bun:test"
import { Schema as S } from "effect"

import {
  KHALA_CODE_MODEL_ROLE_REGISTRY_SCHEMA,
  KhalaCodeModelRoleRegistrySchema,
  defaultKhalaCodeModelRoleRegistry,
} from "../src/shared/model-roles"
import {
  KhalaCodeDesktopRpcBridgeFailure,
  KhalaCodeDesktopRpcMethodNames,
  KhalaCodeDesktopRpcMethodSchemas,
  decodeKhalaCodeDesktopRpcParameters,
  decodeKhalaCodeDesktopRpcResult,
  khalaCodeDesktopRpcDecodeFailure,
  khalaCodeDesktopRpcHandlerFailure,
} from "../src/shared/rpc"
import { createKhalaCodeDesktopRpcRequestHandlers } from "../src/bun/rpc-handlers"

describe("Khala Code desktop schema-first RPC contract", () => {
  test("decodes request parameters and response results by method", () => {
    const args = decodeKhalaCodeDesktopRpcParameters("submitChatMessage", [
      {
        messages: [
          {
            body: "hello",
            id: "message-1",
            role: "user",
          },
        ],
        sessionId: "session-1",
      },
    ])

    expect(args).toHaveLength(1)
    expect(args[0]).toMatchObject({ sessionId: "session-1" })

    const result = decodeKhalaCodeDesktopRpcResult("submitChatMessage", {
      backend: {
        kind: "mock",
        model: "test-model",
      },
      messages: [
        {
          body: "hello",
          id: "message-1",
          role: "user",
        },
      ],
      ok: true,
      toolNames: [],
      usedTools: [],
    })

    expect(result).toMatchObject({ ok: true })
  })

  test("accepts neutral harnessItem while keeping codexItem back-compat", () => {
    const harnessItem = {
      itemId: "item-1",
      itemType: "commandExecution",
      status: "completed",
      title: "Command",
    }

    const neutral = decodeKhalaCodeDesktopRpcResult("submitChatMessage", {
      backend: {
        kind: "claude_app_sdk",
        model: "claude-app-sdk",
        runtimeMode: "claude_runtime",
      },
      messages: [
        {
          body: "ran",
          harnessItem,
          id: "message-neutral",
          role: "tool",
        },
      ],
      ok: true,
      toolNames: [],
      usedTools: [],
    }) as { readonly messages: readonly [{ readonly harnessItem?: typeof harnessItem }] }
    expect(neutral.messages[0]).toMatchObject({ harnessItem })

    const backCompat = decodeKhalaCodeDesktopRpcResult("submitChatMessage", {
      backend: {
        kind: "codex_app_server",
        model: "gpt-5.1-codex",
        runtimeMode: "codex_harness",
      },
      messages: [
        {
          body: "ran",
          codexItem: harnessItem,
          id: "message-codex",
          role: "tool",
        },
      ],
      ok: true,
      toolNames: [],
      usedTools: [],
    }) as { readonly messages: readonly [{ readonly codexItem?: typeof harnessItem }] }
    expect(backCompat.messages[0]).toMatchObject({ codexItem: harnessItem })
  })

  test("rejects malformed request parameters before a handler runs", () => {
    expect(() =>
      decodeKhalaCodeDesktopRpcParameters("codexThreadRead", [{ includeTurns: true }]),
    ).toThrow()

    const failure = khalaCodeDesktopRpcDecodeFailure(
      "codexThreadRead",
      new Error("threadId is required"),
    )

    expect(S.decodeUnknownSync(KhalaCodeDesktopRpcBridgeFailure)(failure)).toEqual({
      error: "threadId is required",
      method: "codexThreadRead",
      ok: false,
      tag: "rpc_decode_failed",
    })
  })

  test("decodes renderer QA metric sample bridge requests", () => {
    const sample = {
      context: { source: "thread_switch", threadId: "thread-qa-bridge" },
      metric: "thread_switch.rpc_ms",
      observedAt: "2026-07-02T12:00:00.000Z",
      unit: "ms",
      value: 33,
    }

    expect(decodeKhalaCodeDesktopRpcParameters("qaMetricSample", [sample])).toEqual([sample])
    expect(decodeKhalaCodeDesktopRpcResult("qaMetricSample", {
      ok: true,
      observedAt: "2026-07-02T12:00:01.000Z",
    })).toMatchObject({ ok: true })

    const percentSample = {
      metric: "transcript.scroll_dropped_frames_pct",
      observedAt: "2026-07-02T12:00:00.000Z",
      unit: "percent",
      value: 4.5,
    }
    expect(decodeKhalaCodeDesktopRpcParameters("qaMetricSample", [percentSample]))
      .toEqual([percentSample])
  })

  test("decodes the schema-first cross-harness session catalog RPC", () => {
    expect(decodeKhalaCodeDesktopRpcParameters("sessionCatalog", [{
      limit: 20,
      scope: "app",
      searchTerm: "plan",
    }])).toEqual([{ limit: 20, scope: "app", searchTerm: "plan" }])

    expect(decodeKhalaCodeDesktopRpcResult("sessionCatalog", {
      ok: true,
      schemaVersion: "khala-code-desktop.session-catalog.v1",
      scope: "app",
      diagnostics: [],
      entries: [{
        catalogEntryId: "claude:session-1",
        harnessKind: "claude",
        sessionRef: "session-1",
        threadRef: "session-1",
        desktopSessionRef: "desktop-1",
        lastTurnRef: null,
        title: "Claude plan",
        preview: "Plan",
        cwd: "/repo",
        projectLabel: "repo",
        status: "ready",
        statusLabel: "Claude session",
        source: "claude_sdk_list_sessions",
        createdAt: null,
        updatedAt: 1782910100000,
        recencyAt: 1782910100000,
        exactTotals: {
          totalTokens: 80,
          source: "claude_sdk_list_sessions",
        },
      }],
    })).toMatchObject({
      entries: [{
        harnessKind: "claude",
        exactTotals: { totalTokens: 80 },
      }],
    })
  })

  test("decodes optional real-work claim refs on fleet RPC requests", () => {
    expect(decodeKhalaCodeDesktopRpcParameters("codexFleetDelegateRun", [{
      branch: "main",
      claimRef: "claim.public.t4_2.rpc_delegate",
      commit: "0123456789abcdef0123456789abcdef01234567",
      mode: "real_work",
      objective: "Run pinned public work.",
      repo: "OpenAgentsInc/openagents",
      verify: "command.public.pylon_khala.verify.d32c71ee8e1025e99460d008",
    }])[0]).toMatchObject({
      claimRef: "claim.public.t4_2.rpc_delegate",
      mode: "real_work",
    })

    expect(decodeKhalaCodeDesktopRpcParameters("codexFleetPromoteThread", [{
      claimRef: "claim.public.t4_2.rpc_promote",
      commit: "0123456789abcdef0123456789abcdef01234567",
      contextBoundary: {
        allowedRefs: [],
        includeTranscript: false,
        mode: "explicit_objective",
        summary: null,
      },
      objective: "Promote pinned public work.",
      repo: "OpenAgentsInc/openagents",
      sessionId: "session-1",
      threadId: "thread-1",
      verify: "command.public.pylon_khala.verify.d32c71ee8e1025e99460d008",
    }])[0]).toMatchObject({
      claimRef: "claim.public.t4_2.rpc_promote",
      objective: "Promote pinned public work.",
    })
  })

  test("decodes the schema-first model role registry RPCs", () => {
    const registry = defaultKhalaCodeModelRoleRegistry()
    expect(S.decodeUnknownSync(KhalaCodeModelRoleRegistrySchema)(registry)).toEqual(registry)
    expect(registry.schema).toBe(KHALA_CODE_MODEL_ROLE_REGISTRY_SCHEMA)

    expect(decodeKhalaCodeDesktopRpcResult("modelRoleRegistryRead", {
      ok: true,
      path: "/tmp/desktop-settings.json",
      registry,
    })).toMatchObject({
      registry: {
        roles: {
          architect: { role: "architect", harness: "codex" },
          coder: { role: "coder", harness: "codex" },
          judge: { role: "judge", harness: "codex" },
          advisor: { role: "advisor", harness: "codex" },
        },
      },
    })

    expect(decodeKhalaCodeDesktopRpcParameters("modelRoleRegistryWrite", [{
      entry: {
        role: "coder",
        harness: "claude",
        model: "claude-opus-4-1",
        effort: "high",
      },
    }])[0]).toMatchObject({
      entry: { role: "coder", harness: "claude", effort: "high" },
    })
  })

  test("decodes Forum RPC requests and rejects non-Forum proxy paths", async () => {
    expect(decodeKhalaCodeDesktopRpcParameters("forumRequest", [{
      body: { amountSat: 21 },
      headers: { "Idempotency-Key": "test-key" },
      method: "POST",
      path: "/api/forum/posts/post.1/tips/ladder",
    }])[0]).toMatchObject({
      method: "POST",
      path: "/api/forum/posts/post.1/tips/ladder",
    })

    expect(decodeKhalaCodeDesktopRpcResult("forumRequest", {
      ok: true,
      payload: { topics: [] },
      status: 200,
    })).toMatchObject({ ok: true, status: 200 })

    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })

    await expect(handlers.forumRequest({
      method: "GET",
      path: "/api/not-forum",
    })).rejects.toThrow("Forum RPC path must stay under /api/forum")
  })

  test("models handler failures as distinct tagged bridge errors", () => {
    const failure = khalaCodeDesktopRpcHandlerFailure(
      "appInfo",
      new Error("boom"),
    )

    expect(S.decodeUnknownSync(KhalaCodeDesktopRpcBridgeFailure)(failure)).toEqual({
      error: "boom",
      method: "appInfo",
      ok: false,
      tag: "rpc_handler_failed",
    })
  })

  test("covers every current request method with a schema entry", () => {
    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("not used")
      },
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("not used")
      },
      workingDirectory: process.cwd(),
    })
    const handlerNames = Object.keys(handlers).sort()
    const schemaNames = Object.keys(KhalaCodeDesktopRpcMethodSchemas).sort()

    const rpcMethodNames = KhalaCodeDesktopRpcMethodNames as readonly string[]
    expect([...rpcMethodNames].sort()).toEqual(schemaNames)
    expect(schemaNames).toEqual(handlerNames)
  })
})
