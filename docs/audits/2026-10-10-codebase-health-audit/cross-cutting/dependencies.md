# Dependency hygiene & crate graph

**Scope:** every Rust workspace and lockfile in the repo (root, `crates/psionic`, `crates/openagents-mobile`, `mc-bridge`, `vendor/boltz-client`), checked with `cargo metadata --no-deps`, the lockfiles and source greps only. No builds. Snapshot commit `3168c986aa11e18a8bd30f52c270f609e49b3815`, 2026-10-10.

**Health grade: C**

The dependency graph builds and runs, but nobody is maintaining it. There are four Rust workspaces: the root (182 member crates, about 2.31M lines of Rust), `crates/psionic` (31 manifests), `crates/openagents-mobile` and `mc-bridge`. `vendor/boltz-client` adds a fifth lockfile. The root workspace barely uses the tools meant to keep this many crates consistent. `[workspace.dependencies]` has 3 entries, all iroh, and only one crate inherits any of them. The most common external crates are declared by hand hundreds of times, `tokio` is spelled 85 different ways, and `sha2` is split between 0.10 and 0.11. The 0.11 side includes `nostr`, which 54 crates depend on.

The root `Cargo.lock` resolves 1,567 packages. 146 crate names appear in more than one version, for 209 extra copies. The two Lightning wallet stacks account for 254 of the packages and 66 of the duplicate copies, and they bring 3 versions of `lightning` and 4 of `secp256k1`.

Three problems are structural:

1. **The dependency gate disagrees with the lockfile.** `deny.toml` sets `unknown-git = "deny"` while the lockfile has 32 git-sourced packages. The docs say no git dependency is admitted. The gate's accepted baseline is 29 errors. It is skipped when cargo-deny is not installed, it checks the root workspace only, and it will fail on its own date check on 2026-10-20.
2. **The layering is inverted.** The production servers `gateway` and `openagents-web` link the whole 262k-line `coder` app crate just to reach a few domain modules.
3. **Several "host" default features are on by default.** Nearly every consumer has to remember `default-features = false`.

There are real strengths. The TLS stack is a single ring/rustls stack. Lint and edition inheritance are close to complete. No test-only feature leaks into normal dependencies. Workspace splits and the vendored patch are documented where they occur.

## Measurements

| Metric | Value |
|---|---|
| Rust workspaces | root (182 members, ~2,310,550 Rust LOC under `crates/*`), `crates/psionic` (31 Cargo.toml, own lock), `crates/openagents-mobile` (own lock), `mc-bridge` (own lock, nightly-2026-08-03, edition 2021) |
| Vendored | `vendor/boltz-client`: own lock and toolchain, 102 files, 20,346 Rust LOC, 1.3 MB |
| Root lock | 1,567 packages; 146 duplicated names; 209 extra copies |
| Psionic lock | 754 packages; 60 duplicated names; 71 extra copies |
| Mobile lock | 943 packages; 83 duplicated names; 108 extra copies |
| mc-bridge lock | 375 packages; 16 duplicated names; 19 extra copies |
| Worst root duplicates | hashbrown x6; windows-sys x5; windows-targets family x4; secp256k1 x4 (0.27/0.29/0.30/0.31); base64 x4; ark-ff/std/serialize x4 (0.3–0.6); lightning x3 (0.1.13 registry, 0.2.5 moneydevkit git, 0.3.0+git LDK); lightning-types x4; rand, syn, tokio-tungstenite, getrandom, digest x3; reqwest 0.12/0.13; axum 0.7/0.8; thiserror 1/2; tokenizers 0.22/0.23 |
| Packages reachable only via one subtree | wallet stacks (openagents-spark, openagents-wallet, boltz-client, breez-sdk-spark) 254 of 1,562 (66 of 204 duplicate copies); kev/laya 70; plugin (wasmtime) 56; iroh 90 |
| Git sources in root lock | 32 packages from 7 repos: moneydevkit/rust-lightning 12, breez/spark-sdk 9, lightningdevkit/rust-lightning 5, lightsparkdev/frost 3, arik-so/rust-musig2 1, moneydevkit/ldk-node 1, moneydevkit/bitcoin-payment-instructions 1 |
| EVM packages | 27 `alloy-*` |
| `[workspace.dependencies]` | 3 entries; 1 inheriting crate (openagents-connect); `iroh-relay` inherited by none |
| External dependency names | 154 |
| Hand-written declarations (incl. dev-deps, verifier re-measure) | serde_json 158 (8 spellings), serde 138, tempfile 107, sha2 86, tokio 124 lines / 85 spellings |
| Version splits | 22 external crates with differing requirements, e.g. sha2 0.10 x73 vs 0.11 x13; hmac 0.12/0.13; getrandom 0.3/0.4; objc2 =0.5.2/0.6 |
| Declared but unreferenced deps | 83 static hits, ~75 after excluding feature-activation and self dev-dep cases |
| Internal fan-in (top) | nostr 54, atif 29, coder-access 20, jev 19, coder-ui 19 |
| Internal fan-out (top) | openagents-cli 60, verse 46, coder 36 |
| Transitive internal closure | gateway 70, openagents-web 80, verse 97, coder 61 |
| Crates with no internal reverse dep | 37, all bin or cdylib |
| Largest crates (LOC) | coder 262,296; coder-one 132,534; gym 110,396; verse-world 93,199; openagents-cli 89,408; nostr 84,833 |
| Workspace inheritance | version 177/182, edition 175, rust-version 176, `[lints]` 176 (missing: wallet, x402, oa-copy, openagents-ui; bitcoin-amount and rust-native copy lints by hand) |
| License metadata | 7 root crates plus the psionic workspace; 3 use invalid SPDX `CC-0` |
| Cross-lock skew | mobile vs root: 8 of 799 shared packages differ; psionic vs root: 210 of 485 differ |
| Cargo.lock churn | 555 commits since 2026-09-10 |
| `crates/psionic/target` | 448 MB, not git-ignored |

