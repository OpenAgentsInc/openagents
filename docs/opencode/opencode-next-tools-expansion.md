# OpenCode → Ecosystem Tools Expansion Plan

> Source context: `docs/inference/2026-06-25-khala-inference-gtm-push.md` §3
> (Next tools after OpenCode), Pillar 2 — The ecosystem-tools playbook.
> Planning document only. The canonical shipped #6240 recipes live in
> [`khala-ecosystem-tool-recipes.md`](./khala-ecosystem-tool-recipes.md).
> Defers to the product-promise registry for public claims.

The playbook for every tool is identical: **one config line, zero rewrites.** Each tool below is an OpenAI-compatible consumer. The deliverable for each is a short, exact, copy-pasteable recipe (base URL + free key + model id) plus a "what to test" checklist. Keep recipes in this repo's docs and on `/khala` once the copy gate clears.

After OpenCode is verified and its runbook published, next in priority order:

---

## 1. Aider

**Priority:** Highest (big coding audience, CLI-native, one env-var config line)

**Why:** Aider is the most popular CLI coding agent outside OpenCode. It accepts `OPENAI_API_BASE` and `--model` flags, making Khala a one-liner. Large, well-known audience that generates real token volume.

**Exact recipe:**
```bash
export OPENAI_API_BASE=https://openagents.com/api/v1
export OPENAI_API_KEY="$OPENAGENTS_API_KEY"
aider --model openai/openagents/khala
```

Aider uses the OpenAI Python client under the hood. Base URL override maps directly to our `/api/v1/chat/completions`.

**Config details to verify:**
- Aider's `OPENAI_API_BASE` env var or `--openai-api-base` flag maps correctly to our endpoint (it appends `/chat/completions` — confirm it does not append twice)
- Model name `openai/openagents/khala` selects the OpenAI-compatible route and sends `openagents/khala` as the model field
- Tool/function calling: Aider's edit workflow depends on the model producing `read_file` / `write_file` / `run` tool calls. This is the critical correctness test.
- Streaming: Aider uses streaming by default for real-time output. Verify SSE works end-to-end.
- `--no-git` mode: Aider commits by default; test without git hooks first for a clean signal.

**What to test:**
1. Aider connects and lists files with `/files` — proves base chat completion works
2. Aider makes an edit (tool call round-trip) — `read_file` → edit → `write_file` → `run lint` — the full tool-calling loop
3. Aider streaming output renders cleanly (no garbled deltas)
4. Free-tier quota: one session stays within 2,500,000 tokens; a second session after quota exhaust returns a readable 402
5. Token counter: session tokens appear on `/api/public/khala-tokens-served`
6. `--model-settings-role map` — if Aider maps model ids to capability profiles, ensure `openagents/khala` is recognized or falls back gracefully

**Adoption path:**
1. Verify with a live Aider session (manual smoke test)
2. Publish the one-liner recipe as a runbook doc
3. Consider upstream contribution to Aider's `aider/models.py` model registry so `openagents/khala` is recognized without env-var gymnastics
4. Track: Aider-originating tokens via user-agent header or dedicated API key prefix

---

## 2. Cline / Continue

**Priority:** High (IDE-native, large VS Code audience, custom provider UX)

**Why:** Cline and Continue are the dominant VS Code agent extensions. Both support custom OpenAI-compatible providers through their settings UI. IDE-native adoption means developers discover Khala without leaving their editor.

### Cline

**Exact recipe:**
Open Cline settings → API Provider → select **OpenAI Compatible** → fill:

| Field | Value |
|---|---|
| Base URL | `https://openagents.com/api/v1` |
| API Key | `<free key from POST /api/keys/free>` |
| Model ID | `openagents/khala` |

Or in VS Code settings.json:
```json
{
  "cline.apiProvider": "openai",
  "cline.openAiBaseUrl": "https://openagents.com/api/v1",
  "cline.openAiKey": "<free key>",
  "cline.openAiModel": "openagents/khala"
}
```

