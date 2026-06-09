# `@openagentsinc/pylon`

Bootstrap the latest tagged standalone `Pylon` release asset from GitHub
Releases, fall back to a deterministic source build when no matching asset
exists for the local platform, stream first-run status updates in the terminal,
and start the Pylon terminal UI without Cargo when prebuilt binaries are
available. The terminal UI manages the long-lived earning worker.

## Usage

```bash
npx @openagentsinc/pylon
bunx @openagentsinc/pylon
npm install -g @openagentsinc/pylon && pylon
bun install -g @openagentsinc/pylon && pylon
npx @openagentsinc/pylon --version 0.1.16
npx @openagentsinc/pylon --no-launch
npx @openagentsinc/pylon --no-updates
npx @openagentsinc/pylon status --json
npx @openagentsinc/pylon --register-openagents --no-launch --json
npx @openagentsinc/pylon --register-openagents --setup-mdk-wallet --no-launch --json
npx @openagentsinc/pylon --download-curated-cache --model gemma-4-e2b --run-diagnostics
npx @openagentsinc/pylon --verbose
```

The launcher:

- supports direct `npx` / `bunx` execution plus global `npm install -g` /
  `bun install -g` installs with the same `pylon` command
- checks GitHub for the latest tagged `pylon-v...` release on each default run,
  or resolves a specific tagged `Pylon` version when `--version` is provided
- only installs releases initiated by `AtlantisPleb` in GitHub Releases
- resolves the correct release asset for the current machine:
  `pylon-v<version>-<os>-<arch>.tar.gz` on darwin/linux and
  `pylon-v<version>-windows-x86_64.zip` on native Windows x86_64
- falls back to the exact tagged source checkout and builds `pylon` plus
  `pylon-tui` locally when no matching release asset exists for the machine
- prompts before installing the Rust toolchain via `rustup` if a source build
  is needed and `cargo` / `rustc` are missing
- emits best-effort anonymous installer telemetry to `openagents.com` so the
  public stats page can show install starts, completions, source-build
  fallbacks, update checks, restart behavior, Rust prompts, and smoke-test
  outcomes
- downloads the archive and published SHA-256 checksum
- verifies the checksum before extracting
- caches the unpacked binaries under `~/.openagents/pylon/bootstrap/`
- never links or copies those cached standalone binaries into a shared global
  bin directory, so the package-managed `pylon` launcher remains the command on
  `PATH`
- prints status lines such as release resolution, runtime checks, and local
  model scanning while it runs
- ends first run with an explicit verdict such as `fully online`, `runtime
  ready`, or `installed but runtime missing`, plus exact next-step guidance
- runs `pylon --help`, `init`, `status --json`, and `inventory --json`
- skips Gemma diagnostics by default because hosted homework training does not
  require local Gemma weights
- only runs `pylon gemma diagnose <model> --json` when `--run-diagnostics` is
  set
- only runs `pylon gemma download <model>` when `--download-curated-cache` is
  set, because the optional GGUF cache does not satisfy the sellable runtime by
  itself
- falls back to `curl` for release metadata and asset downloads when the Node
  fetch path fails in constrained network contexts
- starts the installed `pylon-tui` by default after the smoke path; that TUI
  starts and supervises the earning worker
  unless `--no-launch` is set
- forwards CLI subcommands such as `pylon status --json` to the installed
  `pylon` binary after bootstrap instead of opening `pylon-tui`
- can register the local Pylon with OpenAgents and send a public-safe
  heartbeat when `--register-openagents` is explicitly set
- can initialize or reuse a local MoneyDevKit agent wallet, generate local
  receive readiness, and report only redacted wallet/payout-target refs when
  `--setup-mdk-wallet` is explicitly set
- on native Windows x86_64, installs `pylon.exe` and `pylon-tui.exe` from the
  Windows `.zip` release asset and forwards CLI subcommands to `pylon.exe`
- while the TUI is running on the default release track, checks GitHub Releases
  on a six-hour cadence and restarts the TUI from a newer trusted cached release
  without replacing the global npm/bun command
- use `--no-updates` to keep the current installed release running without
  background GitHub release checks; `--version` remains a pinned release run.
  Set `GITHUB_TOKEN` or `GH_TOKEN` when you want authenticated GitHub release
  lookups.
- owns the current auto-update contract. Directly extracted GitHub release
  assets do not contain a native updater today; if an operator runs
  `./pylon` from an unpacked archive, that process stays on its compiled
  version until the operator manually replaces the archive or switches back to
  the npm/bun launcher.
