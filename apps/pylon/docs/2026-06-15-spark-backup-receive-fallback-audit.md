# Pylon Spark Backup Receive Fallback Audit

Date: 2026-06-15

## Scope

This audit covers the previous Spark implementation that lived in Pylon and
the adjacent OpenAgents payment surfaces, why it was replaced by LDK and then
MDK, and how to bring Spark back as a narrow backup receive option.

The target reintroduction is intentionally smaller than the old wallet:

- primary Pylon wallet rail remains MDK;
- Spark is a backup receive target when MDK is not online or cannot create a
  receive request;
- fallback funds are collected to a Spark address or Spark receive request;
- Spark does not regain active payout, accepted-work settlement, or public
  payout-target authority without a separate gate.

No raw historical Spark credential material is copied into this document.

## Executive Finding

The old Pylon Spark implementation was real, broad, and already solved useful
receive problems:

- Pylon derived a Spark signer from the local identity mnemonic.
- It opened a Spark wallet with a persistent storage directory under the Pylon
  home.
- It exposed `wallet status`, `wallet balance`, `wallet address`,
  `wallet invoice`, `wallet pay`, and `wallet history`.
- It wrote receive addresses, balances, invoices, and payment summaries into
  the Pylon ledger.
- Later fixes added on-chain deposit visibility, unclaimed deposit inspection,
  manual claim support, and SDK disconnect cleanup.

The old implementation should not be restored wholesale. Its send/payout
surface conflicts with the current MDK-first payment truth and with current
public projection rules. The part worth reviving is a typed, opt-in,
receive-only fallback that can return a local Spark address/request when MDK is
offline, then reconcile or sweep later under the existing legacy Spark migration
consent model.

## External SDK Reality Check

The current Breez SDK Spark documentation still supports the receive modes that
matter for this fallback:

- receive via BOLT11 invoice;
- receive via Bitcoin deposit address;
- receive via static Spark address;
- receive via single-use Spark invoice.

The docs explicitly state that Spark addresses are static and show the
JavaScript receive call as `receivePayment({ paymentMethod: { type:
"sparkAddress" } })`. They also document on-chain deposit lifecycle and
unclaimed deposit handling through `listUnclaimedDeposits`. Relevant sources:

- Breez SDK Spark receiving payments:
  https://sdk-doc-spark.breez.technology/guide/receive_payment.html
- Breez SDK Spark configuration and deposit claim fee controls:
  https://sdk-doc-spark.breez.technology/guide/config.html

This means the backup receive design does not need MDK to be online in order
to hand out a pre-provisioned Spark address. It still needs a periodic Spark
sync path before Pylon can truthfully mark funds as detected, claimable,
credited, swept, or settled.

## Commit History

### Initial Spark-backed product/payment surface

`6403f9566` on 2026-02-16 added `openagents.com` Spark-backed agent payments
and an L402 payer. `afa2f8789` on 2026-02-12 added a Spark NIP-06 desktop L402
payment path. These predate the modern Pylon app shape, but they show the
earliest direction: local identity keys, wallet keys, and agent payment flows
were deliberately tied together.

### Pylon standalone Spark runtime

`1a4ea8960` on 2026-04-05 added the standalone Spark wallet runtime:

- `apps/pylon/src/wallet_runtime.rs`;
- Spark runtime dependencies in `apps/pylon/Cargo.toml`;
- exports from `apps/pylon/src/lib.rs`;
- Pylon docs updates.

The command surface was:

- `wallet status`;
- `wallet balance`;
- `wallet address`;
- `wallet invoice <amount_sats>`;
- `wallet pay <payment_request>`;
- `wallet history`.

The runtime used:

- `openagents_spark::{SparkSigner, SparkWallet, WalletConfig}`;
- `SparkSigner::from_mnemonic(mnemonic, "")`;
- `SparkWallet::new(..., WalletConfig { network, api_key,
  storage_dir })`;
- `wallet.get_spark_address()`;
- `wallet.get_bitcoin_address()`;
- `wallet.create_bolt11_invoice(...)`;
- `wallet.send_payment_simple(...)`;
- `wallet.list_payments(...)`.

