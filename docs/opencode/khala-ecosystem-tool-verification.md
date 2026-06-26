# Khala Ecosystem Tool Verification

> Per-tool verification record for issue #6306. The recipes live in
> [`khala-ecosystem-tool-recipes.md`](./khala-ecosystem-tool-recipes.md) and, for
> OpenCode, [`opencode-khala-recipe.md`](./opencode-khala-recipe.md). This
> document is the proof that each in-scope recipe actually drives a real session
> against the live Khala gateway. Repo documentation, not marketing copy.

## Scope (#6306, final owner direction)

- **Top priority, verified:** OpenCode, Hermes (Nous `hermes-agent`).
- **Next, verified:** Vercel AI SDK.
- **Last, verified:** LangChain (JS + Python).
- **Descoped by owner direction (not part of the #6306 verified set):** Aider,
  Cline, Continue, LiteLLM. Their recipe sections remain as #6240 reference
  material only.

## How These Were Verified

All smokes ran on 2026-06-26 against the live gateway with a fresh free key
minted from `POST https://openagents.com/api/keys/free`. The shared
OpenAI-compatible surface is `https://openagents.com/api/v1`, model
`openagents/khala`. Internal verification runs self-tagged
`x-openagents-demand-kind: internal` and a per-tool
`x-openagents-demand-source: <tool>_smoke` so they do not pollute the external
demand corpus (per #6298).

The default `openagents/khala` open lane served `openagents/glm-5.2-reap-504b`
on the `hydralisk` supply lane during these runs (visible in the
`openagents.served_model` field of each direct response). `claude`/premium lanes
are out of scope here; they still route to the balance + premium gate and never
the free lane.

The public counter (`GET /api/public/khala-tokens-served`) moved across the
session window. The counter is a global projection of `token_usage_events` and
other agents serve concurrently, so a positive delta proves traffic was served
but is **not** per-tool attribution. Per-tool public adoption still needs the
owner-gated F1 rollups (#6252) or a dedicated fresh-key test window.

## Shared Gateway Smoke (substrate under every tool)

Run directly against `/api/v1/chat/completions` with a fresh free key. Every
in-scope tool sits on exactly these behaviors.

| Behavior | Result | Evidence |
|---|---|---|
| Free key mint | PASS | `POST /api/keys/free` returns `credential.token` (`oa_agent_…`). |
| Non-streaming completion | PASS | `model: openagents/khala`, HTTP 200, `usage.total_tokens` present. |
| Streaming (SSE) | PASS | `chat.completion.chunk` deltas, terminal `finish_reason: "stop"`, usage telemetry in the final chunk. |
| Tool/function calling | PASS | `finish_reason: "tool_calls"`, valid `tool_calls[]` with correct function name and JSON `arguments`. |
| Multi-turn tool round-trip | PASS | Tool result fed back as `role: "tool"` → model emits final answer (`finish_reason: "stop"`). |
| Bad-key error legibility | PASS | Invalid bearer → HTTP 401 `{"error":"unauthorized"}`. |

The 402/quota path is documented in
[`opencode-free-tier-402-playbook.md`](./opencode-free-tier-402-playbook.md);
it returns an HTTP 402 OpenAI-style error body once a key exhausts the daily
request or token quota. It was not re-exhausted here (that would burn a key's
daily quota for no new information beyond the playbook); the recipe checklists
keep the 402 legibility check as a per-session acceptance item.

## Per-Tool Results

| Tool | Smoke depth | Result |
|---|---|---|
| OpenCode | Live CLI: chat + agentic Read-tool loop | VERIFIED |
| Hermes | Live CLI: one-shot + agentic tool loop | VERIFIED |
| Vercel AI SDK | Live SDK: generate + stream + multi-step tools | VERIFIED |
| LangChain JS | Live SDK: invoke + stream + bindTools | VERIFIED |
| LangChain Python | Live SDK: invoke + stream + bind_tools | VERIFIED |

### OpenCode — VERIFIED (chat + tool loop)

Used the published `opencode.json` provider config from
[`opencode-khala-recipe.md`](./opencode-khala-recipe.md) (provider id
`openagents`, model key `khala`, `api.id: "openagents/khala"`,
`baseURL: https://openagents.com/api/v1`, `apiKey: {env:OPENAGENTS_API_KEY}`).
OpenCode 1.17.9.

- Non-tool: `opencode run --model openagents/khala "Reply with exactly:
  khala-opencode-ok"` → `khala-opencode-ok`.
- Tool loop: in a repo with a `marker.txt`, `opencode run --model
  openagents/khala --agent plan "Use your read tool to read marker.txt … reply
  with exactly the secret marker value"` → OpenCode invoked its `Read` tool
  (`→ Read marker.txt`) and returned `MANGO-3310` — a value it could only obtain
  by actually executing the tool. Full tool-calling round-trip.

No tool-call malformation or stall surfaced in this run. (The open #6310/#6319
serving issues, owned by another agent, did not block this smoke; if a tool loop
ever stalls, that is the lane to check, and a stall must be reported honestly,
not papered over.) This also covers the spirit of #6305 — OpenCode → Khala is
verified end-to-end with tool calling.

### Hermes (Nous hermes-agent) — VERIFIED (one-shot + tool loop)

Hermes routes Khala through its `custom` provider (config-only; not a
`--provider` CLI choice). Ran in an isolated `HERMES_HOME` so the owner's real
Hermes install was untouched, with config:

```yaml
model:
  default: "openagents/khala"
  provider: "custom"
  base_url: "https://openagents.com/api/v1"
```

and `OPENAI_API_KEY` set to the free key.

- One-shot: `hermes chat -q "Reply with exactly: khala-hermes-ok" -Q
  --max-turns 2` → `khala-hermes-ok`.
- Tool loop: with a `marker.txt` present, `hermes chat -q "Use your tools to
  read marker.txt and tell me the secret marker value, then stop." -Q --yolo
  --max-turns 6` → Hermes ran its agentic tool loop and returned `BANANA-7741`,
  the file's content. Proves the full Hermes harness (multi-turn tool-calling
  loop) works through Khala, not just plain chat.

### Vercel AI SDK — VERIFIED

`@ai-sdk/openai-compatible@3.0.0` + `ai@7.0.2` via `createOpenAICompatible`:

- `generateText` → `"khala-ai-sdk-ok"`, `usage.totalTokens: 372`.
- `streamText` → 154 streamed chars with usage metadata.
- Multi-step tool calling (`tool` + `stepCountIs(3)`) → `get_weather` called
  with `{city:"Tokyo"}`, then final natural-language answer.

High leverage: OpenCode and many other agents sit on this provider, so the
verified AI SDK path backstops them.

### LangChain JS — VERIFIED

`@langchain/openai@1.5.3` `ChatOpenAI` with `configuration.baseURL` +
`defaultHeaders`:

- `invoke` → `"khala-langchain-js-ok"`.
- `stream` → 134 streamed chars.
- `bindTools` → `get_weather({city:"Berlin"})`.

### LangChain Python — VERIFIED

`langchain-openai` `ChatOpenAI(model="openagents/khala", base_url=…)`:

- `invoke` → `"khala-langchain-python-ok"`.
- `stream` → 133 streamed chars.
- `bind_tools` → `get_weather({'city':'Lisbon'})`.

## Descoped Tools (not part of #6306 verified set)

By owner direction, Aider, Cline, Continue, and LiteLLM are not in the #6306
verified set. Their recipes remain in
[`khala-ecosystem-tool-recipes.md`](./khala-ecosystem-tool-recipes.md) as #6240
reference. (For the record: during exploration before the descope, Aider's CLI
applied a real edit through Khala and LiteLLM's direct Python SDK completion,
streaming, and tool calling all worked; Cline/Continue are VS Code GUI
extensions and were never smoked headless. None of this is claimed as #6306
verification.)

## Attribution Note (#6298)

External users following these recipes resolve to `external`/`unlabeled`.
Internal verification runs above self-tagged `demand_kind=internal` and
`demand_source=<tool>_smoke` so they stay out of the external corpus. Per-tool
external adoption becomes visible once the owner-gated F1 rollups (#6252) land.
