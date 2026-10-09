# Inference

Our own inference gateway: one front for every model account we hold,
for our apps and for outside developers.

| Document | What it covers |
| --- | --- |
| [The inference gateway (spec, 2026-10-09)](gateway.md) | Every place we call a model today; the Open Responses API we serve and the Chat Completions backup; upstream accounts and adapters (Vertex and the Google credit, Z.ai GLM-5.3 Flash, the Pro door, OpenRouter, Vercel, direct, BYOK, Pylon); credit-aware routing; measurement; keys and billing on the existing gateway; the public rate card; privacy; a feature comparison with OpenRouter; the public docs outline and Decisions guide; where the code goes; rollout; owner steps and open decisions. |
| [Acceptance and SDK runs (2026-10-09)](2026-10-09-acceptance-run.md) | The Open Responses acceptance suite (17 of 17) and the OpenAI Python and JavaScript SDK runs against a local gateway, the fixes they needed, and how to run them again. |

Related: the [Pro inference door](../gateway/README.md), the
[decision gateway](../decision-models/service/gateway.md), the
[OpenAgents API design](../api/README.md), [BYOK](../byok/README.md), and the
[chat worker](../deployment/chat-worker.md).
