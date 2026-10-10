# OpenUI (Thesys): what OpenAgents should adapt

OpenUI is a TypeScript generative-UI framework from Thesys, built around
**OpenUI Lang**: a line-oriented language where the model writes one named
statement per line (`header = CardHeader("Revenue", "Last 6 months")`) using
components from a typed catalog. Its best ideas are the ones issues #11113,
#11112 and #11114 already point at: cut a streamed answer at the last complete
statement, keep the last good render, validate against a catalog, and repair
locally. We should use the **syntax** (a declarative subset) as the wire format
for interactive answers, write our own Rust parser, and skip their runtime,
renderers, gateway and client-side tool calls.

Related issues: #11112 (V1 streamed Markdown cut rule), #11113 (interactive
answers), #11114 (grounded values).

## Scope and evidence

| Field | Snapshot |
| --- | --- |
| Repo | https://github.com/thesysdev/openui, cloned read-only at `projects/repos/openui` (commit `adc0234`) |
| License | MIT, "Copyright (c) 2011-2024 Thesys Inc." Ideas and the spec can be implemented freely; we copy no code. |
| Read | `packages/lang-core/src/parser/*` (lexer, `statements.ts`, `parser.ts`, `validation.ts`, `materialize.ts`), `packages/react-lang/src/Renderer.tsx`, `responseBundle.ts`, spec v0.1 and v0.5, docs on architecture, reliability, incremental editing, gateway, `benchmarks/`, blog posts `generative-ui-benchmark.mdx` and `how-chatgpt-intelligent-ui-works.mdx`, `packages/a2ui` |
| Compared with | `projects/repos/json-render` (Vercel, JSON spec / JSONL patch stream, many renderers) |
| Our side | `crates/openagents-web/src/markdown.rs` (pulldown-cmark, raw HTML as text, no streaming handling today), `crates/openagents-ui`, `crates/inference` |

## What is in the repo

