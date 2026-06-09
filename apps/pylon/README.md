# Pylon

![Pylon](docs/images/pylon.png)

## Tech stack
- [Bun](https://bun.sh)
- [Effect](https://effect.website/)
- [OpenTUI](https://github.com/anomalyco/opentui)

## Launch Package

The v0.3 release-candidate package is `@openagentsinc/pylon@0.3.0-rc1` and
exposes the `pylon` binary. Do not treat `0.3.0` as stable until the launch
gates pass.

Initial supported operator platforms are macOS and Linux. No other operator
platforms are in scope for the first v0.3 launch path.

## Runtime Backends

Pylon now carries the former Probe runtime as `@openagentsinc/pylon-runtime`.
The public `pylon` binary bundles that runtime source, keeps the OpenTUI node
dashboard as the default, and routes backend/runtime commands through the same
binary:

```sh
pylon runtime backend gemini smoke
pylon backend gemini complete --prompt "Summarize the current task."
pylon backend psionic doctor --json
pylon psionic doctor --json
pylon psionic install --channel rc --manifest-url <release-manifest-url> --yes
pylon psionic models install qwen35-0_8b-q8_0 --manifest-url <model-manifest-url> --yes
pylon apple-fm status
pylon apple-fm tool-stream-demo
```

## Bootstrap And Status

```sh
pylon bootstrap --json
pylon bootstrap --register-openagents --setup-mdk-wallet --pylon-ref <ref> --display-name <name> --resource-mode background_20 --capability-ref <ref> --json
pylon status --json
```

`bootstrap` creates the local v0.3 home/cache/release layout and writes a
minimal public-safe config summary. Live registration and MDK mutation are
tracked by later launch gates.

`status --json` loads or creates the local identity/runtime state and emits a
redacted public-safe projection for headless diagnostics.

Presence commands are available for fake-server and later live endpoint
integration:

```sh
pylon presence register --base-url https://openagents.com
pylon presence heartbeat --base-url https://openagents.com
pylon presence link-complete --base-url https://openagents.com
pylon presence link-refresh --base-url https://openagents.com
```

Wallet readiness commands wrap MDK without exposing wallet secrets:

```sh
pylon wallet status
pylon wallet receive --amount 1000
pylon wallet send --destination-ref payout.bolt12.<hash> --amount 21
pylon wallet admit-payout-target --kind bolt12_offer --ref payout.bolt12.<hash>
```

Wallet status uses MDK readiness evidence and records idempotent local ledger
events. Send readiness remains blocked unless MDK returns explicit evidence;
balance and receive readiness are not treated as spendable settlement.

Assignment worker commands are available for signed fake-server and live API
smokes:

```sh
pylon assignment poll --base-url https://openagents.com
pylon assignment run-no-spend --base-url https://openagents.com
```

`run-no-spend` polls for a no-spend lease, applies local admission gates,
accepts idempotently, submits progress with artifact/proof refs, and closes the
assignment with `settlementState: not_applicable` and
`payoutClaimAllowed: false`. Paid leases are blocked unless wallet send
readiness is explicitly proven.

The runtime includes:

- Apple Foundation Models bridge support, readiness receipts, streaming tool
  callbacks, and Program Run evidence.
- Gemini direct API and Omega-brokered Gemini materialization.
- Psionic OpenAI-compatible `/v1/chat/completions` client with text, tool-call
  loop, streaming delta tool-call parsing, max round-trip guard, and redacted
  transcript/tool-call receipts.
- Psionic Qwen3.5 model-row admission gates for `0.8B` and `2B`: rows are
  advertised only after a retained artifact digest or public-safe manifest ref,
  and coding-agent selection prefers 2B when both rows are ready.
- Optional Psionic binary/model installer scaffold with explicit `--yes`
  consent, macOS/Linux machine checks, release/model manifest verification,
  SHA-256 verification, and digest-addressed cache placement. This is never
  part of startup or default package installation.
- Provider-neutral LLM message/request/tool/usage contracts.
- Blueprint signature lookup, tool-menu planning, Action Submission boundaries,
  and contribution release gates.
- Retained OpenTUI Markdown rendering helpers and markdown/code streaming
  fixtures.
- GEPA/Terminal-Bench candidate execution, closeout bundles, token telemetry,
  runner identity, and Omega grant/account contracts.
- Psionic Qwen3.5 attach-only backend discovery and doctor support with
  `PYLON_PSIONIC_BASE_URL` / `PROBE_PSIONIC_BASE_URL`, 0.8B and 2B model-row
  refs, and redacted availability receipts. See
  `docs/2026-06-09-pylon-qwen35-local-inference-roadmap.md`; this is not a
  training, bundled-model, startup auto-download, or paid-capacity claim.

## GEPA Capability Envelope

`src/gepa-capability.ts` maps v0.3 assignment leases onto the in-repo benchmark
runtime contracts. The rc envelope is GEPA-first: retained Terminal-Bench
fixtures, Probe runtime backend refs, artifact upload refs, proof/receipt refs,
assignment closeout refs, local sandbox isolation, wall-clock/cost budgets, and
capacity fields are modeled separately from payout readiness.

This does not advertise neural training or Qwen work. Those tracks remain
postponed until the GEPA lease, closeout, import, and payment-mode gates are
solid.

## Host Inventory

```sh
pylon inventory --json
```

`status --json` and the dashboard include the same host inventory projection:
supported platform, CPU/memory/disk counts, network counts, accelerator class,
backend health refs, model-cache state, and blocker refs. The projection does
not expose interface names, cache paths, env dumps, provider auth, private
topology, or raw local model paths.

Host inventory now includes an optional Psionic Qwen3.5 row. `qwen3.5:0.8b`
is the lowest-footprint smoke/fallback row and `qwen3.5:2b` is the first
coding-agent tool-loop quality row. Machines that cannot or do not want to run
local ML keep working with precise Psionic blocker refs and no binary/model
download.

## Operator Snapshot

```sh
pylon operator snapshot --json
```

The default dashboard includes bounded operate, wallet, inspect, and recovery
state. The headless snapshot is for support and service-manager runs: it shows
refs, blockers, readiness, and recovery gates without exposing raw wallet
material, provider tokens, private repo content, or local cache paths.

## Release Gate

```sh
bun run release:gate
```

The local release gate runs tests, JSON smokes, dashboard startup smoke, package
dry-run, and local package install smoke. Public copy must stay inside the
allowed claim matrix in `docs/launch-gates-no-overclaim.md`.
