// Agent-discovery surfaces for Khala + the OpenAgents Agent Cloud (EPIC #6049,
// Phase 1). These are plain-language, machine-readable documents that describe
// the live Khala inference API so agents (and crawlers like StripeBot, which
// feeds the Stripe Directory) can FIND and understand the service.
//
// Ship-ready, no flag: these are static documents. They make NO money claim and
// require no payment configuration — they only DESCRIBE the API and point at the
// pricing + (forward-ref) machine-payment surfaces. They are served crawlable
// (cache-friendly, no auth, no robots block) so the Stripe Directory crawler can
// match OpenAgents on "llm inference api" / "openai-compatible inference" /
// "pay-per-call" searches.
//
// Mirrors the live PostalForm/Zinc directory pattern: `/llms.txt`, `/agents.md`,
// `/ai.md`, `/skill.md`. Copy is intentionally honest and accurate to what the
// gateway actually serves today (`/v1/chat/completions`, `openagents/khala-mini`
// + `openagents/khala-code`, receipt-first metering, the `openagents` disclosure
// block). The MPP/x402 paid endpoint is a FORWARD reference (Phase 2, default
// OFF) — it is described as "coming / flagged" rather than claimed as live.

import { Effect } from 'effect'

import { methodNotAllowed } from '../http/responses'

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

const KHALA_ENDPOINT = `${ORIGIN}/v1/chat/completions`
const KHALA_MODELS_ENDPOINT = `${ORIGIN}/v1/models`
const PRICING_DOC = `${ORIGIN}/docs/product-promises`
const MPP_ENDPOINT = `${ORIGIN}/mpp/v1/chat/completions`

// `/llms.txt` — the plain-language directory entry. This is what a crawler /
// agent reads first to decide whether Khala fits the job.
const llmsTxt = (): string => `# OpenAgents — Khala inference API

> Khala is an OpenAI-compatible LLM inference API, pay-per-call. Point any
> OpenAI client at the endpoint below, change the model to an \`openagents/khala-*\`
> id, and you get a completion plus a verifiable usage receipt. No subscription;
> you pay per request for exactly the tokens you use.

## What this is

OpenAgents runs the **Agent Cloud** — inference, verified coding outcomes, and
agent compute, sold per call. Khala is the inference front door.

- OpenAI-compatible Chat Completions endpoint: ${KHALA_ENDPOINT}
- Model catalog (OpenAI \`/v1/models\` shape): ${KHALA_MODELS_ENDPOINT}
- Pay-per-call. Microtransaction-friendly. Receipt-first metering (you are
  charged for the provider-reported usage, never an estimate).

## Models

- \`openagents/khala-mini\` — cheap, fast general chat/inference tier.
- \`openagents/khala-code\` — coding tier. Coding completions are run through a
  deterministic acceptance verifier; the response carries an \`openagents\`
  disclosure block recording which concrete worker served it and the
  verification verdict, so a coding outcome is auditable rather than opaque.

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

## Pay per call (machine payments)

A machine-payable, 402-gated Khala endpoint is being rolled out at
${MPP_ENDPOINT} (Machine Payments Protocol / x402). An agent with a crypto
wallet or a shared payment token can pay per request with no signup: the first
request returns \`402 Payment Required\` with a payment challenge, the agent's
payment tool pays and retries, and the verified request returns the completion.
This endpoint is gated and OFF until launch; until then it returns a clear
"not configured" response and never charges. The contributor payout rail stays
Bitcoin/Spark.

## Agent surfaces

- ${ORIGIN}/agents.md — how an agent calls Khala (and the machine-payment flow)
- ${ORIGIN}/ai.md — short AI instructions
- ${ORIGIN}/skill.md — skill / tool setup
- ${ORIGIN}/AGENTS.md — full agent onboarding
`

