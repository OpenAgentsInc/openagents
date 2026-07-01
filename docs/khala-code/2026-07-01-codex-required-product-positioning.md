# Khala Code Codex-Required Product Positioning

Status: product copy and implementation guardrail for issue
[`#7794`](https://github.com/OpenAgentsInc/openagents/issues/7794). Parent epic:
[`#7780`](https://github.com/OpenAgentsInc/openagents/issues/7780). Source audit:
[`2026-07-01-codex-harness-wrapper-port-audit.md`](./2026-07-01-codex-harness-wrapper-port-audit.md).

## Product Statement

Khala Code is a desktop/web wrapper around the user's local Codex install. The
default coding harness requires:

```sh
npm install -g @openai/codex
codex login
```

The default chat, thread, slash-command, approval, MCP, plugin, skill, settings,
and headless JSONL paths should flow through `codex app-server --stdio`. Khala
Code must not promise a separate TypeScript implementation of Codex Core for the
normal product path.

## What Khala Adds

Khala Code adds a desktop shell around Codex:

- sidebar and web-native thread navigation;
- Unified Inbox for approvals, MCP/auth blockers, and worker closeouts;
- Settings and ecosystem panels backed by Codex app-server processors;
- Pylon/Fleet controls for isolated worker Codex accounts;
- Gym/proof panes and desktop smoke-test harnesses;
- headless JSONL output that preserves Codex thread and turn correlation ids.

These are wrapper and orchestration advantages. They are not replacements for
Codex sandboxing, approvals, slash commands, tools, app/plugin/skill policy,
thread storage, account state, or model selection.

## Install And Missing-State Copy

When Codex is missing, the user-facing next step is:

```sh
npm install -g @openai/codex
```

When Codex auth is missing, the next step is:

```sh
codex login
```

Khala Code should say that `codex login` is a primary user action. The app must
not run that command automatically against the default `~/.codex` home, because
starting a login flow can clear an active auth file. A custom primary home may
be selected with `CODEX_HOME`; a custom binary may be selected with
`KHALA_CODE_CODEX_BINARY` or `KHALA_CODE_CODEX_COMMAND`.

## Session Boundaries

There are three distinct session categories:

| Category | Home | Purpose | Mutation rule |
| --- | --- | --- | --- |
| Primary user Codex session | `CODEX_HOME` or default `~/.codex` | Default Khala Code chat and thread surface | User signs in intentionally with `codex login`; Khala Code starts app-server only after the gate is ready |
| Worker Codex accounts | `<pylon home>/accounts/codex/<ref>` | Swarm/Pylon delegated work | `khala fleet connect` / `codex_spawn` use isolated homes and never reuse the primary home |
| Legacy Khala-native mode | explicit feature flag | Fallback/prototype testing | Experimental only; not product-center and not a silent fallback when Codex is unavailable |

Settings and Fleet copy should keep those categories separate. "Connect account"
in Fleet means "connect an isolated worker Codex account", not "sign in the
primary user session".

## Legacy Runtime Position

The legacy hosted Khala/OpenRouter path may remain available for development
and fallback work behind:

```sh
KHALA_CODE_DESKTOP_RUNTIME=khala_native_runtime
KHALA_CODE_DESKTOP_LEGACY_KHALA_NATIVE_RUNTIME=1
```

That mode is explicitly experimental. It may register Codex-equivalent
filesystem, shell, patch, search, browser, and planning helpers for tests, but
future parity work should use Codex app-server methods first and record upstream
gaps instead of expanding the TypeScript harness.

## Future-Issue Acceptance Language

Future Khala Code parity issues should include these checks:

- Default behavior depends on a ready local Codex install and `codex app-server`.
- Missing Codex install/auth produces clear install or sign-in instructions.
- Fleet worker login uses isolated Pylon Codex homes and does not mutate the
  primary user Codex home.
- Settings expose Codex app-server state instead of shadow config owned by
  Khala Code.
- New slash-command, tool, approval, MCP, plugin, skill, model, thread, or
  sandbox behavior is delegated to Codex app-server whenever an upstream method
  exists.
- A TypeScript implementation is allowed only for Khala-specific wrapper
  affordances, smoke harnesses, projections, or explicitly documented legacy
  fallback.
