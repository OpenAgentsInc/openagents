# Verse world & zones

Scope: `crates/{verse-world, verse-zone-everglade, verse-zone-grove, verse-zone-lab, verse-zone-lagrange, verse-zone-coast, verse-zone-water, verse-zone-crypt, everglade-web}` plus `assets/verse`. Snapshot `3168c986aa`, audited 2026-10-10.

**Health grade: C**

This area has about 172.6k lines of Rust across 9 crates (verse-world 93k, verse-zone-everglade 60k, plus 7 small crates) and a 271 MB asset tree in `assets/verse`. All of it is new. Every crate was first committed between 2026-10-03 and 2026-10-08, and verse-world alone has 272 commits. The code inside each module is careful. Server and networking code has almost no panicking calls outside tests, wire and queue sizes are bounded, pinned packs are fetched and verified defensively, there are about 980 tests, and every `docs/verse` reference in the code points to a file that exists. The problems come from fast growth without consolidation. First, repository weight: every pack "repin" commits another 10-28 MB `.vtp`, and the large-file hook allowlists `assets/*`. Second, crate coupling: the general-purpose zone-pack format and compiler live inside the Everglade crate, so coast, water and grove depend on all of Everglade and its app-level dependencies. Inside verse-world, a 39k-line server and persistence stack shares a crate with the simulation core. Checkpoint decoding repeats about 60 fields by hand, and errors are mostly `String`s. There are two concrete correctness bugs: cache pruning skips the 7 oldest packs, and the two browser identity loaders handle a bad stored key in different ways.

## Measurements

| Metric | Value |
|---|---|
| Rust LOC (tracked `.rs`) | verse-world 93,199 (150 files); verse-zone-everglade 59,663 (118 files); grove 8,217; lab 2,621; lagrange 1,640; coast 1,338; water 1,165; crypt 995; everglade-web 3,809. Total about 172.6k |
| `#[test]` count (excludes `tokio::test`) | verse-world 618, everglade 299, grove 12, lab 12, coast 12, crypt 11, lagrange 3, water 3, everglade-web 2 (native-only; about 3.7k wasm lines untested) |
| unwrap counts | Raw: verse-world 5,963, everglade 428, almost all in tests. Production sites: play.rs 19, admission.rs 17 (`lock().unwrap()`), realm.rs 16, multiplayer.rs 13, rules.rs 9. Production `panic!`/`unreachable!` in `service/`: 0 |
| Error typing | 697 `Result<_, String>` in verse-world; 1 error enum (`social/nav.rs`) |
| `#[allow]` | 26 sites, mostly `clippy::too_many_arguments` (7 in everglade `compute/draw.rs`), plus 1 `dead_code` and 1 `unused_mut` |
| TODO/FIXME | 0 |
| Largest files | play.rs 5,366 (2,842 prod + 2,524 test); prediction/local.rs 3,693; demolition/town.rs 3,143; service/client.rs 3,110 (1,197 prod + 1,912 test); service/worker.rs 3,044 (774 prod + 2,270 test); net.rs 2,931; demolition/site.rs 2,807; wire.rs 2,383 |
| verse-world surface | 1,973 `pub` items; 31 top-level modules; `service/` 39,381 LOC |
| Dependents | verse-world: 17 crates; verse-zone-everglade: 6 (verse, verse-water-spells, grove, coast, verse-bake, water) |
| Git activity | verse-world 272 commits (first 2026-10-03); everglade 120 (first 10-05); everglade-web 44; lab and lagrange 1 each |
| `assets/verse` | 1,813 tracked files, 271.4 MB in HEAD, 0 LFS files. History: 3,104 blobs, 1,005.8 MB raw; 42 `.vtp` versions; 26 PNGs over 2 MB (132 MB total, several 4096x4096 at 10-14 MB); 35 duplicate blobs (0.85 MB) |
| Local `.git` | 9.7 GB |

## Strengths

