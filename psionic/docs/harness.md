# A harness for Psionic models: notes from DeepSeek Harness

Read-only survey, 2026-10-29, of
[deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/f3803c96974ad4a94195fb97e5c637e9c2a3afe3)
at `f3803c96974ad4a94195fb97e5c637e9c2a3afe3` (MIT, developer preview, built on
the [Cordis](https://github.com/cordiverse/cordis) plugin framework). Its
source was read through the GitHub API; nothing was installed or run. A
pre-reset survey of an earlier revision is at
`git show 0b401561a0^:docs/coder-new/deepseek-harness-lessons.md`. Part XXI of
the [book plan](../book/PLAN.md) takes its scope from here.

## Why study this harness

DeepSeek publishes the whole stack: its models, its serving engine and article
([DeepSeek natively](targets/deepseek.md)), and this harness. The harness is
built for that stack, and the clearest place this shows is the model-visible
prefix. DeepSeek's serving reports a 96% input cache hit rate on 77K-token
average inputs, and the harness is designed to keep those hits: every harness
addition to a request (identity headers, plugin inventory, session metadata)
travels outside `messages`, system prompts and tool schemas, so it "does not
add model-input tokens or alter the model-visible prefix"
(`docs/deepseek-llm-api-wire-extensions.md`). Psionic owns both sides, so it
can design the harness's prompt assembly and the server's prefix cache
(chapter 174) together and measure the result (chapter 200).

## What it does, and what Psionic takes

| DeepSeek Harness | Where | Psionic chapter |
| --- | --- | --- |
| A turn is zero or more steps; a step is one model request plus its tools; exact durable `turn/*`, `step/*` events | `docs/architecture.md` "Turn flow" | 198 |
| "Model-visible means logged": every request reconstructable from an append-only session log; history derived from the log; versioned formats with readers for old ones | `core/session`, `docs/architecture.md` "Session log" | 199 |
| Extensions outside the model input so the prefix stays byte-stable | `docs/deepseek-llm-api-wire-extensions.md` | 200 |
| A scoped tool registry and guarded pipeline (`tools/pre-execute`, `execute`, `post-execute`); filesystem and process providers sharing one execution world | `core/tools`, "Capability seams" | 201 |
| Sandbox backends and policy; timeout policy; a reminder against repeating the same tool call | `packages/sandbox/`, `packages/guard/` | 202 |
| Compaction bracketed by `compaction/start` and `compaction/end` in the log, with a stable summary marker; tool-result pruning; overflow recovery | `packages/compaction/` | 204 |
| Programmatic tool calling: a model-written program run once against host-provided bindings, with typed failure kinds | `packages/ptc-runtime/` | 205 |
| Forks at a turn boundary from a seeded log; subagents; background jobs; goals | `packages/subagent/`, `jobs/`, `goal/` | 206 |
| Everything is a plugin: profiles of ordered bundles, service definition/provider/consumer, registrations owned and unwound by their plugin | `docs/architecture.md`, `packages/boot/` | 207 |
| MCP client and resources; ACP for editors | `packages/mcp/`, `packages/acp/` | 208 |
| Benchmarks for session open, conversation fold, long browser sessions, stream reconnect and agent continuation | `benchmarks/` | 209 |

## What Psionic does differently

- **The loop stays simple.** The harness exposes a great many extension
  points. Psionic starts from one loop (chapter 198) and keeps each addition
  only with a measured gain on tasks it was not built from, the rule that
  already governs Coder's agent.
- **Coax, not Node.** DeepSeek Harness is TypeScript on Node, and ordinary host
  plugins run with the host process's authority; the earlier survey noted that
  Cordis's scoped contexts are lifecycle composition, not containment.
  Psionic's harness logic is Coax; third-party plugins run in bounded Wasm with
  admitted capabilities, distributed as signed Nostr events (chapter 207).
- **Consent before sharing.** `dsh_session_log` is enabled by default and sends
  a contiguous suffix of the canonical session log with each model request to
  DeepSeek's API. Psionic sends no session content anywhere without the
  person's explicit, per-session, revocable opt-in (chapter 211).
- **Typed protocols from `proto/`.** Tool schemas, the session log's event
  types and any wire extensions are versioned contracts in `proto/`, not ad hoc
  JSON (chapters 199 and 201).
- **Bitcoin-only money.** Anything that meters or prices agent work uses the
  Spark wallet outside Psionic.

## Licensing

MIT. Porting designs needs no notice; copying code or text would carry the MIT
notice. Its third-party dependencies are listed in its
`THIRD_PARTY_NOTICES.md`; Psionic adopts none of them.
