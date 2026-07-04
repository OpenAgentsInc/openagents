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
- `openagents.agent_readiness_domain_task.v1` for fleet-dispatchable work.

Reports contain bounded public evidence refs, status codes, content types, and
one-line impacts. They do not store page bodies, prospect names, lead lists, or
client-identifying data.