The Pylon ledger then stored:

- `runtime_status`;
- `last_error`;
- `network`;
- `last_balance_sats`;
- `last_balance_at_ms`;
- `spark_address`;
- `bitcoin_address`;
- `invoices`;
- `payments`.

This is the core implementation to mine for a backup receive design.

### TUI exposure

`168b1fcde` on 2026-04-05 exposed retained wallet slash commands in the Pylon
TUI. The TUI command list included:

- `/wallet status`;
- `/wallet balance`;
- `/wallet address`;
- `/wallet invoice`;
- `/wallet pay`;
- `/wallet history`.

That was broader than the proposed fallback. The modern replacement should not
bring back `/wallet pay` as Spark send authority. It should expose only a
backup receive target and read-only reconciliation status unless a separate
operator-gated migration action is invoked.

### Release fallback and credential lesson

`783f33d5f` on 2026-04-06 embedded a default Spark API credential so packaged
Pylon releases could boot without shell env injection.

Do not repeat that pattern. A backup receive implementation should require one
of:

- a local configured Spark credential env ref;
- a packaged helper that uses user-provided local credentials;
- an explicitly cached Spark address generated during an earlier online setup.

The public projection should report a credential/readiness blocker ref, never
the credential value.

### Lifecycle cleanup

`20ab4dc6d` on 2026-04-08 disconnected Spark wallet clients after commands.

This matters because a fallback receive helper will likely be invoked as a
short-lived sidecar from the current Bun Pylon CLI. It should initialize,
return status/address/history, and disconnect/exit cleanly. Long-lived Spark
background tasks should not be added to the normal Pylon process until there is
a clear operator need.

### Deposit visibility and recovery

`4383275c1` on 2026-04-08 fixed Spark on-chain deposit visibility and claim
recovery across desktop, Nexus treasury, Pylon, and the shared Spark crate.

The shared Spark wallet code introduced or used:

- `DepositClaimFeePolicy`;
- `list_unclaimed_deposits`;
- `claim_unclaimed_deposit`;
- error classification for maximum-fee-exceeded and missing-UTXO cases;
- `NetworkRecommended { leeway_sat_per_vbyte: 1 }` for mainnet auto policy;
- regtest-friendly fee behavior.

This is directly relevant to a fallback because a Spark address can receive
funds, but the user-facing state machine must distinguish:

- address generated;
- payment detected;
- payment pending;
- on-chain deposit mature;
- claim succeeded;
- claim failed because fee policy blocked it;
- balance credited;
- later migration/sweep to MDK requested;
- migration/sweep completed.

The fallback should not collapse those states into "paid".

### Earnings fallback around wallet outages

`7e2870dec` on 2026-04-09 and `caf0e3b41` on 2026-04-10 fixed Pylon earnings
fallback reporting and preserved persisted provider earnings during wallet
fallbacks.

Those commits are the old warning label: wallet reachability changes should
not erase or rewrite economic truth. For Spark fallback receive, Pylon should
append idempotent local events and keep provider/assignment earnings separate
from wallet receive evidence.

### Transition away from Spark

`27a0a4abf` on 2026-05-16 inventoried Spark touchpoints for the LDK
transition. `ad9da40c7` on 2026-05-17 removed the Spark-shaped Pylon wallet
surface:

- default wallet storage changed from `spark` to `wallet`;
- `spark_sats` became `credited_sats`;
- `spark_address` became payout-destination oriented;
- tests changed from Spark method labels to Lightning/LDK labels.

`9bfd9fb1e` on 2026-05-18 removed Spark from active Nexus and Pylon paths:

- Pylon receive UI changed from "Spark + Bitcoin ready" to
  "LDK payout + Bitcoin ready";
- Spark target inference was removed from the active payout target path;
- Pylon and provider-substrate stopped treating Spark as an active target
  family.

The removal reason was not "Spark can never receive." The removal reason was
that active payout, accepted-work settlement, and public target registration
needed to move to LDK/MDK truth. That leaves room for a receive-only fallback
as long as it is not advertised as settlement authority.

