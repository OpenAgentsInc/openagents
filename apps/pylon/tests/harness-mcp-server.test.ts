import { describe, expect, test } from "vite-plus/test"
import type { OpenAgentsMcpReceipt } from "@openagentsinc/mcp-contract"

// FEED-1 (openagents #8783): OpenAgents harness MCP server — list/call round
// trip through a fixture harness-side MCP client, credential rejection
// (absent/wrong/expired), and redaction tripwires.

import {
  codexHarnessMcpConfigOverrides,
  HARNESS_MCP_ASSIGNMENT_CONTEXT_TOOL,
  HARNESS_MCP_ENDPOINT_PATH,
  HARNESS_MCP_FLEET_STATUS_TOOL,
  HARNESS_MCP_RECEIPT_LOOKUP_TOOL,
  HARNESS_MCP_SESSION_SCOPES,
  handleHarnessMcpRequest,
  mintHarnessMcpSessionCredential,
  startHarnessMcpServer,
  verifyHarnessMcpCredential,
  type HarnessMcpSessionContext,
} from "../src/harness-mcp-server"

const now = new Date("2026-07-14T00:00:00.000Z")

const receiptFixture: OpenAgentsMcpReceipt = {
  artifactRefs: ["artifact.public.pylon.codex_agent_task.example"],
  authorityClass: "workspace_read",
  generatedAt: now.toISOString(),
  kind: "read",
  receiptRef: "receipt.pylon.codex_agent_task.workspace_materialized.test",
  sourceRefs: ["run.pylon.codex_agent_task.test"],
  status: "recorded",
  summary: "Bounded assignment workspace materialized for this session.",
  targetRef: "workspace.pylon.codex_agent_task.test",
}

function sessionFixture(overrides: Partial<HarnessMcpSessionContext> = {}): HarnessMcpSessionContext {
  return {
    assignment: {
      assignmentRef: "assignment.public.codex_agent.test",
      leaseRef: "lease.public.codex_agent.test",
      objectivePublicSummary: "Fix the failing sum fixture test.",
      runRef: "run.pylon.codex_agent_task.test",
      verifyCommand: ["bun", "test", "sum.test.ts"],
      workflow: "codex_agent_task",
      workspaceRef: "workspace.pylon.codex_agent_task.test",
    },
    fleetStatus: () => [{
      phase: "running",
      threadRef: "run.pylon.codex_agent_task.test",
      tokensSoFar: 1200,
      tokenCountKind: "exact",
      updatedAtIso: now.toISOString(),
      workflow: "codex_agent_task",
    }],
    lookupReceipt: receiptRef => (receiptRef === receiptFixture.receiptRef ? receiptFixture : null),
    sessionRef: "run.pylon.codex_agent_task.test",
    ...overrides,
  }
}

/** Minimal fixture harness-side MCP client speaking JSON-RPC over HTTP. */
function fixtureMcpClient(url: string, token: string | undefined) {
  let nextId = 0
  return {
    call: async (method: string, params?: Record<string, unknown>) => {
      const response = await fetch(url, {
        body: JSON.stringify({
          id: `client.${++nextId}`,
          jsonrpc: "2.0",
          method,
          ...(params === undefined ? {} : { params }),
        }),
        headers: {
          "content-type": "application/json",
          ...(token === undefined ? {} : { authorization: `Bearer ${token}` }),
        },
        method: "POST",
      })
      return { body: await response.json() as Record<string, unknown>, status: response.status }
    },
  }
}

describe("harness MCP session credential", () => {
  test("mints a session-scoped read-only credential", () => {
    const credential = mintHarnessMcpSessionCredential({ now, sessionRef: "session.test" })
    expect(credential.sessionRef).toBe("session.test")
    expect(credential.token.startsWith("oahm_")).toBe(true)
    expect(credential.token.length).toBeGreaterThan(32)
    expect([...credential.scopes].sort()).toEqual([...HARNESS_MCP_SESSION_SCOPES].sort())
    expect(Date.parse(credential.expiresAtIso)).toBeGreaterThan(now.getTime())
  })

  test("narrowing-only: requesting scopes outside the read-only set rejects the mint", () => {
    expect(() =>
      mintHarnessMcpSessionCredential({
        now,
        requestedScopes: ["operator_read", "workspace_write"],
        sessionRef: "session.test",
      }),
    ).toThrow(/scope_widening_rejected/)
    expect(() =>
      mintHarnessMcpSessionCredential({
        now,
        requestedScopes: ["admin"],
        sessionRef: "session.test",
      }),
    ).toThrow(/scope_widening_rejected/)
  })

  test("narrowing to a subset is allowed", () => {
    const credential = mintHarnessMcpSessionCredential({
      now,
      requestedScopes: ["operator_read"],
      sessionRef: "session.test",
    })
    expect(credential.scopes).toEqual(["operator_read"])
  })

  test("verify rejects absent, wrong, and expired tokens", () => {
    const credential = mintHarnessMcpSessionCredential({ now, sessionRef: "session.test", ttlSeconds: 60 })
    expect(verifyHarnessMcpCredential({ credential, now, presentedToken: credential.token }))
      .toEqual({ ok: true })
    expect(verifyHarnessMcpCredential({ credential, now, presentedToken: undefined }))
      .toEqual({ ok: false, reason: "absent" })
    expect(verifyHarnessMcpCredential({ credential, now, presentedToken: "" }))
      .toEqual({ ok: false, reason: "absent" })
    expect(verifyHarnessMcpCredential({ credential, now, presentedToken: "oahm_wrong-token-value" }))
      .toEqual({ ok: false, reason: "mismatch" })
    expect(verifyHarnessMcpCredential({
      credential,
      now: new Date(now.getTime() + 61_000),
      presentedToken: credential.token,
    })).toEqual({ ok: false, reason: "expired" })
  })
})

