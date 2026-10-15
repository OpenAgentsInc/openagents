# Builder.io Agent-Native: what it is and what we should adapt (2026-10-15)

[Agent-Native](https://github.com/BuilderIO/agent-native) is Builder.io's
open-source TypeScript framework for apps where "the UI and the agent are
equal partners". This note reads its README, docs, templates and source, then
compares it with our stack and says what to take, where it lands, and what to
skip.

Sources: a scratch clone of the repo at
[`56fec085fd`](https://github.com/BuilderIO/agent-native/tree/56fec085fd5e74015761d2191b6be70d171b337c)
(2026-10-15, `main`), read outside this repo and deleted after. The docs live
in the repo under
[`packages/core/docs/content/`](https://github.com/BuilderIO/agent-native/tree/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/docs/content)
and are published at [agent-native.com/docs](https://agent-native.com/docs).
Nothing was installed or run.

**Short answer.** The framework itself is not for us: it is React, Nitro,
Drizzle and Postgres, and we are Rust, Maud, HTMX and Rust Native. Its core
idea is one we half-have and should finish: **define each operation once, and
derive every surface from that one definition** (agent tool, UI call, HTTP,
MCP, WebMCP, CLI). Today we hand-list the same tools in several places. Five
smaller ideas are also worth taking: page context for the agent, per-call
approval predicates, write receipts that the final answer is checked against,
typed tool ranking instead of sending every tool, and MCP Apps that embed our
real pages.

## 1. What it is

### The pitch in one example

```ts
// actions/hello.ts
export default defineAction({
  description: "Return a friendly greeting.",
  schema: z.object({ name: z.string().default("world") }),
  http: { method: "GET" },
  run: async ({ name }) => ({ message: `Hello, ${name}!` }),
});
```

That one file becomes an agent tool, a React hook
(`useActionQuery("hello", …)`), an HTTP route at
`/_agent-native/actions/hello`, an MCP tool at `/mcp`, an A2A skill, a WebMCP
tool in the browser, and `pnpm action hello`. The framework scans `actions/`
at startup; there is no registry file to keep in sync
([README](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/README.md),
[actions overview](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/docs/content/actions-overview.mdx)).

### The contract

The [key concepts page](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/docs/content/key-concepts.mdx)
states five rules every app follows:

| Rule | Meaning |
| --- | --- |
| Data lives in Postgres | Drizzle ORM; PGlite locally. Agent and UI read the same rows. |
| All AI goes through the agent | UIs never call an LLM directly. |
| Actions for operations | Real work is a typed action, not inline code. |
| Live sync | DB changes stream over SSE, polling as fallback. |
| Application state in SQL | Navigation and selection live in an `application_state` table the agent reads. |

Every feature is supposed to touch four areas: **UI, action, skill or
instructions, and app state**. "A feature with only UI is invisible to the
agent … a feature without app-state means the agent is blind to what the user
is doing."

### Three layers

| Layer | What it holds |
| --- | --- |
| Core (`@agent-native/core`) | Actions, DB helpers, auth (Better Auth, orgs, RBAC), app state, the agent loop, MCP server and client, A2A, WebMCP, live sync, run manager |
| Toolkit (`@agent-native/toolkit`) | Optional shadcn/ui-based building blocks: editors, sharing, comments, settings, the agent chat UI |
| Templates (`templates/*`) | Full apps: Mail, Calendar, CRM, Analytics, Slides, Design, Clips, Forms, Dispatch and more |

### How agents and apps interact

| Concern | How Agent-Native does it |
| --- | --- |
| **Actions** | `defineAction({ description, schema, run, … })` with ~40 optional fields. The ones that matter: `readOnly`, `needsApproval` (boolean or predicate on the actual args), `authorize` (runs on every caller), `retrySafe`, `endsTurn`, `agentTool`, `mcpTool`, `deferLoading`, `outputSchema`, `chatUI`, `link`, `mcpApp`, `changeResource` ([`action.ts`](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/src/action.ts)). |
| **State** | The UI writes `navigation`, `selection` and `__url__` keys to `application_state`. A standard `view-screen` action turns them into real records; a standard `navigate` action lets the agent move the UI ([context awareness](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/docs/content/context-awareness.mdx)). |
| **UI in chat** | Three tiers: native widgets an action declares (`data-table`, `data-chart`, `data-insights`); "generative UI", where the agent writes a sandboxed Alpine/Tailwind iframe; and MCP Apps for external hosts ([native chat UI](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/docs/content/native-chat-ui.mdx), [generative UI](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/docs/content/generative-ui.mdx)). |
| **Tool calls** | A compact catalog by default plus a `tool-search` tool for the rest. Before each turn, Jev (TypeSafe's System One model) ranks up to 128 candidate tools and preloads the top 3 to 5, with a 750 ms timeout ([`jev-tool-prefetch.ts`](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/src/agent/jev-tool-prefetch.ts)). |
| **Write receipts** | A writing action returns `{ changed, verified, summary, checks }`. The loop keeps them and guards the final answer against claims no receipt backs ([`write-receipts.ts`](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/src/agent/write-receipts.ts)). |
| **MCP** | Every app serves Streamable HTTP MCP at `/mcp` with OAuth 2.1 and a bearer fallback, plus an `ask-agent` meta-tool that runs the full loop. It also serves `/.well-known/mcp.json` and `POST /mcp/tool/:name` for curl-only agents. Apps consume other MCP servers too ([MCP server](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/docs/content/mcp-protocol.mdx)). |
| **MCP Apps** | An action's `mcpApp` points at a **real app route**. A short-lived embed session renders it inside Claude or ChatGPT, and buttons inside can continue the host conversation ([MCP Apps](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/docs/content/mcp-apps.mdx)). |
| **WebMCP** | Authenticated pages register every eligible action with `document.modelContext` automatically. A `needsApproval` call that trips its predicate is refused with "ask the user in chat" ([WebMCP](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/docs/content/webmcp.mdx)). |
| **A2A** | JSON-RPC at `/_agent-native/a2a` and an agent card at `/.well-known/agent-card.json`, with `A2A_SECRET` JWTs. The Dispatch template is a workspace front door that hands work to specialist apps over A2A ([A2A](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/docs/content/a2a-protocol.mdx)). |
| **Frameworks and models** | Engines for Builder's gateway (the default), Anthropic, OpenAI, the Vercel AI SDK and ChatGPT subscriptions; Google Gemini is a supported key. The chat UI also sits on OpenAI Agents, Claude Agent SDK, AG-UI or a custom stream. `AgentKit` is a provider-neutral conversation protocol with a conformance suite. ACP coding harnesses can be registered. |

### Packages

The repo has 23 packages. The public ones that matter:

| Package | Version | Role |
| --- | --- | --- |
| `@agent-native/core` | 0.209.0 | The framework |
| `@agent-native/toolkit` | 0.209.0 | UI building blocks |
| `@agent-native/agentkit` | 0.209.0 | Conversation protocol, headless client, HTTP transport, conformance tests |
| `@agent-native/dispatch` | 0.40.23 | Workspace control plane: vault, integrations, cross-app delegation |
| `@agent-native/embedding` | 0.2.0 | Embed apps and agents in other apps |
| `@agent-native/pinpoint` | 0.1.84 | Visual feedback and annotation |
| `@agent-native/scheduling`, `migrate`, `otel`, `creative-context`, `code-agents-ui`, `skills`, `recap-cli` | various | Domain primitives and tools |

Private packages include a desktop app, a mobile app, Chrome extensions and a
local dev "frame".

### License and maturity

| Signal | Value |
| --- | --- |
| License | The README and every published package say **MIT**. But there is no root `LICENSE` file, GitHub detects no license, the root `package.json` says ISC, and template `package.json` files declare none. Fine for ideas; ask before copying template code. |
| Stars and forks | 7,314 stars, 664 forks, 123 open issues |
| Age and pace | Created 2026-03-12; 7,480 commits in seven months |
| Contributors | Steve Sewell has 4,048 commits; a Builder.io bot has 1,201 |
| Releases | 3,095 tags; packages publish several times a day with nightlies |
| Stability | Pre-1.0. The [stability page](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/packages/core/docs/content/versioning-and-stability.mdx) says any minor release may break. |

It is fast-moving, mostly one author with agents, and heavily tested (spec
files sit beside nearly every module). The engineering rules in its
[`AGENTS.md`](https://github.com/BuilderIO/agent-native/blob/56fec085fd5e74015761d2191b6be70d171b337c/AGENTS.md)
are worth reading on their own. For example: "a `catch`, default, or coercion
that returns a value callers cannot distinguish from success is a bug, not a
guard", and guards report pass, fail, or "could not run" as three separate
outcomes.

## 2. How it compares with our stack

| Agent-Native | What we have | Gap |
| --- | --- | --- |
| `defineAction` → every surface | Coder: `ExecutionSettings::defs/execute` ([`plugin_tools.rs`](../../crates/coder-new/src/plugin_tools.rs)), served to Claude Code over MCP ([`claude_tools.rs`](../../crates/coder-new/src/claude_tools.rs)). Web: axum routes, the docs MCP tools ([`docs_mcp.rs`](../../crates/openagents-web/src/docs_mcp.rs)), a hand-written WebMCP list ([`webmcp.js`](../../crates/openagents-web/static/webmcp.js)), and a CLI tree generated from help text ([`tree.rs`](../../crates/openagents-cli/src/tree.rs)) | **Large.** The same tools are listed in several places, by hand. `webmcp.js` repeats `docs_mcp.rs`'s four tools and schemas. |
| `readOnly`, `needsApproval(args)`, `authorize` | The CLI route's effect gate (money and secrets never proposed from chat; phone offers read-only only) in [`cli_route/gate.rs`](../../crates/coder/src/cli_route/gate.rs); Coder's approval desk ([`approval.rs`](../../crates/coder-new/src/approval.rs)) | Medium. Effect classes exist but live in the CLI router, not on each operation. MCP tool annotations are not emitted outside WebMCP. |
| `application_state` + `view-screen` + `navigate` | Rust Native carries "view identity" on every intent ([architecture](../coder/rust-native/architecture.md)); the web chat has no page-context channel that we found | Medium. The agent cannot see what page or selection the person is on. |
| Write receipts guard the final answer | Decision receipts ([`crates/receipts`](../../crates/receipts/src/lib.rs)); "end-to-end before handoff" as a working rule | Medium. Nothing stops a chat answer claiming a write no tool confirmed. |
| Compact catalog + `tool-search` + Jev prefetch | CLI routing descends the command tree with typed Jev choices ([`descend.rs`](../../crates/coder/src/cli_route/descend.rs)); Coder sends its whole tool list | Small to medium. We have the typed selector; we do not use it to trim tool lists. |
| MCP Apps embed real routes | Keyed `/mcp` with OAuth, tools that only read docs plus `whoami`; Apps SDK UI design language on the web ([plan](../web/apps-sdk-ui-adoption-plan.md)) | Medium. Our pages already look like ChatGPT apps but cannot render inside one. |
| Native chat widgets from `outputSchema` + `chatUI` | OpenUI Lang answers rendered server-side ([`answer_ui.rs`](../../crates/openagents-web/src/answer_ui.rs), [`openui-lang`](../../crates/openui-lang/src/lib.rs)) | Small. The model writes the component; a tool result cannot declare its own renderer. |
| `/.well-known/mcp.json` + `POST /mcp/tool/:name` | `/.well-known/mcp.json` is our server card ([`agent_ready.rs`](../../crates/openagents-web/src/agent_ready.rs)) | Small. No per-tool HTTP call for curl-only agents. |
| Generative UI (agent writes Alpine/Tailwind HTML) | OpenUI Lang, a declarative subset; strict CSP (`script-src 'self'`) | We chose differently, on purpose. See Skip. |
| shadcn/ui primitives | The shadcn converter plan maps shadcn into `openagents-ui` and Rust Native ([UI system plan](../ui/2026-10-15-shadcn-converter-and-ui-system-plan.md)) | None to take. Their Toolkit is a consumer of shadcn, as our plan is. |

Effect Native's own dossier is no longer in this repo. Rust Native carries
its idea forward: semantic components plus named typed intents, resolved
against the current view. **A Rust Native intent and an Agent-Native action
are the same shape.** That is why recommendation 1 below fits us.

## 3. Recommendations

Sizes: S is a day or two, M is about a week, L is several weeks.

### 3.1 One operation definition, every surface (take, L, staged)

**What.** Define a public operation once in Rust: name, description, input
and output JSON Schema (from `schemars`), effect class, confirm rule, and a
handler. Derive from it the axum route, the keyed `/mcp` tool, the WebMCP
tool, the OpenAPI fragment, the CLI tree entry, and the chat agent's tool.

**Where.**
- A new data-only crate, `crates/oa-actions`, for the definition type and
  the effect classes. It is the same idea as `route-contract`: no I/O. The
  existing `capability` crate is about NIP-CAP host manifests, so it is not
  the right home.
- Adapters in `openagents-web` (`docs_mcp.rs`, `agent_ready.rs`, a generated
  WebMCP manifest) and in `coder-new` (`plugin_tools.rs`, `claude_tools.rs`).

**Stages.**
1. **S.** Serve the WebMCP tool list from `docs_mcp.rs`'s definitions instead
   of repeating them in `webmcp.js`. Add a test that fails when they differ.
   This fixes today's drift.
2. **M.** Move the docs, models, payment-methods and `whoami` tools onto
   `oa-actions`. The keyed `/mcp`, WebMCP and `/openapi.json` all read it.
3. **L.** Move signed-in web operations (chat threads, schedules, account
   memory, tasks) onto it, so the keyed `/mcp` and WebMCP offer what the
   site offers. Adopt Agent-Native's "four areas" as a checklist line for web
   features: page, operation, agent guidance, page context.

**Skip.** Their default of exposing every action everywhere. Ours stays
opt-in per surface, and the effect gate decides as it does now.

### 3.2 Effect metadata on each operation (take, S to M)

**What.** Put on the operation itself what the CLI route gate knows today:
read-only, writes, money, secrets, grants. Add a confirm predicate evaluated
on the **actual arguments** (Agent-Native's `needsApproval(args)`), plus
`retry_safe` and `ends_turn`. Emit MCP annotations (`readOnlyHint`,
`destructiveHint`) everywhere we serve tools, not only in `webmcp.js`.

**Where.** `crates/oa-actions`; read by `crates/coder/src/cli_route/gate.rs`
and `crates/coder-new/src/claude_tools.rs`.

**Take their WebMCP rule.** A call from a surface that cannot ask the person
(WebMCP, external MCP) and that trips the confirm predicate is refused with a
plain "confirm this in the OpenAgents chat" error. It never runs silently.
Money and secrets stay off chat surfaces entirely, as decided.

### 3.3 Page context for the agent (take, M)

**What.** The agent should know where the person is and what they selected.
Take three of their keys: `navigation` (view and ids), `selection`, and the
URL. Add two operations: `view_screen` (hydrate those into records) and
`navigate` (one-shot move, which the page consumes and clears).

**Where.**
- Web: the composer posts the current route and selection with each turn
  (`crates/openagents-web/src/composer.rs` and `chat_store.rs`).
- Rust Native apps: use the view identity that intents already carry
  (`crates/rust-native`, `crates/coder-ui`).
- Keep it server-held per session, not a new database table.

**Why.** "Summarize this" or "fix this run" from a run page should not make
the agent ask which run.

### 3.4 Write receipts checked against the final answer (take, M)

**What.** Every writing operation returns a small receipt: changed, verified
(true, false, or unverified), a one-line summary, and up to eight checks.
Before a chat answer says "done", "sent", or "merged", the turn checks that a
receipt backs it. If none does, the turn retries or says plainly what did not
happen.

**Where.** The receipt type goes in `crates/oa-actions`. Enforce it in the
Coder turn loop (`crates/coder-new`) and in web chat (`openagents-chat`).

**Why.** It is the mechanical form of our "end-to-end before handoff" rule.
It also matches their best rule: a failure must never look like a clean
success.

**Not keyword matching.** Deciding whether an answer claims a write is a
typed predicate through the Decisions API (Clef first), not a regex over
"done".

### 3.5 Ranked tool loading through the Decisions API (take, M)

**What.** Send a compact tool list, plus a `tool_search` tool for the rest.
Before a turn, ask one typed **choice** question over the candidate tools,
and preload the top few. Agent-Native does exactly this with Jev, under a
750 ms budget. We should call `POST /v1/decisions` (Clef first) under our
"Clef first, Jev backup" rule, not Jev directly. The CLI router's beam over
typed choices ([`descend.rs`](../../crates/coder/src/cli_route/descend.rs))
is the pattern to reuse.

**Where.** `crates/coder-new/src/claude_tools.rs` (the `coder` MCP server
Claude Code sees) and the keyed `/mcp` in `docs_mcp.rs` once 3.1 makes its
list long.

**Skip.** Ranking by keyword or substring overlap. Our routing rule forbids
it, and they do not do it either.

### 3.6 MCP Apps that embed our real pages (take, M to L, after 3.1)

**What.** Operations that need a person (review a run, approve a plan, open a
thread) declare a deep link. Hosts that support the
`io.modelcontextprotocol/ui` extension render that **real openagents.com
page** inline, through a short-lived embed session. Others show the link.
Their rule is the right one: no separate hand-written widget HTML, and the
link stays the fallback.

**Where.** `crates/openagents-web/src/docs_mcp.rs` (tool `_meta`,
`resources/read`), plus an embed-session route next to `oauth.rs`. Pages
already follow the Apps SDK UI language, so they will look at home in
ChatGPT.

**Check first.** Framing needs `frame-ancestors` for named host origins only,
and the embed session must be single-use and short-lived. The CSP rule
`script-src 'self'` stays.

### 3.7 Tool results that declare their renderer (take, S to M)

**What.** A tool's output schema names an OpenUI Lang component (table,
chart, install command). The chat renders the result through `answer_ui.rs`
without asking the model to re-type it as OpenUI Lang. This is their
`outputSchema` plus `chatUI` pairing.

**Where.** `crates/openui-lang` (a `from_result` path) and
`crates/openagents-web/src/answer_ui.rs`.

### 3.8 Per-tool HTTP for curl-only agents (take, S)

**What.** `POST /mcp/tool/{name}` runs one tool with a JSON body through the
same dispatcher as `/mcp`. List it in the API catalog. Agents that can fetch
but not speak MCP then get the same tools.

**Where.** `crates/openagents-web/src/agent_ready.rs` and `docs_mcp.rs`.

### 3.9 Smaller habits worth copying (take, S)

| Habit | Where |
| --- | --- |
| Guards report pass, fail, or "could not run". "Could not run" never prints as passed. | Our check scripts and `copy_guard` |
| A diff-scoped guard against silent coercion (`unwrap_or_default()` on a fallible read in a new handler) | A lint step for `openagents-web` |
| `retry_safe` on writes, so the loop may retry with corrected args and never retries appends, sends or charges | `crates/oa-actions` (3.2) |

## 4. What to skip, and why

| Item | Why we skip it |
| --- | --- |
| Adopting the framework, Toolkit or AgentKit | TypeScript, React, Nitro, Drizzle and Postgres; we are Rust, Maud, HTMX and Rust Native. We take ideas, not code. |
| Generative UI (the agent writes Alpine/Tailwind HTML into an iframe) | It breaks our CSP and copy guard, and it shows people model-written UI. OpenUI Lang already gives a typed, safe subset. |
| Extensions (SQL-stored Alpine mini-apps) | Same reasons, and no owner ask. |
| Dispatch routing by agent instructions | Its "which app handles this" choice lives in prompt text. We route with a typed selector. Our vault (`oa-vault`) and delegation already cover the rest. |
| A2A JSON-RPC with a shared `A2A_SECRET` | Our agent-to-agent path is Nostr (NIP-CAP, NIP-DEC) and Coder delegation. A second protocol adds surface without users. We can revisit an agent card for discovery only. |
| Default engines (Builder's gateway, GPT, Claude Sonnet) | Our rule is Google-first: Vertex Gemini for model calls, with OpenRouter and Vercel only as fallbacks. |
| Stripe paths in their templates and Dispatch | Payments are Bitcoin-only: Lightning and Bitcoin-based stablecoins. |
| The agent editing the app's own source in production | Fine for their local dev frame. Not for openagents.com. |
| Templates (Mail, Calendar, CRM, Slides) | Not our product, and their license is unclear. |
| Their commit rules (for example "no Co-Authored-By") | Repo-local to them. |

## 5. Order of work

1. 3.1 stage 1 (S): generate `webmcp.js`'s tool list. It fixes real drift
   today.
2. 3.8 (S) and 3.2 (S to M): per-tool HTTP and effect metadata.
3. 3.1 stage 2 (M), then 3.3 (M) and 3.4 (M).
4. 3.5 (M) once tool lists are long; 3.6 (M to L) and 3.7 after that.

Every step keeps our voice rules ("we" for OpenAgents, "I" for Coder and the
other agents), and no step adds keyword routing.