- for hosted homework/training work, use launcher `0.1.17` or newer so the
  cached standalone binary auto-updates while the dashboard is open. The
  `pylon-v0.1.16` standalone binary keeps the long hosted homework ID hashing
  from `0.1.14`, refuses to seal terminal training windows until the worker
  contribution artifact bundle has uploaded and verified for validator replay,
  and packages the minimal Psionic training runtime so standalone installs can
  advertise homework-worker capability. Launcher `0.1.17` adds CLI subcommand
  forwarding and bounds background GitHub release checks to avoid
  unauthenticated rate-limit churn.
- does not register with OpenAgents by default; registration is an explicit
  token-scoped operator action

## OpenAgents registration

Use this only with an active OpenAgents programmatic agent token. Prefer an
environment variable so the token is not stored in shell history:

```bash
export OPENAGENTS_AGENT_TOKEN="oa_agent_..."

npx @openagentsinc/pylon@latest \
  --register-openagents \
  --openagents-api https://openagents.com \
  --resource-mode background_20 \
  --no-launch \
  --json
```

The launcher runs the normal first-run smoke, derives or reuses a stable public
Pylon ref from the local Pylon identity, registers through
`POST /api/pylons/register`, and sends a heartbeat through
`POST /api/pylons/{pylonRef}/heartbeat`.

Optional public-safe registration flags:

```bash
--pylon-ref pylon.example.local
--pylon-display-name "Example Pylon"
--capability-ref capability.public.gpu
```

The registration payload deliberately excludes local paths, wallet material,
private model inventory, hostnames, SSH details, provider credentials, and raw
payment data.

## MDK wallet readiness

After registration, a Pylon can report wallet receive readiness and request
payout-target admission:

```bash
export OPENAGENTS_AGENT_TOKEN="oa_agent_..."

npx @openagentsinc/pylon@latest \
  --register-openagents \
  --setup-mdk-wallet \
  --openagents-api https://openagents.com \
  --resource-mode background_20 \
  --no-launch \
  --json
```

Optional test/operator isolation flags:

```bash
--mdk-wallet-home /path/to/ignored/mdk-home
--mdk-wallet-port 3457
--mdk-receive-amount-sats 21
```

The launcher uses `npx @moneydevkit/agent-wallet@latest` locally. It may run
`init --show`, `init`, `balance`, and `receive`. It submits only:

- `wallet.public.mdk_agent_wallet.<digest>`;
- `receive.redacted.mdk_agent_wallet.<digest>`;
- `payout_target.public.mdk_agent_wallet.<digest>`;
- bucketed readiness refs such as
  `balance.mdk_agent_wallet.minimum_not_satisfied`.

It does not submit the mnemonic, wallet config, raw receive invoice, payment
hash, preimage, exact wallet balance, wallet home path, or private destination
material to OpenAgents.

## Launch evidence bundle

The JSON bootstrap summary includes `openAgentsLaunchEvidence`. This is a
public-safe launch gate for Omega and operator dashboards. It records:

- install evidence for the resolved `pylon-v...` release and local target;
- registration and heartbeat refs when `--register-openagents` is used;
- MDK receive-readiness refs when `--setup-mdk-wallet` is used;
- required platform smoke refs for macOS arm64, Linux x86_64, native Windows
  x86_64, and WSL Ubuntu x86_64;
- required assignment worker-loop refs for polling leases, accepting,
  reporting progress, submitting artifact/proof refs, and accepted-work
  closeout;
- launcher-copy refs that keep receive readiness, send readiness, assignment
  execution, payout, and settlement as separate claims.

If any required ref is missing, `openAgentsLaunchEvidence.status` is `blocked`
and `earningClaimAllowed` is `false`. The launcher does not claim MDK send
readiness by itself; `sendReadinessClaimed` remains `false` unless a later
operator gate supplies separate send-readiness evidence.

The bundle rejects raw invoices, payment hashes, preimages, mnemonics, wallet
paths, payout targets, provider secrets, customer data, raw logs, and raw
timestamps.

Set `OPENAGENTS_DISABLE_TELEMETRY=1` to disable installer telemetry, or
`OPENAGENTS_TELEMETRY_URL=http://127.0.0.1:8000/api/telemetry/events` to point
the launcher at a non-production telemetry endpoint.

## Publish

Publish directly from this package directory:

```bash
cd packages/pylon-bootstrap
npm pack --dry-run
npm publish
```
