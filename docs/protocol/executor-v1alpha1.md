# OpenAgents Executor Protocol v1alpha1

Status, 2026-11-21: versioned protobuf schemas, canonical JCS JSON profiles,
and a single zero-mutant `#authority` Coax codec for the OpenAgents Executor
Network ([master specification](../../psionic/docs/executor-network-master-spec.md#w21--protocol-schemas--single-codec-wire-contracts-protoopenagentsexecutorv1alpha1),
GitHub [#13914](https://github.com/OpenAgentsInc/openagents/issues/13914),
Epic [#13912](https://github.com/OpenAgentsInc/openagents/issues/13912)).

These contracts define the wire and stored records shared by W2.2 composable
weight modules, W4 Pylon exact-trace execution/validation/settlement, W5
decentralized training windows and quarantine promotion, and W6.3 signed
shadow-to-promote release distribution. They carry data only: a decoded record
never grants execution, promotion, or wallet authority on its own.

## Defining Artifacts

| Artifact | Path |
| --- | --- |
| Protobuf schemas (5 files, profile `executor`) | [`proto/openagents/executor/v1alpha1/`](../../proto/openagents/executor/v1alpha1/) (`module.proto`, `trace.proto`, `training.proto`, `settlement.proto`, `release.proto`) |
| Compiled descriptor closure | [`proto/descriptors/executor.pb`](../../proto/descriptors/executor.pb) |
| Single `#authority` Coax codec | [`psionic/lib/executor-wire.coax`](../../psionic/lib/executor-wire.coax) |
| Golden canonical JSON profile | [`proto/psionic/executor-v1alpha1.json`](../../proto/psionic/executor-v1alpha1.json) |
| Golden + descriptor + `#import_proto` suite | [`psionic/lib/executor-wire-goldens.coax`](../../psionic/lib/executor-wire-goldens.coax) |
| Wire registry rows (13 rows, 0 allowlist additions) | [`proto/registry.json`](../../proto/registry.json) |

## Canonical JCS JSON Carriage & Refusal Contract

Every signed or content-hashed payload serializes as compact RFC 8785 (JCS)
JSON:

1. **Key order:** Every `.proto` message declares its fields in strict
   lexicographical `json_name` order. Writers emit every field in that exact
   order with no whitespace; `pk_text_keys(..., exact = true)` holds every
   golden to `proto/descriptors/executor.pb`.
2. **Closed objects:** Every listed field is required. Unknown fields,
   duplicate keys, escaped key aliases (`\u0061`), and missing fields are
   refused (`ExwErr.unknown_field`), except forbidden secret keys which fail
   immediately with `ExwErr.secret`.
3. **Exact integers:** All numeric fields are exact integers in
   `[-(2^53 - 1), 2^53 - 1]`. Fractions, exponents, `-0`, and quoted numbers
   are refused (`ExwErr.bounds`). Loss and outlier quantities use fixed-point
   integer micro-units (`_e6`, `1_000_000 = 1.0`) or parts-per-billion (`_ppb`)
   so canonical JSON hashing is bit-identical across every host.
4. **Strict UTF-8, digests, and identifiers:** Strings require complete UTF-8
   without NUL. SHA-256 digests and public keys are 64 lowercase hex chars;
   BIP-340 Schnorr signatures are 128 lowercase hex chars (`ExwErr.digest`).
   Identifiers match `[a-z0-9][a-z0-9._-]*` within their per-field ceiling.
5. **No partial output on refusal:** Readers decode into scratch state and
   publish `out[0]` only when every check passes.

### Refusal Codes (`ExwErr`)

| Code | Meaning |
| --- | --- |
| `ok` | Valid message decoded or written |
| `truncated` | Empty/oversized (`> 65536` bytes) frame, truncated/invalid JSON, short arena, or short output buffer |
| `schema` | Missing, non-string, or mismatched `schema` version identifier |
| `unknown_field` | Unknown, duplicate, missing, or non-canonical escaped key, or non-object container |
| `invalid_enum` | Unknown or unspecified enum value or closed vocabulary string |
| `bounds` | Integer range, array count ceiling, string length, step/timestamp order, or `YYYY-MM-DD` period violation |
| `digest` | Malformed 64-hex digest/pubkey or 128-hex signature |
| `secret` | Forbidden secret key (`invoice`, `bolt11`, `preimage`, `private_key`, `nsec`, `mnemonic`) or secret/invoice value prefix (`lnbc`, `lntb`, `lnsb`, `nsec1`, `xprv`, `tprv`) |
| `invariant` | Cross-field semantic invariant violation (see below) |

## Contract Families (12 Wire Shapes)

### 1. Compiled Weight Modules (`module.proto`)

- **`CompiledModuleManifest`** (`openagents.executor.module.v1alpha1`):
  pins `module_id`, `version`, `profile_id`, `compiler_hash`, `weight_sha256`,
  `conformance_suite_sha256`, `input_abi` & `output_abi` (`AbiDescriptor`:
  `scalar_type` in `i32|i64|f64|bytes`, `slot_count` 1..1024, `min_value <= max_value`),
  `verification_class` (`E_EXACT_REPLAY`, `D_DETERMINISTIC_TEST`,
  `S_STATISTICAL`, `N_UNVERIFIED`), `max_steps` (`1..10^9`), `memory_bytes`
  (`1..64 GiB`), and `seams` (`0..64` `ModuleSeam` records with `layer`
  1..4096, `slot` 1..65536, `count` 1..1024, optional 64-hex `upstream_module_sha256`).

### 2. Execution Traces & Independent Replay (`trace.proto`)

- **`ExecutorProgramRef`** (`openagents.executor.program-ref.v1alpha1`):
  content-addressed binding of `compiler_hash`, `executor_artifact_hash`,
  `input_hash`, `profile_id`, and `program_hash`.
- **`ExecutionTraceReceipt`** (`openagents.executor.trace-receipt.v1alpha1`):
  worker receipt pinning `job_id`, `program_hash`, `input_hash`,
  `executor_artifact_hash`, `worker_pubkey`, `worker_device_ref`,
  `step_count` (`1..10^9`), `output_digest`, `trace_digest`,
  `chunk_merkle_root`, `started_at > 0`, and `finished_at >= started_at`.
- **`ReplayVerdict`** (`openagents.executor.replay-verdict.v1alpha1`):
  signed validator verdict (`EXACT_REPLAY`, `FAILED`, `UNVERIFIABLE`) pinning
  `receipt_digest`, `validator_pubkey`, `validator_device_ref`, `signature`,
  `first_divergent_step`, `expected_row` (0..16 ints), and `observed_row`
  (0..16 ints).
  - **Invariant:** `EXACT_REPLAY` and `UNVERIFIABLE` require
    `first_divergent_step == -1` and empty `expected_row`/`observed_row`.
    `FAILED` requires `first_divergent_step >= 0`, equal row lengths, and
    at least one differing element whenever rows are present.

### 3. Decentralized Training & Checkpoint Lineage (`training.proto`)

- **`DatasetShardManifest`** (`openagents.executor.dataset-shard.v1alpha1`):
  immutable shard pinning `shard_id`, `shard_sha256`,
  `exposure_ledger_sha256`, `split` (`TRAIN`, `VALIDATION`, `HELD_OUT`),
  `family_tags` (1..32 IDs), `record_count >= 1`, `byte_count >= 1`, and
  `created_at > 0`.
- **`TrainingWindowLease`** (`openagents.executor.training-lease.v1alpha1`):
  bounded grant pinning `lease_id`, `parent_checkpoint_sha256`,
  `optimizer_state_sha256`, `dataset_shard_sha256`, `training_config_sha256`,
  `worker_pubkey`, `random_seed`, `end_step > start_step`, and
  `expires_at > issued_at`.
- **`TrainingWindowClaim`** (`openagents.executor.training-claim.v1alpha1`):
  worker delta submission pinning `lease_id`, parent/optimizer/shard/config
  digests, `random_seed`, `end_step > start_step`, `delta_sha256`,
  `worker_pubkey`, `worker_device_ref`, and `loss_stats` (`LossStats`).
  - **Invariant:** `loss_stats.non_finite_count == 0` (Tier 0 finite-tensor gate).
- **`TrainingWindowVerdict`** (`openagents.executor.training-verdict.v1alpha1`):
  validator outcome (`ACCEPTED`, `REJECTED`, `QUARANTINED`) pinning
  `claim_digest`, `validator_pubkey`, `validator_device_ref`, `signature`,
  `outlier_metrics` (`cosine_to_median_e6` in `[-10^6, 10^6]`, `l2_norm_e6 >= 0`,
  `loss_delta_e6` in `[-10^9, 10^9]`, `step_distance_ppb >= 0`), and
  `tier_results` (exactly 6 entries ordered `tier = 0..5` with `PASS`,
  `FAIL`, or `SKIPPED`).
  - **Invariant:** `ACCEPTED` requires tier 0 to be `PASS` and tiers 1..4 to
    have no `FAIL`.
- **`CheckpointLineageRecord`** (`openagents.executor.checkpoint-lineage.v1alpha1`):
  immutable quarantine-to-canonical promotion record pinning
  `checkpoint_sha256`, `parent_sha256`, `included_windows` (0..64 digests),
  `shard_hashes` (1..32 digests), `suite_digests` (1..16 digests),
  `promotion_verdict` (`PROMOTED`, `HELD`, `ROLLED_BACK`), and `rollback_from`.
  - **Invariant:** `ROLLED_BACK` requires a 64-hex `rollback_from != checkpoint_sha256`;
    non-rollback records require `rollback_from == ""`; `PROMOTED` requires at
    least one included window.

### 4. Bitcoin-Only Settlement (`settlement.proto`)

- **`VerifiedPayoutIntent`** (`openagents.executor.payout-intent.v1alpha1`):
  idempotent intent keyed by `(receipt_digest, leg)` pinning `verdict_digest`,
  `leg` (`WORKER`, `VALIDATOR`), `amount_msat >= 1`, `network`,
  `payee_pubkey`, `period_id` (`YYYY-MM-DD` UTC), and `owner_policy_digest`.
- **`SettlementReceipt`** (`openagents.executor.settlement-receipt.v1alpha1`):
  reconciled settlement event pinning `receipt_digest`, `leg`, `amount_msat`,
  `network`, `period_id`, `owner_policy_digest`, `real_bitcoin_moved`,
  `settled_at`, and `settlement_commitment`.
  - **Bitcoin-only invariant:** `network` MUST be one of `bitcoin`, `testnet`,
    `signet`, or `regtest`.
  - **No-secret invariant:** Invoices (`lnbc...`, `lntb...`, `lnsb...`),
    preimages, mnemonics, and private keys (`nsec1...`, `xprv...`, `tprv...`)
    are refused in both keys and values with `ExwErr.secret`.
  - **Truthful settlement invariant:** `real_bitcoin_moved == true` requires a
    non-zero 64-hex `settlement_commitment` and `settled_at > 0`;
    `real_bitcoin_moved == false` requires `settlement_commitment == ""` and
    `settled_at == 0`.

### 5. Signed Release Distribution (`release.proto`)

- **`ExecutorReleaseManifest`** (`openagents.executor-release.v1`):
  signed shadow-to-promote manifest pinning `release_id`, `channel`
  (`shadow|beta|stable`), `stage` (`SHADOW|CANARY|PROMOTED|ROLLED_BACK`),
  `checkpoint_sha256`, `lineage_sha256`, `rollback_target_sha256`,
  `max_shadow_disagree_ppm` (`0..10^6`), `min_client_version`,
  `published_at > 0`, `signer_pubkey`, `signature`, and `artifacts` (1..16
  `ReleaseArtifact` items with `target_platform` in
  `macos-arm64|linux-x86_64|linux-arm64|portable`).
  - **Invariant:** `rollback_target_sha256 != checkpoint_sha256`.

## Verification Commands

```sh
scripts/proto.sh generate
scripts/proto.sh check
scripts/proto.sh list
coax/web/tool.sh wire-check
coax/web/tool.sh tools-check
coax/bin/coax test psionic/lib/executor-wire.coax
coax/bin/coax mutate psionic/lib/executor-wire.coax
coax/bin/coax test psionic/lib/executor-wire-goldens.coax
coax/web/tool.sh psionic/lib/executor-wire-goldens.coax
```
