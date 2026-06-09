# Probe To Pylon Port Audit

Date: 2026-06-09

## Status

Probe runtime code has been ported into Pylon as an internal workspace package:

- Package: `@openagentsinc/pylon-runtime`
- Path: `packages/runtime`
- Public entrypoint: exported from the Pylon workspace package
- CLI route: `pylon runtime ...`
- Direct namespace aliases: `pylon apple-fm ...`, `pylon backend ...`,
  `pylon chat ...`, `pylon auth ...`, and `pylon omega ...`

The old Probe repository is now source history for this runtime surface. New
runtime work for Apple FM, Gemini, provider materialization, Blueprint tool
projection, retained Markdown rendering, GEPA candidate execution, token
telemetry, and runner identity should land in Pylon.

## Ported Surfaces

- Apple Foundation Models backend client, readiness contract, streaming bridge
  fixtures, callback tool session, Blueprint tool projection, acceptance cases,
  receipts, and Program Run evidence.
- Gemini backend auth, protocol lowering, streaming parser, tool schema
  sanitization, direct API client, Omega-brokered materialization, receipts, and
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
  Omega account/grant clients, and provider-account projections.

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
- Add Codex backend wiring when the Codex runtime contract is finalized.
- Keep Qwen work postponed; GEPA remains the priority backend/evaluation loop.