| Piece | What it is | Notes |
| --- | --- | --- |
| **OpenUI Lang** (spec v0.1, v0.5) | `name = Component(arg, ...)`, one statement per line; `root = ...` is the entry; strings, numbers, arrays, objects, refs. v0.5 adds `$state` variables, `@Builtins` (`@Count`, `@Filter`, `@Each`...), ternaries, `Action([...])`, and `Query`/`Mutation` (the UI calls named tools at runtime). | **Forward references** allowed: a parent can name a child before it is defined; the renderer shows a placeholder until it arrives. **Positional args** map to props by schema key order, so key order is the API. |
| **Parser** (`lang-core`) | Lexer, statement splitter, expression parser, materializer, JSON-Schema validator. ~4k lines TS. | Framework-free. No Rust implementation exists. |
| **Streaming/partial handling** | `createStreamParser`: keeps a watermark of completed statements (a newline at bracket depth 0, not inside a string or ternary), caches them, and re-parses only the pending tail. `autoClose` closes an open string and open brackets on the tail so it parses. An existing statement is **not** replaced while its new version still needs auto-closing. Enum checks are deferred while streaming (a partial literal may still become valid); type checks stay on. Required-prop checks run only on complete input. Invalid array items are pruned; invalid optional props fall back to the schema default or are dropped. | This is the #11112 rule, done for their language. |
| **Renderer** (`react-lang`) | React renderer plus Vue/Svelte/Angular bindings; React Native via the same React renderer with native components (Expo example). Per-element error boundary keeps `lastValidChildren`, so one failing component shows its last good render instead of breaking the answer. | All JS/React. Nothing we can run under our CSP or in Rust. |
| **Inline mode** | Model writes Markdown prose plus a fenced ```` ```openui-lang ```` block; parser extracts the fence. | Clean way to embed UI in a Markdown answer. |
| **Incremental editing** | A follow-up emits only changed/new statements; merge by name (same name replaces, new name adds, missing kept, unreachable from `root` is dropped). Claimed up to 85% fewer tokens on edits. | Directly answers #11113's "follow-ups can edit the existing interface". |
| **Prompt generation** | `library.prompt()` and `toJSONSchema()` generate the system prompt and schema from the component definitions (Zod). | Catalog is the single source for prompt, validator, renderer. |
| **Component libraries** | Charts, forms, tables, cards, layouts in `react-ui` (React + their CSS). | Not reusable for us. |
| **Gateway / Autofix** | Hosted OpenAI-compatible endpoints (`api.thesys.dev`) that validate and repair OpenUI Lang while streaming, plus failover and monitoring. Paid. Repair = parser error sent to a small model that edits only the broken lines and re-validates. | Closed service. The method is the useful part. |
| **A2UI profile** (`packages/a2ui`) | Google A2UI v1.0 envelopes with component lists carried as OpenUI Lang statements, merged by statement ID; `id = null` deletes. | Shows the statement format works as a patch protocol. |
| **Benchmarks** | (a) token benchmark in `benchmarks/`; (b) reliability benchmark (blog, raw data in a separate `generative-ui-bench` repo). See below. | First-party. |

### The benchmark claims, checked

**Token claim ("up to 67% fewer tokens than JSON").** Seven prompts, **one**
generation each (gpt-5.2, temperature 0). The model only ever wrote OpenUI
Lang; the JSON, JSONL and YAML files are **converted** from its parse tree, so
the comparison is size of encoding, not what a model writes in each format. The
C1 JSON is written **pretty-printed with 2-space indent**
(`thesys-c1-converter.ts`). "67%" is the single best cell (contact form vs
json-render patches); the total is 47-53%. Latency is tokens divided by a fixed
60 tokens/s, not measured. Our own check by characters: OpenUI Lang is 0.61-0.79x
the size of the **minified** nested JSON (vs 0.22-0.30x of the pretty-printed file, where indentation inflates characters more than tokens), so
against compact JSON the real saving is roughly 20-40%, larger only against
flat ID-registry patch streams like json-render's. No validity was measured here.

**Reliability claim (blog, 2026-08-17).** 46 prompts x 6 models x 4 runs = 1,104
runs per format, a 70-component catalog derived from their library, each
format's own prompt generator and parser, same two examples. Structural
validity: OpenUI 96.5%, A2UI 95.7%, json-render 80.2%. Blank screens: 1, 35, 4.
Output tokens per screen: 1,362 vs 2,823 (A2UI) vs 3,258 (json-render). Their
own reading, which we accept: **validity is roughly format-neutral** between a
line DSL and nested JSON; json-render loses because its flat ID registry makes
models drop parent-child links (929 of 959 errors were dangling/orphaned IDs).
The real differences are **cost** (about 2x fewer output tokens) and **failure
shape** (a bad line costs one element, not the whole screen). First-party,
catalog chosen by them, but the method and raw outputs are published. Their
production numbers: about 7% of first-pass generations fail validation (44% no
valid root, 36% dangling/orphaned refs, 16% enum/type, 4% truncation); line-level
repair brings visible failures under 1%.

## Verdicts

| Piece | Verdict | Why | Maps to |
| --- | --- | --- | --- |
| Cut-at-last-complete-statement streaming parser, keep-last-good | **Adapt now** | Exactly the #11112 rule. Port the *rules* to Markdown blocks in Rust: completed-prefix watermark, don't replace a finished block with a version that needs auto-closing, defer checks that more text could fix. | #11112 |
| Per-element "last valid render" | **Adapt** | Their error boundary per element is the "a failing piece removes one element, not the answer" rule. For us: per block / per component on the server, plus the client keeps the last swapped HTML if a patch fails. | #11112, #11113 |
| OpenUI Lang syntax | **Adopt a declarative subset as our wire format** (spec v0.1 + `$state` + a few pure builtins) | Has every property #11113 asked for: one statement per line, auto-closable, small, forward refs for top-down streaming, merge-by-name edits, line-local repair. Benchmarked at JSON-level validity for half the tokens. Taking their syntax instead of inventing one costs nothing and gains interop (A2UI profile, any model prompted with an OpenUI catalog). Models do **not** know it from pretraining (it is new); the catalog prompt teaches it either way, so "models already know it" is not a reason. | #11113 |
| Positional arguments by schema key order | **Adapt: allow named args too** | Key order as the API is brittle when our catalog evolves. Accept positional (spec) and also `name: value`, and generate prompts with positions fixed per catalog version. Small deviation; record it as our profile of the spec. | #11113 |
| Embedding: Markdown prose + fenced ```` ```openui-lang ```` block (inline mode) | **Adopt** | Better than JSX-like tags inside Markdown: pulldown-cmark already sees a fenced block with a language tag, so `markdown.rs` can hand that block to the UI compiler and everything else stays as today. Prose streams with the #11112 rule; the UI block streams with the statement rule. | #11112, #11113 |
| Rust compiler (lexer, statement splitter, validator against our catalog, diagnostics) | **Write our own** | No Rust implementation exists and their TS can't run in our server or CSP. Port the behaviors, not the code: prune invalid array items, default-or-drop invalid optional props, required checks only on complete input, enum checks deferred while streaming, every fix recorded as a diagnostic. | #11113 |
| Catalog-generated prompt + schema | **Adapt** | One Rust catalog (component names, typed props, descriptions, groups) generates the system prompt, the validator and each renderer's allowed set. Matches the Effect Native one-component-set plan. | #11113 |
| Incremental editing (merge by name, drop unreachable) | **Adopt the rule** | Gives "follow-ups edit the interface" and "stream patches, not the whole program" for free: each new statement is a patch keyed by name. | #11113 |
| `Query`/`Mutation`/`@Run`: UI calls tools directly from the client | **Skip** | Client-side tool calls bypass our server authority, metering and CSP. Replace with #11114: tool results get IDs on the server, the model references fields (`r3.price`), the server fills them in. Interactive actions go through our small host API (`new_turn`, `copy`, `open_url`, `open_entity`). | #11114, #11113 |
| `@Each`, `@Filter`, `@Count` etc. | **Adapt later, as pure server-evaluated builtins** | Deterministic, no sandbox needed, useful over referenced tool results (count rows of `r3`). Phase 2. | #11113, #11114 |
| React/Vue/Svelte/Angular/RN renderers, `react-ui` components | **Skip** | Wrong stack. Our renderers are Maud/HTMX (web), mobile, desktop, terminal fallback, drawing `openagents-ui` components on tokens. | #11113 |
| Gateway, Autofix, Cloud observability | **Skip the service; adapt the repair method** | Paid hosted dependency. The method is worth copying in `crates/inference` later: on a validation failure, send the exact diagnostics plus only the broken lines to a small model, re-validate, retry once. Same channel as "feed diagnostics back to the model". | #11113 |
| A2UI profile | **Skip for now** | Only matters if we need to interoperate with A2UI agents. Keep the statement format so the option stays open. | - |
| Token benchmark | **Don't cite as-is** | One run per prompt, converted not generated, pretty-printed JSON. | #11113 |
| Reliability benchmark method | **Adapt** | Run a small version on our models and catalog before locking the format: N prompts x our routed models x 3-4 runs, scoring structural validity, blank screens, output tokens, and time to first useful render. Compare OpenUI Lang subset vs minified nested JSON. | #11113 |