- Server and networking production code has essentially no panics. `service/wire.rs`, `net.rs`, `view.rs` and `session_pipeline.rs` have zero `panic!`/`unreachable!` before their test modules. Test-heavy helpers are kept out of production builds with `#[cfg(test)]` (prediction.rs:2 latency, persistence.rs:13 checks, worker.rs:2 delayed).
- Network inputs are explicitly bounded: wire.rs:9-10 (`MAX_REQUEST_BYTES` 16 KiB, `MAX_RESPONSE_BYTES` 2 MiB) and net.rs:38-46 (`QUEUE` 128, `REPLY_BYTES`, `MAX_HELD_REPLY_BYTES`). The persistence `Committed` record uses `deny_unknown_fields` and a domain-separated SHA-256 digest (persistence.rs:19-33).
- Pinned pack loading is hardened. It uses HTTPS only, `redirect::Policy::none`, connect and total timeouts, an exact length and SHA-256 check, `create_new` temp files followed by rename, and `O_NOFOLLOW`/`fstatat`/`unlinkat` directory-relative pruning with SAFETY comments (everglade_pack/pinned.rs:53-60, 155-159, 225-239, 278-322).
- The test suite is large (about 980 `#[test]` plus many tokio tests) and includes deterministic scenario tests. For example, verse-zone-coast/src/pack.rs:94-99 recompiles the bundled coast pack and asserts that its digest is byte-identical.
- Docs discipline is good. All 19 distinct `docs/verse/*.md` paths cited in code exist, module headers explain intent and SRD sources, and there are 0 TODO/FIXME markers.
- Townsfolk content is data-driven. `verse-zone-everglade/build.rs` compiles the NPC and rumor JSON into the binary, so adding content needs no code change, and the roster admits files by digest.
- Feature guards keep dev-only behaviour out of release builds. lib.rs:14-18 uses `compile_error!` to block dev-destruction on web/phone, and tests scan the release scripts.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| VW-01 | High | repo-hygiene | Each pack repin commits a new 10-28 MB `.vtp` (42 versions, about 540 MB in history), and the size hook is bypassed | M |
| VW-02 | High | architecture | The generic zone-pack format and compiler live inside the Everglade crate, so other zones depend on all of it | L |
| VW-03 | Medium | correctness | Cache prune reads only the first 32 of 39 history digests, so the 7 oldest packs are never deleted | S |
| VW-04 | Medium | build | verse-zone-water declares 5 unused deps; grove and everglade also carry unused deps | S |
| VW-05 | Medium | maintainability | `GameWire` repeats about 60 fields and keeps a hand-written list of 22 Player keys | M |
| VW-06 | Medium | architecture | verse-world combines a 39k-line server, persistence layer and ledger with the simulation core | XL |
| VW-07 | Medium | correctness | The browser identity bootstrap is written twice: Grid overwrites a bad key, chamber refuses to start | S |
| VW-08 | Medium | error-handling | Errors are stringly typed, and a retry loop matches on `"storage_busy"` in its own JSON | L |
| VW-09 | Medium | repo-hygiene | 26 PNGs over 2 MB (132 MB) are committed without LFS or compression | M |
| VW-10 | Medium | architecture | The Everglade zone crate is also the integration point for app services | L |
| VW-11 | Low | maintainability | Spell code is split across two modules with the same name (`crate::gust` and `crate::spells::gust`) | M |
| VW-12 | Low | duplication | Ten foot-to-metre constants exist, and Grove mixes `FT = 0.3` with `0.3048` in one file | S |
| VW-13 | Low | maintainability | Very large files mix production and test code | L |
| VW-14 | Low | testing | everglade-web has about 3.7k wasm-only lines with effectively no tests | M |
| VW-15 | Low | error-handling | Invariant lookups panic in realm leasing (`get_mut().unwrap()`) and admission (`lock().unwrap()`) | S |
| VW-16 | Low | duplication | The ritual fixture is loaded with `include_bytes!` at about 90 sites | S |
| VW-17 | Low | build | The dev profile builds the most-edited Verse crates at opt-level 3 | S |
| VW-18 | Low | maintainability | verse-zone-water is a re-export shim with an out-of-date description | S |
| VW-19 | Low | testing | Small zone crates have weak test coverage (lagrange 3, water 3) | S |

### VW-01 Every pack repin commits a new 10-28 MB incompressible .vtp: 42 versions and about 540 MB in history, with the size hook bypassed

Severity: High · Category: repo-hygiene · Effort: M

Locations:
- [everglade_pack.rs](../../../../crates/verse-zone-everglade/src/zones/everglade_pack.rs): everglade_pack.rs:9-18, everglade_pack.rs:144-151
- [large-files-allowlist.txt](../../../../scripts/dev/large-files-allowlist.txt): large-files-allowlist.txt:9
- [size.md](../../../../docs/repo/size.md): size.md:84

Evidence: `git log --all -- 'assets/verse/**/*.vtp'` shows 42 distinct `.vtp` paths. docs/repo/size.md:84 records `vtp (Verse terrain) | 315 MB | 540 MB | 42`. 45 commits match `--grep=repin` (e.g. 372dbb932e, aa628e69e7, d98c5c653b). The module doc (lines 9-18) tells contributors to rebuild and update `PACK_SHA256` after any source change. Allowlist line 9 is the blanket `assets/*`. `pinned().url` resolves to `raw.githubusercontent.com/OpenAgentsInc/openagents/main/{PACK_DIRECTORY}/{sha}.vtp`. The local `.git` is 9.7 GB.

