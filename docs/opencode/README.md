# OpenCode/Khala Docs

Planning memos and session exports from the five concurrent Khala/OpenCode
planning sessions run on 2026-06-25. These sessions were read-only planning
rounds against the Khala inference GTM push strategy
([`docs/inference/2026-06-25-khala-inference-gtm-push.md`](../inference/2026-06-25-khala-inference-gtm-push.md)).
The north-star metric is **tokens served per day**; the work below routes
internal agents through Khala (Pillar 1), lands ecosystem-tool integrations
starting with OpenCode (Pillar 2), and benchmarks Khala on a ladder of
opponents (Pillar 3).

## Authoritative Recipe

- [`opencode-khala-recipe.md`](./opencode-khala-recipe.md) is the current
  copy-pasteable OpenCode to Khala recipe: free key, `opencode.json`, selector
  decision, smoke test, token-counter check, and 402/quota behavior.
- [`opencode-provider-config-model-selector.md`](./opencode-provider-config-model-selector.md)
  explains why the recipe uses model key `khala` plus
  `api.id: "openagents/khala"` to show `openagents/khala` in OpenCode while
  sending the correct upstream model id.
- [`opencode-free-tier-402-playbook.md`](./opencode-free-tier-402-playbook.md)
  documents the current free tier and quota failure path.
- [`khala-ecosystem-tool-recipes.md`](./khala-ecosystem-tool-recipes.md)
  publishes the next-tool recipes. The #6306 verified set is Hermes, Vercel AI
  SDK, and LangChain (OpenCode has its own recipe doc above); Aider, Cline,
  Continue, and LiteLLM remain as #6240 reference but are descoped from #6306.
  Includes upstream research links and attribution guidance.
- [`khala-ecosystem-tool-verification.md`](./khala-ecosystem-tool-verification.md)
  is the per-tool verification record (#6306): which recipes were end-to-end
  smoked against the live gateway, with evidence, plus the manual in-editor
  checklist for the GUI-only tools (Cline, Continue).

The final-output files below are planning exports from the 2026-06-25 sessions.
They preserve explored configs and historical notes, including older doubled
selector examples; use the authoritative recipe above for implementation.

## Sessions

| Session title | Final output | One-line description |
|---|---|---|
| `khala-opencode-adoption` | [`khala-opencode-adoption-final-output.md`](./khala-opencode-adoption-final-output.md) | Exacts the OpenCode config recipe, model-selector path, and "what to test" checklist for the first external tool integration. |
| `khala-tool-compat` | [`khala-tool-compat-final-output.md`](./khala-tool-compat-final-output.md) | Audits the gateway's API compatibility against OpenCode's tool-calling, streaming, content arrays, and error surfaces; flags bug #6232. |
| `khala-head-to-head-gym` | [`khala-head-to-head-gym-final-output.md`](./khala-head-to-head-gym-final-output.md) | Designs the benchmarking gym (GYM) ladder — Khala vs BigPickle, free models, then paid frontier — on cost-per-accepted-outcome, verified-rate, and latency. |
| `opencode-usage-audit-inventory` | [`opencode-usage-audit-inventory-final-output.md`](./opencode-usage-audit-inventory-final-output.md) | Catalogs every OpenCode surface touched by the integration (auth, provider schema, config files, tool registry, model limits). |
| `khala-internal-dogfood` | [`khala-internal-dogfood-final-output.md`](./khala-internal-dogfood-final-output.md) | Routes qa-runner, OpenCode, Autopilot, Raynor, Probe, and Verse inference through Khala — Pillar 1's concrete dogfood pipeline. |

## Raw Exports

The direct OpenCode SQLite exports are in [`raw/`](./raw/):

- `sessions.json`
- `session_messages.json`
- `messages.json`
- `parts.json`
- `session_inputs.json`
- `todos.json`