### MDK replacement

`cae2d2d04` on 2026-06-06 wrapped MoneyDevKit agent wallet in Pylon.
`20dd63655` isolated the MDK daemon per Pylon home. The old Rust Pylon wallet
runtime gained:

- `wallet_runtime_kind = moneydevkit`;
- a per-Pylon MDK home under wallet storage;
- a stable per-Pylon MDK port;
- MDK `balance`, `receive`, `receive-bolt12`, and `send` subprocess calls;
- redaction for MDK command output.

The current Bun Pylon wallet layer keeps the same policy shape but in a much
smaller form:

- `classifyMdkWallet()` calls `@moneydevkit/agent-wallet balance`;
- `receiveWithMdk(amount)` calls `@moneydevkit/agent-wallet receive`;
- `sendWithMdk(destinationRef, amount)` calls `@moneydevkit/agent-wallet send`;
- raw receive targets and payment outputs become stable
  `wallet.mdk_receive_target.*` and `wallet.payment.*` refs;
- public readiness posts include `wallet.public.mdk.*` refs and blocker refs,
  not raw payment material.

This public-safe ref pattern should be reused for Spark fallback.

### Legacy Spark migration

`073d6f3f6` on 2026-06-10 added the current `pylon wallet migrate-spark`
preflight. `48e12a29f` added local mnemonic recovery. `372e54742` guided
legacy Spark recovery when the old helper reports a missing Breez/Spark API
key.

Current behavior in `apps/pylon/src/wallet.ts`:

- probes `PYLON_LEGACY_SPARK_HELPER` or `spark-wallet-cli`;
- accepts hints such as `PYLON_LEGACY_SPARK_BALANCE_SATS`;
- checks for `OPENAGENTS_SPARK_API_KEY`, `BREEZ_API_KEY`, or an explicit
  `PYLON_LEGACY_SPARK_CREDENTIAL_READY`;
- produces a public-safe
  `openagents.pylon.legacy_spark_migration.v0.3` projection;
- blocks on missing identity/mnemonic recovery, missing credential, failed
  helper init, missing spendable balance, or missing MDK destination invoice;
- requires explicit consent before reporting migration as executed;
- emits only public receipt refs.

This is already the right sweep/migration boundary. The backup receive fallback
should feed into it instead of bypassing it.

## Current Gaps

The current Pylon source has legacy migration support, but no receive fallback.
If `receiveWithMdk()` fails because MDK is offline, `wallet receive --amount`
returns a `wallet.receive_failure.*` ref and stops.

Missing pieces:

- no `SparkBackupReceiveProjection`;
- no Spark helper command for `address` or static `sparkAddress`;
- no local cache of the fallback Spark address/request;
- no fallback chooser that tries MDK first and Spark second;
- no explicit public-safe ref for "fallback receive target generated";
- no reconciliation loop that classifies Spark fallback funds as detected,
  pending, claimable, credited, or sweep-needed;
- no tests proving raw Spark addresses do not leak into public readiness,
  Forum, assignment, or public stats projections.

## Recommended Reintroduction Boundary

Add Spark back as a receive-only backup rail, not as a payout rail.

New local concept:

```ts
type SparkBackupReceiveState =
  | "disabled"
  | "credential-missing"
  | "helper-unavailable"
  | "address-ready"
  | "cached-address-ready"
  | "receive-selected-mdk-offline"
  | "payment-detected"
  | "claim-pending"
  | "claim-blocked"
  | "credited"
  | "sweep-to-mdk-recommended"
  | "swept-to-mdk"
```

New projection shape:

```ts
type SparkBackupReceiveProjection = {
  schema: "openagents.pylon.spark_backup_receive.v0.1"
  enabled: boolean
  state: SparkBackupReceiveState
  selectedBecauseRefs: string[]
  receiveTargetRef: string | null
  rawTargetAvailableLocally: boolean
  credentialReady: boolean
  helperReady: boolean
  detectedBalanceSats: number | null
  unclaimedDepositCount: number | null
  blockerRefs: string[]
  nextActionRefs: string[]
  publicReceiptRefs: string[]
  contentRedacted: true
}
```

