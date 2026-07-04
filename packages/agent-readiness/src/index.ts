import { Schema as S } from "effect"

export const AGENT_READINESS_FINDING_SCHEMA_VERSION =
  "openagents.agent_readiness_finding.v1" as const
export const AGENT_READINESS_REPORT_SCHEMA_VERSION =
  "openagents.agent_readiness_report.v1" as const
export const AGENT_READINESS_DOMAIN_TASK_SCHEMA_VERSION =
  "openagents.agent_readiness_domain_task.v1" as const
export const AGENT_READINESS_BATCH_SCHEMA_VERSION =
  "openagents.agent_readiness_batch.v1" as const
export const AGENT_READINESS_REPORT_RENDER_SCHEMA_VERSION =
  "openagents.agent_readiness_report_render.v1" as const

export const AgentReadinessSeverity = S.Literals([
  "critical",
  "high",
  "medium",
  "low",
  "info",
])
export type AgentReadinessSeverity = typeof AgentReadinessSeverity.Type

export const AgentReadinessProbeKind = S.Literals([
  "mcp_discovery",
  "ai_catalog",
  "crawl_surface",
  "llm_guidance",
  "structured_data",
  "agent_rendering",
  "api_discoverability",
])
export type AgentReadinessProbeKind = typeof AgentReadinessProbeKind.Type

export const AgentReadinessLayer = S.Literals([
  "discovery",
  "identity",
  "access",
  "payments",
  "experience",
])
export type AgentReadinessLayer = typeof AgentReadinessLayer.Type

export const AgentReadinessMaturity = S.Literals(["verified", "emerging"])
export type AgentReadinessMaturity = typeof AgentReadinessMaturity.Type

export const AgentReadinessGrade = S.Literals(["A", "B", "C", "D", "F"])
export type AgentReadinessGrade = typeof AgentReadinessGrade.Type

export const AgentReadinessFindingCode = S.Literals([
  "disallowed_target_url",
  "fetch_failed",
  "missing_mcp_discovery",
  "missing_ai_catalog",
  "missing_robots_txt",
  "missing_sitemap_xml",
  "missing_llms_txt",
  "invalid_json",
  "spa_shell_json",
  "spa_shell_surface",
  "missing_structured_data",
  "agent_empty_shell",
  "api_discoverability_links_missing",
])
export type AgentReadinessFindingCode = typeof AgentReadinessFindingCode.Type

export const AgentReadinessEvidence = S.Struct({
  ref: S.String,
  url: S.String,
  status: S.NullOr(S.Number),
  contentType: S.NullOr(S.String),
})
export type AgentReadinessEvidence = typeof AgentReadinessEvidence.Type

export const AgentReadinessFinding = S.Struct({
  schemaVersion: S.Literal(AGENT_READINESS_FINDING_SCHEMA_VERSION),
  findingRef: S.String,
  domain: S.String,
  probeId: S.String,
  kind: AgentReadinessProbeKind,
  code: AgentReadinessFindingCode,
  severity: AgentReadinessSeverity,
  title: S.String,
  impact: S.String,
  evidenceRefs: S.Array(S.String),
  evidence: S.Array(AgentReadinessEvidence),
  observedAt: S.String,
})
export type AgentReadinessFinding = typeof AgentReadinessFinding.Type

export const AgentReadinessTopFinding = S.Struct({
  code: AgentReadinessFindingCode,
  severity: AgentReadinessSeverity,
  title: S.String,
  impact: S.String,
  evidenceRefs: S.Array(S.String),
})
export type AgentReadinessTopFinding = typeof AgentReadinessTopFinding.Type

export const AgentReadinessLayerScore = S.Struct({
  layer: AgentReadinessLayer,
  score: S.Number,
  earned: S.Number,
  possible: S.Number,
  status: S.Literals(["passed", "attention", "not_applicable"]),
})
export type AgentReadinessLayerScore = typeof AgentReadinessLayerScore.Type

export const AgentReadinessFindingCounts = S.Struct({
  critical: S.Number,
  high: S.Number,
  medium: S.Number,
  low: S.Number,
  info: S.Number,
})
export type AgentReadinessFindingCounts = typeof AgentReadinessFindingCounts.Type

export const AgentReadinessReport = S.Struct({
  schemaVersion: S.Literal(AGENT_READINESS_REPORT_SCHEMA_VERSION),
  domain: S.String,
  baseUrl: S.String,
  generatedAt: S.String,
  status: S.Literals(["passed", "attention", "blocked"]),
  score: S.Number,
  grade: AgentReadinessGrade,
  layerScores: S.Array(AgentReadinessLayerScore),
  summary: S.String,
  topFindings: S.Array(AgentReadinessTopFinding),
  findingCounts: AgentReadinessFindingCounts,
  findings: S.Array(AgentReadinessFinding),
  sourceRefs: S.Array(S.String),
})
export type AgentReadinessReport = typeof AgentReadinessReport.Type

export const AgentReadinessDomainTask = S.Struct({
  schemaVersion: S.Literal(AGENT_READINESS_DOMAIN_TASK_SCHEMA_VERSION),
  domain: S.String,
  analyzerRef: S.Literal("@openagentsinc/agent-readiness/default"),
  maxWorkerCount: S.Literal(1),
  timeoutMs: S.Number,
  outputSchema: S.Literal(AGENT_READINESS_REPORT_SCHEMA_VERSION),
  sourceRefs: S.Array(S.String),
})
export type AgentReadinessDomainTask = typeof AgentReadinessDomainTask.Type

export const AgentReadinessBatchResult = S.Struct({
  schemaVersion: S.Literal(AGENT_READINESS_BATCH_SCHEMA_VERSION),
  generatedAt: S.String,
  reports: S.Array(AgentReadinessReport),
})
export type AgentReadinessBatchResult = typeof AgentReadinessBatchResult.Type

export const AgentReadinessRenderedFinding = S.Struct({
  findingRef: S.String,
  code: AgentReadinessFindingCode,
  severity: AgentReadinessSeverity,
  title: S.String,
  impact: S.String,
  commercialContext: S.String,
  evidenceRefs: S.Array(S.String),
})
export type AgentReadinessRenderedFinding =
  typeof AgentReadinessRenderedFinding.Type

export const AgentReadinessReportRender = S.Struct({
  schemaVersion: S.Literal(AGENT_READINESS_REPORT_RENDER_SCHEMA_VERSION),
  domain: S.String,
  generatedAt: S.String,
  reportGeneratedAt: S.String,
  reportStatus: S.Literals(["passed", "attention", "blocked"]),
  score: S.Number,
  grade: AgentReadinessGrade,
  persistenceMode: S.Literals([
    "private_runtime_only",
    "repo_case_study_allowed",
  ]),
  operatorFindings: S.Array(AgentReadinessRenderedFinding),
  topFindings: S.Array(AgentReadinessRenderedFinding),
  heldBackFinding: S.NullOr(AgentReadinessRenderedFinding),
  internalOperatorView: S.String,
  emailBodyPlainText: S.String,
  emailBodyHtml: S.String,
  bumpBodyPlainText: S.String,
  bumpBodyHtml: S.String,
  sourceRefs: S.Array(S.String),
})
export type AgentReadinessReportRender =
  typeof AgentReadinessReportRender.Type

