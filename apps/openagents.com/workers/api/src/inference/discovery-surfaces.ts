// Agent-discovery surfaces for OpenAgents inference + the Agent Cloud (EPIC #6049,
// Phase 1). These are plain-language, machine-readable documents that describe
// the live inference API so agents (and crawlers like StripeBot, which
// feeds the Stripe Directory) can FIND and understand the service.
//
// Ship-ready, no flag: these are static documents. They make NO live payment
// claim and require no payment configuration — they only DESCRIBE the keyed API
// and pricing posture. They are served crawlable (cache-friendly, no auth, no
// robots block) so agent crawlers can match OpenAgents on "llm inference api" /
// "openai-compatible inference" / "pay-per-call" searches.
//
// Mirrors the live PostalForm/Zinc directory pattern: `/llms.txt`, `/agents.md`,
// `/ai.md`, `/skill.md`. Copy is intentionally honest and accurate to what the
// gateway actually serves today (`/v1/chat/completions`, one public
// `openagents/khala` model id, receipt-first metering, the `openagents`
// disclosure block). The standalone MPP/x402 endpoint was retired in #8387; do
// not advertise `/mpp/v1/chat/completions` unless a fresh owner-approved surface
// is rebuilt.
import { Effect } from 'effect'

// The four discovery document paths, mirroring the PostalForm directory shape.
export type DiscoverySurfacePath =
  | '/llms.txt'
  | '/agents.md'
  | '/ai.md'
  | '/skill.md'

export const DISCOVERY_SURFACE_PATHS: ReadonlyArray<DiscoverySurfacePath> = [
  '/llms.txt',
  '/agents.md',
  '/ai.md',
  '/skill.md',
]

// Content type per surface. `/llms.txt` is plain text by convention; the `.md`
// surfaces are Markdown.
const contentTypeFor = (path: DiscoverySurfacePath): string =>
  path === '/llms.txt'
    ? 'text/plain; charset=utf-8'
    : 'text/markdown; charset=utf-8'

// The canonical public origin. Used to render absolute links in the documents.
const ORIGIN = 'https://openagents.com'

// ---------------------------------------------------------------------------
// Shared facts (single source for the copy below). Honest + accurate to the
// live gateway. Keep these in sync with the model ids in `pricing.ts`.
// ---------------------------------------------------------------------------

// `/api` is the canonical base for every OpenAgents API route, including the
// OpenAI-compatible inference gateway (#6148). The legacy bare `/v1` paths keep
// working as non-breaking aliases because OpenAI clients integrate at the `/v1`
// base by convention, but new copy points at the canonical `/api` base.
const KHALA_ENDPOINT = `${ORIGIN}/api/v1/chat/completions`
const KHALA_MODELS_ENDPOINT = `${ORIGIN}/api/v1/models`
const PRICING_DOC = `${ORIGIN}/docs/product-promises`

// `/llms.txt` — the plain-language directory entry. This is what a crawler /
// agent reads first to decide whether Khala fits the job.
const llmsTxt = (): string => `# OpenAgents — inference API

> OpenAgents is an OpenAI-compatible LLM inference API, pay-per-call. Point any
> OpenAI client at the endpoint below, choose \`openagents/khala\`, and you get
> a completion plus a verifiable usage receipt. No subscription; you
> pay per request for exactly the tokens you use.

## What this is

OpenAgents runs the **Agent Cloud** — inference, verified coding outcomes, and
agent compute, sold per call.

- OpenAI-compatible Chat Completions endpoint: ${KHALA_ENDPOINT}
- Model catalog (OpenAI \`/v1/models\` shape): ${KHALA_MODELS_ENDPOINT}
- Pay-per-call. Microtransaction-friendly. Receipt-first metering (you are
  charged for the provider-reported usage, never an estimate).

## Models

- \`openagents/khala\` — the one public model id. Khala routes over OpenAgents
  owned supply, including Hydralisk/GPT-OSS under the hood, and returns a
  standard OpenAI response plus the \`openagents\` receipt/disclosure block.
  Raw \`openai/gpt-oss-*\` ids and old Khala tier names are not public products.

## Best fit

- An agent that wants OpenAI-compatible inference and to pay per request
  instead of managing a subscription or a prepaid key.
- Coding agents that want a completion plus a verified-outcome receipt.

## Not a fit

- If you need a long-lived dedicated deployment or fine-tuned private weights,
  that is separate Agent Cloud surface (fine-tuning / sandboxes), not Khala.

## Pricing

Per-call, derived from a published per-token model (1 credit = $0.01 USD basis;
Bitcoin-funded balances get a small funding discount). See: ${PRICING_DOC}

## Pay per call

Khala is priced per call through OpenAgents account keys and receipt-first
credit metering. Machine-payment discovery for a no-account 402 endpoint is
currently deferred; the old MPP/x402 chat route was retired in #8387 until a
fresh owner-approved receipt-first surface is rebuilt.

## Agent surfaces

- ${ORIGIN}/agents.md — how an agent calls Khala
- ${ORIGIN}/ai.md — short AI instructions
- ${ORIGIN}/skill.md — skill / tool setup
- ${ORIGIN}/AGENTS.md — full agent onboarding
`