## Strengths

- One TLS stack in the root and psionic locks: ring 0.17.14 and rustls 0.23. There is no `openssl-sys` or `native-tls` anywhere, and no aws-lc in the root lock. Every root `reqwest` declaration sets `default-features = false` with rustls-tls.
- All 182 root crates are `publish = false`. 175 inherit edition 2024. 176 inherit workspace lints, which deny `dbg_macro`, `todo`, `unimplemented` and `unsafe_op_in_unsafe_fn`.
- No test, fake, fixture or mock feature is enabled on a normal internal dependency or listed in a default feature list. Hooks such as `nostr/test-invoice` and `verse-net/test-support` appear only in `[dev-dependencies]`.
- Workspace splits and patches are explained where they occur:
  - `crates/openagents-mobile/Cargo.toml:117-121` explains the sqlite3 link conflict.
  - `mc-bridge/Cargo.toml` explains the nightly split.
  - `vendor/boltz-client/VENDORED.md` gives the upstream revision, the reason for vendoring (404) and a removal condition.
- `deny.toml` exists and explains its choices. Each license exception is scoped to one crate version. The one advisory exception names an owner and a review date. Wildcards are denied. `scripts/check-dependencies.sh` refuses to pass when the review is overdue or the `paste` version changes, so the exception cannot carry forward silently.
- iroh is pinned exactly in `[workspace.dependencies]` with reviewed features (default features off, tls-ring). The rest of the graph should follow this pattern.
- Every dependency declaration has a comment giving its reason, which makes unused declarations easy to spot.
- The mobile workspace tracks the root closely: 8 of 799 shared packages differ.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| X-DEP-01 | High | build | Dependency gate is red and its policy contradicts the lockfile (32 git packages vs `unknown-git = "deny"`) | M |
| X-DEP-02 | High | architecture | Production web and gateway servers link the entire `coder` app crate for a few domain modules | L |
| X-DEP-03 | Medium | maintainability | `[workspace.dependencies]` is effectively unused; ~600 hand declarations, 85 tokio spellings, split crypto versions | M |
| X-DEP-04 | Medium | dead-code | Many declared dependencies are never referenced in source | S |
| X-DEP-05 | Medium | architecture | Two parallel Lightning stacks: 3 `lightning`, 4 `secp256k1`, rusqlite pin, mobile workspace split | L |
| X-DEP-06 | Medium | build | Advisory exception hard-fails on 2026-10-20; cause is kev/laya's candle 0.11 stack | M |
| X-DEP-07 | Medium | architecture | Inverted default features: consumers must remember `default-features = false` | M |
| X-DEP-08 | Medium | security | Dependency gate covers only the root workspace | S |
| X-DEP-09 | Medium | maintainability | Payment crates `openagents-wallet` and `openagents-x402` opt out of workspace lints and metadata | S |
| X-DEP-10 | Low | policy | EVM, Solana and Tron code in the graph via vendored boltz-client (27 alloy packages) | M |
| X-DEP-11 | Low | performance | 209 duplicate crate copies with no reduction plan | M |
| X-DEP-12 | Low | build | Root vs psionic lockfile skew on security-relevant crates | S |
| X-DEP-13 | Low | build | No `default-members`; deprecated microluna and archival crates build with everything | S |
| X-DEP-14 | Low | policy | Invalid `CC-0` license ids; root `workspace.package` has no license | S |
| X-DEP-15 | Low | architecture | x402 depends on BYOK model-access; commercial ledgers depend on the agent app | S |
| X-DEP-16 | Low | maintainability | Directory and package names disagree; two crates claim "OpenAgents Terminal" | S |
| X-DEP-17 | Low | security | Git dependency pinned by a mutable tag in three places | S |
| X-DEP-18 | Low | repo-hygiene | Redundant member, unused workspace dep, 448 MB psionic target dir not ignored | S |

### X-DEP-01 Dependency gate is red and its policy contradicts the lockfile (32 git packages vs `unknown-git = "deny"`)

Severity: High · Category: build · Effort: M