### json-render, for comparison

json-render (Vercel) has the same catalog idea (Zod, prompt from catalog) and
many renderers including React Native, Ink (terminal) and email. Its wire format
is a flat element registry streamed as JSON Patch lines, which the OpenUI
benchmark shows models break by dropping ID links as screens grow. Keep it as a
reference for **renderer breadth** (its terminal and email renderers are good
models for our terminal fallback); don't use its wire format.

## For V1 today (small, low risk)

Only #11112 touches V1. Take these rules from `createStreamParser` and apply them
to Markdown blocks in `markdown.rs` (and the mobile/desktop/terminal renderers):

1. **Completed-prefix watermark.** Track the end of the last complete top-level
   block (blank line outside a fence, closing fence, end of a list or table).
   Render that prefix once and cache it; re-render only the tail on each chunk.
   Cheaper, and the finished part can never change shape.
2. **Close what can be closed, hold back what can't.** Their `autoClose` closes
   strings and brackets. Markdown equivalents: an open code fence gets a
   synthetic close (show code so far); an open `**`, `` ` ``, `[text](` or a
   half table row is held back until it completes; plain partial text shows.
3. **Never replace a good block with one that needs repair.** Their rule: an
   existing statement is replaced only when its new version parses without
   auto-closing. For us: if the tail render fails or would flash raw syntax,
   keep the previous tail HTML.
4. **Defer checks that more text could satisfy** (their enum rule): don't decide
   a line is a table, heading or list item until the line ends.
5. Property test, as #11112 already says: for random cut points of golden
   answers, every intermediate render is a prefix-safe render and the final
   render equals the one-shot render.

No new format, dependency or client code is needed for this.

## After 1.0

- #11113 phase 1: Rust OpenUI Lang subset parser + catalog + validator +
  diagnostics; Markdown-with-fence embedding; web renderer (Maud fragments over
  SSE, one swap per completed statement); Markdown fallback stored with each
  answer. Run the small format benchmark first.
- #11113 phase 2: `$state`, pure builtins, host actions, merge-by-name edits,
  diagnostics fed back to the model, line-local repair pass in the gateway.
- #11114: tool-result IDs and field references (`r3.price`) as plain Markdown
  first, then as typed arguments inside OpenUI Lang statements, resolved and
  checked on the server. This replaces their `Query` and stays server-side.
  The plain-Markdown half is in `crates/inference/src/grounded.rs`: a per-turn
  result ledger (`r1`, `r2`, ...), `{r3.path}` and `{cite:r3.results[0]}`
  references filled on the server with dropped-reference diagnostics fed back
  to the model, rate-card rows by model id, and the golden check that every
  number and URL traces to a result. It is wired in: the gateway's hosted
  web search records each search, tells the model the ids, and fills the
  answer's references as it streams (an open `{...}` is held until it
  closes). The chat worker's product replies reference the rate card and
  their passages the same way. A golden that names `grounded` sources fails
  on any number or URL that is not from them.