Impact: Each content tweak permanently adds 10-28 MB of data that does not delta-compress to every clone. The allowlist defeats the documented size policy, and pack hosting depends on git `main`.

Suggested action:
1. Publish compiled packs to a content-addressed bucket (the `gs://openagents-bench-artifacts` pattern from `scripts/bench-artifacts.py`) or to release assets. Change `pinned().url` in everglade_pack.rs:144-151 and the kit and coast pack pins to match. Keep only the digest and byte-length constants in git.
2. Replace `assets/*` in `scripts/dev/large-files-allowlist.txt` with explicit source globs, and deny `*.vtp`.
3. Optionally, follow size.md option 3 (LFS) for the remaining source media.
4. Verify: `check-large-files.sh` rejects a staged 12 MB `.vtp`.

### VW-02 The generic zone-pack format and compiler live inside the Everglade zone crate, so other zones depend on the whole Everglade crate

Severity: High · Category: architecture · Effort: L

Locations:
- [verse-zone-coast/src/pack.rs](../../../../crates/verse-zone-coast/src/pack.rs): pack.rs:4, pack.rs:49, pack.rs:94
- [verse-zone-coast/src/kit.rs](../../../../crates/verse-zone-coast/src/kit.rs): kit.rs:139
- [verse-zone-coast/Cargo.toml](../../../../crates/verse-zone-coast/Cargo.toml): Cargo.toml:20
- [verse-zone-everglade/Cargo.toml](../../../../crates/verse-zone-everglade/Cargo.toml): Cargo.toml:9-60

Evidence: Coast imports only `verse_zone_everglade::zones::everglade_pack::format::{Limits, ZonePack, decode, AlphaMode}` and `compile::compile` (pack.rs:4, 49, 94; kit.rs:139), but it depends on the full crate (Cargo.toml:20). Everglade's features pull in `coder` (model-host), `openagents-connect` (studio-host), `openagents-chat-app` (desktop) and `pylon` (pylon-relay), and `coder-ui` and other crates are always on.

Impact: An Everglade edit recompiles coast, grove, water, verse-water-spells and verse-bake. Zones cannot be built or tested in isolation, and Everglade internals have become de facto public API.

Suggested action:
1. Extract `everglade_pack/{format.rs, compile*, pinned.rs, kit_bake.rs}` into a new `verse-pack` crate. Its dependencies should be only gltf, png, miniz_oxide, sha2, serde, glam and verse-engine, plus reqwest and libc on native.
2. Re-export it from `everglade_pack` for compatibility.
3. Repoint verse-zone-coast and verse-bake at `verse-pack`, and move `examples/coast_pack.rs` into verse-zone-coast.
4. Verify: `cargo tree -p verse-zone-coast -e normal | grep -c verse-zone-everglade` prints 0.

### VW-03 Pack cache pruning reads only the first 32 history digests, so the 7 oldest Everglade packs are never deleted from users' caches

Severity: Medium · Category: correctness · Effort: S

Locations:
- [pinned.rs](../../../../crates/verse-zone-everglade/src/zones/everglade_pack/pinned.rs): pinned.rs:326, pinned.rs:349-351
- [everglade_pack.rs](../../../../crates/verse-zone-everglade/src/zones/everglade_pack.rs): everglade_pack.rs:87-129

Evidence: `EVERGLADE_PACK_HISTORY` has 39 entries (`PACK_SHA256` plus 38 digests, lines 89-129), with the oldest last. `prune()` runs `for digest in self.history.iter().take(32).filter(|&&d| d != self.sha256)`, so entries 33-39 (`df0e440c…`, `2c6d0e58…`, `f29673c6…`, `3bbdbfc0…`, `12b5f3d7…`, `4bbd3b18…`, `b57e33f7…`) are never unlinked. The comment at lines 87-88 says arbitrary digest names in the shared cache are not treated as ours, and the temp sweep matches only temp names, so nothing else removes them. On non-unix targets prune is a no-op: `pub fn prune(&self, _cache: &Path) {}`.

Impact: Players whose cache holds these old packs keep about 119 MB of dead files indefinitely. Each repin past 32 entries adds another unpruned pack.

Suggested action:
1. Remove `.take(32)`. The history is a compile-time const, so its length is already fixed. Alternatively, add `const _: () = assert!(EVERGLADE_PACK_HISTORY.len() <= 32);`.
2. A better fix: give each pinned file its own cache subdirectory and prune every `<64-hex>.<ext>` file except the current digest.
3. Implement non-unix prune using `symlink_metadata().is_file()` and `remove_file`.
4. Verify: add a test with a 40-entry history that asserts only the current file survives.

