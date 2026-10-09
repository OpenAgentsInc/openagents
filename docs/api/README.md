# OpenAgents API

Design notes for putting OpenAgents itself, the open network of agents
behind one conversation, behind an API that partner apps, websites, other agents, and self-hosters
can call. Nothing here is implemented as a public API yet. The public face is
plain HTTP with an API key; Nostr stays inside our own front, as the CLI
hides it today; payment is x402.

| Document | What it covers |
| --- | --- |
| [The OpenAgents API (design, 2026-10-02)](2026-10-02-openagents-api.md) | The owner's decisions; plain HTTP at `api.openagents.com` with curl examples for every main call; how our front turns each call into NIP traffic and where keys live; x402 Lightning payment with the 402, pay, retry flow; the endpoint-to-NIP table and the NIP gaps; where it runs; the phased path; and what is still open. |
| [A router for agentic execution (plan, 2026-10-02)](2026-10-02-agentic-execution-router.md) | Outcome-based routing, explicit execution and disclosure grants, durable task recovery, payment and settlement boundaries, evidence, and phased implementation gates. |

Related: the existing keyed HTTP [decision gateway](../decision-models/service/gateway.md)
(System One judgments, not the agent), the [chat worker](../deployment/chat-worker.md)
that serves OpenAgents chat today, and the [NIPs](../../nips/openagents/README.md).
