# BYOK (bring your own key)

How a person runs OpenAgents on their own OpenRouter API key instead of ours.
Nothing here is implemented yet. It is tracked in
[#10176](https://github.com/OpenAgentsInc/openagents/issues/10176).

| Document | What it covers |
| --- | --- |
| [BYOK: run everything through the user's own OpenRouter key (design, 2026-10-02)](2026-10-02-byok-openrouter.md) | Every place we pay for a model call today and its OpenRouter equivalent; the CLI flag, the settings commands, and the screens; secure storage; one model-access layer that decides who pays (`ours` / `mine` / `mine_then_ours`); the hosted chat on the person's key (a payer envelope sent with each job); failure messages; who-paid records; the phased plan; open questions. |

Related: the [chat worker](../deployment/chat-worker.md), the
[decision worker](../deployment/decision-worker.md), the
[OpenAgents API design](../api/README.md), and the
[System One cost audit](../cost/README.md).
