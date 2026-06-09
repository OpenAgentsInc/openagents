# Blueprint Smoke And Probe Discipline V1

Issue #235 adds the first Blueprint smoke/probe discipline scaffold.

The rule is split into two lanes:

- no-network smoke tests run under fake Effect layers and are safe for normal
  unit test execution;
- deployed probes validate the live Worker, D1, Resend, and runner paths using
  redacted evidence refs only.

`workers/api/src/blueprint/services/smoke-probe.ts` defines the shared probe
spec, result, plan, executor service, fake layer, no-network plan, and deployed
probe plan. The fake layer lets tests prove probe behavior without hitting the
network or depending on production bindings.

The deployed plan is a scaffold for the future operator runner:

- `blueprint_probe.deployed.worker_http.session` validates the public Worker
  session endpoint.
- `blueprint_probe.deployed.d1.program_run_repository` validates the D1-backed
  Program Run repository path.
- `blueprint_probe.deployed.resend.review_ready_dry_run` validates the
  Resend-backed review-ready email dry-run path.
- `blueprint_probe.deployed.runner.dispatch_dry_run` validates the runner
  dispatch dry-run path.

Probe output must remain safe for logs. Probe results may include IDs, target
kinds, status, evidence refs, retained failure refs, and authority flags. They
must not include provider payloads, raw email bodies, customer private data,
secrets, tokens, invoices, preimages, raw run logs, or full trace text.

When a probe fails, the scaffold converts it into a `retained_failure.*` ref.
Those refs can later be attached to Optimizer Runs, Release Gates, Program
Registry entries, or eval fixtures so failures are replayable without leaking
the raw failure payload.