describe("harness MCP server round trip", () => {
  test("fixture harness-side client lists tools and fetches assignment context", async () => {
    const server = startHarnessMcpServer({ now: () => new Date(), session: sessionFixture() })
    try {
      const client = fixtureMcpClient(server.url, server.credential.token)

      const initialized = await client.call("initialize")
      expect(initialized.status).toBe(200)
      const serverInfo = (initialized.body.result as { serverInfo: { name: string } }).serverInfo
      expect(serverInfo.name).toBe("openagents-harness-mcp")

      const listed = await client.call("tools/list")
      expect(listed.status).toBe(200)
      const tools = (listed.body.result as {
        tools: ReadonlyArray<{ name: string; annotations: { readOnlyHint: boolean } }>
      }).tools
      expect(tools.map(tool => tool.name).sort()).toEqual([
        HARNESS_MCP_ASSIGNMENT_CONTEXT_TOOL,
        HARNESS_MCP_FLEET_STATUS_TOOL,
        HARNESS_MCP_RECEIPT_LOOKUP_TOOL,
      ].sort())
      for (const tool of tools) {
        expect(tool.annotations.readOnlyHint).toBe(true)
      }

      const context = await client.call("tools/call", {
        arguments: {},
        name: HARNESS_MCP_ASSIGNMENT_CONTEXT_TOOL,
      })
      expect(context.status).toBe(200)
      const contextResult = context.body.result as {
        content: ReadonlyArray<{ text: string }>
        isError?: boolean
      }
      expect(contextResult.isError).not.toBe(true)
      const payload = JSON.parse(contextResult.content[0]?.text ?? "{}")
      expect(payload.assignmentRef).toBe("assignment.public.codex_agent.test")
      expect(payload.objectivePublicSummary).toBe("Fix the failing sum fixture test.")
      expect(payload.verifyCommand).toEqual(["bun", "test", "sum.test.ts"])

      const fleet = await client.call("tools/call", {
        arguments: {},
        name: HARNESS_MCP_FLEET_STATUS_TOOL,
      })
      const fleetPayload = JSON.parse(
        (fleet.body.result as { content: ReadonlyArray<{ text: string }> }).content[0]?.text ?? "{}",
      )
      expect(fleetPayload.threads).toHaveLength(1)
      expect(fleetPayload.threads[0].phase).toBe("running")
      expect(fleetPayload.threads[0].tokensSoFar).toBe(1200)

      const receipt = await client.call("tools/call", {
        arguments: { receiptRef: receiptFixture.receiptRef },
        name: HARNESS_MCP_RECEIPT_LOOKUP_TOOL,
      })
      const receiptPayload = JSON.parse(
        (receipt.body.result as { content: ReadonlyArray<{ text: string }> }).content[0]?.text ?? "{}",
      )
      expect(receiptPayload.receiptRef).toBe(receiptFixture.receiptRef)
      expect(receiptPayload.status).toBe("recorded")
      expect(receiptPayload.summary).toContain("workspace materialized")

      const missing = await client.call("tools/call", {
        arguments: { receiptRef: "receipt.does.not.exist" },
        name: HARNESS_MCP_RECEIPT_LOOKUP_TOOL,
      })
      const missingResult = missing.body.result as { isError?: boolean }
      expect(missingResult.isError).toBe(true)
    } finally {
      server.stop()
    }
  })

  test("rejects absent, wrong, and expired credentials with 401", async () => {
    const server = startHarnessMcpServer({ session: sessionFixture(), ttlSeconds: 3600 })
    try {
      const absent = await fixtureMcpClient(server.url, undefined).call("tools/list")
      expect(absent.status).toBe(401)
      expect(absent.body.error).toBe("invalid_mcp_credential")

      const wrong = await fixtureMcpClient(server.url, "oahm_not-the-real-token").call("tools/list")
      expect(wrong.status).toBe(401)
      expect(wrong.body.error).toBe("invalid_mcp_credential")
    } finally {
      server.stop()
    }

    // Expired: server clock moves past the credential expiry.
    let clock = new Date()
    const expiringServer = startHarnessMcpServer({
      now: () => clock,
      session: sessionFixture(),
      ttlSeconds: 60,
    })
    try {
      const client = fixtureMcpClient(expiringServer.url, expiringServer.credential.token)
      expect((await client.call("tools/list")).status).toBe(200)
      clock = new Date(clock.getTime() + 61_000)
      const expired = await client.call("tools/list")
      expect(expired.status).toBe(401)
    } finally {
      expiringServer.stop()
    }
  })

  test("unknown paths and non-POST methods are rejected", async () => {
    const server = startHarnessMcpServer({ session: sessionFixture() })
    try {
      const wrongPath = await fetch(`${server.origin}/other`, { method: "POST" })
      expect(wrongPath.status).toBe(404)
      const wrongMethod = await fetch(server.url, { method: "GET" })
      expect(wrongMethod.status).toBe(405)
      expect(server.endpointPath).toBe(HARNESS_MCP_ENDPOINT_PATH)
    } finally {
      server.stop()
    }
  })
})

