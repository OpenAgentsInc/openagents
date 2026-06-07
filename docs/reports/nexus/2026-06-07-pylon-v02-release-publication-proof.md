# Pylon v0.2 Release Publication Proof

Date: 2026-06-07

## Summary

`pylon-v0.2.0` is published in the canonical `OpenAgentsInc/openagents`
repository and the Darwin arm64 release asset has been verified from a fresh
download outside the source checkout.

This report does not treat the old GCP/native Nexus lane as a release blocker.
The v0.2 release path is the MDK-default Pylon path with Omega/Cloudflare MDK
checkout proof and the Artanis SHC bootstrap proof already recorded in this
repository.

## Published Release

- Repository: `OpenAgentsInc/openagents`
- Release tag: `pylon-v0.2.0`
- Release URL: `https://github.com/OpenAgentsInc/openagents/releases/tag/pylon-v0.2.0`
- Release name: `Pylon v0.2.0`
- Published at: `2026-06-07T15:53:22Z`
- Target commit: `f836eb909a9ce323b4097b30a01b6e358ec03fed`
- Release status: not draft, not prerelease

Published assets:

| Asset | Size | GitHub digest |
| --- | ---: | --- |
| `pylon-v0.2.0-darwin-arm64.tar.gz` | `38848961` bytes | `sha256:9a130516aa6e3b74ab09786bd0f92c2b020d1bc67c6c5cfab4951f6666e71b6f` |
| `pylon-v0.2.0-darwin-arm64.tar.gz.sha256` | `99` bytes | `sha256:25104797aa4d7639c72e82488167e82a8af76fd3f6235558db39a1bfaed08c97` |
| `pylon-v0.2.0-linux-x86_64.tar.gz` | `141199792` bytes | `sha256:ee10e5d3eec003dde1788d616b3cfdfedd990a138f4bd2099c1a35c5a35e046b` |
| `pylon-v0.2.0-linux-x86_64.tar.gz.sha256` | `99` bytes | `sha256:830127758083de6b95564080d82c1ff35c5a0f11066df5d0426ac41758a6f93a` |

## Fresh Release Asset Smoke

The release archive was downloaded through GitHub Releases into a temp
directory, verified with the uploaded checksum file, extracted, and run outside
the source checkout.

Commands run:

```bash
gh release download pylon-v0.2.0 \
  --repo OpenAgentsInc/openagents \
  --pattern 'pylon-v0.2.0-darwin-arm64.tar.gz' \
  --pattern 'pylon-v0.2.0-darwin-arm64.tar.gz.sha256'

shasum -a 256 -c pylon-v0.2.0-darwin-arm64.tar.gz.sha256
tar -xzf pylon-v0.2.0-darwin-arm64.tar.gz
cd pylon-v0.2.0-darwin-arm64
./pylon --version
./pylon --help
./pylon-tui --version
./pylon-tui --help
```

Observed results:

- Checksum verification returned `pylon-v0.2.0-darwin-arm64.tar.gz: OK`.
- `./pylon --version` returned `pylon 0.2.0`.
- `./pylon-tui --version` returned `pylon-tui 0.2.0`.
- `./pylon --help` advertised the MDK-default wallet wrapper and the explicit
  provider and wallet commands.
- `./pylon-tui --help` advertised the TUI as the homework dashboard and stated
  that payouts use the wrapped MoneyDevKit wallet by default.

## Fresh Pylon Home Smoke

The extracted `pylon` binary was then run with an empty isolated Pylon home.

Commands run:

```bash
SMOKE_HOME=$(mktemp -d /tmp/pylon-v02-home.XXXXXX)
OPENAGENTS_PYLON_HOME="$SMOKE_HOME" ./pylon init
OPENAGENTS_PYLON_HOME="$SMOKE_HOME" ./pylon status --json
OPENAGENTS_PYLON_HOME="$SMOKE_HOME" ./pylon inventory --json
OPENAGENTS_PYLON_HOME="$SMOKE_HOME" ./pylon wallet status --json
```