Important details:

- `receiveTargetRef` is a hash ref such as
  `wallet.backup.spark_address.<digest>`.
- The raw Spark address/request may be shown only in local CLI/TUI surfaces
  that are explicitly marked local/private.
- The raw Spark address/request must not be posted to
  `/api/pylons/{pylonRef}/wallet-readiness`, public stats, Forum posts,
  assignment closeouts, or product-promise evidence.
- `PayoutTargetKind` should not gain `spark_address` unless the payout and
  settlement authority model is also changed. Use a separate backup receive
  target, not `admitPayoutTarget`.
- Spark send remains disabled except inside the explicit `migrate-spark`
  sweep path.

## Suggested Command Shape

Keep the operator path small:

```sh
pylon wallet receive --amount 1000
pylon wallet backup-receive --kind spark-address
pylon wallet backup-receive --kind spark-address --show-local-target
pylon wallet backup-status
pylon wallet migrate-spark --destination-invoice-ready --yes --execute
```

Behavior:

1. `wallet receive --amount` tries MDK first.
2. If MDK succeeds, return the current MDK `wallet.mdk_receive_target.*` ref.
3. If MDK fails with a daemon/offline/init timeout class, check Spark backup
   readiness.
4. If Spark backup is enabled and an address/request is available, return:

   - `ok: true`;
   - `rail: "spark_backup"`;
   - `receiptRef: "wallet.backup_receive.<digest>"`;
   - `rawTargetAvailableLocally: true`;
   - `mdkFailureRef: "wallet.receive_failure.<digest>"`.

5. If `--show-local-target` is present, print the raw Spark address/request to
   the local terminal only. Do not include it in any public projection or
   network post.
6. `backup-status` reconciles Spark helper status, balance, payment history,
   and unclaimed deposits, and reports only public-safe refs unless
   `--show-local-target` is explicitly set.
7. `migrate-spark` remains the consented path for moving old Spark balance into
   MDK.

## Helper Strategy

The historical implementation was Rust. The current Pylon app is Bun/TypeScript
and the repo guidance says not to reintroduce the old Cargo workspace unless
explicitly needed.

The lowest-risk implementation is one of these:

1. Prefer a small Bun/TypeScript Spark helper using the current Breez SDK Spark
   JavaScript package, if it works under the packaged Pylon runtime.
2. Otherwise ship an isolated `spark-wallet-cli` helper as an optional support
   binary, following the existing `PYLON_LEGACY_SPARK_HELPER` convention.

Do not revive the old monorepo-wide Rust Spark crate as a normal dependency
unless the JavaScript package or isolated helper cannot satisfy:

- static Spark address generation;
- status/balance;
- payment history;
- unclaimed deposit listing;
- explicit claim/sweep support.

## State And Storage

Use Pylon home, not public state:

- Spark backup config:
  `<pylon-home>/wallet/spark-backup/config.json`
- Spark SDK storage:
  `<pylon-home>/wallet/spark-backup/sdk/`
- cached local target:
  `<pylon-home>/wallet/spark-backup/receive-target.json`
- local reconciliation ledger:
  existing Pylon ledger events with new backup receive kinds.

Private files should follow the existing Pylon private-directory/file
discipline. Any file that can unlock or operate the wallet must be mode 0600 or
equivalent where supported.

## Public Projection Policy

Allowed public refs:

- `wallet.backup.spark_address.<digest>`;
- `wallet.backup.receive_selected.<digest>`;
- `wallet.backup.spark_detected.<digest>`;
- `wallet.backup.spark_claim_blocked.<digest>`;
- `receipt.pylon.legacy_spark_migration.<digest>`;
- blocker refs such as
  `blocker.wallet.spark_backup.credential_missing`,
  `blocker.wallet.spark_backup.helper_unavailable`, and
  `blocker.wallet.spark_backup.sync_unavailable`.

Forbidden public content:

- raw Spark addresses;
- raw Spark invoices;
- raw BOLT11 invoices;
- payment hashes;
- payment preimages;
- mnemonics;
- Breez/Spark API keys;
- SDK storage paths;
- local wallet home paths;
- raw helper stdout/stderr.

