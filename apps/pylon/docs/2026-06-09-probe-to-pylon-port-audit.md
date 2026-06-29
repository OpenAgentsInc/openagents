# Probe To Pylon Port Audit

Date: 2026-06-09

## Status

Probe runtime code has been ported into Pylon as an internal workspace package:

- Package: `@openagentsinc/pylon-runtime`
- Path: `packages/runtime`
- Public entrypoint: exported from the Pylon workspace package
- CLI route: `pylon runtime ...`
- Direct namespace aliases: `pylon apple-fm ...`, `pylon backend ...`,
  `pylon chat ...`, `pylon auth ...`, and `pylon openagents ...`

The old Probe repository is now source history for this runtime surface. New
runtime work for Apple FM, Gemini, provider materialization, Blueprint tool
projection, retained Markdown rendering, GEPA candidate execution, token
telemetry, and runner identity should land in Pylon.

## Ported Surfaces

- Apple Foundation Models backend client, readiness contract, streaming bridge
  fixtures, callback tool session, Blueprint tool projection, acceptance cases,
  receipts, and Program Run evidence.
- Gemini backend auth, protocol lowering, streaming parser, tool schema
  sanitization, direct API client, OpenAgents product surface-brokered materialization, receipts, and
  CLI commands.
- Provider-neutral LLM events, messages, request contracts, tools, tool runtime,
  and usage normalization.
- Workspace file mutation, bounded read/search/edit/write tools, OpenTUI
  renderer helpers, Markdown rendering helpers, syntax styling, and streamed
  assistant text/code renderables.
- Blueprint assignment contracts, registry client, signature lookup, tool menu
  planner, contribution release gates, and Action Submission boundary.
- GEPA and Terminal-Bench candidate execution, retained fixtures, closeout
  writer, live canary receipt, and public-safe import artifacts.
- Fleet backend capability reporting, token usage telemetry, runner identity,
  OpenAgents product surface account/grant clients, and provider-account projections.

## Test Gates

The port keeps Probe's coverage live inside Pylon:

- `bun test`
- `bun run --cwd packages/runtime test`
- CLI dispatch smoke:
  `bun src/index.ts runtime apple-fm status --base-url http://127.0.0.1:9`

The full local test suite currently passes with 195 tests and 3 live-provider
tests skipped. The skipped tests are intentionally gated on live Gemini
credentials or live provider availability.

## Reconfiguration Notes

- Pylon remains the public launch package: `@openagentsinc/pylon`.
- The runtime package is public-ready as `@openagentsinc/pylon-runtime`, but
  end users should start from the `pylon` binary unless they are embedding the
  runtime as a library.
- Probe-specific public command examples should be rewritten to Pylon commands.
  For example, `probe backend gemini smoke` becomes
  `pylon backend gemini smoke`.
- Historical docs copied under `docs/probe-port` retain Probe naming where it
  describes old source history. New launch docs should refer to Pylon.
- Canonical GEPA benchmark canary artifacts now live under `docs/benchmarks`
  so Pylon can retain the launch evidence directly.

## Remaining Follow-Up

- Rename user-facing strings in copied tests/docs from "Probe" to "Pylon"
  where they describe the current product rather than historical source.
- Decide whether `@openagentsinc/pylon-runtime` should be published separately
  or kept as packaged source inside `@openagentsinc/pylon`.
- Keep Qwen work postponed; GEPA remains the priority backend/evaluation loop.

## 2026-06-13 Pylon Autopilot Follow-Up

- Issues #4868-#4870 added per-session Codex/Claude account selection,
  multi-session local proof orchestration, and loopback control-session verbs.
- Issue #4871 adds the account usage diagnostics path: Pylon now parses Codex
  rate-limit header/event shapes (`x-codex-primary-*`,
  `x-codex-secondary-*`, multi-bucket `x-{limit-id}-*`, and
  `codex.rate_limits`) into public-safe snapshots, stores observed
  provider/local-session truth by hashed account ref, exposes
  `pylon accounts list --json` and `pylon accounts usage --json`, and includes
  an optional usage summary in `pylon dev doctor --json` / `pylon context
  --json`.
- The local Codex source check found that the current TypeScript SDK event
  surface exposes token usage on `turn.completed`; richer rate-limit state is
  represented in Codex core/app-server `TokenCount` /
  `account/rateLimits/read` paths. Pylon therefore captures provider
  snapshots whenever those structured payloads appear, and labels missing
  provider truth honestly rather than deriving it from token totals.
- Issue #4872 completed the worker-side companion leg in `openagents.com`:
  token-usage rows can now carry provider-account refs, admin/owner usage
  aggregates are exposed without credential material, and per-account budget
  signals remain advisory events rather than enforcement authority.
- The local dogfood run exposed a targeting papercut: unnamed default homes
  were listed but could not be selected by a human-friendly ref. The CLI now
  accepts provider/default selectors such as `--account codex`, `--account
  chatgpt`, `--provider codex`, and `--provider claude` while preserving
  registered account refs.