### VW-04 verse-zone-water declares 5 unused dependencies and makes verse-zone-everglade a normal dependency for one test constant; grove and everglade also carry unused deps

Severity: Medium · Category: build · Effort: S

Locations:
- [verse-zone-water/Cargo.toml](../../../../crates/verse-zone-water/Cargo.toml): Cargo.toml:9-19
- [verse-zone-water/src/lib.rs](../../../../crates/verse-zone-water/src/lib.rs): lib.rs:1-9
- [verse-zone-water/src/coast_tests.rs](../../../../crates/verse-zone-water/src/coast_tests.rs)
- [verse-zone-grove/Cargo.toml](../../../../crates/verse-zone-grove/Cargo.toml): Cargo.toml:10-13
- [verse-zone-everglade/Cargo.toml](../../../../crates/verse-zone-everglade/Cargo.toml): Cargo.toml:45

Evidence: verse-zone-water's `[dependencies]` list verse-content (compiler), verse-core, verse-gfx, verse-world, verse-zone-everglade and verse-zone-grove. An `rg -l` for those crate names in `crates/verse-zone-water` finds only `src/coast_tests.rs`. In verse-zone-grove, `rg serde` matches only Cargo.toml lines 12-13, and `coder_ui` matches 0 files. In verse-zone-everglade, no `.rs` file references `serde` (`serde_json` is used in 12 files).

Impact: A 9-line re-export crate compiles grove and all of Everglade, and verse-zone-coast inherits that cost. These false dependency edges inflate rebuild sets.

Suggested action:
1. Remove verse-content, verse-core, verse-gfx, verse-world and verse-zone-grove from verse-zone-water `[dependencies]`, and move verse-zone-everglade to `[dev-dependencies]`.
2. Remove coder-ui, serde and serde_json from `verse-zone-grove/Cargo.toml`, and remove serde from `verse-zone-everglade/Cargo.toml`.
3. Add `cargo-machete` to the lint lane for `crates/verse*`.
4. Verify: `cargo check` and `cargo test` pass for the three crates, and `cargo machete crates/verse-zone-*` reports nothing.

### VW-05 Game checkpoint decoding repeats ~60 fields in GameWire and keeps a hand-written 22-key list of flattened Player fields

Severity: Medium · Category: maintainability · Effort: M

Locations:
- [play.rs](../../../../crates/verse-world/src/play.rs): play.rs:189-246, play.rs:256-323, play.rs:325-412
- [play/multiplayer.rs](../../../../crates/verse-world/src/play/multiplayer.rs): multiplayer.rs:8-44
- [play/caster.rs](../../../../crates/verse-world/src/play/caster.rs): caster.rs:596-597

Evidence: `GameWire` (play.rs:259) repeats Game's fields. The `Deserialize` impl (line 325 onward) moves a literal list of 22 keys (`admission` … `died_at`) into `primary`. `GameWire` has no `deny_unknown_fields`, so if a new Player field is left off the list, the key stays at top level, is ignored, and the primary player gets the `#[serde(default)]` value. A checkpoint round-trip test exists (caster.rs:596-597), but it catches this only when its fixture sets a non-default value for the new field.

Impact: Adding a Player field means remembering to update the key list. If someone forgets, restoring a checkpoint silently resets that field for the primary player. Adding a Game field takes 3 edits.

Suggested action:
1. Derive the key list from Player. For example, define a `const FIELDS` and add a test that checks it against `serde_json::to_value(sample_player).keys()`, or serialize a Player and move every matching key.
2. Add `#[serde(deny_unknown_fields)]` to `GameWire`, or a test asserting that no top-level key collides with a Player field.
3. Longer term, move the non-primary Game fields into a sub-struct so `GameWire` becomes `primary: Player` plus `#[serde(flatten)] rest`.
4. Verify: a test that adds a non-default value to every Player field survives a checkpoint round trip.

### VW-06 verse-world mixes a 39k-line networked server, persistence and rewards ledger with the portable simulation core

Severity: Medium · Category: architecture · Effort: XL

Locations:
- [verse-world/Cargo.toml](../../../../crates/verse-world/Cargo.toml): Cargo.toml:7
- [service.rs](../../../../crates/verse-world/src/service.rs): service.rs:5-51
- [lib.rs](../../../../crates/verse-world/src/lib.rs): lib.rs:239-268

