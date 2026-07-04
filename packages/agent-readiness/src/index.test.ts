import { describe, expect, test } from "bun:test"

import {
  AGENT_READINESS_REPORT_SCHEMA_VERSION,
  agentReadinessTaskForDomain,
  decodeAgentReadinessReport,
  defaultAgentReadinessProbeSet,
  normalizeAgentReadinessTarget,
  parseAgentReadinessCliArgs,
  runAgentReadinessBatch,
  scanAgentReadinessDomain,
} from "./index.js"

const json = (body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  })

const text = (body: string, contentType = "text/plain", status = 200): Response =>
  new Response(body, {
    status,
    headers: { "content-type": contentType },
  })

const openAgentsHomepage = `<!doctype html>
<html>
  <head>
    <title>OpenAgents</title>
    <meta name="description" content="OpenAgents sells agent-ready software services and public machine-readable interfaces." />
    <link rel="canonical" href="https://openagents.com/" />
    <link rel="mcp" href="/.well-known/mcp.json" />
    <link rel="ai-catalog" href="/.well-known/ai-catalog.json" />
    <link rel="openapi" href="/api/openapi.json" />
    <script type="application/ld+json">{"@context":"https://schema.org","@type":"Organization","name":"OpenAgents"}</script>
  </head>
  <body>
    <main>
      <h1>OpenAgents</h1>
      <p>OpenAgents builds agent-ready business automation, public discovery surfaces, MCP manifests,
      OpenAI-compatible APIs, proof receipts, product promises, and operator-reviewed quick wins.</p>
      <p>Agents can inspect llms.txt, AGENTS.md, API docs, product promises, and well-known manifests.</p>
    </main>
  </body>
</html>`

const spaShell = `<!doctype html><html><head><title>SPA</title></head><body><div id="root"></div><script src="/assets/app.js"></script></body></html>`

const fixtureFetch = (kind: "openagents" | "spa"): typeof fetch =>
  (async (input, init) => {
    const url = new URL(String(input))
    if (kind === "openagents") {
      switch (url.pathname) {
        case "/.well-known/mcp.json":
        case "/.well-known/mcp/manifest.json":
          return json({ servers: [{ name: "openagents", transport: "streamable_http" }] })
        case "/mcp/manifest.json":
          return text("not found", "text/plain", 404)
        case "/.well-known/ai-catalog.json":
          return json({ schemaVersion: "ai-catalog.v1", resources: ["llms.txt"] })
        case "/robots.txt":
          return text("User-agent: *\nAllow: /\nSitemap: https://openagents.com/sitemap.xml\n")
        case "/sitemap.xml":
          return text("<urlset><url><loc>https://openagents.com/.well-known/mcp.json</loc></url></urlset>", "application/xml")
        case "/llms.txt":
        case "/llms-full.txt":
          return text("# OpenAgents\nAgent-readable resources and API discovery.")
        case "/":
          expect(init?.headers).toBeDefined()
          return text(openAgentsHomepage, "text/html")
        default:
          return text("not found", "text/plain", 404)
      }
    }
    switch (url.pathname) {
      case "/.well-known/mcp.json":
      case "/.well-known/mcp/manifest.json":
      case "/mcp/manifest.json":
      case "/.well-known/ai-catalog.json":
        return text(spaShell, "text/html")
      case "/robots.txt":
        return text(spaShell, "text/html")
      case "/sitemap.xml":
      case "/llms.txt":
      case "/llms-full.txt":
        return text("not found", "text/plain", 404)
      case "/":
        return text(spaShell, "text/html")
      default:
        return text("not found", "text/plain", 404)
    }
  }) as typeof fetch