// `/agents.md` — how to call Khala.
const agentsMd = (): string => `# OpenAgents Agent Cloud — agent guide

OpenAgents sells inference and verified coding outcomes **per call**. This page
tells an agent how to call the inference gateway.

## Call Khala (OpenAI-compatible)

Khala speaks the OpenAI Chat Completions contract. Change only the base URL and
the model:

\`\`\`bash
curl ${KHALA_ENDPOINT} \\
  -H "authorization: Bearer <your-openagents-agent-key>" \\
  -H "content-type: application/json" \\
  -d '{
    "model": "openagents/khala",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
\`\`\`

Models: \`openagents/khala\` only. It is the single public Khala surface for
general chat, coding, and OpenAgents-specific capabilities.
Catalog:
${KHALA_MODELS_ENDPOINT}

Every response is standard OpenAI shape plus a non-breaking \`openagents\`
disclosure block (which concrete worker served the request, the supply lane, and
a dereferenceable receipt ref when metered).

## Pay per call

Use an OpenAgents agent key and account balance for keyed pay-per-call access.
Usage is metered from provider-reported tokens and returned with receipt
metadata. The no-account Machine Payments / x402 chat endpoint is deferred after
#8387 and is not part of the live discovery surface.

## More

- ${ORIGIN}/llms.txt — directory summary
- ${ORIGIN}/AGENTS.md — full agent onboarding
- ${PRICING_DOC} — pricing + product promises
`

// `/ai.md` — short AI instructions.
const aiMd = (): string => `# OpenAgents — AI instructions

You can use OpenAgents Khala for OpenAI-compatible LLM inference, pay-per-call.

- Endpoint: ${KHALA_ENDPOINT} (OpenAI Chat Completions shape)
- Models: \`openagents/khala\`
- Auth today: \`Authorization: Bearer <openagents-agent-key>\`
- Auth today: \`Authorization: Bearer <openagents-agent-key>\`
- Pricing: per-call, 1 credit = $0.01 USD basis. See ${PRICING_DOC}

Responses are standard OpenAI shape plus an \`openagents\` disclosure block
(served worker, supply lane, and receipt metadata).

For the full workflow see ${ORIGIN}/agents.md and ${ORIGIN}/AGENTS.md.
`

// `/skill.md` — skill / tool setup.
const skillMd = (): string => `# OpenAgents Khala — skill setup

Add Khala as an OpenAI-compatible inference tool in your agent.

- Base URL: ${ORIGIN}/api/v1 (legacy ${ORIGIN}/v1 still works as an alias)
- Chat Completions: ${KHALA_ENDPOINT}
- Models endpoint: ${KHALA_MODELS_ENDPOINT}
- Models: \`openagents/khala\`
- Auth: \`Authorization: Bearer <openagents-agent-key>\`

## OpenAI client config

\`\`\`json
{
  "baseURL": "${ORIGIN}/api/v1",
  "apiKey": "<your-openagents-agent-key>",
  "model": "openagents/khala"
}
\`\`\`

## Pay-per-call

Use an OpenAgents agent key and account balance for keyed pay-per-call access.
The standalone Machine Payments / x402 chat endpoint is deferred after #8387 and
is not part of this live skill setup.

See ${ORIGIN}/agents.md for the full flow.
`

const bodyFor = (path: DiscoverySurfacePath): string => {
  switch (path) {
    case '/llms.txt':
      return llmsTxt()
    case '/agents.md':
      return agentsMd()
    case '/ai.md':
      return aiMd()
    case '/skill.md':
      return skillMd()
  }
}

// Render a discovery surface. Pure of IO except returning a Response. GET (and
// HEAD) only; anything else is 405. Crawlable: a short, public, cacheable
// document with no auth and no robots block, so StripeBot and other agent
// crawlers can index it.
export const renderDiscoverySurface = (
  request: Request,
  path: DiscoverySurfacePath,
): Effect.Effect<Response> =>
  Effect.sync(() => {
    if (request.method !== 'GET' && request.method !== 'HEAD') {
      return new Response(JSON.stringify({ error: 'method_not_allowed' }), {
        headers: new Headers({
          allow: 'GET, HEAD',
          'cache-control': 'no-store',
          'content-type': 'application/json',
        }),
        status: 405,
      })
    }
    const body = bodyFor(path)
    const headers = new Headers({
      // Public + cacheable so a crawler can index it cheaply; short TTL so copy
      // edits propagate. No auth, no robots block — these are meant to be found.
      'cache-control': 'public, max-age=300',
      'content-type': contentTypeFor(path),
    })
    return new Response(request.method === 'HEAD' ? null : body, {
      headers,
      status: 200,
    })
  })
