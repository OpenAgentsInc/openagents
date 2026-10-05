# Verse operator and recovery acceptance

V19 is complete for the dedicated TLS chamber host in
[#10735](https://github.com/OpenAgentsInc/openagents/issues/10735).
The [final acceptance manifest](acceptance-main.json) binds source, executables,
test logs, and structured receipts after main integration.
[Source metadata](source-main.json) records base
`992e3d6621b8fc04864a0a5ee5c6992e55d8fcb9`, the local build revision, a compressed
source patch, and 207 source/input hashes. The [initial acceptance](acceptance.json)
and [initial guide](initial-README.md) retain the earlier 562-test run at
`ad9f62ed8829bd2d5f092b1116e4996036987e51`. Documentation and evidence amendments
leave those Rust source hashes unchanged.

The [final integration record](final-integration.json) verifies the subsequent
`67ae7561b5e1c40fcc0d06bfd741ef283fca8c98` workspace reorganization. It changes
only workspace profile/package records outside the tested dependency graph; all
owning source, manifests, the toolchain, and the fixture remain unchanged.

## Accepted checks

```sh
CARGO_TARGET_DIR=~/work/openagents-target-agent1 CARGO_BUILD_JOBS=2 \
  cargo test -p verse-world --features service-net --lib -- --nocapture
CARGO_TARGET_DIR=~/work/openagents-target-agent1 CARGO_BUILD_JOBS=2 \
  cargo test -p verse-host -- --nocapture
cargo fmt --all --check
```

Use your own existing target directory outside the checkout. Run the package
checks sequentially. A scratch checkout at the recorded base plus
[source-main.patch.gz](source-main.patch.gz) reproduces the source delta; decompress it
and apply it with `git apply`. The final source/input hashes must match
`source-main.json`. Executable hashes describe the recorded local build.

| Check | Result |
| --- | --- |
| World crate | 572 passed; three ignored child helpers run through parent crash tests. |
| Dedicated host unit tests | Four passed. |
| Dedicated host integration | Passed; two original worlds, with actual CLI operations and durable recovery for the ritual. |
| Formatting | Passed. |

The [world receipts](world-main-receipts.json) retain the live stall, exact recovery,
and five restore crash-boundary results. The [host receipts](host-main-receipts.json)
retain actual status, export, verify, restore, and final command results. Full
accepted output is in [world-main-tests.log](world-main-tests.log) and
[host-main-tests.log](host-main-tests.log).

## Observations and limits

The live scratch TLS run reported one authenticated idle client, one pending
handshake, two writer copies, a 1,864 ms oldest pending commit, and a 47.81 ms
injected expensive tick. Readiness was false while stalled. Drain retained the
writer lock until completion, then recovered authority tick 4 and revision 6.
A work-budget test exhausted projections while preserving redacted client
observations. Tests refuse incompatible build/wire identity and stale or offline
readiness, and retain only the latest sample and 128 transition records.

A 104-file, 132,075-byte backup restored all 300 reward transactions, matching
character state and the original retry receipt. Restores refuse mismatched
content/instance, corruption, symlinks, existing destinations, and an active
writer. Process death at `reserved`, `copied`, `durable`, `before_publish`, and
`published` retains a verified source. Each retry recovers all 140 transactions;
partial destinations cannot start a host. Even the published boundary holds the
writer lock until restore exits. Migration archives survive export and restore,
retain reviewed rollback, and refuse rollback after later progress. Offline
pruning preserves current and migration receipt roots.

These are Linux scratch correctness checks. The 45 ms sleep and blocked writer
are injected faults, not production performance measurements. The backups cover
one chamber store, not coordinated realm transfers or account registries.
Backup age determines lost progress; no production recovery-time or availability
claim follows. Use the [host operations guide](../../../../crates/verse-host/README.md#local-operations)
for budgets, privacy, storage-failure behavior, drain completion, and commands.
No owner device, deployment, release gate, or external live engine was used.

## Retained development failures

[host-development-check.log](host-development-check.log) records a test assertion
that called a private method; the corrected integration compares complete public
checkpoints. [restore-development-check.log](restore-development-check.log)
records a hard-coded actor ID in a test; the corrected check derives its actor
from the fixture. Both corrected paths pass in the accepted logs.

[combined-build-no-space.log](combined-build-no-space.log) records a combined
package build that exhausted disk before running tests. The
[obsolete executable record](obsolete-executables.json) lists the completed,
unused audit executables removed after checking their paths, hashes, and running
inodes. Warm caches remain. The accepted separate package checks pass afterward.