export const AgentReadinessExpectedContent = S.Literals([
  "json",
  "text",
  "xml",
  "homepage_structured_data",
  "homepage_api_links",
  "agent_render_diff",
])
export type AgentReadinessExpectedContent =
  typeof AgentReadinessExpectedContent.Type

export const AgentReadinessProbeDefinition = S.Struct({
  id: S.String,
  kind: AgentReadinessProbeKind,
  layer: AgentReadinessLayer,
  maturity: AgentReadinessMaturity,
  points: S.Number,
  path: S.String,
  expectedContent: AgentReadinessExpectedContent,
  required: S.Boolean,
  requiredAnyGroup: S.optional(S.String),
  severity: AgentReadinessSeverity,
  missingCode: AgentReadinessFindingCode,
  missingTitle: S.String,
  missingImpact: S.String,
})
export type AgentReadinessProbeDefinition =
  typeof AgentReadinessProbeDefinition.Type

export const defaultAgentReadinessProbeSet: ReadonlyArray<AgentReadinessProbeDefinition> = [
  {
    id: "mcp_well_known",
    kind: "mcp_discovery",
    layer: "discovery",
    maturity: "verified",
    points: 18,
    path: "/.well-known/mcp.json",
    expectedContent: "json",
    required: true,
    requiredAnyGroup: "mcp_discovery",
    severity: "high",
    missingCode: "missing_mcp_discovery",
    missingTitle: "MCP discovery manifest is missing",
    missingImpact: "Agents cannot discover callable business capabilities from the standard well-known manifest.",
  },
  {
    id: "mcp_well_known_manifest",
    kind: "mcp_discovery",
    layer: "discovery",
    maturity: "emerging",
    points: 0,
    path: "/.well-known/mcp/manifest.json",
    expectedContent: "json",
    required: true,
    requiredAnyGroup: "mcp_discovery",
    severity: "high",
    missingCode: "missing_mcp_discovery",
    missingTitle: "MCP discovery manifest alias is missing",
    missingImpact: "Agents following the manifest alias cannot discover callable business capabilities.",
  },
  {
    id: "mcp_manifest",
    kind: "mcp_discovery",
    layer: "discovery",
    maturity: "emerging",
    points: 0,
    path: "/mcp/manifest.json",
    expectedContent: "json",
    required: true,
    requiredAnyGroup: "mcp_discovery",
    severity: "high",
    missingCode: "missing_mcp_discovery",
    missingTitle: "MCP discovery manifest path is missing",
    missingImpact: "Agents following the /mcp/manifest.json convention cannot discover callable business capabilities.",
  },
  {
    id: "ai_catalog",
    kind: "ai_catalog",
    layer: "discovery",
    maturity: "verified",
    points: 14,
    path: "/.well-known/ai-catalog.json",
    expectedContent: "json",
    required: true,
    severity: "high",
    missingCode: "missing_ai_catalog",
    missingTitle: "AI catalog is missing",
    missingImpact: "Agent directories cannot read a bounded catalog of the site's AI-facing resources.",
  },
  {
    id: "robots",
    kind: "crawl_surface",
    layer: "discovery",
    maturity: "verified",
    points: 8,
    path: "/robots.txt",
    expectedContent: "text",
    required: true,
    severity: "medium",
    missingCode: "missing_robots_txt",
    missingTitle: "robots.txt is missing or unreadable",
    missingImpact: "Crawlers and agents lose the site's explicit crawl policy and sitemap hints.",
  },
  {
    id: "sitemap",
    kind: "crawl_surface",
    layer: "discovery",
    maturity: "verified",
    points: 8,
    path: "/sitemap.xml",
    expectedContent: "xml",
    required: true,
    severity: "medium",
    missingCode: "missing_sitemap_xml",
    missingTitle: "sitemap.xml is missing or unreadable",
    missingImpact: "Agents cannot cheaply enumerate the public pages they should inspect.",
  },
  {
    id: "llms",
    kind: "llm_guidance",
    layer: "identity",
    maturity: "verified",
    points: 12,
    path: "/llms.txt",
    expectedContent: "text",
    required: true,
    severity: "medium",
    missingCode: "missing_llms_txt",
    missingTitle: "llms.txt is missing",
    missingImpact: "Language-model crawlers have no concise, site-authored guide to the domain.",
  },
  {
    id: "llms_full",
    kind: "llm_guidance",
    layer: "identity",
    maturity: "emerging",
    points: 0,
    path: "/llms-full.txt",
    expectedContent: "text",
    required: false,
    severity: "low",
    missingCode: "missing_llms_txt",
    missingTitle: "llms-full.txt is missing",
    missingImpact: "Agents cannot fetch a fuller LLM-oriented documentation bundle from the conventional path.",
  },
  {
    id: "homepage_structured_data",
    kind: "structured_data",
    layer: "identity",
    maturity: "verified",
    points: 12,
    path: "/",
    expectedContent: "homepage_structured_data",
    required: true,
    severity: "medium",
    missingCode: "missing_structured_data",
    missingTitle: "Homepage lacks JSON-LD or basic metadata",
    missingImpact: "Agents cannot extract a reliable organization identity, description, or canonical resource graph.",
  },
  {
    id: "agent_render_diff",
    kind: "agent_rendering",
    layer: "experience",
    maturity: "verified",
    points: 18,
    path: "/",
    expectedContent: "agent_render_diff",
    required: true,
    severity: "high",
    missingCode: "agent_empty_shell",
    missingTitle: "Agent user agents see an empty shell",
    missingImpact: "An agent trying to understand the business receives little or no meaningful page content.",
  },
  {
    id: "api_discoverability",
    kind: "api_discoverability",
    layer: "access",
    maturity: "verified",
    points: 10,
    path: "/",
    expectedContent: "homepage_api_links",
    required: true,
    severity: "medium",
    missingCode: "api_discoverability_links_missing",
    missingTitle: "Homepage does not advertise API or agent discovery links",
    missingImpact: "Agents must guess where OpenAPI, MCP, AI-catalog, or developer docs live.",
  },
].map((probe) => S.decodeUnknownSync(AgentReadinessProbeDefinition)(probe))

export type AgentReadinessFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>