describe("@openagentsinc/agent-readiness", () => {
  test("declares a config-driven probe set for every LG-1 surface", () => {
    expect(defaultAgentReadinessProbeSet.map((probe) => probe.path)).toEqual([
      "/.well-known/mcp.json",
      "/.well-known/mcp/manifest.json",
      "/mcp/manifest.json",
      "/.well-known/ai-catalog.json",
      "/robots.txt",
      "/sitemap.xml",
      "/llms.txt",
      "/llms-full.txt",
      "/",
      "/",
      "/",
    ])
    expect(defaultAgentReadinessProbeSet.map((probe) => probe.expectedContent))
      .toContain("agent_render_diff")
  })

  test("rejects local, private, credentialed, and non-http targets", () => {
    expect(() => normalizeAgentReadinessTarget("localhost:3000")).toThrow()
    expect(() => normalizeAgentReadinessTarget("http://127.0.0.1:8787")).toThrow()
    expect(() => normalizeAgentReadinessTarget("http://192.168.1.4")).toThrow()
    expect(() => normalizeAgentReadinessTarget("https://user:pass@example.com")).toThrow()
    expect(() => normalizeAgentReadinessTarget("file:///tmp/report")).toThrow()
    expect(normalizeAgentReadinessTarget("example.com").toString()).toBe("https://example.com/")
  })

  test("our own public-safe fixture passes with typed report output", async () => {
    const report = await scanAgentReadinessDomain("openagents.com", {
      fetch: fixtureFetch("openagents"),
      generatedAt: "2026-07-04T06:30:00.000Z",
      minRequestIntervalMs: 0,
    })

    expect(report.schemaVersion).toBe(AGENT_READINESS_REPORT_SCHEMA_VERSION)
    expect(report.domain).toBe("openagents.com")
    expect(report.status).toBe("passed")
    expect(report.score).toBe(100)
    expect(report.grade).toBe("A")
    expect(report.layerScores).toContainEqual({
      layer: "payments",
      score: 0,
      earned: 0,
      possible: 0,
      status: "not_applicable",
    })
    expect(report.findings).toEqual([])
    expect(decodeAgentReadinessReport(report)).toEqual(report)
  })

  test("a fixture SPA-shell domain fails with exact readiness findings", async () => {
    const report = await scanAgentReadinessDomain("broken-spa.example", {
      fetch: fixtureFetch("spa"),
      generatedAt: "2026-07-04T06:30:00.000Z",
      minRequestIntervalMs: 0,
    })

    const codes = report.findings.map((finding) => finding.code).sort()
    expect(report.status).toBe("attention")
    expect(report.score).toBe(0)
    expect(report.grade).toBe("F")
    expect(report.layerScores.find((layer) => layer.layer === "discovery")).toMatchObject({
      earned: 0,
      possible: 48,
      status: "attention",
    })
    expect(codes).toContain("spa_shell_json")
    expect(codes).toContain("spa_shell_surface")
    expect(codes).toContain("missing_sitemap_xml")
    expect(codes).toContain("missing_llms_txt")
    expect(codes).toContain("missing_structured_data")
    expect(codes).toContain("agent_empty_shell")
    expect(codes).toContain("api_discoverability_links_missing")
    expect(report.topFindings).toHaveLength(3)
    expect(JSON.stringify(report)).not.toContain("<div id=\"root\"")
  })

  test("exports a fleet-dispatchable one-domain task shape", () => {
    expect(agentReadinessTaskForDomain("https://openagents.com")).toEqual({
      schemaVersion: "openagents.agent_readiness_domain_task.v1",
      domain: "openagents.com",
      analyzerRef: "@openagentsinc/agent-readiness/default",
      maxWorkerCount: 1,
      timeoutMs: 5000,
      outputSchema: "openagents.agent_readiness_report.v1",
      sourceRefs: ["github:OpenAgentsInc/openagents#8262"],
    })
  })

  test("runs bounded batch scans and parses CLI modes", async () => {
    const batch = await runAgentReadinessBatch(
      ["openagents.com", "broken-spa.example"],
      {
        fetch: async (input, init) => {
          const host = new URL(String(input)).hostname
          return fixtureFetch(host === "openagents.com" ? "openagents" : "spa")(input, init)
        },
        generatedAt: "2026-07-04T06:30:00.000Z",
        minRequestIntervalMs: 0,
        concurrency: 2,
      },
    )
    expect(batch.reports.map((report) => report.status)).toEqual(["passed", "attention"])
    expect(parseAgentReadinessCliArgs(["scan", "openagents.com", "--json"]).domain)
      .toBe("openagents.com")
    expect(parseAgentReadinessCliArgs(["scan", "--batch", "domains.txt", "--concurrency", "2"]).batchFile)
      .toBe("domains.txt")
  })
})
