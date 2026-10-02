# OpenAgents API

Design notes for putting OpenAgents itself, the composable general agent,
behind an API that partner apps, websites, other agents, and self-hosters
can call. Nothing here is implemented as a public API yet; the documents say
what exists today that it would sit on.

| Document | What it covers |
| --- | --- |
| [The OpenAgents API (speculative design, 2026-10-02)](2026-10-02-openagents-api.md) | What "OpenAgents behind an API" means, the shape options and the recommended layering, resources and events, where it runs, identity and safety, cost, a phased path, and open questions. |

Related: the existing keyed HTTP [decision gateway](../decision-models/service/gateway.md)
(System One judgments, not the agent), the [chat worker](../deployment/chat-worker.md)
that serves OpenAgents chat today, and the [NIPs](../../nips/openagents/README.md).