**Locations**
- [deny.toml](../../../../deny.toml), lines 53-57
- [Cargo.lock](../../../../Cargo.lock)
- [docs/dependencies.md](../../../../docs/dependencies.md), lines 125-129
- [docs/desktop/verification/2026-09-30-rich-text/verification.md](../../../../docs/desktop/verification/2026-09-30-rich-text/verification.md), lines 57-59
- [scripts/verify-rust.sh](../../../../scripts/verify-rust.sh), lines 393-398
- [crates/spark-wallet/Cargo.toml](../../../../crates/spark-wallet/Cargo.toml), line 13
- [crates/wallet/Cargo.toml](../../../../crates/wallet/Cargo.toml), line 25

**Evidence**
- `deny.toml` `[sources]` sets `unknown-git = "deny"`. It has `allow-registry` but no `allow-git`.
- `Cargo.lock` at HEAD has 32 `source = "git+..."` entries: moneydevkit/rust-lightning 12, breez/spark-sdk 9, lightningdevkit/rust-lightning 5, lightsparkdev/frost 3, arik-so/rust-musig2 1, moneydevkit/ldk-node 1, moneydevkit/bitcoin-payment-instructions 1.
- `docs/dependencies.md:127-128` says "no Git dependency is admitted".
- The rich-text receipt says "The dependency gate reports the same 29 preexisting errors".
- `verify-rust.sh` records the deps phase as SKIP when cargo-deny is missing ("cargo-deny is not installed, so this is a partial gate").

**Impact:** The only automated supply-chain and license check has been failing for some time, and the failure is treated as normal. The docs also tell reviewers that no git sources exist.

**Suggested action**
1. Add `allow-git = [...]` under `[sources]` in `deny.toml` listing the 7 exact repo URLs. Give each a comment naming the consumer crate and the rev.
2. Rewrite the "Source provenance" section of `docs/dependencies.md` to list the git sources and their revs.
3. Fix the remaining license and wildcard errors until `./scripts/check-dependencies.sh` exits 0.
4. Make the `verify-rust.sh` deps phase fail, not skip, when cargo-deny is missing.
5. Verify by running `./scripts/check-dependencies.sh` and recording a receipt with 0 errors.

### X-DEP-02 Production web and gateway servers link the entire `coder` app crate for a few domain modules

Severity: High · Category: architecture · Effort: L

**Locations**
- [crates/commercial-spend/Cargo.toml](../../../../crates/commercial-spend/Cargo.toml), line 11
- [crates/commercial-spend/src/wallet.rs](../../../../crates/commercial-spend/src/wallet.rs), lines 15 and 99
- [crates/gateway/Cargo.toml](../../../../crates/gateway/Cargo.toml), line 50
- [crates/openagents-web/Cargo.toml](../../../../crates/openagents-web/Cargo.toml), line 17
- [crates/coder/Cargo.toml](../../../../crates/coder/Cargo.toml)

**Evidence**
- `commercial-spend/Cargo.toml:11` declares `coder = { path = "../coder", default-features = false }`. `crates/coder/Cargo.toml` has no `[features]` section, so the flag does nothing.
- commercial-spend uses coder in two places only:
  - `wallet.rs:15`: `coder::customer::plugins::Offer`
  - `wallet.rs:99`: `coder::customer::Store::shared_plugin_approval`
- `gateway/Cargo.toml:50` depends on commercial-spend.
- `openagents-web/Cargo.toml:17` depends on `coder = { path = "../coder" }` directly. Its references are mostly `coder::task` (13) and `coder::customer` (4), plus `coder::builtin_plugins`, `coder::efficiency` and the constants `RELAY` and `WORKER`.
- The reviewer measured transitive internal closures of 70 crates for gateway and 80 for openagents-web. These pull in pylon, coder-host, plugin (wasmtime) and iroh. The verifier did not re-measure the closure sizes.

**Impact:** Every gateway and web deploy compiles and links the desktop agent runtime and its heavy dependencies. Any change in coder rebuilds the public servers.

**Suggested action**
1. Extract `coder::task::sales`, `coder::customer::{plugins, Store approval}`, `coder::efficiency` and the constants into a domain crate with only light dependencies (for example `crates/coder-customer`).
2. Re-export them from `coder` under the old paths so existing callers keep compiling.
3. Point commercial-accounts, commercial-spend and openagents-web at the new crate and remove their `coder` dependencies.
4. Remove the no-op `default-features = false` from commercial-spend.
5. Verify that `cargo tree -p gateway -i wasmtime` and `cargo tree -p openagents-web -i coder` both return nothing.

### X-DEP-03 `[workspace.dependencies]` is effectively unused: ~600 hand-written declarations of 5 common crates, ~85 tokio spellings, split crypto versions

Severity: Medium · Category: maintainability · Effort: M

