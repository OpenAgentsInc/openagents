# Pylon LDK Wallet Regtest Harness

The Pylon LDK wallet has an opt-in local harness for proving wallet behavior
with real regtest Bitcoin and real `ldk-node` state. It is intentionally not
part of default CI because it starts `bitcoind`, `electrsd`, and two LDK nodes.

## Run

```bash
scripts/pylon/ldk-wallet-regtest-harness.sh
```

The wrapper runs:

```bash
cargo test -p pylon pylon_ldk_wallet_harness_plan_covers_required_evidence --lib
cargo test -p pylon --test ldk_wallet_regtest_harness -- --ignored --nocapture
```

If the downloaded binaries are not available through `electrsd`, point the
harness at local binaries:

```bash
BITCOIND_EXE=/path/to/bitcoind \
ELECTRS_EXE=/path/to/electrs \
scripts/pylon/ldk-wallet-regtest-harness.sh
```

On Apple Silicon, the `electrsd` 0.36 downloaded `electrs` binary can be
x86_64-only. In that case the wrapper exits before the heavy test and requires
`ELECTRS_EXE` to point at a native electrs 0.10.6-compatible binary.

One repo-local install path that works on this Mac is:

```bash
cargo install electrs --version 0.10.6 \
  --root target/pylon-ldk-wallet-tools-0106 \
  --locked

ELECTRS_EXE="$PWD/target/pylon-ldk-wallet-tools-0106/bin/electrs" \
scripts/pylon/ldk-wallet-regtest-harness.sh
```

By default artifacts are written to:

```text
target/pylon-ldk-wallet-regtest/latest/
```

Override that path with:

```bash
OPENAGENTS_PYLON_LDK_HARNESS_ARTIFACTS_DIR=/tmp/pylon-ldk-proof \
scripts/pylon/ldk-wallet-regtest-harness.sh
```

## What It Proves

The harness performs the following evidence-producing steps:

- starts isolated regtest `bitcoind` and `electrsd`;
- starts payer and receiver `ldk-node` wallets with persisted storage;
- mines funds into both wallets and records on-chain balances;
- opens and confirms a channel from payer to receiver;
- records channel-readiness cases for no channel, pending channel, usable
  channel, and a projected receive route/liquidity failure;
- creates a receiver BOLT11 invoice and pays it from payer;
- attempts BOLT12 receive/send and records whether the current LDK build can
  complete it;
- sends an on-chain withdrawal from receiver back to payer;
- restarts both nodes and asserts the payment and channel state remain present;
- copies receiver storage into a backup artifact, restores it into a fresh
  directory, and asserts the restored node can still see the received payment.

## Artifacts

The main machine-readable artifact is:

```text
harness-summary.json
```

It records:

- payer and receiver node IDs;
- funding and withdrawal transaction IDs;
- channel funding outpoint;
- BOLT11 payment ID and payment hash;
- payer and receiver balances before and after;
- channel readiness proof cases, including inbound/outbound liquidity and
  typed Lightning receive warning codes;
- harness receipt IDs for funding, channel open, BOLT11 payment, withdrawal,
  and backup restore;
- BOLT12 attempt status;
- restart and restored payment status;
- receiver backup digest;
- storage and backup artifact paths.

The same artifact directory keeps the storage snapshots:

```text
payer-storage/
receiver-storage/
receiver-backup/
restored-receiver-storage/
```

## CI Posture

Default Pylon tests only validate the harness contract:

```bash
cargo test -p pylon pylon_ldk_wallet_harness_plan_covers_required_evidence --lib
```

The full real-payment harness must be run explicitly with the ignored test or
the wrapper script above.

When `jq` is installed, the wrapper also asserts the machine-readable
`channel_readiness` section of `harness-summary.json` after the Rust harness
completes. The Rust test contains the same assertions, so the proof does not
depend on shell tooling alone.
