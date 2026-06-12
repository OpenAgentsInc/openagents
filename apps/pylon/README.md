# Pylon

![Pylon](docs/images/pylon.png)

## Tech stack
- [Bun](https://bun.sh)
- [Effect](https://effect.website/)
- [OpenTUI](https://github.com/anomalyco/opentui)

## Launch Package And Version Truth

**The only published, installable Pylon is `@openagentsinc/pylon@0.2.5`**
(`npm view @openagentsinc/pylon dist-tags` → `latest: 0.2.5`). That launcher
is the supported operator path and the one AGENTS.md tells agents about.

**v0.3 is a source-only release candidate.** This directory is at
`0.3.0-rc2` per `package.json`; no `0.3.0-rc*` artifact has been published
to npm (verified 2026-06-11: `@openagentsinc/pylon@0.3.0-rc1` returns 404).
Do not write public copy that names an installable v0.3 package until the
release gates pass and the publish actually happens (#4662 tracks the
stable release).

### Running the v0.3 RC from source (testing only)

```sh
git clone https://github.com/OpenAgentsInc/openagents
cd openagents && bun install
bun run --cwd apps/pylon start     # equivalently: bun apps/pylon/src/index.ts
```

**Non-readiness warnings — read before running:**

- This is a release candidate behind open launch gates. Expect breakage,
  unannounced behavior changes, and TUI/runtime surfaces that are mid-build.
- Nothing about running the RC creates earning expectations: paid work
  classes, settlement, and marketplace routing are gated by the
  product-promise registry exactly as for v0.2, and no v0.3-only feature
  (TUI dashboard, runtime backends, Nostr credentials) may be described
  publicly as released until the registry says so.
- Run it only with explicit owner approval, on a machine whose owner
  understands it is pre-release software with wallet-adjacent surfaces.
  Wallet operations always end in an explicit confirmation dialog, but the
  posture is: test with sats you can afford to lose, or with none.
- The local gate (`bun run release:gate`) is the bar the RC has to pass;
  if you run the RC and find a gate the suite misses, that report is more
  valuable than the testing itself - file it or post it on the Forum.

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
pylon psionic smoke --json
bun run smoke:psionic-qwen -- --base-url http://127.0.0.1:8080
pylon psionic install --channel rc --manifest-url <release-manifest-url> --yes
pylon psionic models install qwen35-0_8b-q8_0 --manifest-url <model-manifest-url> --yes
pylon apple-fm status
pylon apple-fm tool-stream-demo
```

## Dashboard (TUI)

Running `pylon` with no subcommand opens the observational dashboard: an
execution-log feed (left), wallet/telemetry/operator panes (right), a
composer (bottom), and a one-line key-hint footer.

- Startup is quiet by default; launch with `--verbose` (or `PYLON_VERBOSE=1`)
  for full service logs, or press `f2` to toggle verbosity at runtime.
- `ctrl+k` opens the command palette (fuzzy search over every command),
  `f1` shows all keybindings, `tab` switches focus between the log feed and
  the composer, and `ctrl+c` exits cleanly.
- Wallet operations (`wallet: send sats`, `wallet: receive`, `wallet: admit
  payout target`) run from the palette and always end in an explicit
  confirmation dialog before any money moves.
- Keybindings are user-configurable via `keybinds.json` in the Pylon home
  directory: `{ "bindings": { "palette.open": "ctrl+p" } }`. Keys are
  command names (see `f1`); values are `@opentui/keymap` key strings.
  Invalid files are reported and ignored.
- The composer submits with `meta+return` and streams Codex SDK events into
  the feed, running in the current working directory by default
  (`PYLON_CODEX_CWD` or `PYLON_ACTIVE_REPO` can override it). Missing Codex
  SDK/auth readiness is shown as a typed Codex blocker instead of falling
  back to OpenCode. The default composer mode is local bounded
  `workspace-write`; `pylon dev --codex-danger`, `pylon --codex-danger`, or
  `"dev": { "codexExecutionMode": "local_supervised_danger" }` explicitly
  switches the **local dashboard composer only** to SDK
  `danger-full-access` with `approvalPolicy: "never"` and labels the feed as
  `Codex DANGER`. Submitted prompts persist across restarts: cycle them with
  `ctrl+p` / `ctrl+n`, and an unsent draft is stashed on exit and restored
  on the next launch.
- `pylon dev doctor --json` returns the redacted local context projection for
  the dashboard/dev loop: active repo provider/name, branch, commit, dirty
  count, instruction/config digest refs, Codex SDK/CLI/auth readiness,
  Claude/Fable readiness, active Codex execution mode, and backend refs. It
  never prints raw keys, auth file paths, instruction text, changed filenames,
  or local absolute paths.
- The sidebar renders a live 3D network view (`@opentui/three` on a native
  WebGPU device, quantized to terminal glyphs): satellites orbit the market
  core, wallet status drives color and speed, new feed activity pulses
  nodes, and a balance increase fires a bitcoin-orange burst. It soft-fails
  to a placeholder without a GPU, hides on terminals under 32 rows, and
  `PYLON_DISABLE_3D=1` turns it off.
- Views: `f3` dashboard, `f4` assignments (poll/accept work leases - accept
  always confirms first), `f5` wallet (status, readiness, session balance
  history). All views are also reachable from the palette.

### Headless node and attach

- `pylon node` runs node-core headless: all services, durable feed log, and
  a loopback control API (default port 4716, `PYLON_CONTROL_PORT` to
  change) authenticated by the bearer token in `<pylon-home>/control-token`.
- On a Linux VM, use `scripts/install-cloud-node.sh` plus
  `docs/cloud-node-deployment.md` to install the headless node as a systemd
  service. Set `PYLON_ASSIGNMENT_WORKER=1` to continuously pick up eligible
  no-spend owner assignments while the VM stays online.
- `pylon attach [url]` opens the dashboard as a client of a running node:
  it restores the node's scrollback from the connection snapshot, follows
  live events over SSE (reconnecting with 1s-30s backoff), and routes
  wallet commands through the node's control API after the usual confirm
  dialogs. Detaching (`ctrl+c`) never interrupts the node.
- The interactive `pylon` dashboard also serves the control API, so a
  second terminal can attach to it.

### TUI test harness

`src/tui/harness.tsx` mounts the real dashboard headlessly (no TTY) via
`@opentui/solid`'s `testRender`: inject keys programmatically, capture
character frames, snapshot them with `bun:test`, and drive the real
runtime/bridge with fake `PylonEvent` streams. See
`tests/tui-render-harness.test.ts`; the harness is importable by the
runtime package's renderer tests as well.

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

## NIP-90 Provider Loop

GO ONLINE for the NIP-90 provider lane is persisted through the provider
command:

```sh
pylon provider go-online
pylon provider approve-labor --approved-by-ref operator.public.<ref> --job-type code_task
pylon provider once
pylon provider go-offline
bun run smoke:nip90-provider
```

`go-online` marks the local runtime online, adds
`capability.public.pylon.nip90.text_inference.v0.3` and
`capability.public.pylon.labor.local_agent.v0.3`, and records the relay and
admission policy that the OpenTUI background loop will use. Labor jobs require
an explicit first-run operator approval record from `provider approve-labor`
before they execute on a machine. `provider once` is the headless smoke path
for one relay loop iteration; the default dashboard starts the same loop
automatically only when the persisted lifecycle is `online` or
`assignment-ready`.

The provider loop subscribes to the scoped OpenAgents market relay by default,
publishes NIP-89 handler info, admits public kind `5050` text-inference
requests and OpenAgents labor kinds `5934` code task, `5935` review, and
`5936` document work, then publishes NIP-90 `7000` feedback plus result kinds
`6050` or `6934`-`6936`. Text inference executes the local Apple FM runtime.
Labor jobs execute through the contributor's configured local agent path
(`codex`, `opencode`, or `claude`) inside a bounded workspace and return
public-safe artifact refs. It uses the shared `@openagents/nip90` package,
which re-exports the local `nostr-effect` protocol helpers.

Environment controls:

- `PYLON_NIP90_RELAYS`: comma-separated relay URLs; defaults to
  `wss://openagents-market-relay.openagents.workers.dev`.
- `PYLON_NIP90_PRICE_MSATS`: price floor and requested invoice amount;
  defaults to `1000`.
- `PYLON_NIP90_REQUEST_TTL_SECONDS`: request age limit; defaults to one year.
- `PYLON_NIP90_MAX_INFLIGHT`: total local inflight admission leases; defaults
  to `1`.
- `PYLON_NIP90_PER_BUYER_MAX_INFLIGHT`: per-buyer inflight leases; defaults
  to `1`.
- `PYLON_LABOR_AGENT`: optional local labor agent selector: `codex`,
  `opencode`, or `claude_code`. If unset, Pylon detects `codex`, then
  `opencode`, then `claude`.
- `PYLON_LABOR_AGENT_COMMAND`: optional explicit local command prefix for
  advanced operators. Pylon appends the generated public-safe labor prompt.

Wallet boundary: the loop may put a raw BOLT 11 invoice into Nostr relay
events because NIP-90 payment-required/result tags require it, but local state,
ledger records, OpenAgents API payloads, logs, and issue evidence must only
carry public-safe receipt refs, amounts, event ids, and readiness refs. See
`docs/nip90-provider-loop.md`.

Legacy Spark/Breez migration boundary: `pylon wallet migrate-spark` is a
preflight-first compatibility path for old v0.2.x balances. It reports missing
Breez/Spark credential material as an actionable blocker and only proceeds with
explicit local consent. Users must never paste a 12-word mnemonic into GitHub,
support threads, logs, or issue comments. See
`docs/legacy-spark-wallet-migration.md`.

Labor boundary: Pylon rejects labor requests that carry provider-auth-shaped
material, requests outside the bounded workspace, or a policy ref other than
`provider.compliant_usage_labor.v1`. The contributor's own local provider
accounts or API budgets stay on the contributor machine; OpenAgents pays for
accepted work output only and never resells, proxies, brokers, or transfers
provider credentials, sessions, account access, or subscription/API capacity.

The runtime includes:

- Apple Foundation Models bridge support, readiness receipts, streaming tool
  callbacks, and Program Run evidence.
- Gemini direct API and OpenAgents product surface-brokered Gemini materialization.
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
- Psionic training-boundary contracts for signed training release manifests,
  digest-verified artifacts, healthy sidecar lifecycle projection, and signed
  worker receipt import. `supportsTraining` remains false until that complete
  boundary is real. See `docs/psionic-training-boundary.md`.
- Provider-neutral LLM message/request/tool/usage contracts.
- Blueprint signature lookup, tool-menu planning, Action Submission boundaries,
  and contribution release gates.
- Retained OpenTUI Markdown rendering helpers and markdown/code streaming
  fixtures.
- GEPA/Terminal-Bench candidate execution, closeout bundles, token telemetry,
  runner identity, and OpenAgents product surface grant/account contracts.
- Psionic Qwen3.5 attach-only backend discovery and doctor support with
  `PYLON_PSIONIC_BASE_URL` / `PROBE_PSIONIC_BASE_URL`, 0.8B and 2B model-row
  refs, assignment-runner admission, typed unattached refusal, and redacted
  availability/transcript/tool-call receipts. See
  `docs/2026-06-09-pylon-qwen35-local-inference-roadmap.md` and
  `docs/psionic-qwen-live-smoke.md`; this is not a training, bundled-model,
  startup auto-download, or paid-capacity claim.

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
