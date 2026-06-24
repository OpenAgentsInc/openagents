#!/usr/bin/env bun
// Long-running QA-runner daemon scaffold.
//
// Mirrors acceptance-runner's shape: a pull loop that leases a QA task, runs it
// through `runQaSession`, and (later) posts the result receipt back. It is INERT
// by default — without a configured job source it logs that it has nothing to do
// and exits, rather than inventing work. The hosted job source + receipt
// callback are owner-gated follow-ups (the run = receipt wrapper, epic #6174).
//
// For now, local development uses `run-once` / `demo:login`; this entry exists so
// the app has the same serve/run-once/test surface as acceptance-runner and a
// clear home for the future lease loop.

function main(): void {
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