**What to test:**
1. Chat completion renders in Cline's conversation panel
2. Tool calls (Cline's `read_file`, `write_file`, `execute_command`) round-trip correctly
3. Streaming output renders incrementally (Cline processes SSE deltas)
4. Cline's token usage display matches our `usage` field in non-streaming responses
5. Error states: 402 displays as a readable error, not a crash
6. Model context window: Cline may send the full conversation history; verify 128k context works end to end

### Continue

**Exact recipe (`~/.continue/config.yaml`):**
```yaml
name: OpenAgents Khala
version: 0.0.1
schema: v1

models:
  - name: Khala
    provider: openai
    model: openagents/khala
    apiBase: https://openagents.com/api/v1
    apiKey: <credential.token>
```

**What to test:**
1. Chat and tab-autocomplete both work (tab-autocomplete may need a separate lightweight model — just test chat)
2. Context retrieval: Continue sends retrieved context as system messages; verify the system-message limit
3. Streaming in the Continue sidebar renders cleanly
4. Slash commands (`/edit`, `/comment`) produce valid tool calls (Continue uses different tool schemas than OpenCode — verify compatibility)
5. Free tier: a typical Continue session fits within 2,500,000 tokens

**Adoption path:**
1. Verify with live Cline and Continue sessions
2. Publish combined "VS Code Agent Extensions" runbook doc with both recipes
3. Consider listing in Cline's community provider directory and Continue's model registry
4. Track: VS Code agent adoption via user-agent header pattern

---

## 3. Vercel AI SDK

**Priority:** High (substrate under OpenCode and many other tools; one documented snippet reaches every AI SDK consumer)

**Why:** The AI SDK's `@ai-sdk/openai-compatible` provider is the foundation OpenCode itself uses. A clean, documented snippet means every AI SDK application — from Next.js chatbots to agent frameworks — can point at Khala. This is a force multiplier, not a single-tool win.

**Exact recipe:**
```typescript
import { createOpenAICompatible } from '@ai-sdk/openai-compatible'

const khala = createOpenAICompatible({
  name: 'openagents',
  baseURL: 'https://openagents.com/api/v1',
  apiKey: process.env.OPENAGENTS_API_KEY,
  includeUsage: true,
  headers: {
    'x-openagents-demand-kind': 'external',
    'x-openagents-demand-source': 'ecosystem',
    'x-openagents-client': 'ai-sdk',
  },
})

const model = khala('openagents/khala')
```

Or with per-instance config:
```typescript
const { text } = await generateText({
  model: khala.chatModel('openagents/khala'),
  messages: [{ role: 'user', content: 'Hello' }],
})
```

**What to test:**
1. `generateText` returns a valid completion (non-streaming)
2. `streamText` returns valid SSE stream (streaming)
3. `streamText` with tool definitions produces tool call deltas
4. Multi-turn conversation (preserves tool calls and tool results)
5. System prompt and multi-modal content arrays
6. AI SDK's built-in rate-limit retry logic handles 402 correctly (may need `maxRetries: 0` doc note)
7. Context window: AI SDK sends `maxTokens` in the request body; verify our API respects it

**Adoption path:**
1. Verify the snippet with a live AI SDK client (Node.js script)
2. Publish as a "Using Khala with the AI SDK" runbook doc — short, copy-pasteable
3. Optionally contribute the provider preset to the AI SDK's built-in provider list (upstream PR to `packages/providers/openai-compatible/`)
4. Track: AI SDK adoption is harder to attribute directly (it's a library, not a tool) — recommend `x-openagents-client: ai-sdk` plus `x-openagents-demand-source: ecosystem` for attribution

---

## 4. LiteLLM / LangChain

**Priority:** Medium (reaches the long tail of agent frameworks; lower per-tool traffic than coding agents but broad surface area)

**Why:** LiteLLM and LangChain are the middleware layers for hundreds of agent-based applications. Getting `openagents/khala` into their provider configurations makes Khala available to every consumer of those frameworks — a distribution multiplier.

### LiteLLM

**Exact recipe (`litellm` config or env):**
```yaml
# config.yaml
model_list:
  - model_name: openagents-khala
    litellm_params:
      model: openai/openagents/khala
      api_base: https://openagents.com/api/v1
      api_key: <free key>
```

Or environment variables:
```bash
export OPENAI_API_BASE=https://openagents.com/api/v1
export OPENAI_API_KEY=<free key>
# LiteLLM maps "openai/<model>" to the OpenAI-compatible format
litellm --model openai/openagents/khala
```

LiteLLM has built-in support for custom OpenAI-compatible API bases via the `openai/` prefix convention. Confirm this routes to our `/api/v1/chat/completions` correctly.

**What to test:**
1. LiteLLM proxy starts and serves `/chat/completions` on its local port
2. A request through the LiteLLM proxy reaches our API and returns a valid response
3. LiteLLM's cost tracking works with our usage response (`prompt_tokens`, `completion_tokens` in the response body)
4. Streaming through LiteLLM proxy

### LangChain

**Exact recipe:**
```python
from langchain_openai import ChatOpenAI

llm = ChatOpenAI(
    base_url="https://openagents.com/api/v1",
    api_key="<free key>",
    model="openagents/khala",
)
```

Or with `langchain_community`:
```python
from langchain_community.chat_models import ChatOpenAI
# Same parameters
```

LangChain's `ChatOpenAI` wraps the OpenAI Python client, which supports arbitrary `base_url`. No upstream code changes needed.

**What to test:**
1. `llm.invoke("Hello")` returns a valid `AIMessage`
2. `llm.stream("Hello")` yields incremental chunks
3. LangChain tool calling: `llm.bind_tools([...])` produces valid tool calls
4. LangChain's built-in retry logic handles 402
5. Agent executor: a simple LangChain agent (ReAct, tool-calling) runs through Khala

**Adoption path:**
1. Verify both LiteLLM proxy and LangChain direct integration
2. Publish a single "Middleware / Framework Integration" runbook doc covering both
3. Contribute `openagents/khala` to LiteLLM's `model_prices_and_context_window.json` for cost tracking
4. Contribute `openagents/khala` to LangChain's model registry (it uses `ChatOpenAI` with no registry — but a doc note)
5. Track: LiteLLM and LangChain are proxy layers; use `x-openagents-client`
   where the caller/proxy can set custom headers, otherwise rely on fresh-key
   counter windows until F1 (#6252) ships owner-gated rollups.

---

## 5. Codex / Claude Code-compatible Clients

**Priority:** Medium (captive audiences; OpenAI-compatible clients are a natural fit)

**Why:** Codex CLI, Claude Code, and compatible clients already speak OpenAI Chat Completions. They accept `--base-url` or env vars. Capturing these audiences means being discoverable where developers already look.

**Exact recipe (Codex CLI):**
```bash
export OPENAI_BASE_URL=https://openagents.com/api/v1
export OPENAI_API_KEY=<free key>
codex --model openagents/khala
```

**Exact recipe (Claude Code — if using compatible mode):**
```bash
export ANTHROPIC_BASE_URL=https://openagents.com/api/v1
export ANTHROPIC_API_KEY=<free key>
# Verify Claude Code accepts an OpenAI-compatible endpoint — may need adapter
```

**What to test:**
1. Codex CLI connects and runs a basic task
2. Codex CLI's tool-calling loop (file read/write, bash) works through Khala
3. Claude Code (if compatible): same verification
4. Context window handling: these tools send large system prompts and conversation histories
5. Error handling: 402 from Khala is reported clearly in the client CLI

**Adoption path:**
1. Verify with Codex CLI first (cleanest OpenAI-compatible fit)
2. Publish recipe in the coding-tools runbook doc
3. For Claude Code: verify compatibility model; may need adapter since Claude Code speaks Anthropic API, not OpenAI. If incompatible, defer or document "not yet supported."
4. Track: client-specific user-agent headers (if these clients send identifiable headers)

---

## 6. OpenRouter-style Aggregators

**Priority:** Later-stage (requires reliability, paid loop, and SLA)

**Why:** OpenRouter, Together, Fireworks, and similar aggregators list upstream models for their users. Being listed means every aggregator consumer can discover Khala. This is a distribution multiplier but requires production-grade reliability, a working paid loop (the three-way split), and an SLA before listing.

**Exact recipe (hypothetical — aggregator-specific):**
- OpenRouter: submit `openagents/khala` to their provider registry with base URL and API key
- Together: similar upstream provider registration
- Model aggregator consumers: one config change, same as individual tools

**Gating requirements before listing:**
1. **Paid loop must be operational.** Aggregator users pay for inference; the three-way split (margin / serving node / referrer) must be collectable and auditable. This is owner-gated per the GTM doc §6.
2. **Reliability SLA.** Aggregators expect 99%+ uptime and consistent latency. Pillar 1 (internal dogfood) and Pillar 3 (benchmarking) must demonstrate this first.
3. **Rate limiting and multi-key management.** Aggregators send traffic from many users through one upstream key. We need to support API-key-level rate limiting that does not degrade one user's traffic when another user is over-quota.
4. **Latency requirements.** Aggregators compare providers head-to-head. P50/P90/P99 latency must be competitive with other listed providers. The benchmark harness (§4 of GTM doc) must produce decision-grade reports.
5. **Model quality consistency.** Aggregator users expect the same model quality regardless of routing path. The gym's benchmarking ladder (§4, Pillar 3) must verify consistency across serving lanes.

**Adoption path (deferred):**
1. Do not pursue aggregator listing until the paid loop ships and the owner gates the three-way split
2. Once gating passes, start with OpenRouter (largest aggregator audience, well-documented upstream registration)
3. Then Together, then Fireworks provider list
4. Track: each aggregator provides traffic telemetry on how many tokens they route through us

---

## Expansion Sequence Summary

| Phase | Tools | Gating | Token impact |
|---|---|---|---|
| **Phase 0** (shipped) | OpenCode recipe verified | None (internal recipe) | Low (manual adoption) |
| **Phase 1** (next) | Aider, Cline, Continue | None (one-config recipes) | Medium (CLI + IDE users) |
| **Phase 2** | AI SDK, LiteLLM, LangChain | Codex/Claude Code verification pending | Medium-high (framework distribution) |
| **Phase 3** | Codex CLI, Claude Code | Verify OpenAI vs Anthropic API compatibility | Medium (captive audiences) |
| **Phase 4** (deferred) | OpenRouter / aggregators | Paid loop, SLA, benchmark decision-grade reports | High (aggregator distribution) |

**Overlapping work pattern:** Phases 1 and 2 can run in parallel (Aider + AI SDK recipes are independent). Phase 3 depends on compatibility verification (one afternoon of testing). Phase 4 is gated on owner decisions outside this plan.

---

## Per-Tool Tracking

We need to attribute tokens to each tool to know which landings actually move the north-star metric. Approaches (in order of preference):

1. **Attribution headers where supported.** Use
   `x-openagents-demand-kind`, `x-openagents-demand-source`, and
   `x-openagents-client`; the gateway records them in safe ledger metadata.
2. **Dedicated API key per tool.** For clients that cannot set headers, mint a
   fresh key per tool and use before/after public counter deltas for verification.
3. **User-agent header parsing.** Later F1 rollups may also read tool user agents,
   but do not claim per-tool totals from user-agent guesses alone.
4. **Fallback: manual cohort analysis.** If none of the above, compare
   token-history buckets before and after publishing each recipe.

**Recommendation:** Use headers for AI SDK / LangChain JS / any controlled proxy,
and fresh per-tool keys for Aider, Cline, and Continue until F1 (#6252) ships the
owner-gated analytics split.

---

## Acceptance Checklist (reusable for every tool)

Before marking any tool as "integrated":

- [ ] Tool connects and returns a basic completion (non-streaming)
- [ ] Tool streaming works end to end (SSE deltas, no garbled output)
- [ ] Tool/function calling round-trips correctly (the tool's core workflow)
- [ ] Free-tier quota: one session works, over-quota returns readable 402
- [ ] Token counter increments for the tool's traffic
- [ ] 402/401/429 errors are displayed readably in the tool's UX (not a crash)
- [ ] Recipe published in this repo's docs with exact config and test checklist
- [ ] Recipe verified against the live production endpoint

---

## Status

**Updated:** the shipped #6240 recipes live in
`docs/opencode/khala-ecosystem-tool-recipes.md`. This plan remains the broader
expansion map for later Codex/Claude Code and aggregator work.