**Locations**
- [Cargo.toml](../../../../Cargo.toml), lines 15-22
- [crates/openagents-connect/Cargo.toml](../../../../crates/openagents-connect/Cargo.toml), lines 16-18
- [crates/nostr/Cargo.toml](../../../../crates/nostr/Cargo.toml), lines 28 and 39
- [crates/coder-scheduler/Cargo.toml](../../../../crates/coder-scheduler/Cargo.toml), line 10
- [crates/tenancy/Cargo.toml](../../../../crates/tenancy/Cargo.toml), line 13

**Evidence**
- `[workspace.dependencies]` has 3 iroh entries. Only openagents-connect inherits any of them (`iroh` and `iroh-mdns-address-lookup`). Nothing inherits `iroh-relay`.
- Declaration counts, including dev-dependencies:

  | Crate | Lines | Notes |
  |---|---|---|
  | serde_json | 158 | 8 distinct spellings |
  | serde | 138 | |
  | tempfile | 107 | |
  | sha2 | 86 | |
  | tokio | 124 | 85 distinct spellings |

- sha2 is `"0.10"` in 73 declarations and 0.11 in 13. One of the 0.11 declarations is `nostr/Cargo.toml:28` (`0.11.0`).
- `nostr/Cargo.toml:39` has hmac `"0.13.0"`.
- getrandom `"0.4"` appears in `coder-scheduler:10` and `tenancy:13`.
- `Cargo.lock` changed in 555 commits since 2026-09-10.

**Impact:** Every version bump means editing many manifests. The split crypto versions put duplicate digest, sha2 and hmac copies into binaries. This costs maintenance time but is not a defect.

**Suggested action**
1. Add workspace entries for serde, serde_json, tokio (base features only), tempfile, sha2, hmac, secp256k1, reqwest (`default-features = false`, rustls), futures-util, base64, thiserror, axum, getrandom and rusqlite.
2. Converge sha2 on 0.11 and hmac on 0.13.
3. Convert the manifests mechanically to `{ workspace = true, features = [...] }`.
4. Verify that `grep -rhE '^tokio *=' crates/*/Cargo.toml | sort -u | wc -l` drops to a handful, and that `Cargo.lock` has a single sha2 and a single hmac version from first-party crates.

### X-DEP-04 Many declared dependencies are never referenced in source

Severity: Medium · Category: dead-code · Effort: S

**Locations**
- [crates/verse/Cargo.toml](../../../../crates/verse/Cargo.toml), lines 98, 126, 132, 164, 168 and 171
- [crates/openagents-web/Cargo.toml](../../../../crates/openagents-web/Cargo.toml), lines 23-30
- [crates/verse-zone-water/Cargo.toml](../../../../crates/verse-zone-water/Cargo.toml), lines 9-19
- [crates/spark-wallet/Cargo.toml](../../../../crates/spark-wallet/Cargo.toml), lines 23-24
- [crates/coder-one/Cargo.toml](../../../../crates/coder-one/Cargo.toml), line 18
- [crates/kev/Cargo.toml](../../../../crates/kev/Cargo.toml), lines 11 and 29
- [crates/lev/Cargo.toml](../../../../crates/lev/Cargo.toml), line 31

**Evidence:** A word-boundary grep over each crate's `.rs` files finds no references to these dependencies:

| Crate | Unreferenced dependencies |
|---|---|
| verse | resvg, swash, miniz_oxide, webpki_roots, gym_leaderboard, coder_vt, coder_pty, memmap2 (no verse feature enables `dep:coder-vt`, `dep:coder-pty` or `dep:memmap2`) |
| openagents-web | coder_demo_ui, compute_workbench, receipts, route_contract, workbench |
| verse-zone-water | verse_core, verse_world |
| coder-one | plugin, coder_history |
| spark-wallet | uuid |
| kev, lev | tower_http (listed only in their `serve` features) |
| oak | sha2 |
| microluna | base64 |
| terminal-gfx | bytemuck |
| coder-delegate | reqwest |
| verse-imported | wgpu |
| coder-browser | web_time |

The static scan found about 75 such entries in total. The verifier did not re-check that total.

**Impact:** These dependencies add compile time and lockfile weight, and they misstate what each crate actually uses.

**Suggested action**
1. Run `cargo shear` on the root workspace. It does not compile anything.
2. Delete the confirmed entries listed above.
3. Keep getrandom entries that exist only to activate `wasm_js`, and list them under `[package.metadata.cargo-shear] ignored`.
4. Add `cargo shear` as a phase in `verify-rust.sh`. Verify that it exits 0.

### X-DEP-05 Two parallel Lightning stacks give 3 `lightning` and 4 `secp256k1` versions and force the rusqlite pin and the mobile workspace split

Severity: Medium · Category: architecture · Effort: L

**Locations**
- [crates/wallet/Cargo.toml](../../../../crates/wallet/Cargo.toml), lines 21-29
- [crates/spark-wallet/Cargo.toml](../../../../crates/spark-wallet/Cargo.toml), lines 9-13
- [crates/openagents-mobile/Cargo.toml](../../../../crates/openagents-mobile/Cargo.toml), lines 117-122
- [Cargo.lock](../../../../Cargo.lock)

