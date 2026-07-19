# OpenAgents Desktop owned release coordinator

Status: implemented coordinator core. Production promotion remains blocked by
native-host, signing, and feed-integration evidence tracked in
[#8917](https://github.com/OpenAgentsInc/openagents/issues/8917).

This document describes the production boundary implemented by
`scripts/desktop-release-coordinator.ts`. The root release command still uses
fixture ports. It will use them until the owned-worker and `oa-updates` adapters
are connected. Thus, this module is not yet a live release path. It does not claim that
the current worker inventory can complete or promote a release.

## Authority and invariants

The coordinator consumes one frozen release authority. It contains the source
revision, version, channel, and canonical five-target set. It contains the
staging-ledger ref, toolchain profile, and signing policy. It also contains the
reviewed release-notes digest. The
authority is hashed once. Every dispatch, lease, worker receipt, candidate
handoff, candidate-feed acceptance, and promotion is bound to that digest.

The normative target profile remains:

- `darwin-arm64`: DMG and ZIP
- `darwin-x64`: DMG and ZIP
- `win32-x64`: NSIS
- `linux-arm64`: AppImage, DEB, and RPM
- `linux-x64`: AppImage, DEB, and RPM

Windows is x64-only in the current ProductSpec. `win32-arm64` is not a
ReleaseSet target and cannot be promoted. Unavailable Intel Mac evidence,
missing signing operation, duplicate worker, or mismatched toolchain profile produces a
typed `worker_inventory_unavailable` refusal before worker bring-up.

## Execution boundary

`createOwnedReleaseCoordinator` implements the real `ReleaseCoordinatorPort`
contract defined by `scripts/release.ts`. The release CLI does not construct it
yet. Wiring the following concrete capabilities is required before a real run:

1. worker inventory.
2. worker start, health, heartbeat, dispatch, cancellation, and stop control.
3. coordinator request signer and pinned worker receipt keyring.
4. immutable candidate object HEAD verifier.
5. verified-candidate publisher.
6. external candidate-feed acceptance gate.
7. atomic channel-pointer compare-and-swap promoter.

No build worker receives pointer authority. A worker may upload only immutable
candidate objects and return their identities in its signed receipt.

The worker dispatch request carries a bounded lease ID, monotonic attempt, and
expiry. It carries the exact target and formats. It carries the frozen
authority, plan digest, and detached coordinator signature. The result is
accepted only when its pinned Ed25519 signature verifies. It must match the live
transaction, lease, attempt, worker, target, source, version, and channel. It
must also match the staging ledger, toolchain, and signing policy.

Every target receipt must list its canonical formats. It must provide unique
immutable object keys, hashes, and byte lengths. It must provide component-ledger,
build, and signing refs. It must provide native proofs for installation,
launch, agent runtime, shutdown, and update. It must prove interruption and
resume, the rollback boundary, reinstallation, and uninstallation. Missing, stale, duplicated, noncanonical, unsigned,
or conflicting evidence is refused.

## Durability, retry, and cleanup

`FileCoordinatorStateStore` writes canonical JSON through an exclusive lock,
fsynced temporary file, atomic rename, and parent-directory fsync. Every write
is a compare-and-swap against the prior revision. A one-minute-old lock is
treated as a crash remnant because the guarded operation contains no network
work and is synchronous. A fresh lock always refuses a competing writer.

The five dispatches execute concurrently. State transitions are serialized
through the durable CAS store. A failed attempt is cancelled and may retry only
under a new monotonic lease. Restarted coordinators revalidate persisted worker
signatures and lease bindings before reusing completed cells, so completed
targets are not rebuilt and tampered state is not trusted.

All workers are stopped after candidate handoff and after any terminal fan-out
failure. This keeps the documented GCE workers stopped outside bounded work.
Promotion does not require workers to remain online.

## Candidate and promotion boundary

Candidate publication is impossible before exact five-target/eleven-artifact
convergence and native/signing prerequisite gates. Immediately before handoff,
the coordinator re-HEADs every immutable object and compares its length and
SHA-256 with the signed worker receipt.

Promotion then requires a separate candidate-feed acceptance receipt bound to
the transaction, plan digest, matrix digest, candidate ref, and ReleaseSet
payload digest. The promoter receives the expected previous pointer and uses a
single compare-and-swap. A race or any missing precondition leaves the current
channel pointer unchanged.

## Current infrastructure truth

The infrastructure sub-workstream recorded three stopped, toolchain-provisioned
GCE build workers in project `openagentsgemini`: Linux x64, Linux arm64, and
Windows x64. These are build substrate, not native install/update proof by
themselves. The local Apple Silicon Mac is a Darwin arm64 candidate worker.

The following prevent an honest production matrix today:

- no accepted native Intel-mac receipt path (the intended Intel host is not
  currently accessible. Rosetta is not a silent substitute).
- no completed Windows Authenticode operation exposed to its worker.
- no completed Apple signing/notarization operation exposed to its worker.
- missing native Windows 10/11, Ubuntu 22.04 desktop, and RPM-family acceptance
  receipts.
- the real `#8922` candidate feed/acceptance/promotion adapters have not yet
  supplied a production candidate receipt to this coordinator.

Until those are resolved, the correct result is a typed refusal and an
unchanged public channel pointer. Fixture convergence proves coordinator
behavior, not platform support.

## Verification

The focused deterministic suite is:

```sh
vp test --run scripts/desktop-release-coordinator.test.ts scripts/release.test.ts
```

It covers exact convergence, concurrent fan-out, lost-worker retry, and
cancellation. It covers restart resume, stale leases, wrong source, and
duplicate formats. It covers invalid signing refs, unavailable native hosts,
and immutable-object byte drift. It covers candidate refusal, promotion races,
frozen-plan conflicts, and atomic store revision conflicts.