Observed results:

- `init` created `config.json`, `ledger.json`, and `identity.mnemonic` under
  the isolated home.
- `status --json` returned the expected `desired_mode`, `listen_addr`, and
  `snapshot` top-level fields.
- `inventory --json` returned a structured inventory object.
- `wallet status --json` returned a connected default MDK runtime with:
  - `runtime.runtime_kind=moneydevkit`
  - `runtime.liquidity_provider_kind=moneydevkit`
  - `runtime.network=bitcoin`
  - `runtime.local_daemon_port=36004`
  - `runtime.storage_dir` under the isolated Pylon home
  - `runtime.api_key_source=none:moneydevkit`
  - redacted HKDF node entropy metadata

No mnemonic, preimage, raw invoice, raw offer, or private wallet state was
committed.

## Npm Bootstrap Status

The npm package was bumped locally to `@openagentsinc/pylon@0.2.0`, but it was
not published during this proof because npm requires a one-time publish
authorization.

Evidence:

- `npm view @openagentsinc/pylon versions --json` showed latest published
  version `0.1.17`; `0.2.0` was not present.
- Before the operator refreshed CLI auth, `npm whoami` returned
  `E401 Unauthorized`.
- After CLI auth was refreshed, `npm whoami` returned `openagentsinc`.
- `npm publish --access public` reached the registry and returned `EOTP`:
  npm requires a one-time publish authorization for this package/account.

Release implication:

- The GitHub release asset is live and verified.
- The npm bootstrap remains blocked on an npm OTP or npm web-auth publish
  completion for `@openagentsinc/pylon@0.2.0`.
- Once npm credentials are restored, publish from
  `packages/pylon-bootstrap` with version `0.2.0` and verify the package
  install path from a clean cache.

## Linux SHC Asset Build Follow-Up

After the Darwin publication proof, the release was tested against the Linux
SHC host `oa-shc-katy-01` (`x86_64`). The first Linux archive attempt exposed a
release-script inefficiency: `scripts/release/pylon-binary-release.sh` used
`cargo build --manifest-path <psionic>/Cargo.toml --release -p psionic-train`,
which builds every binary target in the `psionic-train` package. On a cold
Linux cache this led to an hours-long build and was stopped before archive
publication.

The script was fixed after the v0.2.0 Darwin release so the packaged runtime
build uses only the required binary:

```bash
cargo build \
  --manifest-path "${PSIONIC_REPO}/Cargo.toml" \
  --release \
  -p psionic-train \
  --bin psionic-train
```

Linux archive publication should be rerun with that fixed script, uploaded to
the canonical `pylon-v0.2.0` release if the binary source remains unchanged, or
to a `pylon-v0.2.1` patch release if additional code changes are made.

The rerun with the fixed script succeeded on SHC:

- OpenAgents checkout: `4c89bcece` for the fixed script and docs.
- Pylon binary source: unchanged from release target commit
  `f836eb909a9ce323b4097b30a01b6e358ec03fed`.
- Psionic checkout: `fe185894292a86a987348ef1c2602d715a5327d4`.
- The OpenAgents release-profile Pylon binaries were already cached and
  rebuilt in `6.92s`.
- The fixed local Psionic runtime build completed in `2m16s`.
- Archive written on SHC:
  `/tmp/oa-pylon-v02-linux-build/openagents/target/pylon-release/pylon-v0.2.0-linux-x86_64.tar.gz`.
- Checksum written on SHC:
  `/tmp/oa-pylon-v02-linux-build/openagents/target/pylon-release/pylon-v0.2.0-linux-x86_64.tar.gz.sha256`.
- Local checksum verification after copying the assets back to the Mac returned
  `pylon-v0.2.0-linux-x86_64.tar.gz: OK`.
- Both Linux assets were uploaded to
  `https://github.com/OpenAgentsInc/openagents/releases/tag/pylon-v0.2.0`.