**Evidence**
- `Cargo.lock` at HEAD has lightning 0.1.13, 0.2.5 and 0.3.0+git, and secp256k1 0.27.0, 0.29.1, 0.30.0 and 0.31.1.
- `openagents-mobile/Cargo.toml:117-120` says: "Breez's SQLite store links `libsqlite3-sys` 0.30 and `ldk-node` (the x402 receiver, `crates/wallet`) links 0.28, and Cargo allows one `sqlite3` link per lock file."
- The reviewer attributed 254 packages and 66 duplicate copies to the wallet stacks. The verifier did not re-check this attribution.

**Impact:** There are two payment backends to keep secure. They produce a large share of the duplicate crates, one fork freezes the SQLite version, and they force a separate mobile workspace.

**Suggested action**
1. Decide whether to keep or collapse the LDK-fork receiver (`crates/wallet`) and the Spark wallet. Record the decision in `docs/dependencies.md` and link an issue.
2. If both stay, document what each one pins and the conditions under which they could be collapsed.
3. Move rusqlite into `[workspace.dependencies]`.
4. Track the `Cargo.lock` duplicate count (209 today) as the metric, and re-count it after each change.

### X-DEP-06 Dependency advisory exception hard-fails on 2026-10-20 (10 days); cause is kev/laya's candle 0.11 stack in the root workspace

Severity: Medium · Category: build · Effort: M

**Locations**
- [scripts/check-dependencies.sh](../../../../scripts/check-dependencies.sh), lines 6-10
- [deny.toml](../../../../deny.toml), lines 13-17
- [docs/dependencies.md](../../../../docs/dependencies.md), lines 91-123
- [crates/kev/Cargo.toml](../../../../crates/kev/Cargo.toml), lines 13-17
- [crates/laya/Cargo.toml](../../../../crates/laya/Cargo.toml), lines 13-18

**Evidence**
- `check-dependencies.sh` exits 1 when `date -u +%Y%m%d` is 20261020 or later.
- `deny.toml` ignores RUSTSEC-2024-0436 (`paste`) with the note "Review by 2026-10-20", and sets `unused-ignored-advisory = "deny"`.
- kev and laya declare candle-core `"0.11"`. The root lock has candle-core 0.11.0 and the psionic lock has 0.9.2.

**Impact:** From 2026-10-20 the dependency gate fails for every change.

**Suggested action**
1. Before 2026-10-20, choose one of these:
   - re-review the advisory and move the date in both `deny.toml` and `check-dependencies.sh`; or
   - remove `paste` from the root graph, either by moving kev and laya into the psionic workspace or by aligning candle, then delete the ignore entry.
2. Verify with `cargo tree -i paste` (expect empty if it was removed) and a passing `./scripts/check-dependencies.sh`.

### X-DEP-07 Inverted default features: nearly every consumer has to remember `default-features = false`

Severity: Medium · Category: architecture · Effort: M

**Locations**
- [crates/coder-access/Cargo.toml](../../../../crates/coder-access/Cargo.toml), line 10
- [crates/wallet/Cargo.toml](../../../../crates/wallet/Cargo.toml), line 28
- [crates/gym-leaderboard/Cargo.toml](../../../../crates/gym-leaderboard/Cargo.toml), line 10
- [crates/verse/Cargo.toml](../../../../crates/verse/Cargo.toml), lines 10-11

**Evidence**
- coder-access has `default = ["host"]`, and 20 of its 21 declarations across `crates/*/Cargo.toml` set `default-features = false`.
- wallet has `default = ["ldk"]`.
- gym-leaderboard has `default = ["generate"]`.
- verse has `default = ["desktop"]`, and the desktop feature enables more than 10 sub-features.

**Impact:** One forgotten flag pulls host-only stacks (PTY, SQLite, LDK node) into phone, wasm or server builds. Feature unification can also undo the opt-outs.

**Suggested action**
1. Change these crates to `default = []`.
2. Enable the feature explicitly in the few host consumers, or use `required-features` on their bins.
3. Remove the now-redundant `default-features = false` lines.
4. Verify with `cargo tree -e features -i coder-access` that `host` is enabled only from host crates.

### X-DEP-08 The dependency gate covers only the root workspace; mobile, psionic and mc-bridge are never checked

Severity: Medium · Category: security · Effort: S

**Locations**
- [scripts/check-dependencies.sh](../../../../scripts/check-dependencies.sh), lines 13-18
- [crates/openagents-mobile/Cargo.toml](../../../../crates/openagents-mobile/Cargo.toml), lines 117-122
- [mc-bridge/Cargo.toml](../../../../mc-bridge/Cargo.toml)
- [crates/psionic/Cargo.lock](../../../../crates/psionic/Cargo.lock)

**Evidence:** `check-dependencies.sh` changes to the repo root and runs `cargo +1.97.1 deny --locked check ...` once, with no `--manifest-path`. The mobile lock (943 packages, 83 duplicated names) and the psionic lock (754 packages) are separate files that the gate never reads.