describe("harness MCP redaction tripwires", () => {
  const bearerSecret = "Bearer oat_super-secret-token-material-12345"
  const apiKeySecret = "sk-abcdefghijklmnopqrstuvwxyz123456"
  const localPathSecret = "/Users/someowner/private/repo/notes.txt"

  test("secret-shaped receipt fields never surface", async () => {
    const poisonedReceipt: OpenAgentsMcpReceipt = {
      ...receiptFixture,
      receiptRef: "receipt.pylon.codex_agent_task.poisoned",
      summary: `workspace ready; auth was ${bearerSecret} and key ${apiKeySecret}`,
      targetRef: localPathSecret,
    }
    const session = sessionFixture({
      lookupReceipt: receiptRef =>
        receiptRef === poisonedReceipt.receiptRef ? poisonedReceipt : null,
    })
    const response = await handleHarnessMcpRequest(
      {
        id: "tripwire.1",
        jsonrpc: "2.0",
        method: "tools/call",
        params: {
          arguments: { receiptRef: poisonedReceipt.receiptRef },
          name: HARNESS_MCP_RECEIPT_LOOKUP_TOOL,
        },
      },
      session,
    )
    const serialized = JSON.stringify(response)
    expect(serialized).not.toContain("oat_super-secret-token-material-12345")
    expect(serialized).not.toContain(apiKeySecret)
    expect(serialized).not.toContain(localPathSecret)
  })

  test("secret-shaped assignment objective never surfaces", async () => {
    const session = sessionFixture({
      assignment: {
        ...sessionFixture().assignment,
        objectivePublicSummary: `ship it; access_token=deadbeefdeadbeef and ${apiKeySecret}`,
        workspaceRef: localPathSecret,
      },
    })
    const response = await handleHarnessMcpRequest(
      {
        id: "tripwire.2",
        jsonrpc: "2.0",
        method: "tools/call",
        params: { arguments: {}, name: HARNESS_MCP_ASSIGNMENT_CONTEXT_TOOL },
      },
      session,
    )
    const serialized = JSON.stringify(response)
    expect(serialized).not.toContain("deadbeefdeadbeef")
    expect(serialized).not.toContain(apiKeySecret)
    expect(serialized).not.toContain(localPathSecret)
  })

  test("mnemonic-shaped fleet status text never surfaces", async () => {
    const session = sessionFixture({
      fleetStatus: () => [{
        phase: "mnemonic: abandon ability able about above absent absorb abstract absurd abuse access accident",
        threadRef: "run.pylon.codex_agent_task.test",
        updatedAtIso: now.toISOString(),
        workflow: "codex_agent_task",
      }],
    })
    const response = await handleHarnessMcpRequest(
      {
        id: "tripwire.3",
        jsonrpc: "2.0",
        method: "tools/call",
        params: { arguments: {}, name: HARNESS_MCP_FLEET_STATUS_TOOL },
      },
      session,
    )
    const serialized = JSON.stringify(response)
    expect(serialized).not.toContain("abandon ability able")
  })
})

describe("codex harness MCP config overrides", () => {
  test("carries the server url and the token ENV VAR NAME only, never a token value", () => {
    const overrides = codexHarnessMcpConfigOverrides({
      tokenEnvVar: "OPENAGENTS_HARNESS_MCP_SESSION_TOKEN",
      url: "http://127.0.0.1:49152/mcp",
    })
    expect(overrides).toEqual({
      mcp_servers: {
        openagents: {
          bearer_token_env_var: "OPENAGENTS_HARNESS_MCP_SESSION_TOKEN",
          url: "http://127.0.0.1:49152/mcp",
        },
      },
    })
    expect(JSON.stringify(overrides)).not.toContain("oahm_")
  })
})
