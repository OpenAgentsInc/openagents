# How opencode Works With Claude — SDK, Auth, and TUI Rendering Audit

Date: 2026-06-10
Author: agent audit (Claude Code)
Reference repo: `projects/repos/opencode` (clone at commit `fff36b70b`, 2026-06-05; read-only reference lane)
Companion docs:
- `docs/tui/2026-06-10-opencode-vs-pylon-tui-audit.md` (TUI gap analysis)
- `docs/tui/2026-06-10-pylon-tui-parity-roadmap.md` (roadmap, issues #4736–#4742)

## Questions this audit answers

1. Does opencode use the Claude Agent SDK (or shell out to `claude -p`)? **No — neither.**
2. How does it call Claude? **Vercel AI SDK (`@ai-sdk/anthropic`) plus its own
   Anthropic Messages protocol layer; never the official `@anthropic-ai/sdk`.**
3. How does it auth with Anthropic? **`x-api-key` API keys by default, with a
   first-class "Login with Claude Pro/Max" OAuth path delivered through its
   plugin auth-method registry rather than vendored in the repo.**
4. What Claude-specific things does the TUI render, and how? **Thinking blocks
   (hidden by default with a toggle), streamed parts via an AI-SDK event
   adapter, per-message token/cost from models.dev pricing, tool renderers,
   and typed Anthropic error states.**

Findings are grounded in the clone; two corrections to first-pass exploration
are flagged inline (CLAUDE.md reading and the Pro/Max OAuth surface — both
exist and were initially missed).

## 1. No Claude Agent SDK, no `claude -p` — a fully independent agent loop

opencode does **not** depend on `@anthropic-ai/claude-agent-sdk`,
`@anthropic-ai/claude-code`, or the official `@anthropic-ai/sdk`, and it never
spawns a `claude` binary or uses `claude -p`. Its agent loop (turn management,
tool execution, permissioning, compaction, persistence) is entirely its own
code, model-agnostic by design.

The Claude call path:

- **Provider SDK:** `@ai-sdk/anthropic` 3.0.71 under the Vercel AI SDK
  (`ai` 6.0.168) — `packages/opencode/package.json`. The session layer drives
  `streamText()` with middleware (`packages/opencode/src/session/llm.ts`),
  mapping AI-SDK stream parts to internal `LLMEvent`s
  (`session/llm/ai-sdk.ts`).
- **Own protocol layer:** `packages/llm/src/protocols/anthropic-messages.ts`
  is a complete, schema-typed (Effect Schema) implementation of the Anthropic
  Messages API — content blocks, thinking blocks with signatures, tool use,
  `cache_control` — used by a "native runtime"
  (`session/llm/native-runtime.ts`) that can bypass the AI SDK and speak the
  wire format directly for Anthropic. Endpoint default
  `https://api.anthropic.com/v1`. They maintain recorded protocol tests
  (`packages/llm/test/provider/anthropic-messages*.test.ts`).
- So the architecture is: provider-agnostic loop on top, with Anthropic as one
  of many providers — but important enough to earn a hand-rolled wire-protocol
  implementation, dedicated transforms, and its own system prompt.

**Relationship to Claude Code is inverted.** Rather than driving Claude Code,
opencode exposes *itself* as an agent over the **Agent Client Protocol**
(`@agentclientprotocol/sdk` 0.21.0): `opencode acp` runs an ACP server over
stdio (`src/cli/cmd/acp.ts`, `src/acp/`), so ACP clients (Zed-style editors)
can embed opencode the way they'd embed any coding agent.

**Claude Code ecosystem compatibility, though:** opencode deliberately reads
Claude Code's instruction files — `~/.claude/CLAUDE.md` and project-level
`CLAUDE.md` are in its instruction search path unless a
`disableClaudeCodePrompt` flag is set (`src/session/instruction.ts:60-64`,
first project-level match wins so AGENTS.md/CLAUDE.md don't stack across
ancestors). There's also an `x-claude-code-ide-authorization` header in the
TUI's editor-integration path (`tui/context/editor.ts:456`) for Claude Code
IDE-bridge auth. They treat the Claude Code config surface as a de facto
standard worth interoperating with, without taking any Anthropic SDK
dependency.

## 2. Auth with Anthropic

Three layers, cleanly separated:

**Default: API key via `x-api-key`.** The Anthropic provider resolves auth as
explicit config key → `ANTHROPIC_API_KEY` → stored credential, emitted as an
`x-api-key` header (`packages/llm/src/providers/anthropic.ts`,
`src/provider/auth.ts:13-18`). Credentials persist in
`~/.local/share/opencode/auth.json` with `0o600` permissions (overridable via
`OPENCODE_AUTH_CONTENT` env), keyed by provider ID, with two stored shapes:
`{type: "api", key}` and `{type: "oauth", access, refresh, expires}`
(`src/auth/index.ts`).

**OAuth: generic machinery in core, provider flows as plugins.** The
`ProviderAuth` layer (`src/provider/auth.ts`) implements a provider-neutral
authorize → pending → callback state machine: a plugin registers an auth
*method* (`type: "oauth" | "api"`, label, optional prompts); `authorize()`
returns a browser URL + code/redirect method; `callback()` exchanges the code
and stores either an API key or an `{access, refresh, expires}` OAuth record.
Refresh tokens are first-class in the schema. Built-in plugin auth flows in
this clone cover Codex/OpenAI, GitHub Copilot, GitLab, Poe, Azure,
DigitalOcean, xAI, Cloudflare (`src/plugin/index.ts`) — **the Anthropic OAuth
flow itself is not vendored in the repo.**

**Claude Pro/Max login is nonetheless a first-class product surface.** The app
ships dedicated UI for it: `provider.connect.title.anthropicProMax` = "Login
with Claude Pro/Max" across all i18n locales, and the connect dialog
special-cases `provider === "anthropic" && method.label.includes("max")`
(`packages/app/src/components/dialog-connect-provider.tsx:595`). The provider
description reads "Direct access to Claude models, including Pro and Max."
Auth methods are fetched at runtime from the server's plugin-populated method
registry (`provider_auth` sync data), so the Anthropic OAuth/PKCE
implementation arrives as a runtime plugin rather than repo code — consistent
with its history as a separate `opencode-anthropic-auth` package. The repo
itself contains no Anthropic authorize URL, client ID, or
`oauth-2025-04-20` header; those live behind the plugin boundary. (For
reference, Anthropic OAuth bearer tokens go on `Authorization: Bearer` plus
the `anthropic-beta: oauth-2025-04-20` header rather than `x-api-key` — the
plugin owns that translation.)

**Multi-cloud Claude:** the provider layer also routes Claude through Bedrock
(`anthropic.`-prefixed model IDs, `src/provider/provider.ts:370-420`), Google
Vertex (`@ai-sdk/google-vertex/anthropic`), and GitLab's AI gateway (which
forwards `anthropic-beta: context-1m-2025-08-07`). Auth for those rides the
respective cloud credentials, not Anthropic's.