## Failure Modes

1. MDK offline, Spark helper ready:
   fallback receive may provide the local Spark target and emit a redacted
   backup receive ref.

2. MDK offline, Spark helper unavailable, cached Spark address exists:
   fallback may show the cached target locally, but status must say
   `cached-address-ready` and cannot claim fresh sync.

3. MDK offline, Spark credential missing:
   fail with `blocker.wallet.spark_backup.credential_missing`.

4. Spark payment detected, claim not final:
   show pending/detected, not paid or settled.

5. On-chain deposit mature but claim blocked by fees:
   show `claim-blocked` with a fee-policy blocker, not credited.

6. Spark balance credited, MDK back online:
   recommend `migrate-spark`, but require explicit consent before moving funds.

7. Spark send/migration failure:
   keep the Spark balance state and record a public-safe failure ref; never mark
   MDK settlement as completed.

## Test Plan

Add unit tests in `apps/pylon/tests/wallet.test.ts`:

- MDK receive success does not call Spark backup helper.
- MDK daemon offline selects Spark backup receive when enabled.
- MDK non-offline validation errors do not silently switch rails.
- Spark helper missing returns a typed blocker.
- Missing Spark credential returns a typed blocker.
- Cached Spark address works as `cached-address-ready` when helper is offline.
- `--show-local-target` is required before raw target output is allowed.
- `assertPublicProjectionSafe` rejects projections containing raw Spark
  address/request material.
- Backup receive ledger events are idempotent.
- Detected Spark balance recommends `migrate-spark` but does not mark
  settlement.
- Migration still requires destination readiness and explicit consent.

Add an integration smoke only after the helper is selected:

- create Spark address/request on a local test wallet;
- simulate MDK offline;
- route `wallet receive` to Spark backup;
- sync Spark status;
- prove public output includes only refs and blocker/action refs.

## Product Promise Impact

This fallback should not flip broad payment promises green.

Possible new scoped promise:

`pylon.spark_backup_receive.v1`: Pylon can expose a local Spark backup receive
target when MDK receive is unavailable, while public projections reveal only
redacted refs and funds remain non-settled until Spark sync plus migration or
explicit settlement evidence.

Claims it must not make:

- "Pylon Spark payouts are live";
- "Spark fallback funds are settled in MDK";
- "accepted work is paid";
- "wallet is send-ready";
- "public stats contain direct payout target";
- "offline receive is confirmed before Spark sync evidence exists".

## Implementation Checklist

1. Add `SparkBackupReceiveProjection` and helper runner interfaces to
   `apps/pylon/src/wallet.ts`.
2. Add a narrow Spark helper command contract:
   `status`, `address`, `history`, `unclaimed-deposits`.
3. Add fallback classification:
   MDK offline timeout/error -> Spark backup; MDK validation/user error -> no
   fallback.
4. Add `wallet backup-receive` and `wallet backup-status` CLI commands.
5. Update `wallet receive` to choose MDK first and Spark backup second only for
   offline/unavailable classes.
6. Store raw Spark target only in local private state.
7. Publish only refs and blockers.
8. Keep `admitPayoutTarget` unchanged; do not add Spark to
   `PayoutTargetKind`.
9. Reuse `migrate-spark` for sweep-to-MDK, preserving destination readiness and
   consent.
10. Add the tests listed above.
11. Add a short runbook next to `legacy-spark-wallet-migration.md` after the
    helper implementation exists.

## Bottom Line

Bring Spark back as a backup receive target, not as a second active settlement
system. The historical code gives us the working parts: identity-derived
wallet, static Spark address, status/history, deposit claim handling, and
ledger sync. The modern MDK code gives us the safety pattern: public refs,
typed blockers, no raw payment material in projections, and explicit consent
before money moves between rails.

The correct next patch is a receive-only Spark backup projection plus helper
contract in `apps/pylon/src/wallet.ts`, with MDK-first fallback selection and
tests proving Spark cannot leak into public payout authority.