export type AgentReadinessScanOptions = Readonly<{
  fetch?: AgentReadinessFetch
  generatedAt?: string
  timeoutMs?: number
  minRequestIntervalMs?: number
  maxResponseChars?: number
  userAgent?: string
  probeSet?: ReadonlyArray<AgentReadinessProbeDefinition>
  sourceRefs?: ReadonlyArray<string>
}>

type ProbeFetchResult = Readonly<{
  ok: boolean
  url: string
  status: number | null
  contentType: string | null
  body: string
  error: string | null
}>

type ProbeObservation = Readonly<{
  probe: AgentReadinessProbeDefinition
  ok: boolean
  findings: ReadonlyArray<AgentReadinessFinding>
  missingFinding: AgentReadinessFinding | null
}>

const DEFAULT_TIMEOUT_MS = 5_000
const DEFAULT_MIN_REQUEST_INTERVAL_MS = 150
const DEFAULT_MAX_RESPONSE_CHARS = 200_000
const DEFAULT_USER_AGENT =
  "OpenAgents-AgentReadinessProber/0.1 (+https://openagents.com/AGENTS.md)"
const AGENT_RENDER_USER_AGENT =
  "OpenAgents-AgentReadinessProber/0.1 agent-render (+https://openagents.com/AGENTS.md)"
const BROWSER_RENDER_USER_AGENT =
  "Mozilla/5.0 OpenAgents-AgentReadinessProber/0.1 browser-baseline"

const severityRank: Record<AgentReadinessSeverity, number> = {
  critical: 5,
  high: 4,
  medium: 3,
  low: 2,
  info: 1,
}

const compareFindingsByPriority = (
  a: AgentReadinessFinding,
  b: AgentReadinessFinding,
): number =>
  severityRank[b.severity] - severityRank[a.severity] ||
  a.title.localeCompare(b.title) ||
  a.findingRef.localeCompare(b.findingRef)

const orderedLayers: ReadonlyArray<AgentReadinessLayer> = [
  "discovery",
  "identity",
  "access",
  "payments",
  "experience",
]

const emptyCounts = (): AgentReadinessFindingCounts => ({
  critical: 0,
  high: 0,
  medium: 0,
  low: 0,
  info: 0,
})

const sleep = (ms: number): Promise<void> =>
  ms <= 0 ? Promise.resolve() : new Promise((resolve) => setTimeout(resolve, ms))

const safeRefPart = (value: string): string =>
  value.toLowerCase().replace(/[^a-z0-9_.-]+/gu, "_").replace(/^_+|_+$/gu, "")

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const ipv4PrivateOrReserved = (host: string): boolean => {
  const parts = host.split(".")
  if (parts.length !== 4) return false
  const nums = parts.map((part) => Number(part))
  if (nums.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) {
    return false
  }
  const [a, b] = nums as [number, number, number, number]
  return (
    a === 0 ||
    a === 10 ||
    a === 127 ||
    (a === 100 && b >= 64 && b <= 127) ||
    (a === 169 && b === 254) ||
    (a === 172 && b >= 16 && b <= 31) ||
    (a === 192 && b === 168) ||
    (a === 198 && (b === 18 || b === 19)) ||
    a >= 224
  )
}

export const normalizeAgentReadinessTarget = (target: string): URL => {
  const trimmed = target.trim()
  if (trimmed.length === 0) {
    throw new Error("Agent-readiness target is required.")
  }
  const candidate = /^[a-z][a-z0-9+.-]*:\/\//iu.test(trimmed)
    ? trimmed
    : `https://${trimmed}`
  const url = new URL(candidate)
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new Error("Agent-readiness probes only allow public http/https URLs.")
  }
  if (url.username.length > 0 || url.password.length > 0) {
    throw new Error("Agent-readiness target must not include credentials.")
  }
  const host = url.hostname.toLowerCase()
  if (
    host === "localhost" ||
    host.endsWith(".localhost") ||
    host.endsWith(".local") ||
    host.includes(":") ||
    ipv4PrivateOrReserved(host)
  ) {
    throw new Error("Agent-readiness target must be a public host.")
  }
  return new URL(`${url.protocol}//${url.host}/`)
}

const probeUrl = (baseUrl: URL, path: string): URL => {
  if (!path.startsWith("/") || path.startsWith("//")) {
    throw new Error(`Probe path must be a same-origin absolute path: ${path}`)
  }
  const url = new URL(path, baseUrl)
  if (url.origin !== baseUrl.origin) {
    throw new Error(`Probe path escaped target origin: ${path}`)
  }
  return url
}

const contentType = (response: Response): string | null =>
  response.headers.get("content-type")

const evidenceFor = (
  probe: AgentReadinessProbeDefinition,
  result: ProbeFetchResult,
): ReadonlyArray<AgentReadinessEvidence> => [
  {
    ref: `agent_readiness.probe.${probe.id}.${result.status ?? "network"}`,
    url: result.url,
    status: result.status,
    contentType: result.contentType,
  },
]

const makeFinding = (input: {
  readonly domain: string
  readonly probe: AgentReadinessProbeDefinition
  readonly code: AgentReadinessFindingCode
  readonly severity: AgentReadinessSeverity
  readonly title: string
  readonly impact: string
  readonly observedAt: string
  readonly evidence: ReadonlyArray<AgentReadinessEvidence>
}): AgentReadinessFinding => ({
  schemaVersion: AGENT_READINESS_FINDING_SCHEMA_VERSION,
  findingRef: `agent_readiness.${safeRefPart(input.domain)}.${input.probe.id}.${input.code}`,
  domain: input.domain,
  probeId: input.probe.id,
  kind: input.probe.kind,
  code: input.code,
  severity: input.severity,
  title: input.title,
  impact: input.impact,
  evidenceRefs: input.evidence.map((evidence) => evidence.ref),
  evidence: [...input.evidence],
  observedAt: input.observedAt,
})

const looksLikeHtml = (body: string, type: string | null): boolean =>
  (type?.toLowerCase().includes("text/html") ?? false) ||
  /<html[\s>]|<!doctype html|<body[\s>]/iu.test(body)

const visibleText = (html: string): string =>
  html
    .replace(/<script\b[^>]*>[\s\S]*?<\/script>/giu, " ")
    .replace(/<style\b[^>]*>[\s\S]*?<\/style>/giu, " ")
    .replace(/<[^>]+>/gu, " ")
    .replace(/\s+/gu, " ")
    .trim()