**Takeaway for us:** auth-as-plugin with a provider-neutral
authorize/callback/refresh state machine is the design worth copying. It keeps
OAuth client IDs and vendor churn out of the core, lets subscription-auth
flows evolve independently of releases, and gives every provider the same
storage/refresh semantics. It also cleanly separates "how we sign requests"
(`x-api-key` vs `Authorization: Bearer`) per credential type.

## 3. Claude-specific behavior in the agent loop

- **Per-provider system prompt:** Claude models get a dedicated
  `prompt/anthropic.txt` selected in `src/session/system.ts:29`; assembly
  order is agent prompt → provider prompt → system additions → user system
  (`session/llm/request.ts:56-78`).
- **Prompt caching is explicit and budgeted:** their protocol layer places
  `cache_control: {type: "ephemeral"}` breakpoints on text/image/thinking
  blocks, tools, and tool results, with the API's 4-breakpoint cap enforced by
  a remaining-budget counter (`anthropic-messages.ts:31-248`), and TTL
  support (5m/1h). This is a real implementation of cache discipline, not a
  single auto-cache flag.
- **Thinking:** thinking blocks are schema'd with their `signature` preserved
  and replayed (`anthropic-messages.ts:55-60`,
  `session/message-v2.ts:264-284`). Thinking depth is driven by model
  *variants* — e.g. `high: {thinking: {type: "enabled", budget_tokens: 16000}}`,
  `max: {budget_tokens: 31999}` (`provider/transform.ts:989-991`) — and two
  beta headers are pinned globally for Anthropic:
  `interleaved-thinking-2025-05-14, fine-grained-tool-streaming-2025-05-14`
  (`packages/core/src/plugin/provider/anthropic.ts:13`).
  - Note: this targets the older extended-thinking surface. On current models
    (Opus 4.6+; `budget_tokens` is removed on Opus 4.7/4.8 and Fable 5) the
    replacement is adaptive thinking (`thinking: {type: "adaptive"}`) plus
    `output_config.effort`, and the interleaved-thinking beta header is
    subsumed. opencode's reliance on models.dev metadata means this lags until
    the catalog and transforms update — a caution for anyone copying the
    transform tables verbatim.
- **Usage/cost accounting:** per-message tracking of input, output, reasoning,
  cache-read, and cache-write tokens, with Anthropic's
  `cacheCreationInputTokens` metadata extracted explicitly
  (`session/session.ts:384-453`). Cost = each bucket × per-million pricing
  from **models.dev** (their open model catalog, fetched with a 5-minute TTL
  cache, `packages/core/src/models-dev.ts`), with reasoning billed at the
  output rate and tiered context pricing supported. Model capabilities
  (reasoning, tool_call, interleaved, modalities, context limits) come from
  the same catalog — nothing is hardcoded per model in the app.