// `/agents.md` — machine-payment workflow + how to call Khala.
const agentsMd = (): string => `# OpenAgents Agent Cloud — agent guide

OpenAgents sells inference and verified coding outcomes **per call**. This page
tells an agent how to call Khala and how machine payment works.

## Call Khala (OpenAI-compatible)

Khala speaks the OpenAI Chat Completions contract. Change only the base URL and
the model:

\`\`\`bash
curl ${KHALA_ENDPOINT} \\
  -H "authorization: Bearer <your-openagents-agent-key>" \\
  -H "content-type: application/json" \\
  -d '{
    "model": "openagents/khala-mini",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
\`\`\`

Models: \`openagents/khala-mini\` (general) and \`openagents/khala-code\` (coding,
with a verified-outcome receipt). Catalog: ${KHALA_MODELS_ENDPOINT}

Every response is standard OpenAI shape plus a non-breaking \`openagents\`
disclosure block (which concrete worker served the request; for \`khala-code\`,
the verification verdict and a dereferenceable receipt ref).

## Pay per call (no signup) — Machine Payments / x402

For agents that would rather pay per request than hold an account key, the
machine-payment endpoint (rolling out, flagged) is:

    ${MPP_ENDPOINT}

The flow follows the Machine Payments Protocol (MPP) / x402:

1. Your request with **no payment credential** returns
   \`402 Payment Required\` with one or more \`WWW-Authenticate: Payment ...\`
   challenges describing the price and how to pay.
2. Your payment tool (e.g. \`mppx\`) pays and **retries** the request with an
   \`Authorization: Payment ...\` credential.
3. The server verifies the credential, runs the same Khala completion, and
   returns it with the usage receipt.

Accepted rails: **USDC** (x402 on Base; MPP on Solana / Tempo) and **card** via
shared payment tokens. Microtransactions to 0.01 USDC. Settled machine payments
mint Khala credits that flow through the same metering + receipt path as any
other Khala call; the **contributor payout rail stays Bitcoin/Spark**.

Until launch, the endpoint is inert: it returns "not configured" and never
charges.

## More

- ${ORIGIN}/llms.txt — directory summary
- ${ORIGIN}/AGENTS.md — full agent onboarding
- ${PRICING_DOC} — pricing + product promises
`

// `/ai.md` — short AI instructions.
const aiMd = (): string => `# OpenAgents — AI instructions

You can use OpenAgents Khala for OpenAI-compatible LLM inference, pay-per-call.

- Endpoint: ${KHALA_ENDPOINT} (OpenAI Chat Completions shape)
- Models: \`openagents/khala-mini\`, \`openagents/khala-code\`
- Auth today: \`Authorization: Bearer <openagents-agent-key>\`
- Pay-per-call machine payments (MPP / x402, rolling out, flagged):
  ${MPP_ENDPOINT}
- Pricing: per-call, 1 credit = $0.01 USD basis. See ${PRICING_DOC}

Responses are standard OpenAI shape plus an \`openagents\` disclosure block
(served worker; for \`khala-code\`, the verification verdict + receipt).

For the full workflow see ${ORIGIN}/agents.md and ${ORIGIN}/AGENTS.md.
`

// `/skill.md` — skill / tool setup.
const skillMd = (): string => `# OpenAgents Khala — skill setup

Add Khala as an OpenAI-compatible inference tool in your agent.

- Base URL: ${ORIGIN}/v1
- Chat Completions: ${KHALA_ENDPOINT}
- Models endpoint: ${KHALA_MODELS_ENDPOINT}
- Models: \`openagents/khala-mini\`, \`openagents/khala-code\`
- Auth: \`Authorization: Bearer <openagents-agent-key>\`

## OpenAI client config

\`\`\`json
{
  "baseURL": "${ORIGIN}/v1",
  "apiKey": "<your-openagents-agent-key>",
  "model": "openagents/khala-mini"
}
\`\`\`

## Pay-per-call (machine payments)

If your agent pays per request instead of holding a key, use the machine-payment
endpoint (MPP / x402, rolling out, flagged): ${MPP_ENDPOINT}. A request without a
payment credential returns \`402 Payment Required\` with a payment challenge; your
payment tool pays and retries. Accepted rails: USDC and card. The endpoint is
inert ("not configured") until launch.

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
      return methodNotAllowed(['GET', 'HEAD'])
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
