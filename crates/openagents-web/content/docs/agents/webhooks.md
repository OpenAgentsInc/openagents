# Webhooks

We don't send webhooks to API callers today. Everything an agent needs
to know comes back on the request, on a stream, or from a call it makes.

| To know | Instead of a webhook |
| --- | --- |
| When a model answer is done | Stream it and wait for `response.completed`: [Stream the answer](/docs/agents/chat-and-stream) |
| When an OpenAgents chat is done | Read `GET /v1/threads/{id}` until `thread.working` is `false` |
| When a Coder run is done | Read `openagents coder chat --json` until the `finished` line: [Run Coder headless](/docs/agents/coder-headless) |
| How much you have left | `GET /v1/agents/me` or `GET /v1/key`: [Read your plan and usage](/docs/agents/plan-and-usage) |
| That a payment went through | The `PAYMENT-RESPONSE` or `Payment-Receipt` header on the paid answer |

## What's next

Workspace webhooks for managed agents are designed but not built:
[docs/api/managed-agents-operations.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/api/managed-agents-operations.md).
Follow [What works today](https://openagents.com/promises) to see when
they ship.