**Impact:** Advisories in the shipped phone app, which holds wallet keys, and in the inference server go undetected.

**Suggested action**
1. Loop the script over each workspace manifest, running `cargo deny --manifest-path $m --locked check`.
2. Add a per-workspace `deny.toml` where the policy differs.
3. Print one pass/fail line per workspace, and exit non-zero if any workspace fails.
4. Verify that the output lists 4 workspaces.

### X-DEP-09 Payment crates `openagents-wallet` and `openagents-x402` opt out of workspace lints and metadata

Severity: Medium · Category: maintainability · Effort: S

**Locations**
- [crates/wallet/Cargo.toml](../../../../crates/wallet/Cargo.toml), lines 1-32
- [crates/x402/Cargo.toml](../../../../crates/x402/Cargo.toml), lines 1-29
- [crates/oa-copy/Cargo.toml](../../../../crates/oa-copy/Cargo.toml)
- [crates/openagents-ui/Cargo.toml](../../../../crates/openagents-ui/Cargo.toml)
- [crates/bitcoin-amount/Cargo.toml](../../../../crates/bitcoin-amount/Cargo.toml)
- [crates/rust-native/Cargo.toml](../../../../crates/rust-native/Cargo.toml)

**Evidence:** The only `crates/*/Cargo.toml` files without a `[lints]` section are oa-copy, openagents-ui, wallet and x402, plus the separate psionic workspace. wallet and x402 together contain 607 `unwrap()` calls in their `.rs` files, many of them in tests.

**Impact:** The code that moves money is outside the shared lint policy.

**Suggested action**
1. Add `[lints] workspace = true` and the `*.workspace = true` metadata keys to wallet, x402, oa-copy and openagents-ui.
2. Replace the hand-copied lint blocks in bitcoin-amount and rust-native with `workspace = true`.
3. Consider `clippy::unwrap_used = "warn"` for non-test code in the payment crates.
4. Verify that `cargo clippy -p openagents-wallet -p openagents-x402` is clean under the workspace lints.

### X-DEP-10 EVM, Solana and Tron code ships in the graph via the vendored boltz-client (27 alloy packages)

Severity: Low · Category: policy · Effort: M

**Locations**
- [vendor/boltz-client/crates/lib/Cargo.toml](../../../../vendor/boltz-client/crates/lib/Cargo.toml), lines 25-54
- [vendor/boltz-client/VENDORED.md](../../../../vendor/boltz-client/VENDORED.md)
- [Cargo.toml](../../../../Cargo.toml), lines 128-134

**Evidence:** The boltz-client manifest declares these unconditionally:
- alloy-sol-types, alloy-primitives, alloy-signer and alloy-signer-local ("EVM signing")
- bs58 ("Solana / Tron destination addresses")

The root `Cargo.lock` has 27 `alloy-*` packages.

**Impact:** This adds audit surface to binaries that handle keys. It is a transitive dependency only; the product offers no EVM rail.

**Suggested action**
1. Record it as a known exception in `docs/dependencies.md` and `VENDORED.md`.
2. Consider a local patch that puts the EVM and Solana modules behind an off-by-default feature, and ask Breez to make boltz optional.
3. Add a `deny.toml` `[bans]` entry for alloy-signer-local with `wrappers = ["boltz-client"]`.
4. Verify that `cargo tree -i alloy-signer-local` shows boltz-client as the only path.

### X-DEP-11 Cargo.lock carries 209 duplicate crate copies (1,567 packages, 146 names), with no reduction plan

Severity: Low · Category: performance · Effort: M

**Locations**
- [Cargo.lock](../../../../Cargo.lock)
- [deny.toml](../../../../deny.toml), lines 60-66

**Evidence:** A recount of the HEAD lockfile gives 1,358 distinct names, 146 of them duplicated, for 209 extra copies. hashbrown has 6 versions (0.12 through 0.17). `deny.toml` sets `multiple-versions = "warn"`.

**Impact:** Clean builds take longer and the audit surface is larger. Most of the duplicates come from third-party crates.

**Suggested action**
1. Fix the duplicates we control: move reqwest from 0.12 to 0.13 to match iroh, and align tokio-tungstenite.
2. Add `skip-tree` entries for third-party subtrees.
3. Change `multiple-versions` to `deny`.
4. Verify with `cargo deny check bans` and by re-counting duplicates against the baseline of 209.

### X-DEP-12 Root vs psionic lockfile version skew on security-relevant crates

Severity: Low · Category: build · Effort: S

**Locations**
- [Cargo.toml](../../../../Cargo.toml), lines 1-13
- [crates/psionic/Cargo.toml](../../../../crates/psionic/Cargo.toml)
- [crates/openagents-mobile/Cargo.toml](../../../../crates/openagents-mobile/Cargo.toml), lines 122-139
- [mc-bridge/Cargo.toml](../../../../mc-bridge/Cargo.toml)
- [mc-bridge/rust-toolchain.toml](../../../../mc-bridge/rust-toolchain.toml)

