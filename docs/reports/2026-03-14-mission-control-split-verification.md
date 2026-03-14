# Mission Control Split Verification

Date: March 14, 2026
Issue: `#3460`

## Scope

Validate that the Mission Control pane split did not regress:

- the headless compute smoke path
- the headless buyer/provider roundtrip
- the packaged `autopilotctl` buyer/seller/chat roundtrip
- the app-owned desktop-control truth exposed through snapshots and log artifacts

## Environment

- host: macOS (`Darwin`)
- branch: `main`
- Apple FM bridge required by repo policy for desktop compute verification
- Spark wallet preflight: `target/debug/spark-wallet-cli --network mainnet status`

Wallet preflight showed a funded wallet with `10000` sats, which was enough to
run the paid verification loops.

## Bridge preflight

Per the repo contract for Autopilot work:

1. built the bridge with `swift/foundation-bridge/build.sh`
2. ran `swift/foundation-bridge/bin/foundation-bridge`
3. verified `curl -s http://127.0.0.1:11435/health`

Observed health response included:

- `status = ok`
- `platform = macOS`
- `model_available = true`

## Commands run

- `scripts/autopilot/headless-compute-smoke.sh`
- `scripts/autopilot/headless-compute-roundtrip.sh`
- `OPENAGENTS_AUTOPILOTCTL_SKIP_BUILD=1 scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh`

## Findings

### 1. Headless smoke

`scripts/autopilot/headless-compute-smoke.sh` passed without changes.

### 2. Headless roundtrip

The first roundtrip attempt exposed harness drift, not a product regression.
The script failed because its log parser still assumed older log-line shapes.

Fix applied in `scripts/autopilot/headless-compute-roundtrip.sh`:

- buyer settlement parsing now accepts both `provider=` and `provider_nostr=`
- provider settlement parsing now tolerates additional structured fields before
  `balance_before` / `balance_after`

After that parser update, `scripts/autopilot/headless-compute-roundtrip.sh`
passed.

Observed summary:

- forward requested jobs: `6`
- forward settled by buyer: `6`
- forward settled by provider: `6`
- reverse requested jobs: `3`
- reverse executed jobs: `2`
- reverse settled by buyer: `2`
- reverse settled by provider: `2`

The reverse phase executing `2` jobs instead of the requested `3` is expected
current behavior. The harness trims the reverse count when the post-forward
provider balance and fee conditions would make the requested count unaffordable.

### 3. Packaged roundtrip

The packaged roundtrip also exposed harness drift, not app-state drift.

Fix applied in `scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh`:

- split-shell assertions now read:
  - `snapshot.session.shell_mode`
  - `snapshot.session.dev_mode_enabled`

This matches the current `autopilotctl --json status` snapshot shape.

After that assertion update,
`OPENAGENTS_AUTOPILOTCTL_SKIP_BUILD=1 scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh`
passed.

`docs/headless-compute.md` was updated to document the corrected packaged
snapshot contract.

## Artifact inspection

### Headless roundtrip

- summary: `target/headless-compute-roundtrip/summary.json`

Key values:

- `forward.buyerSettledCount = 6`
- `forward.providerSettledCount = 6`
- `reverse.requestedJobCount = 3`
- `reverse.executedJobCount = 2`
- `reverse.buyerSettledCount = 2`
- `reverse.providerSettledCount = 2`

### Packaged roundtrip

- summary: `target/packaged-autopilotctl-roundtrip/summary.json`
- bundle manifest: `target/packaged-autopilotctl-roundtrip/bundle-logs/desktop-control.json`
- runtime manifest: `target/packaged-autopilotctl-roundtrip/runtime-logs/desktop-control.json`
- bundle latest log: `target/packaged-autopilotctl-roundtrip/bundle-logs/latest.jsonl`
- runtime latest log: `target/packaged-autopilotctl-roundtrip/runtime-logs/latest.jsonl`
- bundle session log: `target/packaged-autopilotctl-roundtrip/bundle-logs/sessions/20260314T071847Z-pid19118.jsonl`
- runtime session log: `target/packaged-autopilotctl-roundtrip/runtime-logs/sessions/20260314T071847Z-pid19119.jsonl`

Observed continuity from the packaged artifacts:

- final status snapshots preserved `snapshot.session.shell_mode = "hotbar"`
- final status snapshots preserved `snapshot.session.dev_mode_enabled = false`
- bundle and runtime logs both showed the expected chat event queueing
- bundle and runtime logs both showed buyer/provider payment lifecycle events
- per-session JSONL artifacts showed consistent request/result/settlement
  sequencing across the two app instances

## Conclusion

The Mission Control pane split did not regress the paid compute loop, packaged
desktop-control parity, or the split-shell truth model.

The only failures found in this pass were verification harness assumptions that
lagged behind current structured log and snapshot shapes:

- enriched buyer/provider log fields in the headless roundtrip harness
- `snapshot.session.*` status fields in the packaged roundtrip harness

Those harnesses are now updated, documented, and passing against the current
split layout.