const looksLikeSpaShell = (body: string, type: string | null): boolean => {
  if (!looksLikeHtml(body, type)) return false
  const text = visibleText(body)
  const hasAppMount =
    /<div[^>]+id=["'](?:root|app|__next|app-root)["'][^>]*>\s*<\/div>/iu.test(body) ||
    /<main[^>]*>\s*<\/main>/iu.test(body)
  const scriptCount = (body.match(/<script\b/giu) ?? []).length
  return (hasAppMount && scriptCount > 0 && text.length < 180) || text.length < 80
}

const hasStructuredData = (body: string): boolean => {
  const jsonLdBlocks = body.match(/<script\b[^>]*type=["']application\/ld\+json["'][^>]*>[\s\S]*?<\/script>/giu) ?? []
  if (jsonLdBlocks.length > 0) return true
  const hasMetaDescription = /<meta\b[^>]*(?:name|property)=["'](?:description|og:description)["'][^>]*content=["'][^"']{20,}["']/iu
    .test(body)
  const hasTitle = /<title>[^<]{8,}<\/title>/iu.test(body)
  const hasCanonical = /<link\b[^>]*rel=["']canonical["'][^>]*href=/iu.test(body)
  return hasMetaDescription && hasTitle && hasCanonical
}

const hasApiDiscoverability = (
  body: string,
  headers: Headers | null,
): boolean => {
  const linkHeader = headers?.get("link") ?? ""
  const haystack = `${linkHeader}\n${body}`.toLowerCase()
  return [
    "openapi",
    "swagger",
    "api-docs",
    ".well-known/mcp",
    "ai-catalog",
    "llms.txt",
    "agents.md",
    "application/json",
  ].some((needle) => haystack.includes(needle))
}

const fetchProbeText = async (
  url: URL,
  input: {
    readonly fetchImpl: AgentReadinessFetch
    readonly timeoutMs: number
    readonly maxResponseChars: number
    readonly userAgent: string
    readonly accept: string
  },
): Promise<ProbeFetchResult> => {
  const controller = new AbortController()
  const timeout =
    input.timeoutMs > 0
      ? setTimeout(() => controller.abort(), input.timeoutMs)
      : null
  try {
    const response = await input.fetchImpl(url, {
      method: "GET",
      signal: controller.signal,
      headers: {
        accept: input.accept,
        range: `bytes=0-${input.maxResponseChars - 1}`,
        "user-agent": input.userAgent,
      },
    })
    const body = (await response.text()).slice(0, input.maxResponseChars)
    return {
      ok: response.ok,
      url: url.toString(),
      status: response.status,
      contentType: contentType(response),
      body,
      error: null,
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    return {
      ok: false,
      url: url.toString(),
      status: null,
      contentType: null,
      body: "",
      error: message,
    }
  } finally {
    if (timeout !== null) clearTimeout(timeout)
  }
}

const acceptFor = (expected: AgentReadinessExpectedContent): string => {
  switch (expected) {
    case "json":
      return "application/json,text/json;q=0.9,*/*;q=0.2"
    case "xml":
      return "application/xml,text/xml;q=0.9,*/*;q=0.2"
    case "text":
      return "text/plain,*/*;q=0.2"
    case "agent_render_diff":
    case "homepage_api_links":
    case "homepage_structured_data":
      return "text/html,application/xhtml+xml,*/*;q=0.2"
  }
}

const missingFindingFor = (
  domain: string,
  probe: AgentReadinessProbeDefinition,
  observedAt: string,
  result: ProbeFetchResult,
): AgentReadinessFinding | null => {
  if (!probe.required) return null
  return makeFinding({
    domain,
    probe,
    code: probe.missingCode,
    severity: probe.severity,
    title: probe.missingTitle,
    impact: probe.missingImpact,
    observedAt,
    evidence: evidenceFor(probe, result),
  })
}

const evaluateTextLikeProbe = (
  domain: string,
  probe: AgentReadinessProbeDefinition,
  observedAt: string,
  result: ProbeFetchResult,
): ProbeObservation => {
  const missingFinding = result.ok
    ? null
    : missingFindingFor(domain, probe, observedAt, result)
  const findings: Array<AgentReadinessFinding> = []
  if (result.ok && looksLikeSpaShell(result.body, result.contentType)) {
    findings.push(
      makeFinding({
        domain,
        probe,
        code: "spa_shell_surface",
        severity: probe.severity,
        title: `${probe.path} returns an HTML application shell`,
        impact: "The surface exists, but agents receive a browser app shell instead of the expected machine-readable resource.",
        observedAt,
        evidence: evidenceFor(probe, result),
      }),
    )
  }
  return {
    probe,
    ok: result.ok && findings.length === 0,
    findings,
    missingFinding,
  }
}

const evaluateJsonProbe = (
  domain: string,
  probe: AgentReadinessProbeDefinition,
  observedAt: string,
  result: ProbeFetchResult,
): ProbeObservation => {
  const missingFinding = result.ok
    ? null
    : missingFindingFor(domain, probe, observedAt, result)
  const findings: Array<AgentReadinessFinding> = []
  if (result.ok && looksLikeSpaShell(result.body, result.contentType)) {
    findings.push(
      makeFinding({
        domain,
        probe,
        code: "spa_shell_json",
        severity: "high",
        title: `${probe.path} returns an SPA shell instead of JSON`,
        impact: "Agent-readiness scanners parse the HTML shell as a broken machine-readable manifest.",
        observedAt,
        evidence: evidenceFor(probe, result),
      }),
    )
  } else if (result.ok) {
    try {
      const parsed = JSON.parse(result.body) as unknown
      if (!isRecord(parsed) && !Array.isArray(parsed)) {
        throw new Error("JSON manifest must be an object or array.")
      }
    } catch {
      findings.push(
        makeFinding({
          domain,
          probe,
          code: "invalid_json",
          severity: probe.severity,
          title: `${probe.path} is not valid JSON`,
          impact: "Agents cannot decode the advertised machine-readable resource.",
          observedAt,
          evidence: evidenceFor(probe, result),
        }),
      )
    }
  }
  return {
    probe,
    ok: result.ok && findings.length === 0,
    findings,
    missingFinding,
  }
}

const evaluateHomepageProbe = (
  domain: string,
  probe: AgentReadinessProbeDefinition,
  observedAt: string,
  result: ProbeFetchResult,
): ProbeObservation => {
  const missingFinding = result.ok
    ? null
    : missingFindingFor(domain, probe, observedAt, result)
  const findings: Array<AgentReadinessFinding> = []
  if (result.ok && probe.expectedContent === "homepage_structured_data" && !hasStructuredData(result.body)) {
    findings.push(
      makeFinding({
        domain,
        probe,
        code: "missing_structured_data",
        severity: probe.severity,
        title: probe.missingTitle,
        impact: probe.missingImpact,
        observedAt,
        evidence: evidenceFor(probe, result),
      }),
    )
  }
  if (result.ok && probe.expectedContent === "homepage_api_links" && !hasApiDiscoverability(result.body, null)) {
    findings.push(
      makeFinding({
        domain,
        probe,
        code: "api_discoverability_links_missing",
        severity: probe.severity,
        title: probe.missingTitle,
        impact: probe.missingImpact,
        observedAt,
        evidence: evidenceFor(probe, result),
      }),
    )
  }
  return {
    probe,
    ok: result.ok && findings.length === 0,
    findings,
    missingFinding,
  }
}

const evaluateAgentRenderProbe = async (
  baseUrl: URL,
  domain: string,
  probe: AgentReadinessProbeDefinition,
  observedAt: string,
  input: {
    readonly fetchImpl: AgentReadinessFetch
    readonly timeoutMs: number
    readonly maxResponseChars: number
  },
): Promise<ProbeObservation> => {
  const url = probeUrl(baseUrl, probe.path)
  const browser = await fetchProbeText(url, {
    ...input,
    userAgent: BROWSER_RENDER_USER_AGENT,
    accept: acceptFor("agent_render_diff"),
  })
  const agent = await fetchProbeText(url, {
    ...input,
    userAgent: AGENT_RENDER_USER_AGENT,
    accept: acceptFor("agent_render_diff"),
  })
  const missingFinding = agent.ok
    ? null
    : missingFindingFor(domain, probe, observedAt, agent)
  const browserTextLength = visibleText(browser.body).length
  const agentTextLength = visibleText(agent.body).length
  const agentHasReadableHead =
    hasStructuredData(agent.body) || hasApiDiscoverability(agent.body, null)
  const agentLooksEmpty =
    agent.ok &&
    !agentHasReadableHead &&
    (looksLikeSpaShell(agent.body, agent.contentType) ||
      agentTextLength < 140 ||
      (browserTextLength >= 700 && agentTextLength / browserTextLength < 0.25))
  const findings: Array<AgentReadinessFinding> = []
  if (agentLooksEmpty) {
    findings.push(
      makeFinding({
        domain,
        probe,
        code: "agent_empty_shell",
        severity: "high",
        title: probe.missingTitle,
        impact: probe.missingImpact,
        observedAt,
        evidence: evidenceFor(probe, agent),
      }),
    )
  }
  return {
    probe,
    ok: agent.ok && findings.length === 0,
    findings,
    missingFinding,
  }
}

const evaluateProbe = async (
  baseUrl: URL,
  domain: string,
  probe: AgentReadinessProbeDefinition,
  observedAt: string,
  input: {
    readonly fetchImpl: AgentReadinessFetch
    readonly timeoutMs: number
    readonly maxResponseChars: number
    readonly userAgent: string
  },
): Promise<ProbeObservation> => {
  if (probe.expectedContent === "agent_render_diff") {
    return evaluateAgentRenderProbe(baseUrl, domain, probe, observedAt, input)
  }
  const result = await fetchProbeText(probeUrl(baseUrl, probe.path), {
    ...input,
    accept: acceptFor(probe.expectedContent),
  })
  if (probe.expectedContent === "json") {
    return evaluateJsonProbe(domain, probe, observedAt, result)
  }
  if (
    probe.expectedContent === "homepage_structured_data" ||
    probe.expectedContent === "homepage_api_links"
  ) {
    return evaluateHomepageProbe(domain, probe, observedAt, result)
  }
  return evaluateTextLikeProbe(domain, probe, observedAt, result)
}

const topFindings = (
  findings: ReadonlyArray<AgentReadinessFinding>,
): ReadonlyArray<AgentReadinessTopFinding> =>
  [...findings]
    .sort(compareFindingsByPriority)
    .slice(0, 3)
    .map((finding) => ({
      code: finding.code,
      severity: finding.severity,
      title: finding.title,
      impact: finding.impact,
      evidenceRefs: finding.evidenceRefs,
    }))

const countFindings = (
  findings: ReadonlyArray<AgentReadinessFinding>,
): AgentReadinessFindingCounts => {
  const counts: Record<AgentReadinessSeverity, number> = emptyCounts()
  for (const finding of findings) {
    counts[finding.severity] += 1
  }
  return counts
}

const reportStatus = (
  findings: ReadonlyArray<AgentReadinessFinding>,
): AgentReadinessReport["status"] => {
  if (findings.some((finding) => finding.code === "disallowed_target_url")) {
    return "blocked"
  }
  return findings.some((finding) => severityRank[finding.severity] >= severityRank.medium)
    ? "attention"
    : "passed"
}

const gradeForScore = (score: number): AgentReadinessGrade => {
  if (score >= 90) return "A"
  if (score >= 80) return "B"
  if (score >= 70) return "C"
  if (score >= 60) return "D"
  return "F"
}

const scoreFromObservations = (
  observations: ReadonlyArray<ProbeObservation>,
  findings: ReadonlyArray<AgentReadinessFinding>,
): Pick<AgentReadinessReport, "grade" | "layerScores" | "score"> => {
  const failedProbeIds = new Set(findings.map((finding) => finding.probeId))
  const layerScores = orderedLayers.map((layer) => {
    const scored = observations.filter((observation) =>
      observation.probe.layer === layer &&
      observation.probe.maturity === "verified" &&
      observation.probe.points > 0,
    )
    const possible = scored.reduce((sum, observation) => sum + observation.probe.points, 0)
    const earned = scored
      .filter((observation) => observation.ok && !failedProbeIds.has(observation.probe.id))
      .reduce((sum, observation) => sum + observation.probe.points, 0)
    const score = possible === 0 ? 0 : Math.round((earned / possible) * 100)
    return {
      layer,
      score,
      earned,
      possible,
      status: possible === 0 ? "not_applicable" : score === 100 ? "passed" : "attention",
    } satisfies AgentReadinessLayerScore
  })
  const applicable = layerScores.filter((layer) => layer.possible > 0)
  const possible = applicable.reduce((sum, layer) => sum + layer.possible, 0)
  const earned = applicable.reduce((sum, layer) => sum + layer.earned, 0)
  const score = possible === 0 ? 0 : Math.round((earned / possible) * 100)
  return {
    grade: gradeForScore(score),
    layerScores,
    score,
  }
}

const reportSummary = (
  findings: ReadonlyArray<AgentReadinessFinding>,
): string => {
  const top = topFindings(findings)
  if (top.length === 0) {
    return "Agent-readable discovery, crawl, LLM guidance, structured data, rendering, and API links passed the public-safe probe set."
  }
  return top.map((finding) => `${finding.severity}: ${finding.title}`).join("; ")
}

const findingsFromObservations = (
  observations: ReadonlyArray<ProbeObservation>,
): ReadonlyArray<AgentReadinessFinding> => {
  const satisfiedGroups = new Set(
    observations
      .filter((observation) => observation.ok)
      .map((observation) => observation.probe.requiredAnyGroup)
      .filter((group): group is string => group !== undefined),
  )
  const findings: Array<AgentReadinessFinding> = []
  for (const observation of observations) {
    const equivalentProbeSatisfied =
      observation.probe.requiredAnyGroup !== undefined &&
      satisfiedGroups.has(observation.probe.requiredAnyGroup)
    if (!equivalentProbeSatisfied || observation.ok) {
      findings.push(...observation.findings)
    }
    if (
      observation.missingFinding !== null &&
      (observation.probe.requiredAnyGroup === undefined ||
        !satisfiedGroups.has(observation.probe.requiredAnyGroup))
    ) {
      findings.push(observation.missingFinding)
    }
  }
  const byRef = new Map<string, AgentReadinessFinding>()
  for (const finding of findings) {
    byRef.set(finding.findingRef, finding)
  }
  return [...byRef.values()]
}

export const scanAgentReadinessDomain = async (
  target: string,
  options: AgentReadinessScanOptions = {},
): Promise<AgentReadinessReport> => {
  const generatedAt = options.generatedAt ?? new Date().toISOString()
  let baseUrl: URL
  try {
    baseUrl = normalizeAgentReadinessTarget(target)
  } catch (error) {
    const domain = safeRefPart(target) || "invalid"
    const probe = defaultAgentReadinessProbeSet[0]
    if (probe === undefined) throw new Error("Default probe set is empty.")
    const finding = makeFinding({
      domain,
      probe,
      code: "disallowed_target_url",
      severity: "critical",
      title: "Target is not a public URL",
      impact: "The analyzer is read-only and refuses local, private, credentialed, or non-http targets.",
      observedAt: generatedAt,
      evidence: [
        {
          ref: "agent_readiness.target.disallowed",
          url: target,
          status: null,
          contentType: null,
        },
      ],
    })
    return S.decodeUnknownSync(AgentReadinessReport)({
      schemaVersion: AGENT_READINESS_REPORT_SCHEMA_VERSION,
      domain,
      baseUrl: target,
      generatedAt,
      status: "blocked",
      score: 0,
      grade: "F",
      layerScores: orderedLayers.map((layer) => ({
        layer,
        score: 0,
        earned: 0,
        possible: 0,
        status: "not_applicable",
      })),
      summary: finding.title,
      topFindings: topFindings([finding]),
      findingCounts: countFindings([finding]),
      findings: [finding],
      sourceRefs: options.sourceRefs ?? ["github:OpenAgentsInc/openagents#8262"],
    })
  }
  const fetchImpl = options.fetch ?? fetch
  const probeSet = options.probeSet ?? defaultAgentReadinessProbeSet
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS
  const maxResponseChars = options.maxResponseChars ?? DEFAULT_MAX_RESPONSE_CHARS
  const minRequestIntervalMs =
    options.minRequestIntervalMs ?? DEFAULT_MIN_REQUEST_INTERVAL_MS
  const userAgent = options.userAgent ?? DEFAULT_USER_AGENT
  const observations: Array<ProbeObservation> = []
  for (const probe of probeSet) {
    observations.push(
      await evaluateProbe(baseUrl, baseUrl.hostname, probe, generatedAt, {
        fetchImpl,
        timeoutMs,
        maxResponseChars,
        userAgent,
      }),
    )
    await sleep(minRequestIntervalMs)
  }
  const findings = findingsFromObservations(observations)
  const scored = scoreFromObservations(observations, findings)
  const report = {
    schemaVersion: AGENT_READINESS_REPORT_SCHEMA_VERSION,
    domain: baseUrl.hostname,
    baseUrl: baseUrl.toString(),
    generatedAt,
    status: reportStatus(findings),
    score: scored.score,
    grade: scored.grade,
    layerScores: scored.layerScores,
    summary: reportSummary(findings),
    topFindings: topFindings(findings),
    findingCounts: countFindings(findings),
    findings,
    sourceRefs: options.sourceRefs ?? ["github:OpenAgentsInc/openagents#8262"],
  }
  return S.decodeUnknownSync(AgentReadinessReport)(report)
}

export const agentReadinessTaskForDomain = (
  domain: string,
  options: Pick<AgentReadinessScanOptions, "timeoutMs" | "sourceRefs"> = {},
): AgentReadinessDomainTask =>
  S.decodeUnknownSync(AgentReadinessDomainTask)({
    schemaVersion: AGENT_READINESS_DOMAIN_TASK_SCHEMA_VERSION,
    domain: normalizeAgentReadinessTarget(domain).hostname,
    analyzerRef: "@openagentsinc/agent-readiness/default",
    maxWorkerCount: 1,
    timeoutMs: options.timeoutMs ?? DEFAULT_TIMEOUT_MS,
    outputSchema: AGENT_READINESS_REPORT_SCHEMA_VERSION,
    sourceRefs: options.sourceRefs ?? ["github:OpenAgentsInc/openagents#8262"],
  })

export const runAgentReadinessBatch = async (
  domains: ReadonlyArray<string>,
  options: AgentReadinessScanOptions & Readonly<{ concurrency?: number }> = {},
): Promise<AgentReadinessBatchResult> => {
  const generatedAt = options.generatedAt ?? new Date().toISOString()
  const concurrency = Math.max(1, Math.min(options.concurrency ?? 4, domains.length || 1))
  const reports = new Array<AgentReadinessReport>(domains.length)
  let next = 0
  await Promise.all(
    Array.from({ length: concurrency }, async () => {
      while (next < domains.length) {
        const index = next
        next += 1
        const domain = domains[index]
        if (domain !== undefined) {
          reports[index] = await scanAgentReadinessDomain(domain, {
            ...options,
            generatedAt,
          })
        }
      }
    }),
  )
  return S.decodeUnknownSync(AgentReadinessBatchResult)({
    schemaVersion: AGENT_READINESS_BATCH_SCHEMA_VERSION,
    generatedAt,
    reports,
  })
}

export type AgentReadinessReportRenderOptions = Readonly<{
  generatedAt?: string
  commercialContextByFindingRef?: Readonly<Record<string, string>>
  commercialContextByCode?: Partial<Record<AgentReadinessFindingCode, string>>
  sourceRefs?: ReadonlyArray<string>
}>

const OPENAGENTS_CASE_STUDY_DOMAINS = new Set(["openagents.com"])
const MAX_COMMERCIAL_CONTEXT_CHARS = 240

const normalizeOneLineField = (
  label: string,
  value: string,
  maxChars = MAX_COMMERCIAL_CONTEXT_CHARS,
): string => {
  const normalized = value.replace(/\s+/gu, " ").trim()
  if (normalized.length === 0) {
    throw new Error(`Agent-readiness ${label} is required.`)
  }
  if (normalized.length > maxChars) {
    throw new Error(
      `Agent-readiness ${label} must be ${maxChars} characters or fewer.`,
    )
  }
  return normalized
}

const commercialContextForFinding = (
  finding: AgentReadinessFinding,
  options: AgentReadinessReportRenderOptions,
): string => {
  const context =
    options.commercialContextByFindingRef?.[finding.findingRef] ??
    options.commercialContextByCode?.[finding.code]
  if (context === undefined) {
    throw new Error(
      `Missing commercial context for finding ${finding.findingRef}.`,
    )
  }
  return normalizeOneLineField("commercial context", context)
}

const escapeHtml = (value: string): string =>
  value
    .replace(/&/gu, "&amp;")
    .replace(/</gu, "&lt;")
    .replace(/>/gu, "&gt;")
    .replace(/"/gu, "&quot;")
    .replace(/'/gu, "&#39;")

const renderFinding = (
  finding: AgentReadinessFinding,
  options: AgentReadinessReportRenderOptions,
): AgentReadinessRenderedFinding => ({
  findingRef: finding.findingRef,
  code: finding.code,
  severity: finding.severity,
  title: normalizeOneLineField("finding title", finding.title, 180),
  impact: normalizeOneLineField("finding impact", finding.impact, 260),
  commercialContext: commercialContextForFinding(finding, options),
  evidenceRefs: [...finding.evidenceRefs],
})

const plainFindingBlock = (
  finding: AgentReadinessRenderedFinding,
  index: number,
): string =>
  [
    `${index}. [${finding.severity}] ${finding.title}`,
    `   Impact: ${finding.impact}`,
    `   Commercial context: ${finding.commercialContext}`,
    `   Evidence refs: ${finding.evidenceRefs.join(", ") || "none"}`,
  ].join("\n")

const htmlFindingItem = (finding: AgentReadinessRenderedFinding): string =>
  [
    "<li>",
    `<strong>[${escapeHtml(finding.severity)}] ${escapeHtml(finding.title)}</strong>`,
    `<p>Impact: ${escapeHtml(finding.impact)}</p>`,
    `<p>Commercial context: ${escapeHtml(finding.commercialContext)}</p>`,
    `<p>Evidence refs: ${escapeHtml(finding.evidenceRefs.join(", ") || "none")}</p>`,
    "</li>",
  ].join("")

const renderEmailPlainText = (
  report: AgentReadinessReport,
  findings: ReadonlyArray<AgentReadinessRenderedFinding>,
): string => {
  const heading = `Agent-readiness snapshot for ${report.domain}: ${report.score}/100 (${report.grade}), status ${report.status}.`
  if (findings.length === 0) {
    return [
      heading,
      "The current public probe set did not find blocking agent-readiness issues.",
    ].join("\n")
  }
  return [
    heading,
    "Top findings:",
    findings.map((finding, index) => plainFindingBlock(finding, index + 1)).join("\n"),
  ].join("\n")
}

const renderEmailHtml = (
  report: AgentReadinessReport,
  findings: ReadonlyArray<AgentReadinessRenderedFinding>,
): string => {
  const heading = `Agent-readiness snapshot for ${report.domain}: ${report.score}/100 (${report.grade}), status ${report.status}.`
  if (findings.length === 0) {
    return [
      `<section data-agent-readiness-report="${escapeHtml(report.domain)}">`,
      `<p>${escapeHtml(heading)}</p>`,
      "<p>The current public probe set did not find blocking agent-readiness issues.</p>",
      "</section>",
    ].join("")
  }
  return [
    `<section data-agent-readiness-report="${escapeHtml(report.domain)}">`,
    `<p>${escapeHtml(heading)}</p>`,
    "<ol>",
    findings.map(htmlFindingItem).join(""),
    "</ol>",
    "</section>",
  ].join("")
}

const renderBumpPlainText = (
  report: AgentReadinessReport,
  finding: AgentReadinessRenderedFinding | null,
): string => {
  if (finding === null) {
    return `No held-back bump finding for ${report.domain}.`
  }
  return [
    `One more agent-readiness finding for ${report.domain}:`,
    plainFindingBlock(finding, 1),
  ].join("\n")
}

const renderBumpHtml = (
  report: AgentReadinessReport,
  finding: AgentReadinessRenderedFinding | null,
): string => {
  if (finding === null) {
    return `<p>No held-back bump finding for ${escapeHtml(report.domain)}.</p>`
  }
  return [
    `<section data-agent-readiness-bump="${escapeHtml(report.domain)}">`,
    `<p>One more agent-readiness finding for ${escapeHtml(report.domain)}:</p>`,
    "<ol>",
    htmlFindingItem(finding),
    "</ol>",
    "</section>",
  ].join("")
}

const renderInternalOperatorView = (
  report: AgentReadinessReport,
  input: {
    readonly generatedAt: string
    readonly operatorFindings: ReadonlyArray<AgentReadinessRenderedFinding>
    readonly topFindings: ReadonlyArray<AgentReadinessRenderedFinding>
    readonly heldBackFinding: AgentReadinessRenderedFinding | null
  },
): string => {
  const originalByRef = new Map(
    report.findings.map((finding) => [finding.findingRef, finding]),
  )
  const fullLedger =
    input.operatorFindings.length === 0
      ? "- No findings in the current public-safe report."
      : input.operatorFindings
          .map((finding, index) => {
            const original = originalByRef.get(finding.findingRef)
            const evidence = original?.evidence ?? []
            const evidenceLines =
              evidence.length === 0
                ? "  - Evidence: none"
                : evidence
                    .map((item) =>
                      `  - Evidence: ${item.ref} | ${item.status ?? "network"} | ${item.contentType ?? "unknown"} | ${item.url}`,
                    )
                    .join("\n")
            return [
              `${index + 1}. [${finding.severity}] ${finding.title}`,
              `   Impact: ${finding.impact}`,
              `   Commercial context: ${finding.commercialContext}`,
              evidenceLines,
            ].join("\n")
          })
          .join("\n")
  return [
    `# Agent-readiness operator review: ${report.domain}`,
    "",
    `Report generated: ${report.generatedAt}`,
    `Rendered: ${input.generatedAt}`,
    `Status: ${report.status}`,
    `Score: ${report.score}/100`,
    `Grade: ${report.grade}`,
    "",
    "## Email Step 1 Findings",
    input.topFindings.length === 0
      ? "- No findings selected for the first email."
      : input.topFindings
          .map((finding, index) => plainFindingBlock(finding, index + 1))
          .join("\n"),
    "",
    "## Held-Back Bump Finding",
    input.heldBackFinding === null
      ? "- No held-back finding available."
      : plainFindingBlock(input.heldBackFinding, 1),
    "",
    "## Full Internal Finding Ledger",
    fullLedger,
  ].join("\n")
}

export const renderAgentReadinessReport = (
  report: AgentReadinessReport,
  options: AgentReadinessReportRenderOptions = {},
): AgentReadinessReportRender => {
  const generatedAt = options.generatedAt ?? new Date().toISOString()
  const sortedFindings = [...report.findings].sort(compareFindingsByPriority)
  const operatorFindings = sortedFindings.map((finding) =>
    renderFinding(finding, options),
  )
  const renderedByRef = new Map(
    operatorFindings.map((finding) => [finding.findingRef, finding]),
  )
  const top = sortedFindings
    .slice(0, 3)
    .map((finding) => renderedByRef.get(finding.findingRef))
    .filter((finding): finding is AgentReadinessRenderedFinding => finding !== undefined)
  const heldBackFindingRef = sortedFindings[3]?.findingRef
  const heldBackFinding =
    heldBackFindingRef === undefined
      ? null
      : renderedByRef.get(heldBackFindingRef) ?? null
  const persistenceMode = OPENAGENTS_CASE_STUDY_DOMAINS.has(report.domain)
    ? "repo_case_study_allowed"
    : "private_runtime_only"
  const rendered = {
    schemaVersion: AGENT_READINESS_REPORT_RENDER_SCHEMA_VERSION,
    domain: report.domain,
    generatedAt,
    reportGeneratedAt: report.generatedAt,
    reportStatus: report.status,
    score: report.score,
    grade: report.grade,
    persistenceMode,
    operatorFindings,
    topFindings: top,
    heldBackFinding,
    internalOperatorView: renderInternalOperatorView(report, {
      generatedAt,
      operatorFindings,
      topFindings: top,
      heldBackFinding,
    }),
    emailBodyPlainText: renderEmailPlainText(report, top),
    emailBodyHtml: renderEmailHtml(report, top),
    bumpBodyPlainText: renderBumpPlainText(report, heldBackFinding),
    bumpBodyHtml: renderBumpHtml(report, heldBackFinding),
    sourceRefs: options.sourceRefs ?? [
      ...report.sourceRefs,
      "github:OpenAgentsInc/openagents#8266",
    ],
  }
  return S.decodeUnknownSync(AgentReadinessReportRender)(rendered)
}

export const renderAgentReadinessCaseStudyArtifact = (
  report: AgentReadinessReport,
  options: AgentReadinessReportRenderOptions = {},
): string => {
  if (!OPENAGENTS_CASE_STUDY_DOMAINS.has(report.domain)) {
    throw new Error(
      "Only the OpenAgents own-domain report may be rendered as a repo-persisted case-study artifact.",
    )
  }
  const rendered = renderAgentReadinessReport(report, options)
  return [
    "<!-- public-safe: generated only from the openagents.com agent-readiness fixture -->",
    rendered.internalOperatorView,
    "",
    "## Sendable Plain Text Fragment",
    "",
    "```text",
    rendered.emailBodyPlainText,
    "```",
    "",
    "## Sendable HTML Fragment",
    "",
    "```html",
    rendered.emailBodyHtml,
    "```",
    "",
    "## Held-Back Bump Plain Text Fragment",
    "",
    "```text",
    rendered.bumpBodyPlainText,
    "```",
  ].join("\n")
}

export type AgentReadinessCliArgs = Readonly<{
  command: "scan"
  domain: string | null
  batchFile: string | null
  json: boolean
  timeoutMs: number
  concurrency: number
}>

export const parseAgentReadinessCliArgs = (
  argv: ReadonlyArray<string>,
): AgentReadinessCliArgs => {
  const [command, ...rest] = argv
  if (command !== "scan") {
    throw new Error("Usage: agent-readiness scan <domain> [--json] [--batch file]")
  }
  let domain: string | null = null
  let batchFile: string | null = null
  let json = false
  let timeoutMs = DEFAULT_TIMEOUT_MS
  let concurrency = 4
  for (let index = 0; index < rest.length; index += 1) {
    const arg = rest[index]
    if (arg === "--json") {
      json = true
    } else if (arg === "--batch") {
      const value = rest[index + 1]
      if (value === undefined) throw new Error("--batch requires a file path.")
      batchFile = value
      index += 1
    } else if (arg === "--timeout-ms") {
      const value = Number(rest[index + 1])
      if (!Number.isFinite(value) || value <= 0) {
        throw new Error("--timeout-ms requires a positive number.")
      }
      timeoutMs = value
      index += 1
    } else if (arg === "--concurrency") {
      const value = Number(rest[index + 1])
      if (!Number.isInteger(value) || value <= 0) {
        throw new Error("--concurrency requires a positive integer.")
      }
      concurrency = value
      index += 1
    } else if (arg !== undefined && !arg.startsWith("--") && domain === null) {
      domain = arg
    } else {
      throw new Error(`Unknown argument: ${arg ?? ""}`)
    }
  }
  if (domain === null && batchFile === null) {
    throw new Error("Provide a domain or --batch file.")
  }
  if (domain !== null && batchFile !== null) {
    throw new Error("Use either a single domain or --batch, not both.")
  }
  return { command: "scan", domain, batchFile, json, timeoutMs, concurrency }
}

export const domainsFromBatchFile = async (path: string): Promise<ReadonlyArray<string>> => {
  const text = await Bun.file(path).text()
  return text
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("#"))
}

const printHumanReport = (report: AgentReadinessReport): void => {
  console.log(`${report.domain}: ${report.status}`)
  console.log(report.summary)
  for (const finding of report.topFindings) {
    console.log(`- [${finding.severity}] ${finding.title}: ${finding.impact}`)
  }
}

export const runAgentReadinessCli = async (
  argv: ReadonlyArray<string>,
): Promise<number> => {
  try {
    const args = parseAgentReadinessCliArgs(argv)
    if (args.batchFile !== null) {
      const domains = await domainsFromBatchFile(args.batchFile)
      const result = await runAgentReadinessBatch(domains, {
        timeoutMs: args.timeoutMs,
        concurrency: args.concurrency,
      })
      if (args.json) {
        console.log(JSON.stringify(result, null, 2))
      } else {
        for (const report of result.reports) {
          printHumanReport(report)
        }
      }
      return result.reports.some((report) => report.status === "blocked") ? 2 : 0
    }
    if (args.domain === null) throw new Error("Missing domain.")
    const report = await scanAgentReadinessDomain(args.domain, {
      timeoutMs: args.timeoutMs,
    })
    if (args.json) {
      console.log(JSON.stringify(report, null, 2))
    } else {
      printHumanReport(report)
    }
    return report.status === "blocked" ? 2 : 0
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    return 1
  }
}

export const decodeAgentReadinessFinding = S.decodeUnknownSync(
  AgentReadinessFinding,
)
export const decodeAgentReadinessReport = S.decodeUnknownSync(AgentReadinessReport)
export const decodeAgentReadinessReportRender = S.decodeUnknownSync(
  AgentReadinessReportRender,
)
export const decodeAgentReadinessDomainTask = S.decodeUnknownSync(
  AgentReadinessDomainTask,
)
