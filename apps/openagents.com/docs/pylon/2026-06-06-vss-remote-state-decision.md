# VSS Remote State Decision

Issue #354 / `OPENAGENTS-L-006` evaluates Versioned Storage Service as an optional
Pylon wallet backup/sync backend.

## Decision

Treat VSS as **optional remote wallet-state infrastructure**, not as a silent
replacement for Pylon encrypted backups.

Pylon should keep the local encrypted backup contract as the user-visible
baseline. A VSS-backed mode can be added later only when it is explicit,
opt-in, observable, recoverable, and tested on signet/regtest with stale-state
and restore drills.

## Reviewed Local Sources

- `/Users/christopherdavid/work/projects/moneydevkit/repos/vss-server/README.md`
- `/Users/christopherdavid/work/projects/moneydevkit/repos/ldk-node/src/builder.rs`
- `/Users/christopherdavid/work/projects/moneydevkit/repos/lightning-js/src/lib.rs`
- `/Users/christopherdavid/work/projects/moneydevkit/repos/mdkd/README.md`
- `/Users/christopherdavid/work/openagents/apps/pylon/src/wallet_runtime.rs`
- `/Users/christopherdavid/work/openagents/apps/pylon/src/wallet_harness.rs`
- `/Users/christopherdavid/work/openagents/apps/pylon-tui/src/lib.rs`
- `/Users/christopherdavid/work/openagents/docs/2026-05-15-ldk-nexus-treasury-transition-audit.md`
- `/Users/christopherdavid/work/projects/mutiny/2026-05-15-mutiny-lessons-for-ldk-nexus-treasury-audit.md`

## Source Summary

`vss-server/README.md` describes VSS as remote, versioned storage for
non-custodial Lightning wallet state, with recovery and future multi-device
support as the main use case. It emphasizes client-side encryption, key
versioning, authorization, HTTPS, rate limiting, and the ability to self-host or
use a third-party provider.

The MoneyDevKit `ldk-node` fork exposes `build_with_vss_store` and related
builders. The same builder comments mark VSS support as alpha/experimental and
warn that unrecoverable remote persistence failures can make LDK panic after
internal retries are exhausted.

Current Pylon wallet code already has an explicit local backup surface:

- encrypted wallet backup export;
- backup inspection;
- backup restore;
- phrase restore;
- backup manifest status;
- stale backup warnings;
- explicit `--yes` gates in the TUI command surface; and
- telemetry fields for backup readiness, manifest presence, artifact count,
  stale state, last export time, and last file digest.

That baseline should remain visible even if VSS is added later.

## Required State Scope

A future VSS adapter may store only wallet-state material that the native
Pylon/LDK runtime already needs to persist:

- channel monitor state;
- LDK node key-value state;
- pathfinding/scoring state if the chosen runtime persists it there;
- wallet metadata needed for recovery;
- backup status metadata; and
- redacted restore evidence.

OpenAgents product surface must not store that material. OpenAgents product surface should receive only redacted
projection refs and readiness state.

## Encryption And Secret Boundary

VSS must be treated as remote storage of sensitive wallet state, even if
client-side encryption is used.

The following are never allowed in OpenAgents product surface, public docs, receipts, issue
comments, agent manifests, customer-visible surfaces, or logs:

- recovery phrases;
- wallet entropy;
- private keys;
- raw channel monitor state;
- raw VSS payloads;
- VSS auth headers;
- wallet credentials;
- payment preimages;
- raw invoices;
- raw payout targets; or
- unredacted payment identifiers.

Pylon may display safe backup state such as:

- `local_backup_current`;
- `local_backup_stale`;
- `vss_disabled`;
- `vss_configured`;
- `vss_sync_fresh`;
- `vss_sync_stale`;
- `vss_sync_failed`;
- `restore_drill_passed`; and
- `restore_drill_missing`.

## Restore Semantics

VSS cannot be marketed as "backup handled" unless restore is explicit and
tested.

A future restore flow should require:

1. user consent before enabling VSS;
2. clear disclosure that Lightning channel state is not the same as a simple
   phrase restore;
3. a local encrypted backup export before enabling VSS for mainnet;
4. a restore drill on signet/regtest before production defaulting;
5. stale-state detection before starting a restored wallet;
6. a single-writer lock so two active runtimes never use the same wallet state;
7. operator-visible failed persistence and failed restore states; and
8. a local rollback or recovery runbook when remote persistence fails.

Pylon should continue to say whether local backup is current, stale, or missing
even when VSS is configured.

## Failure Modes To Surface

The user and operator surfaces need explicit states for:

- VSS disabled;
- VSS configured but never synced;
- VSS sync stale;
- VSS auth failed;
- VSS unavailable;
- VSS write conflict;
- VSS version conflict;
- VSS restore attempted;
- VSS restore blocked by stale local state;
- VSS restore succeeded;
- VSS restore failed;
- local backup missing;
- local backup stale; and
- single-writer lock conflict.

These are not generic warning strings. They are product states that determine
whether the wallet can safely start, receive, send, or restore.

## User Consent And Migration

VSS should be opt-in for early Pylon releases.

Migration should be staged:

1. Keep local encrypted backup as mandatory.
2. Add a read-only VSS readiness projection.
3. Add signet/regtest VSS sync and restore harnesses.
4. Add explicit Pylon config fields for VSS endpoint, auth mode, store id, and
   backup policy.
5. Require a local encrypted backup before first VSS enablement.
6. Require a one-time user acknowledgement explaining remote wallet-state risk.
7. Only after successful drills, allow VSS as an optional production backend.

## Operator Visibility

Operator projection should show:

- VSS mode;
- auth mode;
- provider kind;
- last successful sync display time;
- last failed sync display time;
- restore drill status;
- backup freshness;
- conflict count;
- blocked reason refs; and
- remediation refs.

Public/customer/agent projections should hide provider details and show only
coarse readiness state.

## OpenAgents Boundary

Pylon owns the wallet runtime and restore flow.

Nexus and Treasury own payout acceptance, payout dispatch, settlement
reconciliation, and receipts.

OpenAgents product surface owns public-safe projection, docs, order/Site receipt linking, and
operator UI. OpenAgents product surface must not become a wallet-state store or VSS proxy unless a
later issue explicitly defines that authority and redaction model.

## Roadmap Implication

This issue closes the VSS decision gate only. It does not add VSS code or
change wallet persistence.

The next implementation work should continue with read-only payout and graph
projection issues:

- #355: accepted-work payout SLO projection;
- #356: safe public accepted-work payout rows;
- #357: read-only Lightning/Pylon graph API contract; and
- #358: accepted-work proof links in Sites/order receipts.

Later Pylon/Rust work should add a VSS signet/regtest harness, single-writer
restore policy, and explicit opt-in config before any production wallet uses
remote persistence.