**Evidence**
- Version pairs, root vs psionic HEAD locks:

  | Crate | Root | Psionic |
  |---|---|---|
  | tokio | 1.53.1 | 1.50.0 |
  | wgpu | 29.0.4 | 26.0.1 |
  | candle-core | 0.11.0 | 0.9.2 |
  | rustls | 0.23.45 | 0.23.37 |
  | hyper | 1.11.1 | 1.8.1 |

- psionic's `Cargo.toml` has no `rust-version`.
- The mobile manifest repeats the root lints and the boltz `[patch]` block by hand ("The lints repeat the repository's").
- mc-bridge is edition 2021 on nightly-2026-08-03.
- `vendor/boltz-client` keeps its own `Cargo.lock` and `rust-toolchain.toml`.

**Impact:** Patch releases of rustls and hyper reach the workspaces at different times, and the duplicated patch and lint blocks can drift apart.

**Suggested action**
1. Add `scripts/check-workspace-sync.sh`. It should diff the mobile `[patch]` and lints blocks against the root's, and report version skew for rustls, hyper, tokio and ring across the lockfiles.
2. Run `cargo update -p rustls -p hyper` in each workspace on a fixed schedule.
3. Add `rust-version` to psionic's `[workspace.package]`.
4. Verify that the sync script reports no skew on those four crates.

### X-DEP-13 Root build has no `default-members`; the deprecated microluna and the archival coder-one build with everything

Severity: Low · Category: build · Effort: S

**Locations**
- [Cargo.toml](../../../../Cargo.toml), lines 1-13
- [crates/microluna/README.md](../../../../crates/microluna/README.md), lines 1-27
- [crates/coder-one/Cargo.toml](../../../../crates/coder-one/Cargo.toml)
- [crates/gym/Cargo.toml](../../../../crates/gym/Cargo.toml), line 40

**Evidence**
- The root `Cargo.toml` has no `default-members`.
- The microluna README is titled "Microluna (deprecated)" and says "Don't build new work on this crate".
- coder-one has no Cargo dependents. However, `gym/Cargo.toml:40` says "The Runs pane runs `coder-one ask` as a supervised child process", so coder-one is still used at runtime as a binary.

**Impact:** Workspace-wide gates compile and test archival code.

**Suggested action**
1. Add `default-members` that leaves out microluna.
2. Also leave out coder-one, coder-compositor, voyager and laya if they are not needed by default. They still build with `-p`.
3. Check that any script that needs coder-one's binary (such as gym's Runs pane) builds it explicitly with `-p coder-one`.
4. Verify that `cargo metadata --format-version 1 --no-deps | jq '.workspace_default_members | length'` is lower than 182.

### X-DEP-14 Invalid and missing license metadata: 3 crates declare non-SPDX "CC-0"; root workspace.package has no license

Severity: Low · Category: policy · Effort: S

**Locations**
- [crates/bitcoin-amount/Cargo.toml](../../../../crates/bitcoin-amount/Cargo.toml), line 6
- [crates/wallet/Cargo.toml](../../../../crates/wallet/Cargo.toml), line 5
- [crates/x402/Cargo.toml](../../../../crates/x402/Cargo.toml), line 5
- [Cargo.toml](../../../../Cargo.toml), lines 24-28
- [LICENSE](../../../../LICENSE)

**Evidence**
- bitcoin-amount, wallet and x402 declare `license = "CC-0"`. The SPDX id is `CC0-1.0`.
- Only 7 root crates have a `license =` line, plus the psionic workspace.
- `LICENSE` is Apache-2.0.
- The root `[workspace.package]` has version, publish, edition and rust-version, but no license.

**Impact:** SBOMs and anyone reusing the code see an invalid or unknown license.

**Suggested action**
1. Add `license = "Apache-2.0"` to the root `[workspace.package]`.
2. Add `license.workspace = true` to every crate.
3. If CC0 is intended for those three crates, change `CC-0` to `CC0-1.0`.
4. Verify with `cargo metadata --no-deps` that every package has a valid SPDX license.

### X-DEP-15 Payment protocol crate x402 depends on BYOK model-access; commercial ledgers depend on the agent app

Severity: Low · Category: architecture · Effort: S

**Locations**
- [crates/x402/Cargo.toml](../../../../crates/x402/Cargo.toml), line 15
- [crates/x402/src/front.rs](../../../../crates/x402/src/front.rs), lines 51 and 564-568
- [crates/commercial-accounts/Cargo.toml](../../../../crates/commercial-accounts/Cargo.toml), lines 9-11

**Evidence:** `x402/Cargo.toml:15` declares `model-access = { path = "../model-access" }`. `front.rs` uses it in two places:
- line 51: `Option<&'a model_access::Keys>`
- lines 564 and 568: the header name and `Keys::from_header_values`

**Impact:** The payment code rebuilds whenever key handling changes, and provider-key parsing sits inside the facilitator's trust boundary.

