#!/usr/bin/env bun
// Long-running QA-runner daemon.
//
// TWO modes:
//
//   1) API mode (#6196) — the QA CONTROL HTTP daemon. Drive the full
//      autonomous-QA / eval flow over HTTP (submit -> run -> fetch artifacts +
//      verdict + /pro link), auth'd by a Khala agent bearer token. It runs the
//      existing runner/evals engine IN-PROCESS (the runner drives a real Chrome
//      via Playwright, which cannot run inside a Cloudflare Worker — so this
//      lives on a machine WITH Chrome). Enable with `--api` or
//      QA_CONTROL_MODE=api. See `bun run api`.
//
//   2) Lease-loop mode (default) — a pull loop that leases a QA task, runs it
//      through `runQaSession`, and (later) posts the result receipt back. It is
//      INERT by default — without a configured job source it logs that it has
//      nothing to do and exits, rather than inventing work. The hosted job
//      source + receipt callback are owner-gated follow-ups (epic #6174).
//
// For local one-shot runs use `run-once` / `demo:login`.

import { serveControlApi } from "./api-server";

function runApiMode(): void {
  const server = serveControlApi();
  console.log(
    JSON.stringify({
      kind: "qa_control_api",
      message: "qa-runner control API listening",
      url: `http://${server.hostname}:${server.port}`,
      armReal: process.env.QA_CONTROL_ARM_REAL === "1",
      // Honest: an empty token allowlist fails closed (every request 401s).
      tokensConfigured: Boolean(process.env.QA_CONTROL_TOKENS),
    }),
  );
}

function main(): void {
  const apiMode =
    process.argv.includes("--api") || process.env.QA_CONTROL_MODE === "api";
  if (apiMode) {
    runApiMode();
    return;
  }

  const leaseUrl = process.env.QA_JOB_LEASE_URL;
  if (!leaseUrl) {
    console.log(
      JSON.stringify({
        kind: "inert",
        message:
          "qa-runner daemon is inert: no QA_JOB_LEASE_URL configured. Use `bun run run-once` " +
          "or `bun run demo:login` for local runs. The hosted lease loop + receipt callback " +
          "are owner-gated follow-ups.",
      }),
    );
    return;
  }
  // The real lease loop (lease -> runQaSession -> post receipt -> ack) is wired
  // when the hosted job source lands. Until then, fail closed rather than poll a
  // half-configured endpoint.
  console.log(
    JSON.stringify({
      kind: "not_implemented",
      message: "QA_JOB_LEASE_URL is set but the hosted lease loop is not armed yet.",
    }),
  );
}

if (import.meta.main) {
  main();
}
