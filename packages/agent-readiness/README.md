# @openagentsinc/agent-readiness

Deterministic, read-only agent-readiness probes for public domains.

This package is the LG-1 analyzer for the Fable sell-side lane. It checks the
same public surfaces OpenAgents now serves itself: MCP discovery, ARD/AI
catalog, crawl surfaces, `llms.txt`, structured data, agent-readable rendering,
and API discoverability. Prospect findings are sales material and must not be
committed to the repo; only the rubric, tests, and the public-safe
`openagents.com` fixture report live here.

The report borrows the useful shape of ora's public agent-readiness framing:
each domain receives a 0-100 score, A-F grade, and five-layer breakdown
(`discovery`, `identity`, `access`, `payments`, `experience`). The concrete
checks and evidence remain OpenAgents-owned. Verified checks count toward the
score; emerging checks are reported but excluded from the denominator until the
rubric promotes them. Layers with no applicable checks, such as payments for
this first LG-1 website analyzer, are marked `not_applicable`.

## CLI

```sh
bun packages/agent-readiness/src/cli.ts scan openagents.com --json
bun packages/agent-readiness/src/cli.ts scan --batch domains.txt --json
```

The scanner accepts only public `http`/`https` URLs, rejects local/private
hosts, uses a polite OpenAgents user agent, applies per-domain timeouts, and
runs probes sequentially per domain. Batch mode is bounded by `--concurrency`.

## Output Contract

- `openagents.agent_readiness_finding.v1` for each finding.
- `openagents.agent_readiness_report.v1` for the per-domain report.
- `openagents.agent_readiness_report_render.v1` for operator/email render
  output.
- `openagents.agent_readiness_domain_task.v1` for fleet-dispatchable work.
- `openagents.model_custody_analyzer_config.v1` and
  `openagents.model_custody_report.v1` for the RX-8 Own-your-AI public-signal
  analyzer.

Reports contain bounded public evidence refs, status codes, content types, and
one-line impacts. They do not store page bodies, prospect names, lead lists, or
client-identifying data.

## Report Renderer

LG-5 turns an LG-1 report object into review and outreach fragments without
persisting prospect report output in the repo:

```ts
import { renderAgentReadinessReport } from "@openagentsinc/agent-readiness"

const rendered = renderAgentReadinessReport(report, {
  commercialContextByFindingRef: {
    [findingRef]: "This blocks agents evaluating your product catalog.",
  },
})
```

The renderer returns:

- `internalOperatorView`: markdown for private operator review.
- `emailBodyPlainText` and `emailBodyHtml`: first-email fragments with the
  top three findings.
- `bumpBodyPlainText` and `bumpBodyHtml`: the held-back fourth finding for the
  one allowed bump step.

Every rendered finding requires a one-line commercial context from enrichment.
HTML output is escaped. Prospect renders are marked `private_runtime_only`;
only the `openagents.com` own-domain fixture may be rendered through
`renderAgentReadinessCaseStudyArtifact` into a repo-persisted case-study
artifact. PDF/attachment export is intentionally deferred.

## Model-Custody Analyzer

RX-8 adds a second LG-1 config for the Own-your-AI Reactor segment. It scans
only public URLs for reproducible signals a buyer can verify themselves:
published subprocessors/DPA pages, privacy or AI-feature disclosures, and
careers/tech-stack pages that name frontier-lab or model-provider tooling.

The analyzer emits `apollo_model_custody` reports as public facts only. It
stores evidence refs, status codes, content types, matched public terms, and an
explicit inference boundary. It never stores raw page bodies, never scans
private/login-gated surfaces, and never presents model-provider mentions as
proof that customer data moved, model training happened, or a compliance
posture exists.