**Suggested action**
1. Pass raw header values, or a trait object, into x402's front layer, and parse BYOK keys in gateway.
2. Remove model-access from x402's dependencies.
3. Move `CommissionSource` and `Offer` into the customer crate extracted in X-DEP-02, so commercial-accounts no longer depends on coder.
4. Verify that `cargo tree -p openagents-x402 -i model-access` returns nothing.

### X-DEP-16 Crate directory names and package names disagree; two crates both claim "OpenAgents Terminal"

Severity: Low · Category: maintainability · Effort: S

**Locations**
- [crates/wallet/Cargo.toml](../../../../crates/wallet/Cargo.toml), line 2
- [crates/x402/Cargo.toml](../../../../crates/x402/Cargo.toml), line 2
- [crates/spark-wallet/Cargo.toml](../../../../crates/spark-wallet/Cargo.toml), line 2
- [crates/terminal-app/Cargo.toml](../../../../crates/terminal-app/Cargo.toml), lines 1-11
- [crates/openagents-terminal/Cargo.toml](../../../../crates/openagents-terminal/Cargo.toml), lines 1-10

**Evidence**
- Three directories have different package names:

  | Directory | Package |
  |---|---|
  | `wallet` | `openagents-wallet` |
  | `x402` | `openagents-x402` |
  | `spark-wallet` | `openagents-spark` |

- terminal-app builds a bin named `openagents-terminal`. Its description is "OpenAgents Terminal: the shared smart terminal in a native window."
- The package openagents-terminal has the description "OpenAgents Terminal: a full-screen chat ...".
- Both crates are version 1.0.0-rc.2. The release script ties the two terminal versions together on purpose.

**Impact:** People and tools have to know two names for each of these crates, and it is easy to edit the wrong terminal crate.

**Suggested action**
1. Rename the three directories with `git mv` to match their package names, and update the path dependencies.
2. Either rename the terminal library package or give the two terminal crates distinct descriptions. Update the release script, which ties their versions together, in the same change.
3. Verify with `cargo metadata --no-deps` that each directory name matches its package name.

### X-DEP-17 Git dependency pinned by a mutable tag in three places

Severity: Low · Category: security · Effort: S

**Locations**
- [crates/spark-wallet/Cargo.toml](../../../../crates/spark-wallet/Cargo.toml), lines 13 and 32
- [crates/openagents-mobile/Cargo.toml](../../../../crates/openagents-mobile/Cargo.toml), line 59

**Evidence:** `breez-sdk-spark = { git = "https://github.com/breez/spark-sdk", tag = "0.26.0", ... }` appears three times, each with a different feature set.

**Impact:** If upstream re-tags the release, a lock refresh can change wallet code without any manifest diff to review.

**Suggested action**
1. Pin by `rev = "<full SHA from Cargo.lock>"` and keep the tag in a comment.
2. Add the URL to `allow-git` in `deny.toml` (see X-DEP-01).
3. Verify that `grep -rn 'tag = ' crates/*/Cargo.toml` finds no git dependencies.

### X-DEP-18 Root manifest nits: redundant member, unused workspace dependency, 448 MB psionic target dir not ignored

Severity: Low · Category: repo-hygiene · Effort: S

**Locations**
- [Cargo.toml](../../../../Cargo.toml), lines 2-3, 10-11 and 19
- [.gitignore](../../../../.gitignore)
- [crates/psionic/target](../../../../crates/psionic/target)

**Evidence**
- `members = ["crates/coderbench","crates/*"]` lists coderbench, which the glob already covers.
- No crate inherits the `iroh-relay` workspace entry.
- `git check-ignore crates/psionic/target` matches nothing, and `du` reports 448M. There is no tracked `crates/psionic/.gitignore`.

**Impact:** A broad `git add` could commit 448 MB of build output.

**Suggested action**
1. Change `members` to `["crates/*"]`.
2. Delete the `iroh-relay` workspace entry, or make the crates that use it inherit it.
3. Add `/crates/psionic/target/` to `.gitignore`.
4. Verify that `git check-ignore crates/psionic/target` prints the path.

## Verifier adjustments

No reviewer claim was rejected outright. The verifier changed these, so they should not be raised again in their original form:

- **X-DEP-03:** lowered from high to medium because it is hygiene with no correctness impact. The counts were re-measured including dev-dependencies (tokio 124 lines with 85 spellings; sha2 split 73/13).
- **X-DEP-09:** the unwrap count is 607, including tests, not 393.
- **X-DEP-10:** lowered to low. The Bitcoin-only rule covers which rails the product offers, and this is only a transitive dependency.
- **X-DEP-11:** lowered to low. The cost is build time only, and most duplicates are third-party.
- **X-DEP-12:** lowered to low. psionic and mc-bridge are separate on purpose and the docs say so; the risk is drift in patch-level updates.
- **X-DEP-13:** the claim that coder-one is dead was dropped. gym spawns the `coder-one` binary at runtime, so the microluna README's pointer to Coder One is plausibly accurate.
- **X-DEP-14:** 7 root crates plus psionic declare a license, not 9.