Evidence: The crate description is "Portable command admission and owned world authority for Verse", but `src/service` holds 39,381 lines. Re-measured file sizes: play.rs 5,366, client.rs 3,110, worker.rs 3,044.

Impact: A rules edit recompiles the server stack for all 17 dependents. Each feature combination adds to the build matrix, and durability changes share one review surface with game-rule changes.

Suggested action:
1. Split `service/*` and the features `service-net`, `service-reach` and `reach-client` into a new `verse-service` crate, leaving a rules-only `verse-world`.
2. Add a temporary `pub use verse_service as service` re-export, then migrate the call sites.
3. Verify: `cargo tree -p verse-world -e normal | grep -c tokio` prints 0.

### VW-07 The browser identity bootstrap is written twice with different failure behaviour: Grid overwrites an unparsable stored key, chamber refuses to start

Severity: Medium · Category: correctness · Effort: S

Locations:
- [chamber.rs](../../../../crates/everglade-web/src/chamber.rs): chamber.rs:451-467
- [grid.rs](../../../../crates/everglade-web/src/grid.rs): grid.rs:29-35, grid.rs:73-90

Evidence: chamber.rs uses the literal `"openagents.grid.secret"`. It returns an error on an invalid secret (`from_secret_hex("Grid", &secret)?`) and on a `set_item` failure. grid.rs does `match get(SECRET_KEY).map(...) { Some(Ok(identity)) => identity, _ => { …random_secret()…; set(SECRET_KEY, …) } }`, which overwrites an unparsable value, and `let _ = storage.set_item(key, value)` ignores write failures. The profiles also differ: chamber uses `"Grid"` and grid.rs uses `PROFILE = "browser"`.

Impact: One page silently replaces a player's identity, which loses the name and blocklists tied to the old pubkey. The other page refuses to load. The duplicated key literal and the different profile strings will drift further apart.

Suggested action:
1. Add `everglade-web/src/identity.rs` with `fn browser_identity(window, profile) -> Result<Identity, String>`, and call it from both pages.
2. Never overwrite an unparsable stored value. Either return an error or move the value aside to `openagents.grid.secret.bad`.
3. Treat a `set_item` failure as an error, and use one profile string.
4. Verify: unit-test the decision table natively (missing, valid, invalid and write-failure cases).

### VW-08 Pervasive stringly-typed errors, including an internal retry loop that parses its own JSON and matches on "storage_busy"

Severity: Medium · Category: error-handling · Effort: L

Locations:
- [service/net.rs](../../../../crates/verse-world/src/service/net.rs): net.rs:1227-1259
- [service/realm.rs](../../../../crates/verse-world/src/service/realm.rs): realm.rs:400-433

Evidence: net.rs:1233 starts a loop with a 2 s deadline. Line 1248 runs `serde_json::from_slice(&result.0)`, lines 1251-1252 check `response.body.kind != "refused" || response.body.code.as_deref() != Some("storage_busy")`, and line 1257 sleeps a fixed 33 ms. realm.rs returns string literals such as `"Realm authority epochs exhausted"`. verse-world has 697 `Result<_, String>` signatures and one error enum.

Impact: Retry behaviour depends on wire strings, and callers cannot match on errors.

Suggested action:
1. Have the host channel return a typed refusal (e.g. `Refusal::StorageBusy`) alongside the reply bytes, and match on that in `request_with_storage_backpressure`.
2. Introduce a `RealmError` enum for the lease errors in realm.rs.
3. Verify: add a test showing the retry triggers on the typed value even if the wire `code` string changes.

### VW-09 Oversized raw PNG textures (26 files over 2 MB, 132 MB) committed without LFS

Severity: Medium · Category: repo-hygiene · Effort: M

Locations:
- [7598fa7b…eb9a67280f7b13c61e28a9110eeb679.png](../../../../assets/verse/characters/quaternius/textures/7598fa7b63e46fd4dac29708bf5a81e05eb9a67280f7b13c61e28a9110eeb679.png)
- [T_Page_Noise.png](../../../../assets/verse/props/quaternius/T_Page_Noise.png)

Evidence: `git ls-tree -r -l HEAD assets/verse` lists 26 PNGs over 2 MB, totalling 132.4 MB. The largest are character textures at 14.1 MB, 12.7 MB and 10.8 MB. `git lfs ls-files | grep -c assets/verse` prints 0.

Impact: The textures add clone weight, and 4K textures put pressure on mobile GPU memory.

Suggested action:
1. In `scripts/blender/everglade_admit.py`, re-encode textures to KTX2/Basis, or downscale them to 2048 and run oxipng.
2. Consider LFS for `assets/**/*.png`, per docs/repo/size.md option 3.
3. Verify: `git ls-tree -r -l HEAD assets/verse | awk '$4>2097152 && /\.png$/'` returns no rows (or only rows in LFS).