The Linux asset was then redownloaded from the public GitHub release URL on
SHC with `curl -fL`, verified with `sha256sum -c`, extracted, and run from an
isolated home:

```bash
curl -fL -o pylon-v0.2.0-linux-x86_64.tar.gz \
  https://github.com/OpenAgentsInc/openagents/releases/download/pylon-v0.2.0/pylon-v0.2.0-linux-x86_64.tar.gz
curl -fL -o pylon-v0.2.0-linux-x86_64.tar.gz.sha256 \
  https://github.com/OpenAgentsInc/openagents/releases/download/pylon-v0.2.0/pylon-v0.2.0-linux-x86_64.tar.gz.sha256
sha256sum -c pylon-v0.2.0-linux-x86_64.tar.gz.sha256
tar -xzf pylon-v0.2.0-linux-x86_64.tar.gz
cd pylon-v0.2.0-linux-x86_64
./pylon --version
./pylon-tui --version
OPENAGENTS_PYLON_HOME="$HOME_DIR" ./pylon init
OPENAGENTS_PYLON_HOME="$HOME_DIR" ./pylon status --json
OPENAGENTS_PYLON_HOME="$HOME_DIR" ./pylon wallet status --json
```

Observed SHC smoke results:

- Checksum verification returned `pylon-v0.2.0-linux-x86_64.tar.gz: OK`.
- `./pylon --version` returned `pylon 0.2.0`.
- `./pylon-tui --version` returned `pylon-tui 0.2.0`.
- Isolated home:
  `/tmp/pylon-v02-linux-home.SYyREJ`.
- `status --json` returned the expected `desired_mode`, `listen_addr`, and
  `snapshot` fields.
- `wallet status --json` returned:
  - `runtime.runtime_kind=moneydevkit`
  - `runtime.liquidity_provider_kind=moneydevkit`
  - `runtime.network=bitcoin`
  - `runtime.local_daemon_port=38080`
  - `runtime.storage_dir=/tmp/pylon-v02-linux-home.SYyREJ/wallet`
  - `runtime_status=connected`

No GitHub credentials were installed on SHC for this smoke; the asset was
downloaded through the public release URL.

## Misplaced Psionic Release Cleanup

The stale `OpenAgentsInc/psionic` release `v0.2.0` was inspected and removed.
It was titled `Psionic v0.2.0 Pylon release`, had no assets, pointed at
`main`, and conflicted with the decision that Pylon releases belong in the
`OpenAgentsInc/openagents` repository under `pylon-v...` tags.

Cleanup command:

```bash
gh release delete v0.2.0 \
  --repo OpenAgentsInc/psionic \
  --cleanup-tag \
  --yes
```

Verification:

- `gh release view v0.2.0 --repo OpenAgentsInc/psionic` no longer resolves.

## Remaining Gate

The remaining rollout gate is the post-release Artanis/Pylon paid-work proof:

- Artanis dispatches or simulates a paid work assignment against the v0.2
  Pylon path.
- The proof records the assignment/run id, Pylon identity, settlement/payment
  state, and public-safe receipt evidence.
- The proof uses the current Omega/Cloudflare MDK path or a documented local
  proof-runtime bridge, not the old GCP/native Nexus lane unless that legacy
  lane is explicitly under test.

Current post-release proof artifact:

- `docs/reports/nexus/pylon-v02-post-release-artanis-paid-work-proof-20260607-155959.json`
- status: `completed`
- lane: `cs336-a1-hosted-starter`
- namespace: `pylon-v02-post-release-20260607-155959`
- detail:
  `window window.cs336.a1.starter.20260607160246.69d216ae.0001 reconciled with 1 accepted contribution(s), closeout=rewarded, workers_healthy=2, validators_healthy=1`

That proof was run from the downloaded Darwin `pylon-v0.2.0` binary using an
isolated Pylon home. It proves released-binary accepted-work closeout through
the local proof runtime with simulated treasury enabled. It does not claim
real public Bitcoin settlement.
