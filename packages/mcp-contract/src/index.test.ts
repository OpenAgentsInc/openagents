import { describe, expect, test } from "bun:test"

import {
  decodeOpenAgentsMcpContractStatus,
  decodeOpenAgentsMcpGrant,
  decodeOpenAgentsMcpPromptDescriptor,
  decodeOpenAgentsMcpResourceDescriptor,
  decodeOpenAgentsMcpServerConfig,
  decodeOpenAgentsMcpToolDescriptor,
  decodeOpenAgentsMcpTransportConfig,
  filterOpenAgentsMcpDescriptorsByGrantSet,
  isValidOpenAgentsMcpName,
  isOpenAgentsMcpHighRiskAuthority,
  openAgentsMcpAuthorityClasses,
  openAgentsMcpConfigSources,
  openAgentsMcpContractStatus,
  openAgentsMcpHighRiskAuthorityClasses,
  openAgentsMcpLifecycleStatuses,
  openAgentsMcpResourceNamespaces,
  openAgentsMcpTransportKinds,
  parseOpenAgentsMcpResourceUri,
  projectOpenAgentsMcpServerConfigPublic,
  type OpenAgentsMcpGrant,
} from "./index.js"

describe("@openagentsinc/mcp-contract", () => {
  test("exports a phase 0 status without exposing runtime transports", () => {
    expect(decodeOpenAgentsMcpContractStatus(openAgentsMcpContractStatus)).toEqual({
      schemaVersion: "openagents.mcp.phase0.v1",
      packageName: "@openagentsinc/mcp-contract",
      phase: "phase_0_contract_groundwork",
      runtimeTransportExposed: false,
    })
  })

  test("decodes every authority class and classifies high-risk grants", () => {
    expect(openAgentsMcpAuthorityClasses).toHaveLength(13)
    for (const authorityClass of openAgentsMcpAuthorityClasses) {
      const decoded = decodeOpenAgentsMcpGrant({
        grantRef: `grant.test.${authorityClass}`,
        subjectRef: "client.test.operator",
        authorityClass,
        decision: "granted",
        scopeRefs: ["scope.test"],
        grantedAt: "2026-06-22T00:00:00.000Z",
        sourceRefs: ["github:OpenAgentsInc/openagents#5936"],
      })
      expect(decoded.authorityClass).toBe(authorityClass)
    }

    expect(openAgentsMcpHighRiskAuthorityClasses).toEqual([
      "workspace_write",
      "payment_spend",
      "deployment",
      "admin",
    ])
    expect(isOpenAgentsMcpHighRiskAuthority("payment_spend")).toBe(true)
    expect(isOpenAgentsMcpHighRiskAuthority("payment_receive")).toBe(false)
  })

  test("filters ungranted descriptors out of list results", () => {
    const descriptors = [
      { name: "pylon.health", requiredAuthorities: ["public_read"] as const },
      { name: "pylon.account.read", requiredAuthorities: ["private_account_read"] as const },
      { name: "pylon.session.cancel", requiredAuthorities: ["coding_session_control"] as const },
    ]
    const grants: ReadonlyArray<OpenAgentsMcpGrant> = [
      {
        grantRef: "grant.test.public_read",
        subjectRef: "client.test.read_only",
        authorityClass: "public_read",
        decision: "granted",
        scopeRefs: ["scope.test"],
        grantedAt: "2026-06-22T00:00:00.000Z",
        sourceRefs: ["github:OpenAgentsInc/openagents#5936"],
      },
      {
        grantRef: "grant.test.private_account_read.denied",
        subjectRef: "client.test.read_only",
        authorityClass: "private_account_read",
        decision: "denied",
        scopeRefs: ["scope.test"],
        grantedAt: "2026-06-22T00:00:00.000Z",
        sourceRefs: ["github:OpenAgentsInc/openagents#5936"],
      },
    ]

    expect(filterOpenAgentsMcpDescriptorsByGrantSet(descriptors, grants).map((d) => d.name))
      .toEqual(["pylon.health"])
  })

  test("keeps high-risk tools absent without explicit grants", () => {
    const descriptors = [
      { name: "pylon.wallet.status", requiredAuthorities: ["payment_read"] as const },
      { name: "pylon.wallet.spend", requiredAuthorities: ["payment_spend"] as const },
      { name: "autopilot.deploy.start", requiredAuthorities: ["deployment"] as const },
      { name: "openagents.admin.reconcile", requiredAuthorities: ["admin"] as const },
    ]
    const grants: ReadonlyArray<OpenAgentsMcpGrant> = [
      {
        grantRef: "grant.test.payment_read",
        subjectRef: "client.test.wallet_read",
        authorityClass: "payment_read",
        decision: "granted",
        scopeRefs: ["scope.test"],
        grantedAt: "2026-06-22T00:00:00.000Z",
        sourceRefs: ["github:OpenAgentsInc/openagents#5936"],
      },
    ]

    expect(filterOpenAgentsMcpDescriptorsByGrantSet(descriptors, grants).map((d) => d.name))
      .toEqual(["pylon.wallet.status"])
  })

  test("decodes every planned transport kind", () => {
    const configs = [
      {
        kind: "stdio",
        label: "Local Pylon stdio",
        commandRef: "command.pylon.local",
        argumentRefs: ["arg.safe.pylon"],
        sourceRefs: ["github:OpenAgentsInc/openagents#5937"],
      },
      {
        kind: "loopback_http",
        label: "Local loopback",
        origin: "http://127.0.0.1:3939",
        streamPath: "/mcp",
        sourceRefs: ["github:OpenAgentsInc/openagents#5937"],
      },
      {
        kind: "streamable_http",
        label: "Remote streamable HTTP",
        origin: "https://openagents.com",
        endpointPath: "/api/mcp",
        authRef: "credential.ref",
        sourceRefs: ["github:OpenAgentsInc/openagents#5937"],
      },
      {
        kind: "sse",
        label: "Remote SSE",
        origin: "https://openagents.com",
        eventsPath: "/api/mcp/events",
        messagesPath: "/api/mcp/messages",
        authRef: "credential.ref",
        sourceRefs: ["github:OpenAgentsInc/openagents#5937"],
      },
      {
        kind: "websocket",
        label: "Bridge WebSocket",
        url: "wss://openagents.com/mcp/ws",
        protocolRef: "openagents.mcp.websocket.v1",
        authRef: "credential.ref",
        sourceRefs: ["github:OpenAgentsInc/openagents#5937"],
      },
      {
        kind: "ide_local",
        label: "IDE local MCP",
        ideRef: "ide.cursor.local",
        serverRef: "mcp.server.ide.cursor",
        sourceRefs: ["github:OpenAgentsInc/openagents#5937"],
      },
      {
        kind: "in_process",
        label: "Desktop in-process",
        runtimeRef: "runtime.autopilot.desktop",
        serviceRef: "service.mcp.desktop",
        sourceRefs: ["github:OpenAgentsInc/openagents#5937"],
      },
      {
        kind: "bridge_proxy",
        label: "Pylon bridge proxy",
        bridgeRef: "bridge.pylon.local",
        targetRef: "pylon.local.node",
        authRef: "credential.ref",
        sourceRefs: ["github:OpenAgentsInc/openagents#5937"],
      },
    ]

    expect(configs.map((config) => decodeOpenAgentsMcpTransportConfig(config).kind))
      .toEqual([...openAgentsMcpTransportKinds])
  })

  test("tracks every config source and lifecycle status", () => {
    expect(openAgentsMcpConfigSources).toEqual([
      "local_private",
      "shared_project",
      "user",
      "managed",
      "dynamic",
      "plugin",
      "ide",
      "desktop_discovered",
    ])
    expect(openAgentsMcpLifecycleStatuses).toEqual([
      "discovered",
      "pending_approval",
      "enabled",
      "connecting",
      "connected",
      "needs_auth",
      "disabled",
      "rejected",
      "failed",
      "revoked",
      "blocked_by_policy",
    ])
  })

  test("projects transport config without secret refs for public/debug views", () => {
    const config = decodeOpenAgentsMcpServerConfig({
      serverRef: "mcp.server.private.codex",
      displayName: "Private Codex tools",
      source: "local_private",
      lifecycleStatus: "needs_auth",
      requestedAuthorities: ["workspace_read", "coding_session_control"],
      secretRefs: [
        "credential.local.oauth_token.codex",
        "credential.local.bearer_token.codex",
      ],
      sourceRefs: ["github:OpenAgentsInc/openagents#5937"],
      transport: {
        kind: "streamable_http",
        label: "Private Codex stream",
        origin: "https://example.invalid",
        endpointPath: "/mcp",
        authRef: "credential.local.oauth_token.codex",
        sourceRefs: ["github:OpenAgentsInc/openagents#5937"],
      },
    })

    const projection = projectOpenAgentsMcpServerConfigPublic(config)
    expect(projection).toEqual({
      serverRef: "mcp.server.private.codex",
      displayName: "Private Codex tools",
      source: "local_private",
      lifecycleStatus: "needs_auth",
      transportKind: "streamable_http",
      requestedAuthorities: ["workspace_read", "coding_session_control"],
      sourceRefs: ["github:OpenAgentsInc/openagents#5937"],
    })
    expect(JSON.stringify(projection)).not.toContain("oauth_token")
    expect(JSON.stringify(projection)).not.toContain("bearer_token")
    expect(JSON.stringify(projection)).not.toContain("secretRefs")
    expect(JSON.stringify(projection)).not.toContain("authRef")
  })

  test("decodes tool, resource, and prompt descriptors with grant metadata", () => {
    const tool = decodeOpenAgentsMcpToolDescriptor({
      name: "pylon.health",
      title: "Pylon health",
      description: "Read local Pylon health without mutation.",
      requiredAuthorities: ["public_read"],
      riskClass: "read_only",
      inputSchemaRef: "schema.openagents.mcp.pylon.health.input.v1",
      outputSchemaRef: "schema.openagents.mcp.pylon.health.output.v1",
      receiptBehavior: "read_receipt",
      progressBehavior: "none",
      publicSummary: "Read-only health check.",
      sourceRefs: ["github:OpenAgentsInc/openagents#5938"],
    })
    const resource = decodeOpenAgentsMcpResourceDescriptor({
      uri: "mcp://openagents/verse/scene-state",
      name: "verse.scene.state",
      title: "Verse scene state",
      description: "Read the current Verse scene projection.",
      namespace: "verse",
      requiredAuthorities: ["operator_read"],
      staleness: {
        maxStalenessSeconds: 1,
        transitionRefs: ["verse.scene_projection_updated"],
      },
      publicProjectionSafe: false,
      sourceRefs: ["github:OpenAgentsInc/openagents#5938"],
    })
    const prompt = decodeOpenAgentsMcpPromptDescriptor({
      name: "coding.session.summarize",
      title: "Summarize coding session",
      description: "Ask an agent to summarize a coding session.",
      audience: "coding_agent",
      requiredAuthorities: ["coding_session_control"],
      inputSchemaRef: "schema.openagents.mcp.coding.session.summarize.input.v1",
      outputHandling: "operator_only",
      sourceRefs: ["github:OpenAgentsInc/openagents#5938"],
    })

    expect(tool.requiredAuthorities).toEqual(["public_read"])
    expect(resource.namespace).toBe("verse")
    expect(prompt.outputHandling).toBe("operator_only")
  })

  test("enforces stable lowercase dotted names", () => {
    const validNames = [
      "pylon.health",
      "autopilot.desktop.status",
      "verse.scene.state",
      "worker.public.stats",
      "forum.post.tip",
      "payments.pylon.receive",
      "coding.session.spawn",
    ]
    for (const name of validNames) {
      expect(isValidOpenAgentsMcpName(name)).toBe(true)
    }

    const invalidNames = [
      "pylon",
      "Pylon.health",
      "pylon.health/now",
      "pylon health",
      "pylon.health.0780e08837abcdef",
      "pylon.health#fragment",
      "pylon.health_v2",
    ]
    for (const name of invalidNames) {
      expect(isValidOpenAgentsMcpName(name)).toBe(false)
    }
  })

  test("parses all planned resource namespaces", () => {
    expect(openAgentsMcpResourceNamespaces).toEqual([
      "pylon",
      "autopilot",
      "verse",
      "worker",
      "forum",
      "payments",
      "coding-session",
    ])
    const uris = [
      "mcp://openagents/pylon/node-status",
      "mcp://openagents/autopilot/desktop/status",
      "mcp://openagents/verse/scene-state",
      "mcp://openagents/worker/public-stats",
      "mcp://openagents/forum/post/public-activity",
      "mcp://openagents/payments/receipt/recent",
      "mcp://openagents/coding-session/session/current",
    ]

    expect(uris.map((uri) => parseOpenAgentsMcpResourceUri(uri).namespace))
      .toEqual([...openAgentsMcpResourceNamespaces])
    expect(() => parseOpenAgentsMcpResourceUri("https://openagents.com/pylon")).toThrow(
      "Invalid OpenAgents MCP resource URI",
    )
    expect(() => parseOpenAgentsMcpResourceUri("mcp://openagents/pylon/../secret")).toThrow(
      "Invalid OpenAgents MCP resource URI",
    )
  })
})
