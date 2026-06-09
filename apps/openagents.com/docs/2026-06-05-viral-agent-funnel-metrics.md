# Viral Agent Funnel Metrics

Status: initial OpenAgents product surface implementation for `OPENAGENTS-VIRAL-019`.

The first viral-agent metric slice records public-safe reads of the machine
interfaces that an external AI agent or human operator can use to understand
OpenAgents:

- `/.well-known/openagents.json` -> `capability_manifest_read`
- `/api/openapi.json` -> `openapi_read`
- `https://openagents.com/AGENTS.md` -> `agent_doc_read`
- `/api/public/proof/otec` -> `public_proof_read`

Events are stored in `viral_agent_funnel_events`.

## Privacy Boundary

The event row intentionally stores only bounded routing and class metadata:

- event kind;
- route;
- actor class: public anonymous, possible signed-in browser, or possible
  scoped agent;
- user-agent class: agent/CLI, browser, crawler, or unknown;
- optional Site/proof refs;
- bounded metadata JSON;
- created timestamp.

It does not store raw prompts, request bodies, bearer tokens, cookies, IP
addresses, provider account refs, auth grants, callback tokens, runner
payloads, or private customer material.

## Current Hooks

The Worker route table records public funnel reads in the background so the
public response remains fast and the metric insert does not alter the route
contract.

The current implementation is intentionally coarse. It measures whether public
machine-readable surfaces are being read and by what broad actor class, not who
the reader is.

## Future Metrics

Owner-claim, contribution, accepted outcome, and paid referral events should
extend this model with separate receipt-backed tables or event refs. They
should not overload public read metrics with private prompts or identity
material.

Useful next events:

- copy-to-agent CTA copied;
- first owner-claim attempt;
- first scoped action attempt;
- contribution proposal submitted;
- contribution accepted;
- accepted outcome linked to a Site/order;
- paid or credited referral event recorded.