### VW-10 The Everglade zone crate is also the integration point for app services (studio host, chat app, Pylon relay, private-asset CLI)

Severity: Medium · Category: architecture · Effort: L

Locations:
- [verse-zone-everglade/Cargo.toml](../../../../crates/verse-zone-everglade/Cargo.toml): Cargo.toml:9-25
- [src/bin/verse_private](../../../../crates/verse-zone-everglade/src/bin/verse_private)

Evidence: The crate's features include `model-host = ["dep:coder"]`, `studio-host = ["dep:openagents-connect"]`, `desktop = ["dep:openagents-chat-app"]` and a `pylon-relay` feature. It ships `src/bin/verse_private`, even though a separate `crates/verse-private` crate exists.

Impact: Zone changes and product-service changes trigger each other's builds, and the zone cannot be reused in lightweight hosts.

Suggested action:
1. Define small traits in the zone crate (`StudioSource`, `ComputeFieldSource`, `NoticeSink`) and implement them in the `verse` app crate.
2. Move the `verse_private` bin into `crates/verse-private`.
3. Verify: `cargo tree -p verse-zone-everglade --all-features` no longer lists openagents-chat-app, openagents-connect or pylon.

### VW-11 Spell code is split into two modules with the same name in verse-world (crate::gust and crate::spells::gust, etc.)

Severity: Low · Category: maintainability · Effort: M

Locations:
- [lib.rs](../../../../crates/verse-world/src/lib.rs): lib.rs:239-268
- [spells/mod.rs](../../../../crates/verse-world/src/spells/mod.rs): mod.rs:10-31
- [spells/black_tentacles.rs](../../../../crates/verse-world/src/spells/black_tentacles.rs): black_tentacles.rs:1-2

Evidence: Nine files in `spells/` import the root module under an `as rules` alias: wall_of_stone, wind_wall, gust, reverse_gravity, feather_fall, meteor_swarm, telekinesis, black_tentacles and levitate. For example, `use crate::{black_tentacles as rules, play::Game};`. spells/mod.rs:10-12 says "To add a spell, write spells/<name>.rs" and does not mention the root rules module.

Impact: Contributors cannot tell where spell logic belongs, and the contributing note is incomplete.

Suggested action:
1. Move each root rules module to `spells/<name>/rules.rs`, with the adapter in `spells/<name>/mod.rs`. Keep the root `pub use` re-exports temporarily.
2. Update spells/mod.rs:10-12 to describe the layout.
3. Verify: `rg 'as rules' crates/verse-world/src` returns 0 matches.

### VW-12 Ten separate foot-to-metre constants plus inline 0.3048 literals, and Grove mixes FT = 0.3 and 0.3048 in the same file

Severity: Low · Category: duplication · Effort: S

Locations:
- [verse-world/src/spells/mod.rs](../../../../crates/verse-world/src/spells/mod.rs): mod.rs:44
- [verse-world/src/gust.rs](../../../../crates/verse-world/src/gust.rs): gust.rs:25
- [verse-world/src/wall_of_stone/mod.rs](../../../../crates/verse-world/src/wall_of_stone/mod.rs): mod.rs:30
- [verse-water-spells/src/spells.rs](../../../../crates/verse-water-spells/src/spells.rs): spells.rs:34
- [verse-water-spells/src/rules.rs](../../../../crates/verse-water-spells/src/rules.rs): rules.rs:148
- [grove/kit.rs](../../../../crates/verse-zone-grove/src/zones/grove/kit.rs): kit.rs:38
- [grove/cast.rs](../../../../crates/verse-zone-grove/src/zones/grove/cast.rs): cast.rs:25, cast.rs:36

Evidence: A `FEET`/`FOOT = 0.3048` constant is defined 10 times: in spells/mod.rs, gust, feather_fall, telekinesis, reverse_gravity, meteor_swarm, black_tentacles, wind_wall, wall_of_stone/mod.rs and verse-water-spells/spells.rs. Inline `* 0.3048` appears in grove cast.rs:25, 278 and 563 and in water-spells rules.rs:148, 371 and 611. grove kit.rs:38 has `pub const FT: f32 = 0.3;`, documented as "at the combat model's 5 ft = 1.5 m". cast.rs:36 uses it (`ICE_BURST = 5.0 * FT`) right next to cast.rs:25 (`GUST_PUSH = 15.0 * 0.3048`).

Impact: Two foot conventions coexist in one Grove file, and the constant is copied into each spell.

