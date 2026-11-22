# External Pylon Operator Quickstart (Executor Network)

Join the **OpenAgents Executor Network** from any Apple Silicon Mac (Metal 4) or Linux machine (CPU or NVIDIA CUDA) to execute compiled weight programs, independently replay traces on distinct devices, run bounded DiLoCo training windows, and settle verified work in Bitcoin via Breez Spark.

Every gate below is enforced in `#authority` Coax (`psionic/lib/pylon/onboard.coax`, `psionic/lib/pylon/executor-selfcheck.coax`, `pay/executor-settlement.coax`). Nothing advertises a slot or claims settled Bitcoin unless the real digest self-test and real settlement receipt pass.

---

## 1. Build the CLI & Create Your Identity Key

```sh
coax/build.sh
apps/openagents-cli/build.sh
apps/openagents-cli/build/openagents nostr new
```

Your NIP-06 identity key is stored at `0600` under `~/.openagents/` (never printed or uploaded).

## 2. Install the 8 Class-E Compiled Weight Modules

```sh
apps/openagents-cli/build/openagents module install-builtins
apps/openagents-cli/build/openagents module selftest
```

This compiles, conformance-checks (`24/24` cases each), and installs all 8 Class-E coding-agent cores plus 2 zero-recompilation linked composites under `~/.openagents/modules/<sha256>/`, proving 1-byte tamper refusal on disk.

## 3. Run the Truthful Onboarding Audit (`openagents pylon onboard`)

```sh
apps/openagents-cli/build/openagents pylon onboard --json
```

`pylon onboard` compiles and loads the real `wasm-exec-v1` dense `binary64` safetensors core, runs the `#authority` digest self-test (`pe_run_selfcheck`) and a 1-byte tamper falsifier, checks your NIP-06 identity key, counts verified installed modules, reads real `real_bitcoin_moved == true` settled satoshis from your ledger, and reports exact per-lane eligibility:

- `can_run_executor`: ready to join `cj-executor`
- `can_run_validator`: ready to join `cj-validate` (requires a distinct `device_ref` and distinct key from the worker!)
- `can_run_training`: ready to join `cj-training` (`>= 4` verified modules installed)
- `can_settle_mainnet`: **default OFF** (`blocker: "mainnet_unarmed"`) until you explicitly pass `--armed` with a valid `--daily-cap` (`1..1,000,000` sats) and a configured Spark wallet

## 3b. Run the One-Command Pre-Mainnet Relay Rehearsal (`openagents pylon verify-relay`)

Before arming real Bitcoin mainnet settlement, rehearse the complete relay round-trip end-to-end with zero real satoshis moved:

```sh
apps/openagents-cli/build/openagents pylon verify-relay \
  --relay wss://relay.openagents.com \
  --worker-device my-worker-box \
  --validator-device my-validator-box \
  --json
```

`pylon verify-relay` runs the real `wasm-exec-v1` digest self-test + 1-byte artifact tamper falsifier, executes a live 10×10 Hungarian job (`20,966` steps, `6,575` weight calls), signs and publishes a kind `30210` `ExecutionTraceReceipt` to `--relay`, fetches it back under a distinct ephemeral validator identity/device, proves Ch 234 exact replay and 1-byte tamper-injected trace refusal, publishes and fetches back a signed kind `30211` `ReplayVerdict`, and checks `#authority` 3-leg settlement admission (default-OFF unarmed hold + armed dry-run release across worker, validator, and module author). Passing identical `--worker-device` and `--validator-device` values is refused fail-closed (`same_device_self_validation_refused`).

## 4. Run a Worker (`cj-executor`) or Independent Validator (`cj-validate`)

On **Device A** (e.g. Apple Silicon Mac):
```sh
apps/openagents-cli/build/openagents pylon executor start \
  --relay wss://relay.openagents.com \
  --device mac-m5-worker
```

On **Device B** (distinct physical machine and distinct key, e.g. Linux RTX 4080):
```sh
apps/openagents-cli/build/openagents pylon validate --auto \
  --relay wss://relay.openagents.com \
  --device linux-rtx-validator
```

Same-key or same-device self-validation is refused automatically by `#authority` `psionic/lib/peer/executor-audit.coax`.

## 5. Check Live Status & Reconciled Pool Feed

```sh
apps/openagents-cli/build/openagents pylon status --json
apps/openagents-cli/build/openagents pylon pool-feed
```

For owner-armed Bitcoin mainnet settlement (`200 sats` worker + `50 sats` validator, `0` sat Spark fee, rolling 24h UTC caps, zero double-spend on restart), see [the W4.3 mainnet verification record](../verification/executor-w43-mainnet-settlement.md) and [Bitcoin Chapter 123](../../bitcoin/book/ch123-paying-for-requests.md).