## 4. What the TUI renders that's Claude-shaped

- **Thinking/reasoning blocks:** a dedicated `thinking_mode` context
  (`tui/context/thinking.ts`) with two states — `show` / `hide`, **default
  hidden** — toggled by command. A `reasoningSummary()` helper splits a bolded
  first line into a styled title + body so summarized reasoning renders as a
  collapsible header rather than a wall of text, with graceful handling of
  incomplete summaries mid-stream.
- **Streaming:** AI-SDK `fullStream` parts are normalized to internal events,
  so text deltas, tool-call starts, and reasoning deltas update the Solid
  store incrementally; the message feed re-renders only affected parts
  (batched, per the TUI audit).
- **Tool calls:** per-tool renderers — bash (status + streamed output), edit
  (diff visualization via `@pierre/diffs`), read (content preview) — and
  inline permission prompts when a tool requires approval. This is the
  "promote actions to dedicated tools so the harness can render them"
  principle: the TUI can show a diff for an edit precisely because edit is a
  typed tool, not an opaque shell command.
- **Cost/usage:** tokens (input/output/reasoning/cache) and USD cost surface
  in the session UI from the per-message accounting above — the user sees
  cache effectiveness directly.
- **Model selection:** the model dialog lists Claude models from models.dev
  metadata (Anthropic identified by `api.npm === "@ai-sdk/anthropic"`), with
  pricing/context/capability info available to the picker.
- **Errors:** Anthropic 429 (rate limit, with header-derived retry info), 529
  (overloaded), and 401 surfaces render as typed error states rather than raw
  strings.

## 5. Relevance to Pylon / OpenAgents

Where this touches our stack (Pylon already embeds the OpenCode runtime for
composer inference, and `apps/pylon/packages/runtime` has an optional Claude
Agent SDK integration):

1. **Two valid integration shapes, pick deliberately.** opencode proves the
   provider-agnostic shape: own loop + AI SDK + own wire protocol, no Anthropic
   SDK, subscription OAuth via plugin. Our runtime's optional Claude Agent SDK
   path is the opposite shape: delegate loop mechanics to Anthropic's SDK and
   inherit Claude Code's harness behavior. For Pylon's NIP-90 provider lane
   (sell inference into a market), opencode's shape fits better — provider
   plurality is the product. For "drive a Claude-quality coding agent,"
   the Agent SDK path remains simpler than re-deriving opencode's ~30k-LOC
   loop. Don't blend the two in one code path.
2. **TUI rendering patterns transfer regardless of loop choice.** Thinking
   hidden-by-default with a toggle + title/body summary parsing; per-message
   token/cost lines including cache read/write; typed tool renderers gated on
   typed tools; inline permission prompts. These map directly onto roadmap
   Phase 2/3 work (issues #4738, #4739) for rendering Claude Agent SDK or
   OpenCode-runtime events in the Pylon dashboard.
3. **Auth-as-plugin** (one authorize/callback/refresh state machine, provider
   flows as plugins, credentials in a 0600 JSON keyed by provider) is the
   right pattern if Pylon ever holds operator AI credentials — same shape
   our MDK wallet sidecar already uses for money, applied to API auth.
4. **Catalog-driven model metadata** (models.dev) beats hardcoding: pricing,
   context windows, and capability flags as data, with cost computed from
   usage × catalog. If Pylon prices NIP-90 inference jobs, this is the
   pattern for quoting and settling against actual token usage.
5. **Currency caution:** opencode's Claude transforms (budget_tokens variants,
   interleaved-thinking beta header) trail the current API surface (adaptive
   thinking / `output_config.effort` on Opus 4.6+; `budget_tokens` removed on
   4.7+). Any harvesting of their transform/protocol code must be re-based on
   the current Messages API rather than copied as-is.

## Bottom line

opencode treats Claude as its most important *provider*, not as its *runtime*:
no Claude Agent SDK, no `claude -p`, an independently implemented agent loop
speaking the Anthropic Messages API through the Vercel AI SDK plus its own
typed protocol layer, real prompt-cache budgeting, signature-preserving
thinking replay, and per-message cost accounting from an external model
catalog. Anthropic auth defaults to plain API keys, with Claude Pro/Max
subscription OAuth shipped as a runtime plugin behind a generic
authorize/callback/refresh machine — visible in the product UI but
deliberately absent from the core repo. Meanwhile it interoperates with the
Claude Code ecosystem from the outside: reading `CLAUDE.md`, bridging Claude
Code IDE auth, and exposing itself over ACP to be embedded just like Claude
Code would be.