Suggested action:
1. Keep `verse_world::spells::FEET` and add an `f32` twin. Alias the per-module constants to it.
2. Replace the inline `0.3048` literals in grove/cast.rs and verse-water-spells/rules.rs.
3. Rename Grove's `FT` to `GRID_FT`, keeping its comment, so the intentional 5 ft = 1.5 m grid convention is explicit, and use it consistently within cast.rs.
4. Verify: `rg '0\.3048' crates/verse-*` matches only the canonical definition.

### VW-13 God files that mix production and test code: play.rs 5,366, client.rs 3,110, worker.rs 3,044, plus large Everglade demolition files

Severity: Low · Category: maintainability · Effort: L

Locations:
- [play.rs](../../../../crates/verse-world/src/play.rs)
- [service/client.rs](../../../../crates/verse-world/src/service/client.rs)
- [service/worker.rs](../../../../crates/verse-world/src/service/worker.rs)
- [demolition/town.rs](../../../../crates/verse-zone-everglade/src/zones/everglade/demolition/town.rs)

Evidence: Re-measured line counts: play.rs 5,366, client.rs 3,110, worker.rs 3,044. The crate already uses sibling test files elsewhere (realm.rs:692-693, `#[cfg(test)] mod tests;`).

Impact: Reviews are slower, and many concurrent agents editing the same files cause more merge conflicts.

Suggested action:
1. Move inline test modules longer than about 500 lines into sibling files (`#[cfg(test)] #[path = "client_tests.rs"] mod tests;`) for client.rs, worker.rs, play.rs and prediction/local.rs.
2. Split demolition/town.rs by concern.
3. Add a simple `wc`-based cap on production-section length in `scripts/dev`.
4. Verify: no production section exceeds the cap, and `cargo test -p verse-world` reports the same test count.

### VW-14 everglade-web has about 3.7k lines of wasm-only code with effectively no tests

Severity: Low · Category: testing · Effort: M

Locations:
- [presence_ui.rs](../../../../crates/everglade-web/src/presence_ui.rs)
- [everglade-web/Cargo.toml](../../../../crates/everglade-web/Cargo.toml)

Evidence: Only presence_ui.rs has `#[test]` (2). `grep -c '"Navigator"' Cargo.toml` prints 2, meaning the web-sys feature is listed twice.

Impact: The browser entry point (pack verification, identity, input) can regress without any test noticing.

Suggested action:
1. Extract platform-neutral logic (query parsing, pack URL and digest selection, the identity decision from VW-07) into modules that compile on all targets, and unit-test them.
2. Add `wasm-bindgen-test` coverage for the pack verification path.
3. Remove the duplicate `"Navigator"` feature.
4. Verify: `cargo test -p everglade-web` runs the new native tests.

### VW-15 Server-side state panics on invariant lookups: get_mut().unwrap() in realm leasing and Mutex lock().unwrap() in admission

Severity: Low · Category: error-handling · Effort: S

Locations:
- [service/realm.rs](../../../../crates/verse-world/src/service/realm.rs): realm.rs:411, realm.rs:431-432
- [service/net/admission.rs](../../../../crates/verse-world/src/service/net/admission.rs): admission.rs:133, admission.rs:276-277

Evidence: realm.rs:411 has `self.games.get_mut(&instance).unwrap().close_all()?;`, and lines 431-432 have similar calls, both inside functions that already return `Result`. admission.rs has 13 production `lock().unwrap()` or `clients.get_mut(&self.serial).unwrap()` sites before its test module at line 414.

Impact: A desync between games and the manifest, or a panic while the admission mutex is held (which poisons it), would take down the host instead of refusing a single request. No concrete desync path was shown; these are invariant lookups on maps the same struct maintains.

Suggested action:
1. In realm.rs, use `.ok_or("Realm instance missing")?`.
2. In admission.rs, use `lock().unwrap_or_else(PoisonError::into_inner)` (or switch to parking_lot), and treat a missing client serial as a no-op.
3. Verify: add a realm test that removes a games entry and asserts that the call returns `Err` instead of panicking.

### VW-16 The ritual test fixture is loaded via include_bytes! at ~90 sites instead of one helper

Severity: Low · Category: duplication · Effort: S

Locations:
- [verse-world/src/play.rs](../../../../crates/verse-world/src/play.rs)
- [verse-world/src/combat.rs](../../../../crates/verse-world/src/combat.rs)
- [verse-imported/src/imported/overlay.rs](../../../../crates/verse-imported/src/imported/overlay.rs)

Evidence: `grep -rn 'ritual.json' crates | wc -l` prints 94.

