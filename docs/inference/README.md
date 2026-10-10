# Inference

Our own inference gateway: one front for every model account we hold,
for our apps and for outside developers.

| Document | What it covers |
| --- | --- |
| [The inference gateway (spec, 2026-10-09)](gateway.md) | Every place we call a model today; the Open Responses API we serve and the Chat Completions backup; upstream accounts and adapters (Vertex and the Google credit, Z.ai GLM-5.3 Flash, the Pro door, OpenRouter, Vercel, direct, BYOK, Pylon); credit-aware routing; measurement; keys and billing on the existing gateway; the public rate card; privacy; a feature comparison with OpenRouter; the public docs outline and Decisions guide; where the code goes; rollout; owner steps and open decisions. |
| [Model providers (2026-10-10)](providers.md) | Every model and embedding call site in the repository, its first door and fallbacks: Google (Vertex AI on the prepaid credit) first on our keys, BYOK and user-machine paths unchanged. |
| [Acceptance and SDK runs (2026-10-09)](2026-10-09-acceptance-run.md) | The Open Responses acceptance suite (17 of 17) and the OpenAI Python and JavaScript SDK runs against a local gateway, the fixes they needed, and how to run them again. |
| [Self-hosting Clef (2026-10-09)](clef-self-host.md) | Cloudflare's open-weight Clef and Clef-Flash decision models on this Mac (Ollama, MLX) and coderos-4080 (llama.cpp CUDA): verified facts, exact commands, API differences from TypeSafe, latency and memory, agreement with hosted Jev on 75 real router requests, and whether to add a Clef door to the judge failover chain. |
| [File-relevance bench: Clef vs Jev (2026-10-09)](clef-jev-relevance-bench.md) | "Is this file relevant to this issue?" on 10 closed issues with ground truth: decisions/s sequential, batched and concurrent, latency, F1 and calibration for Clef-Flash and Clef 27B (Ollama and llama.cpp on the Mac and coderos-4080) and hosted Jev; why the backends differ; which to use in Coder. |
| [Tools for the briefed agent (2026-10-10)](briefed-agent-tools.md) | Every custom tool worth trying for the issue → PR agent (verify, related, outline/read_symbol, finish, apply_patch, find_symbol, example_change, codemods, rules, decide, self_review and more): inputs, outputs, why each might help, the result that keeps or drops it, and the experiment order (#11211). |
| [File finding: issue to files (#11210)](file-finding-bench.md) | `scripts/filefind/filefind.py`: from an issue to the ranked files its fix needs in about a second, from indexes mined from Git history (co-change, similar past issues, commit subjects, token index, embeddings) and a small trained scorer; end-to-end bench over 100 closed issues replayed at the fix's parent (recall per stage, at 20/50/100/200, which kinds of file are missed), Jev and planner stages, and an assessment on open issues. |
| [Clef in Psionic (plan)](clef-native.md) | Serving Clef natively in Psionic, with no Ollama: what is missing, the `/v1/systemone` route, speed targets, and the issues (psionic#1159–1162, #11191–11193). |

Related: the [Pro inference door](../gateway/README.md), the
[decision gateway](../decision-models/service/gateway.md), the
[OpenAgents API design](../api/README.md), [BYOK](../byok/README.md), and the
[chat worker](../deployment/chat-worker.md).
