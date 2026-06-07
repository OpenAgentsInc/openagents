# Pylon Verification Matrix

This document is the release gate for standalone `Pylon`.

It exists to answer one question honestly:

**Can a new operator install, run, verify, and reason about Pylon without `Autopilot`, while preserving the launch truth of the OpenAgents Compute Market?**

## Launch Truth Checklist

These statements must remain true in docs, code, and operator guidance:

- `Pylon` is a standalone provider connector, not the whole product.
- The market is still the **OpenAgents Compute Market**.
- Launch compute product families are `inference` and `embeddings`.
- The current standalone `Pylon` operator lane is one honest local Gemma inference product.
- Non-Gemma local models must not make the node look sellable.
- Raw accelerator trading is not live at launch.
- Capability-envelope fields refine supply; they are not the primary product identity.
- `sandbox_execution` is the next planned family, not the current generally released launch family.

If any release note, marketing text, or operator doc violates one of those statements, the rollout should stop.

## Deterministic Matrix

| Area | Verification Path | Expected Result |
| --- | --- | --- |
| Release asset help | download the matching release archive, extract it, run `./pylon --help` outside the repo | headless CLI runs without a Rust toolchain |
| Release asset init | download the matching release archive, extract it, run `./pylon init` and `./pylon status --json` outside the repo | config and identity are created without a source checkout |
| Install/init | `cargo run -p pylon -- init` | config and identity are created without `Autopilot` |
| Unconfigured truth | `cargo run -p pylon -- status` before init | reports `unconfigured` |
| Backend visibility | `cargo run -p pylon -- backends --json` | returns a `local_gemma` backend entry plus any declared sandbox backend state |
| Launch products | `cargo run -p pylon -- products --json` | shows the canonical Gemma inference product and does not expose the legacy Apple FM lane |
| Sandbox status truth | `cargo run -p pylon -- status` on a node with declared sandbox profiles | status includes sandbox execution classes, profile IDs, and scan/runtime errors when relevant |
| Sandbox runtime/profile view | `cargo run -p pylon -- sandbox --json` | returns declared runtimes/profiles, ready execution classes, and profile digests |
| Inventory | `cargo run -p pylon -- inventory` | shows inventory rows with capability summaries and explicit eligibility |
| Probe capability | `cargo run -p pylon -- doctor` with `PROBE_ADMIN_CHAT_BRIDGE_SECRET` set | advertises `probe_agent` only when the Probe CLI, signed bridge, backend profile, bridge secret, and workspace mapping are ready |
| Jobs | `cargo run -p pylon -- jobs` | shows recent jobs or an empty truthful list, including sandbox family/profile/failure detail when present |
| Earnings | `cargo run -p pylon -- earnings` | shows earnings summary or explicit none-state |
| Receipts | `cargo run -p pylon -- receipts` | shows receipt summaries or an empty truthful list, including sandbox profile/termination/failure detail when present |
| Activity | `cargo run -p pylon -- activity` | shows retained relay and settlement activity, or an explicit none-state |
| Regtest config surface | `cargo run -p pylon -- config set wallet_network regtest` then `config show` | persists `regtest` as the retained wallet network without changing unrelated local state |
| Wallet runtime selection | `cargo run -p pylon -- wallet status --json`, then `cargo run -p pylon -- config set wallet_runtime_kind mock` and re-run status | reports `runtime.runtime_kind=moneydevkit` by default and `runtime.runtime_kind=mock` after the explicit test override; `external_target` is only an advanced migration override |
| Wallet runtime lifecycle | `cargo run -p pylon -- wallet start --json`, `wallet restart --json`, and `wallet stop --json` on a mock home and an MDK smoke home | returns structured lifecycle reports through the selected Pylon wallet runtime, allows agents/operators to control the wrapped MDK daemon without calling MDK directly, and leaves no temp smoke daemon running after stop |
| MoneyDevKit default wallet wrapper | on a clean Pylon home, run `wallet status --json`, `wallet balance --json`, `wallet invoice 21 --description "pylon receive" --json`, `wallet offer --description "pylon offer" --json`, and `wallet history --json`; repeat with a second clean Pylon home on the same machine | auto-initializes a Pylon-scoped MDK agent-wallet home, reports JSON status/balance/history without printing mnemonic or preimages, returns BOLT11 and BOLT12 receive artifacts, records runtime kind `moneydevkit` and `runtime.local_daemon_port` in retained wallet metadata, and sets a stable Pylon-scoped `MDK_WALLET_PORT` so two local Pylon homes do not share the same MDK daemon |
| Omega Cloudflare MDK checkout proof | verify or reference a current public-safe Omega report showing `openagents.com` Worker -> `MDK_SIDECAR` -> Cloudflare Container -> MDK platform, a real bitcoin-denominated checkout, local MDK wallet payment, merchant paid status, and payer balance delta | proves the MDK-default release path does not depend on old GCP/native Nexus reachability; raw access tokens, mnemonics, invoices, payment hashes, preimages, and checkout client secrets are not included in public evidence |
| Post-release Artanis/Pylon paid-work bundle | run `scripts/nexus/artanis-pylon-integrated-proof-bundle.sh` with a fresh redacted MDK payment summary after public release publication | records the live Artanis SHC bootstrap, public npm/release Pylon install, SHC no-source accepted/rewarded work, and real MDK agent-wallet bitcoin movement in one public-safe bundle. The 2026-06-07 `pylon-v0.2.2` bundle is `completed_with_settlement_bridge_gap`, meaning the remaining blocker is the direct production bridge from Artanis assignment id to MDK settlement receipt. |
| Wallet entropy derivation | `cargo run -p pylon -- wallet entropy status --json` | reports redacted one-phrase HKDF derivation metadata and a node entropy digest without printing the mnemonic or raw 64-byte entropy |
| Wallet storage lock | `cargo run -p pylon -- wallet lock status --json` after startup; run a second lock acquisition in tests | creates the private `wallet/ldk/` layout, reports lock owner/stale state, refuses a second active writer, and only clears stale locks explicitly |
| LDK Node no-network open | `cargo run -p pylon -- config set wallet_runtime_kind ldk_node`, `cargo run -p pylon -- config set wallet_network regtest`, then `cargo run -p pylon -- wallet status --json` | builds the local LDK Node from derived entropy, reports a stable `ldk_node.node_id`, opens SQLite storage, and stays non-running with `wallet_chain_source_kind=none` |
| LDK Node chain source config | set `wallet_chain_source_kind=esplora` plus `wallet_esplora_url`, or `wallet_chain_source_kind=electrum` plus `wallet_electrum_url`, then run `wallet status --json` | starts the live LDK Node when the configured backend is reachable, surfaces running state and sync timestamps, and records last startup/sync error when the backend is unavailable |
| LDK Node status, sync, balance, and address | after selecting `wallet_runtime_kind=ldk_node` and `wallet_network=regtest`, run `wallet status --json`, `wallet sync --json`, `wallet balance --json`, and `wallet address --json` | reports node ID, chain/gossip source, backup status, redacted storage generation, LDK on-chain/Lightning balance buckets, and returns a real wallet-owned regtest Bitcoin address without printing mnemonic, entropy, preimages, or private state |
| LDK Node telemetry and redaction | run `wallet telemetry --json` with `wallet_rgs_url` or chain-source URLs that include credentials/query tokens, then run focused wallet telemetry tests | reports health/payable state, sync, balances, channel counts, liquidity buckets, backup status, and typed warning/error codes while redacting endpoint credentials and excluding recovery phrases, raw entropy, private keys, preimages, and raw channel state |
| LDK Node channel and liquidity readiness | run `wallet channels --json`, inspect `wallet status --json`, then run the regtest harness with `scripts/pylon/ldk-wallet-regtest-harness.sh` | reports usable/pending channel counts, inbound/outbound liquidity, peer connectivity, on-chain-vs-Lightning receive readiness, typed liquidity/route warnings, and LSPS1/LSPS2 LSP readiness without claiming an LSP is configured |
| LDK Node receive artifacts | after selecting `wallet_runtime_kind=ldk_node` and `wallet_network=regtest`, run `wallet invoice 21 --description "pylon receive" --json` and `wallet offer --amount-sats 21 --description "pylon offer" --json` | returns a real BOLT11 receive invoice with payment hash/runtime/expiry metadata retained in the local ledger, returns a BOLT12 offer when the linked LDK runtime supports it, and otherwise reports an actionable BOLT12-unavailable fallback without printing mnemonic, entropy, or preimages |
| LDK Node send and withdrawal guards | with `wallet_runtime_kind=ldk_node`, run focused tests for `wallet pay ... --json`, non-JSON send confirmation, failed BOLT11 send receipt, and direct on-chain withdrawal with zero spendable balance | BOLT11/BOLT12 sends dispatch through LDK Node, non-JSON headless sends require `--yes`, failures persist typed wallet-send receipts, on-chain withdrawal refuses before broadcast when `amount_sats` exceeds spendable balance after anchor reserve, and payout withdrawal records failed payout rows |
| LDK Node payment ledger projection | create a local LDK invoice, run `wallet history --json` twice, and run receipt fixture tests | projects LDK `PaymentDetails` into wallet payments and receipts, exposes payment hash/txid/receipt linkage without secrets, and repeated sync/history runs update existing rows rather than duplicating them |
| Encrypted wallet backup export | set `PYLON_WALLET_BACKUP_PASSPHRASE`, run `wallet backup export ./pylon-wallet-backup.json --passphrase-env PYLON_WALLET_BACKUP_PASSPHRASE --json`, then `wallet backup inspect ./pylon-wallet-backup.json --json` | writes a single private encrypted backup file with a redacted public manifest, includes LDK node/sqlite/backup-staging/registration state inside ciphertext, refuses wrong passphrases/corrupt ciphertext in tests, records a wallet backup receipt, and moves status from `backup_missing` to `backup_current` without printing mnemonic, entropy, or channel state |
| Wallet restore paths | run focused tests for `wallet restore phrase --mnemonic-env ... --wallet-network regtest --yes` and `wallet restore backup ./pylon-wallet-backup.json --passphrase-env ... --yes` | phrase-only restore recreates identity and deterministic entropy while warning that Lightning state is not restored; full-backup restore validates network/passphrase/derivation, restores LDK files and registration metadata, refuses active locks, detects stale backup status, and records typed restore receipts |
| Wallet-owned Nexus registration | on a clean Pylon home, run the normal online loop against Nexus or the provider payout-target mock | registers paid-work eligibility without manual external payout config, signs the challenge with a wallet-owned MoneyDevKit BOLT12 target or BOLT11 fallback, includes runtime/network/derivation/backup metadata, includes node ID only for the native LDK path, rejects Spark targets, and marks explicit `external_payout_target` values as non-default overrides |
| Artanis Pylon launch bootstrap | in the sibling `cloud` repo, run `cargo test -p openagents-cloud-contract artanis_bootstrap_assignment_fixture_parses_and_validates` and `cargo test -p oa-codex-control artanis_bootstrap`; before public release, also run one account-backed `/v1/artanis/bootstrap/start` workroom on SHC with no wallet authority | proves the Artanis bootstrap assignment validates, translates into a bounded Codex workroom request, emits Artanis context events, persists the assignment, and captures required launch artifacts; the 2026-06-07 live proof is `docs/reports/nexus/2026-06-07-pylon-v02-live-artanis-shc-bootstrap-proof.md` with run id `artanis.bootstrap.pylon-launch.20260607141825`, `wallet_authority=false`, Omega `agent_runs.status=completed`, and all eight required artifact digests captured |
| Online transition | `cargo run -p pylon -- online` | desired mode becomes `online`; unhealthy supply becomes `degraded`, not falsely healthy |
| Pause/resume | `cargo run -p pylon -- pause` then `resume` | transitions are explicit and truthful |
| Offline transition | `cargo run -p pylon -- offline` | desired mode becomes `offline` |
| No hidden auto-online | start `pylon serve` after offline state | runtime does not silently force `online` |
| Restart safety | stop and restart `pylon serve`; re-run `status`, `jobs`, `earnings`, `receipts` | persisted desired mode and persisted provider snapshot remain coherent |
| Autopilot parity | desktop provider-admin tests + shared substrate tests | `Autopilot` and `Pylon` use the same persisted provider snapshot contract |