Impact: Moving the fixture would take about 90 edits.

Suggested action:
1. Add `#[cfg(any(test, feature = "test-fixtures"))] pub mod fixtures { pub fn ritual() -> Scene }` to verse-world, loading the file with `concat!(env!("CARGO_MANIFEST_DIR"), …)`.
2. Replace the call sites with `fixtures::ritual()`.
3. Verify: `grep -rn 'ritual.json' crates` matches only the helper.

### VW-17 Dev profile builds the most-edited Verse crates at opt-level 3

Severity: Low · Category: build · Effort: S

Locations:
- [Cargo.toml](../../../../Cargo.toml): Cargo.toml:60-100

Evidence: `[profile.dev.package.*]` sets `opt-level = 3` for verse-world, verse-zone-everglade, verse-zone-grove, verse-zone-lab and verse-zone-lagrange (and for verse-gym, verse-host, verse-engine and others), but not for verse-zone-coast, water or crypt. This is probably deliberate, to keep frame rates playable in dev builds, but the cost has not been measured.

Impact: Every edit-compile cycle on 150k lines pays for full optimisation, and the crate list is inconsistent.

Suggested action:
1. Measure with `cargo build -p verse-world --timings` after a one-line edit.
2. If the cost is significant, drop the most-edited crates to opt-level 1, or move opt-level 3 into a dedicated `verse-play` profile used by the run scripts.
3. Make the zone crate list consistent either way.

### VW-18 verse-zone-water is a re-export shim whose description no longer matches its contents

Severity: Low · Category: maintainability · Effort: S

Locations:
- [verse-zone-water/Cargo.toml](../../../../crates/verse-zone-water/Cargo.toml): Cargo.toml:7
- [verse-zone-water/src/lib.rs](../../../../crates/verse-zone-water/src/lib.rs): lib.rs:1-9

Evidence: The description reads "Verse's Water Lab zone: a cove with a sea, a river, a waterfall…", but lib.rs is only `pub use verse_water_spells::*; pub mod coast;`, with the comment "these re-exports preserve the Water Lab's public paths".

Impact: One area is spread across three crates, and the compatibility shim has no removal plan.

Suggested action:
1. Move coast.rs and coast_tests.rs into verse-zone-coast.
2. Point `verse/src/zones/mod.rs` at `verse_water_spells`.
3. Delete verse-zone-water. This also resolves VW-04's water items.
4. Verify: `cargo check -p verse` passes and no crate references `verse_zone_water`.

### VW-19 Weak test coverage in the small zone crates (lagrange 3 tests for 1.6k LOC, water 3)

Severity: Low · Category: testing · Effort: S

Locations:
- [verse-zone-lagrange/src/lib.rs](../../../../crates/verse-zone-lagrange/src/lib.rs)
- [verse-zone-lagrange/src/draw.rs](../../../../crates/verse-zone-lagrange/src/draw.rs)
- [verse-zone-lagrange/src/light.rs](../../../../crates/verse-zone-lagrange/src/light.rs)

Evidence: `#[test]` counts: lib.rs 1, draw.rs 2, light.rs 0.

Impact: Lighting and mesh output are not verified.

Suggested action:
1. Add bounds and vertex-count tests for structure generation in draw.rs.
2. Add output-range tests for light.rs.
3. Verify: `cargo test -p verse-zone-lagrange` covers light.rs.

## Refuted during verification

- **Staged townsfolk proposals are stale content committed beside the admitted roster.** `crates/townsfolk/src/files.rs:1-9` documents the checked-in townsfolk directory, with `proposals/ID.json` as a staged proposal awaiting owner admission, and `lib.rs:21` describes proposals that anyone may stage. Tracking them is the designed review workflow. The clustered `clippy::too_many_arguments` allows and the single `dead_code` allow are too minor to stand as a finding.
- **Coast LOD1/LOD2 files are byte-identical duplicates of the base model.** The `lod/*.source.json` files set mode `Reference` for meshes already under the triangle budget (e.g. `beach.net.lod2`: 112 triangles against a 599 budget, 18,180 bytes). The passthrough is intentional and costs a negligible amount of space.
- **The primary-player legacy serde aliases make old flat checkpoints fail.** Before commit 58f8b25934 introduced Player and the aliases, Game's flat fields were already named `player`/`previous_player`/`pending_movement`/`held_movement`, so top-level `position`/`previous`/`pending_move`/`held_move` keys never existed. The aliases serve the `additional_players` layout, which caster.rs:600-642 tests.
- **wall_of_stone has no `spells/` adapter (from the VW-11 review).** `spells/wall_of_stone.rs` exists and imports the root module `as rules`.
