# Pylon v0.2 Release Candidate Verification

Date: 2026-06-07

## Scope

This report records the release-candidate verification run for the
MDK-default Pylon v0.2 rollout. It does not revive the old GCP/native Nexus
lane as a default blocker. The accepted payment-control plane is Omega on
Cloudflare with the MDK sidecar proof recorded in Omega and summarized in
`docs/reports/nexus/2026-06-07-pylon-v02-production-blockers.md`.

## Source State

- OpenAgents branch: `main`
- Workspace package version: `0.2.2`
- Pylon npm bootstrap package version: `0.2.2`
- Live Artanis SHC proof:
  `docs/reports/nexus/2026-06-07-pylon-v02-live-artanis-shc-bootstrap-proof.md`
- Existing Omega MDK proof: 100-bitcoin-sat checkout, local MDK wallet payment,
  merchant status `PAYMENT_RECEIVED`, payer balance delta.

## Green Checks

Repo-level Pylon checks:

```bash
cargo test -p pylon -- --nocapture --test-threads=1
cargo clippy -p pylon --all-targets -- -D warnings
cargo test -p pylon pylon_probe --lib -- --nocapture
cargo test -p pylon probe_agent --lib -- --nocapture
cargo test -p openagents-provider-substrate -- --nocapture
cargo clippy -p openagents-provider-substrate --all-targets -- -D warnings
cargo test -p autopilot-desktop provider_admin -- --nocapture
cargo check -p autopilot-desktop
cargo clippy -p autopilot-desktop --all-targets -- -D warnings
scripts/lint/ownership-boundary-check.sh
scripts/lint/workspace-dependency-drift-check.sh
scripts/pylon/verify_standalone.sh
scripts/pylon/verify_nip90_wallet.sh
```

Observed results:

- Pylon serial suite: 315 library tests passed, 6 CLI-path tests passed,
  `cloud_node_v1_fixture` passed, heavy LDK regtest harness remained ignored.
- Pylon clippy: passed with `-D warnings`.
- Pylon probe-focused tests: passed.
- Provider substrate tests and clippy: passed.
- Autopilot desktop provider-admin check, `cargo check`, and clippy: passed.
- Ownership/dependency lint scripts: passed.
- Standalone verifier: passed.
- Retained NIP-90/wallet verifier: passed.

The parallel `cargo test -p pylon -- --nocapture` run exposed a single
parallel-state collision in
`gemma_diagnose_persists_latest_report_with_runtime_metrics`; the same test
passed in isolation and the full Pylon suite passed with `--test-threads=1`.
The release evidence therefore uses the serial full-suite command above.

Artanis/Pylon bootstrap checks:

```bash
(cd ../cloud && cargo test -p openagents-cloud-contract artanis_bootstrap_assignment_fixture_parses_and_validates)
(cd ../cloud && cargo test -p oa-codex-control artanis_bootstrap)
```

Observed results:

- Cloud contract Artanis fixture test: passed.
- `oa-codex-control` Artanis bootstrap tests: 2 passed.

Sandbox-specific checks:

```bash
cargo --manifest-path ../psionic/Cargo.toml test -p psionic-sandbox execution::tests::policy_rejection_is_receipted -- --nocapture
cargo --manifest-path ../psionic/Cargo.toml test -p psionic-sandbox execution::tests::local_subprocess_success_emits_receipt_and_artifacts -- --nocapture
cargo test -p pylon sandbox_reports_surface_profiles_status_and_failures -- --nocapture
cargo test -p autopilot-desktop snapshot_signature_changes_when_sandbox_truth_changes -- --nocapture
```

Observed results:

- Both Psionic sandbox execution proof tests passed.
- Pylon sandbox status projection test passed.
- Autopilot desktop sandbox signature projection test passed.

MoneyDevKit wrapper checks:

- A two-home Pylon wrapper smoke passed with `runtime.runtime_kind=moneydevkit`
  on both homes.
- The two homes exposed distinct `runtime.local_daemon_port` values:
  `39397` and `44854`.
- Both homes returned JSON status, balance, history, BOLT11 invoice, and
  BOLT12 offer responses.
- No mnemonic, preimage, full invoice, or full offer was committed.
- Temporary smoke daemons were stopped after the run.

## Candidate Fixes Included

- `scripts/pylon/verify_standalone.sh` now runs the explicit `pylon` binary,
  uses a random loopback admin port, asserts the current launch products, and
  checks `provider_mode` for lifecycle transitions.
- `scripts/pylon/verify_nip90_wallet.sh` now runs the explicit `pylon` binary.
- `wallet status --json` now includes non-secret
  `runtime.local_daemon_port` for the MoneyDevKit wrapper, giving CLI-level
  proof that two Pylon homes do not share one MDK daemon.
- Stale docs now identify the MDK/Omega Cloudflare path as the current release
  path and scope old GCP/native Nexus work to explicit native-LDK tasks.

## Release Publication Follow-Up

After this release-candidate report, the public binary release was patched to
`pylon-v0.2.2` and verified from public archives.

1. Commit and push the release candidate to `origin/main`. Done for the first
   candidate:
   `f836eb909a9ce323b4097b30a01b6e358ec03fed`.
2. Patch and publish the stable GitHub binary release after packaging defects
   were found. Done:
   `https://github.com/OpenAgentsInc/openagents/releases/tag/pylon-v0.2.2`.
3. Verify fresh public-style installs from the released assets. Done:
   `docs/reports/nexus/2026-06-07-pylon-v02-release-publication-proof.md`.
4. Publish `@openagentsinc/pylon@0.2.2` when npm publish authorization is
   available. Done after operator completed npm CLI authorization.
5. Verify a fresh npm bootstrap path after the npm package is published. Done
   on SHC with isolated `HOME`, npm cache, install root, and Pylon home; see
   `docs/reports/nexus/2026-06-07-pylon-v02-release-publication-proof.md`.
6. Keep the post-release paid-work claim bounded: the current evidence is
   Omega/Cloudflare MDK checkout proof plus public-release local proof-runtime
   accepted-work closeout. Do not claim real public Bitcoin settlement for
   Artanis-dispatched work until the settlement bridge is explicitly completed.