## Repo-Level Checks

These checks should pass before a release candidate is called valid:

```bash
cargo test -p pylon -- --nocapture
cargo test -p pylon pylon_probe --lib -- --nocapture
cargo test -p pylon probe_agent --lib -- --nocapture
cargo clippy -p pylon --all-targets -- -D warnings
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

Artanis/Pylon bootstrap evidence from the private Cloud workroom lane should
also pass before a release candidate is called valid:

```bash
(cd ../cloud && cargo test -p openagents-cloud-contract artanis_bootstrap_assignment_fixture_parses_and_validates)
(cd ../cloud && cargo test -p oa-codex-control artanis_bootstrap)
```

Those tests prove the contract and fake-workroomd path only. A public Pylon
v0.2 release also needs one live account-backed SHC Artanis bootstrap run with
wallet authority set to `false` and the required artifacts captured. The
current accepted proof is
`docs/reports/nexus/2026-06-07-pylon-v02-live-artanis-shc-bootstrap-proof.md`.

The current MDK-default release path also needs Omega Cloudflare MDK evidence.
As of 2026-06-07, the accepted proof is recorded in the Omega docs and
summarized in `docs/reports/nexus/2026-06-07-pylon-v02-production-blockers.md`:
the `openagents.com` Worker reached a Cloudflare Container MDK sidecar, created
a 100-bitcoin-sat checkout, paid it from a local MDK agent wallet, observed
merchant status `PAYMENT_RECEIVED`, and recorded the payer balance delta. A
newer equivalent proof may replace that evidence. Do not block this MDK-default
release on old GCP/native Nexus public-edge `530` / `1033` or stale native-LDK
continuity state unless the task explicitly changes that native lane.

The current public Pylon v0.2 release publication proof is
`docs/reports/nexus/2026-06-07-pylon-v02-release-publication-proof.md`. It
records `pylon-v0.2.2` as the stable `OpenAgentsInc/openagents` GitHub binary
release, checksum-verified Darwin arm64 and Linux x86_64 assets, fresh
extracted-binary proof smokes on macOS and SHC Linux, the SHC no-source proof
with Cargo removed from `PATH`, the published `@openagentsinc/pylon@0.2.2`
npm bootstrap smoke on SHC, and cleanup of the misplaced
`OpenAgentsInc/psionic` `v0.2.0` release/tag. Treat `pylon-v0.2.0` and
`pylon-v0.2.1` as superseded packaging attempts.

The current post-release Artanis/Pylon paid-work bundle is
`docs/reports/nexus/2026-06-07-artanis-pylon-v022-integrated-paid-work-proof.md`
with JSON evidence in
`docs/reports/nexus/artanis-pylon-v022-integrated-paid-work-proof-20260607193426.json`.
It proves real MDK bitcoin movement plus public-release accepted work, while
explicitly not claiming that production Artanis assignment settlement is fully
live.

Sandbox-specific evidence that should remain green:

```bash
cargo --manifest-path ../psionic/Cargo.toml test -p psionic-sandbox execution::tests::policy_rejection_is_receipted -- --nocapture
cargo --manifest-path ../psionic/Cargo.toml test -p psionic-sandbox execution::tests::local_subprocess_success_emits_receipt_and_artifacts -- --nocapture
cargo test -p pylon sandbox_reports_surface_profiles_status_and_failures -- --nocapture
cargo test -p autopilot-desktop snapshot_signature_changes_when_sandbox_truth_changes -- --nocapture
```

## Operational Smoke Path

The minimum operator smoke path is:

```bash
cargo run -p pylon -- status
cargo run -p pylon -- init
cargo run -p pylon -- backends --json
cargo run -p pylon -- products
cargo run -p pylon -- sandbox
cargo run -p pylon -- inventory
cargo run -p pylon -- online
cargo run -p pylon -- pause
cargo run -p pylon -- resume
cargo run -p pylon -- offline
```

If those commands do not run cleanly and report truthful state transitions, the release is not ready.

The retained NIP-90 and wallet lane has its own local verification script:

```bash
scripts/pylon/verify_nip90_wallet.sh
```

That script is intentionally local and honest. It does four things:

- initializes a fresh standalone Pylon home
- pins the retained wallet config to `regtest`
- checks the retained headless report surfaces for jobs, earnings, receipts, payout, and activity
- runs the focused local websocket-relay and wallet-hook roundtrip tests for relay auth, announcement publish, provider intake, provider payment-required flow, provider settlement, buyer submit/watch/pay, wallet send guards, payment projection, encrypted backup export, restore paths, payout withdrawal persistence, and retained activity replay

It does not claim a live funded external Spark regtest backend. The current retained release gate is local relay plus regtest-shaped wallet configuration, with wallet receive artifacts, send guards, failed-send receipts, payment projection, encrypted backup export, restore paths, and payout withdrawal persistence proven through the checked-in focused tests.

## Packaging and Service Expectations

Standalone packaging is valid only if the install story is explicit:

- how the binary is started
- where config and identity live
- how desired mode is changed
- how logs/status are checked
- how restarts are handled

The preferred distribution lane is a GitHub Release asset that contains the standalone `pylon` and `pylon-tui` binaries for one verified platform. Source build remains the fallback when no matching asset exists.

The current supported operational posture is a service-style `pylon serve` process managed by:

- `systemd`
- `launchd`
- `tmux` or another persistent operator-managed session

This is sufficient for a truthful first standalone release. It is not necessary to pretend there is a universal cross-platform packaging story if it has not been verified.

## Rollout Checklist

Pre-release:

- verify launch truth statements in docs and user-facing messaging
- verify legacy Apple FM references are clearly marked disabled or historical
- verify `Pylon` still reads as a provider connector, not a monolithic local runtime
- verify lifecycle remains explicit and not hidden behind `serve`
- verify backend-health and product surfaces distinguish healthy vs degraded vs unsupported vs misconfigured vs disabled states plainly
- verify sandbox status, backend, job, and receipt surfaces expose declared execution classes, profile IDs, and failure detail truthfully

Release candidate:

- run the full repo-level checks above
- run the Artanis/Pylon bootstrap contract and fake-workroomd checks above
- verify the live Artanis SHC bootstrap proof above, or produce a newer
  public-safe equivalent if the release candidate changes that boundary
- verify current Omega Cloudflare MDK checkout proof or produce a newer
  public-safe equivalent
- publish the canonical `OpenAgentsInc/openagents` `pylon-v...` release asset
  and verify it from a fresh extracted archive outside the source checkout
- run the standalone smoke path on at least one machine with local Gemma supply
- run the sandbox-specific evidence commands on at least one machine with declared sandbox profiles
- confirm persisted status, jobs, earnings, and receipts survive a restart
- run `scripts/pylon/verify_nip90_wallet.sh`

Launch approval:

- confirm docs and smoke results match current code truth
- confirm no unsupported backend or market functionality is being claimed
- confirm rollout posture remains aligned with the broader compute-market verification and observability gates

Post-launch:

- monitor operator reports for lifecycle-control confusion
- monitor backend-state misclassification
- monitor discrepancies between `Autopilot` and standalone `Pylon` snapshot truth
- monitor sandbox runtime/profile misclassification and missing failure/termination detail in job and receipt views
- update this matrix before broadening launch claims
