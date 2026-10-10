# BYOK (bring your own key)

How a person runs OpenAgents on their own provider keys (OpenRouter, Vercel AI
Gateway, TypeSafe) instead of ours.
Nothing here is implemented yet. It is tracked in
[#10176](https://github.com/OpenAgentsInc/openagents/issues/10176).

| Document | What it covers |
| --- | --- |
| [Your Claude plan in OpenAgents (plan, 2026-10-10)](claude-plan.md) | Running work on a person's own Claude plan, mapped from Amp's "Use Your Claude Plan": connecting a plan, your computer versus Cloud Environments, the Agent SDK engine, plan usage, pause and resume, failover to Vertex, decisions on our Pylons, one task at a time, and Anthropic's terms. |
| [BYOK: run everything through the user's own keys (design, 2026-10-02)](2026-10-02-byok-openrouter.md) | Every place we pay for a model call today and its equivalent on OpenRouter, Vercel AI Gateway, and TypeSafe; the `provider-key` commands, flags, and screens; secure storage; one model-access layer that decides who pays (`ours` / `mine`, with a fixed rule for which of the person's keys answers); the hosted chat on the person's keys (a payer envelope sent with each job); API callers bringing a key instead of paying x402 for model cost; failure messages; who-paid records; the owner's decisions; the phased plan. |

Related: the [chat worker](../deployment/chat-worker.md), the
[decision worker](../deployment/decision-worker.md), the
[OpenAgents API design](../api/README.md), and the
[System One cost audit](../cost/README.md).
