import { describe, expect, test } from "bun:test"

import {
  AGENT_READINESS_REPORT_RENDER_SCHEMA_VERSION,
  AGENT_READINESS_REPORT_SCHEMA_VERSION,
  MODEL_CUSTODY_ANALYZER_CONFIG,
  MODEL_CUSTODY_REPORT_SCHEMA_VERSION,
  agentReadinessTaskForDomain,
  analyzeModelCustodyPublicSurfaces,
  decodeAgentReadinessReport,
  decodeAgentReadinessReportRender,
  defaultAgentReadinessProbeSet,
  normalizeAgentReadinessTarget,
  parseAgentReadinessCliArgs,
  renderAgentReadinessCaseStudyArtifact,
  renderAgentReadinessReport,
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

const modelCustodyFetch = (async (input) => {
  const url = new URL(String(input))
  switch (url.pathname) {
    case "/subprocessors":
      return text(
        "Subprocessors include OpenAI API, Anthropic Claude, and Google Vertex AI for optional AI features.",
        "text/html",
      )
    case "/privacy":
      return text(
        "Privacy notice: customer content may be processed by AI assistants. Prompt data is retained for abuse monitoring; vendor foundation models are not trained unless a customer opts in.",
        "text/html",
      )
    case "/careers":
      return text(
        "Hiring AI Platform Engineer with Azure OpenAI, AWS Bedrock, and LLM evaluation experience.",
        "text/html",
      )
    default:
      return text("not found", "text/plain", 404)
  }
}) as typeof fetch

const openAgentsReportFixtureUrl = new URL(
  "../fixtures/openagents-com-report.json",
  import.meta.url,
)
const openAgentsRenderFixtureUrl = new URL(
  "../fixtures/openagents-com-render-case-study.md",
  import.meta.url,
)

const openAgentsReportFixture = async () =>
  decodeAgentReadinessReport(
    JSON.parse(await Bun.file(openAgentsReportFixtureUrl).text()) as unknown,
  )

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

  test("renders our own public-safe report as the case-study snapshot artifact", async () => {
    const report = await openAgentsReportFixture()
    const rendered = renderAgentReadinessCaseStudyArtifact(report, {
      generatedAt: "2026-07-04T07:00:00.000Z",
    })

    expect(rendered).toBe(
      (await Bun.file(openAgentsRenderFixtureUrl).text()).trimEnd(),
    )
    expect(renderAgentReadinessReport(report, {
      generatedAt: "2026-07-04T07:00:00.000Z",
    }).persistenceMode).toBe("repo_case_study_allowed")
  })

  test("renders prospect-style reports in memory with a separate held-back bump finding", async () => {
    const report = await scanAgentReadinessDomain("broken-spa.example", {
      fetch: fixtureFetch("spa"),
      generatedAt: "2026-07-04T06:30:00.000Z",
      minRequestIntervalMs: 0,
    })
    const contexts = Object.fromEntries(
      report.findings.map((finding, index) => [
        finding.findingRef,
        index === 0
          ? "Your <catalog> buyers lose the machine-readable path before evaluation."
          : `Commercial context for ${finding.code} in a browser-based buying flow.`,
      ]),
    )
    const rendered = renderAgentReadinessReport(report, {
      commercialContextByFindingRef: contexts,
      generatedAt: "2026-07-04T07:00:00.000Z",
    })

    expect(rendered.schemaVersion).toBe(AGENT_READINESS_REPORT_RENDER_SCHEMA_VERSION)
    expect(decodeAgentReadinessReportRender(rendered)).toEqual(rendered)
    expect(rendered.persistenceMode).toBe("private_runtime_only")
    expect(rendered.topFindings).toHaveLength(3)
    expect(rendered.heldBackFinding).not.toBeNull()
    expect(rendered.emailBodyPlainText).toContain("Top findings:")
    expect(rendered.emailBodyPlainText).toContain(rendered.topFindings[0]?.title)
    expect(rendered.emailBodyPlainText).not.toContain(
      rendered.heldBackFinding?.evidenceRefs[0] ?? "",
    )
    expect(rendered.bumpBodyPlainText).toContain(rendered.heldBackFinding?.title ?? "")
    expect(rendered.bumpBodyPlainText).toContain(
      rendered.heldBackFinding?.evidenceRefs[0] ?? "",
    )
    expect(rendered.emailBodyHtml).toContain("Your &lt;catalog&gt; buyers")
    expect(rendered.emailBodyHtml).not.toContain("Your <catalog> buyers")
    expect(() => renderAgentReadinessCaseStudyArtifact(report, {
      commercialContextByFindingRef: contexts,
      generatedAt: "2026-07-04T07:00:00.000Z",
    })).toThrow(/own-domain report/)
  })

  test("requires one-line commercial context for every rendered finding", async () => {
    const report = await scanAgentReadinessDomain("broken-spa.example", {
      fetch: fixtureFetch("spa"),
      generatedAt: "2026-07-04T06:30:00.000Z",
      minRequestIntervalMs: 0,
    })

    expect(() => renderAgentReadinessReport(report, {
      generatedAt: "2026-07-04T07:00:00.000Z",
    })).toThrow(/Missing commercial context/)
    expect(() => renderAgentReadinessReport(report, {
      commercialContextByCode: {
        agent_empty_shell: "This context is present only for one code.",
      },
      generatedAt: "2026-07-04T07:00:00.000Z",
    })).toThrow(/Missing commercial context/)
  })

  test("declares the RX-8 model-custody analyzer as public-only and no-speculation", () => {
    expect(MODEL_CUSTODY_ANALYZER_CONFIG).toMatchObject({
      analyzerRef: "@openagentsinc/agent-readiness/model-custody.v1",
      campaignRef: "campaign.own_your_ai",
      dossierFormatRef: "dossier.model_custody.public_facts.v1",
      evidenceBoundary: {
        publicUrlsOnly: true,
        rawPageBodiesStored: false,
        speculationAllowed: false,
      },
      sourceRef: "apollo_model_custody",
    })
    expect(MODEL_CUSTODY_ANALYZER_CONFIG.disallowedClaimRefs).toEqual(
      expect.arrayContaining([
        "claim_lint.hipaa_sovereign",
        "claim_lint.published_prices",
        "claim_lint.customer_data_transfer_inferred",
        "claim_lint.provider_training_inferred",
      ]),
    )
    expect(MODEL_CUSTODY_ANALYZER_CONFIG.probes.map((probe) => probe.kind))
      .toEqual(expect.arrayContaining([
        "subprocessors_dpa",
        "privacy_training_terms",
        "ai_feature_disclosure",
        "careers_tech_stack",
      ]))
  })

  test("records model-custody findings only as reproducible public facts", async () => {
    const report = await analyzeModelCustodyPublicSurfaces(
      "regulated-saas.example",
      {
        fetch: modelCustodyFetch,
        generatedAt: "2026-07-04T17:30:00.000Z",
        minRequestIntervalMs: 0,
      },
    )

    expect(report.schemaVersion).toBe(MODEL_CUSTODY_REPORT_SCHEMA_VERSION)
    expect(report.sourceRef).toBe("apollo_model_custody")
    expect(report.status).toBe("signals_observed")
    expect(report.findings.map((finding) => finding.code)).toEqual(
      expect.arrayContaining([
        "frontier_lab_subprocessor_named",
        "frontier_lab_stack_hiring_signal",
        "privacy_training_terms_published",
      ]),
    )
    for (const finding of report.findings) {
      expect(finding.factuality).toBe("public_surface_only")
      expect(finding.speculationAllowed).toBe(false)
      expect(finding.inferenceBoundary).toMatch(/does not|not proof|not infer/i)
      expect(finding.publicStatement).not.toMatch(
        /\b(probably|likely|must|definitely)\b/i,
      )
      expect(
        finding.evidence.every((item) =>
          item.url.startsWith("https://regulated-saas.example/"),
        ),
      ).toBe(true)
    }
    expect(JSON.stringify(report)).not.toContain("Prompt data is retained")
  })

  test("model-custody analyzer refuses local and credentialed targets", async () => {
    const local = await analyzeModelCustodyPublicSurfaces("localhost:8787", {
      generatedAt: "2026-07-04T17:30:00.000Z",
    })
    const credentialed = await analyzeModelCustodyPublicSurfaces(
      "https://user:pass@example.com",
      { generatedAt: "2026-07-04T17:30:00.000Z" },
    )

    expect(local.status).toBe("blocked")
    expect(credentialed.status).toBe("blocked")
    expect(local.findings[0]?.code).toBe("target_disallowed")
    expect(credentialed.findings[0]?.speculationAllowed).toBe(false)
  })
})
