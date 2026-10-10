# Web app & UI primitives

**Scope:** `crates/{openagents-web, openagents-ui, rust-native, rust-native-web, rust-native-desktop, openui-lang, code-highlight, markdown-stream, paper-mono, oa-copy, bunny-web}`
**Snapshot:** 2026-10-10, commit `3168c986aa11e18a8bd30f52c270f609e49b3815`
**Health grade:** B

This area holds about 119.6k lines of Rust in 11 crates. Most of it is `openagents-web` (54.8k LOC, 248 commits). The rest is `rust-native` and its adapters (`rust-native` 14.4k, `rust-native-desktop` 15.5k, `rust-native-web` 1.3k), `openagents-ui` (21.8k, of which 5.3k is generated icons), and several small leaf crates. The engineering discipline is good. There are about 830 tests, and nearly every `unwrap` is inside a test module. CSP and security headers are applied in one place. The layout FFI catches panics and matches its C header. `oa-copy` enforces copy quality in tests, and streaming Markdown and OpenUI parsing live in small shared crates.

The problems are mostly in `openagents-web`, which is now a monolith with 30 path dependencies. Its security-relevant request routing depends on several hand-maintained string lists that have to stay in agreement. It has no graceful shutdown: SIGTERM calls `process::exit`. It still compiles about 1.75k LOC of dead Cloud host and effects code, including raw `libc` unsafe. It also has several modules over 2k lines, a 3.2k-line test file, duplicated GCS clients and auth caches, and inconsistent link-safety policies. In the UI layer, design tokens, syntax highlighters and HTML escapers each exist in two to four copies, and `rust-native` pulls 11 tree-sitter C grammars into every crate that depends on it. Nothing here is urgent. Fix the shutdown path and the proxy's credential and forwarding-header handling first. After that, a typed route registry and a module split would do the most good.

## Measurements

| Crate | LOC (.rs, tracked) | Files | Commits | Test attrs | Dependents |
|---|---:|---:|---:|---:|---:|
| openagents-web | 54,769 | 103 | 248 | 376 | (binary) |
| openagents-ui | 21,795 (icons/generated.rs 5,310) | 72 | 62 | 139 | 1 |
| rust-native-desktop | 15,471 | 25 | 69 | 89 | 3 |
| rust-native | 14,432 | 28 | 58 | 137 | 17 |
| bunny-web | 5,747 | 11 | 8 | 17 | 0 (served wasm) |
| code-highlight | 2,466 | 8 | 6 | 39 | 4 |
| openui-lang | 2,074 | 6 | 1 | 10 | 5 |
| rust-native-web | 1,302 | 4 | 2 | 7 | 3 |
| markdown-stream | 837 | 3 | 3 | 11 | 3 |
| oa-copy | 639 | 1 | 5 | 6 | 12 |
| paper-mono | 107 | 1 | 2 | 3 | 9 |
| **Total** | **~119.6k** | | | **~834** | |

Every crate was touched between 2026-10-04 and 2026-10-09.

- **`unwrap()`/`expect()`** (mostly in tests): web 1370/62, rust-native 331/16, desktop 163/33, ui 39/21. Outside tests, web has about 0 in `chat_store`, `coder_sync` and `traces`, and 2 in `lib.rs` `pay_proxy`. Desktop `window.rs` has 9 GPU/compositor/scene `expect`s.
- **`unsafe`:** rust-native 92 (85 in `layout/ffi.rs`, with 25 SAFETY comments). Web has 10 `libc` blocks: 9 in the dead `cloud/effects.rs` and 1 `flock` in `chat_store.rs:2198`. Desktop has 3.
- **TODO/FIXME:** 2 (`code-highlight/src/grok/open_code.rs:117,217`).
- **`#[allow]`:** web 10 (4 `dead_code`), desktop 12, rust-native 9. About 25 `too_many_arguments` across the area.
- **Largest files:** web `tests.rs` 3,154; `chat_store.rs` 2,834; `traces.rs` 2,434; `pages/chat.rs` 2,275; desktop `layout.rs` 2,631, `window.rs` 2,090; bunny `app.rs` 1,927.
- **Longest functions:** rust-native-web `html.rs` `node` 407 lines; bunny `hud::new` 396; bunny `kit::obstacle` 374; desktop `window_event` 360; rust-native `view::validate` 287; web `main()` 256; `chat.rs` `answer` 217.
- **Hot files since 2026-09-01:** `tests.rs` 79 commits, `lib.rs` 63, `upstream.rs` 45, `pages/chat.rs` 44.
- **openagents-web shape:** about 30 path deps, a 30-field `Config`, and upstream lists with 65 `OWNED_EXACT` entries, 17 `OWNED_PREFIXES` and 9 `REMOVED`.
- **Icons:** 755 variants, 56 referenced outside the icon module.

## Strengths

- **No panics on live web paths.** The store, sync and traces modules keep every `unwrap`/`expect` inside `#[cfg(test)]`. For example, `chat_store.rs` has none before its test module at line 2341. Errors become user-facing problem pages.
- **Security headers are applied in one place.** The `lib.rs` guard sets a default CSP that allows no scripts (`SITE_POLICY`), plus `nosniff` and a referrer policy, on every response. Pages opt into stricter or wasm policies explicitly, and tests assert there is no `unsafe-inline` or `unsafe-eval` (`tests.rs:273-276`).
- **The layout C FFI is well built.** `rust-native/src/layout/ffi.rs` catches panics, caps input size (`MAX_UPDATE_BYTES`), reference-counts frames, and matches `include/rust_native_layout.h` symbol for symbol.
- **Copy quality is checked in tests.** The `oa-copy` guard against machine-talk wording runs in 12 crates' tests, with an allowlist per surface.
- **Small shared crates have clear contracts.** `markdown-stream` gives every chat surface the same streaming cut. `openui-lang` is a streaming parser with typed validation and no code execution. `paper-mono` holds fonts in a `forbid(unsafe_code)` data crate.
- **`openagents-ui/build.rs` needs no manual asset list.** It finds component CSS/JS itself, emits content-hash versions for cache busting, and reruns when any one file changes.
- **The durable store code is careful.** It uses atomic writes, `flock`-based local locks, size caps (`MAX_RECORD_BYTES`, `MAX_LIST`), schema strings and retention expiry. Broadcast lag is handled (`chat_live.rs:320,366`).
- **Docs are thorough.** `traces.rs` has a route table in its module doc, desktop `layout.rs` lists its support rules, and the crate READMEs track features.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| WEB-01 | High | error-handling | No graceful shutdown: SIGTERM calls `process::exit(0)`, and `axum::serve` has no `with_graceful_shutdown` | S |
| WEB-02 | Medium | architecture | Security-relevant routing is split across hand-maintained string lists, and the `REMOVED` doc comment is stale | M |
| WEB-03 | Medium | dead-code | About 1,750 LOC of dead Cloud host and effects code is compiled and configurable | S |
| WEB-04 | Medium | security | `pay_proxy` forwards site cookies and `Authorization` to the pay upstream, and unwraps request data | S |
| WEB-05 | Medium | security | The upstream proxy trusts client-sent XFF and X-Forwarded-Host and has no request timeout | S |
| WEB-06 | Medium | maintainability | Large mixed-purpose modules (`chat_store`, `traces`, `pages/chat`, `coder_sync`) | L |
| WEB-07 | Medium | duplication | Duplicate GCS clients that authenticate through the metadata server | M |
| WEB-08 | Medium | duplication | Identical global auth caches in two modules, plus an avatar cache with no byte limit | S |
| WEB-09 | Medium | duplication | coder-ui re-implements syntect highlighting and includes code-highlight's assets by relative path | S |
| WEB-10 | Medium | build | rust-native always pulls 11 tree-sitter C grammars into every dependent | S |
| WEB-11 | Low | testing | A single 3,154-line `tests.rs` | M |
| WEB-12 | Low | maintainability | 30-field `Config`, hand-written argument parser, duplicated GitHub redirect logic | M |
| WEB-13 | Low | security | HTML built with string concatenation alongside maud, and four separate HTML escapers | M |
| WEB-14 | Low | security | Prose and components use different link-safety rules | S |
| WEB-15 | Low | duplication | rust-native-desktop hard-codes theme tokens instead of using oa-tokens | M |
| WEB-16 | Low | dead-code | `Backend` trait with one real implementation; every deployment logs "(development backend)" | S |
| WEB-17 | Low | dead-code | The retired pilot page still accepts `POST /pilot` | S |
| WEB-18 | Low | maintainability | Desktop `Shell` is a struct of `Option`s unwrapped with `expect`, and `window_event` is 360 lines | L |
| WEB-19 | Low | maintainability | About 25 `too_many_arguments` suppressions in the layout and paint code | M |
| WEB-20 | Low | build | Lints and manifest fields differ between crates | S |
| WEB-21 | Low | testing | The flow.js test passes without node; the openagents-ui build succeeds without token files | S |
| WEB-22 | Low | testing | bunny-web's largest modules are wasm-only and have no native tests | M |
| WEB-23 | Low | performance | 755 generated icons always compiled in, with a linear name lookup | S |

### WEB-01 No graceful shutdown: SIGTERM calls std::process::exit(0) and axum::serve has no with_graceful_shutdown

**Severity:** High · **Category:** error-handling · **Effort:** S

**Locations:** [main.rs:241](../../../../crates/openagents-web/src/main.rs), [main.rs:251](../../../../crates/openagents-web/src/main.rs), [main.rs:262](../../../../crates/openagents-web/src/main.rs)

**Evidence:**
- `main.rs:243-252` waits for SIGTERM, flushes analytics, then calls `std::process::exit(0)`.
- `axum::serve(listener, ...)` at lines 262-266 has no `.with_graceful_shutdown`. There is none anywhere in the crate.
- When no analytics store is configured, SIGTERM falls through to the default handler.
- The `spawn_expiry` `JoinHandle` (main.rs:175) is dropped.

**Impact:** Every Cloud Run rollout or scale-in immediately cuts open chat SSE streams and any GCS conversation writes in progress.

**Suggested action:**
1. Build a shutdown future that resolves on SIGTERM or ctrl-c.
2. Pass it to `axum::serve(...).with_graceful_shutdown(...)`.
3. Flush analytics after `serve` returns instead of inside the signal task.
4. Give SSE handlers a `CancellationToken` so they can end their streams cleanly.
5. To verify, add an integration test that starts the router on an ephemeral port, opens a slow request, signals shutdown, and asserts that the request completes.

### WEB-02 Security-relevant request routing is spread across several hand-maintained string lists, with a stale contract on REMOVED

**Severity:** Medium (lowered from High) · **Category:** architecture · **Effort:** M

**Locations:** [lib.rs:336](../../../../crates/openagents-web/src/lib.rs), [lib.rs:401](../../../../crates/openagents-web/src/lib.rs), [upstream.rs:109](../../../../crates/openagents-web/src/upstream.rs), [upstream.rs:131](../../../../crates/openagents-web/src/upstream.rs), [upstream.rs:138](../../../../crates/openagents-web/src/upstream.rs), [traces.rs:71](../../../../crates/openagents-web/src/traces.rs)

**Evidence:** `guard` (lib.rs:336-420) classifies each request with inline lists: `browser`, `agent` (`agent_work::path`), `intake`, `cloud` (14 checks) and `chat`. It then calls `upstream::owned`, which combines `/api/stats`, `/api/flow/`, `OWNED_EXACT`, `agent_ready::owns`, `phone_api::owns`, `OWNED_PREFIXES` (17) and `REMOVED` (9). None of these lists is derived from the `.route()` registrations. The `REMOVED` doc comment (upstream.rs:129-130) says "They answer `404` here", but the list contains `/trace` and `/traces`, and `/trace` is the live public page (`traces::PUBLIC`, traces.rs:71). This works only because `owned()` treats `REMOVED` entries as owned, so those requests fall through to the router.

**Impact:** A new route can require edits to up to four separate lists. The guard decides what is forwarded upstream and what is served locally, so a missed entry either proxies a path to the legacy upstream or serves it on the wrong host. The stale `REMOVED` comment invites someone to "fix" `/trace` into a real 404.

**Suggested action:**
1. Add `src/routes.rs` with a typed `classify(path) -> Class`, where `Class` is one of `Public`, `LocalOnly`, `Credential`, `Chat`, `AgentWork`, `Intake`, `Removed` or `Proxied`.
2. Make `guard`, `upstream::owned` and `agent_work::path` all call `classify`.
3. Move `/trace` and `/traces` out of `REMOVED` into the owned lists, and correct the `REMOVED` doc comment.
4. To verify, add a table test that maps representative paths to their expected outcome (forwarded, local or public).

### WEB-03 About 1,750 LOC of dead Cloud host-binding and effects-journal code is compiled and configurable

**Severity:** Medium (lowered from High) · **Category:** dead-code · **Effort:** S

**Locations:** [cloud/mod.rs:12](../../../../crates/openagents-web/src/cloud/mod.rs), [lib.rs:145](../../../../crates/openagents-web/src/lib.rs), [lib.rs:148](../../../../crates/openagents-web/src/lib.rs), [main.rs:52](../../../../crates/openagents-web/src/main.rs), [main.rs:61](../../../../crates/openagents-web/src/main.rs)

**Evidence:**
- `cloud/mod.rs:12-17` declares `#[allow(dead_code)] mod effects;` and `#[allow(dead_code)] pub mod hosts;`, with the comment "nothing calls them until it lands".
- `effects.rs` is 1,093 lines and `hosts.rs` is 657, about 1,750 in total.
- `cloud_hosts` and `cloud_build` appear only at their field definitions (lib.rs:145,148), their defaults (lib.rs:197-198) and their assignments in main.rs:52,61. Nothing reads them.
- Live code also uses `libc`: `cloud/private.rs:100-123` (`openat`, `geteuid`) and `cloud/custody.rs:481-599` (`O_NOFOLLOW`, `geteuid`).

**Impact:** Unsafe filesystem code and two flags that do nothing ship in the production binary. `allow(dead_code)` hides further rot and adds to every security review.

**Suggested action:**
1. Delete `cloud/effects.rs` and `cloud/hosts.rs`, or move them into the crate that will own `/environments` host execution.
2. Remove `Config.cloud_hosts`, `Config.cloud_build` and their flags. If deploy specs still pass the flags, accept them as no-ops that print one warning.
3. Replace `libc::flock` at `chat_store.rs:2198` with `File::try_lock`.
4. Keep the `libc` dependency, because `private.rs` and `custody.rs` still need it.
5. To verify, run `rg 'allow\(dead_code\)' crates/openagents-web/src/cloud` and expect no hits, then run `cargo test -p openagents-web`.

### WEB-04 pay_proxy forwards site cookies and Authorization to the pay upstream, and unwraps request data

**Severity:** Medium (lowered from High) · **Category:** security · **Effort:** S

**Locations:** [lib.rs:236](../../../../crates/openagents-web/src/lib.rs), [lib.rs:606](../../../../crates/openagents-web/src/lib.rs), [lib.rs:610](../../../../crates/openagents-web/src/lib.rs)

**Evidence:**
- `api_proxy` strips cookies (`request.headers_mut().remove(header::COOKIE)`, lib.rs:606).
- `pay_proxy` (lib.rs:610-627) forwards the request unchanged, after calling `.path_and_query().unwrap()`, `.strip_prefix("/api").unwrap()` and `.expect(...)`.
- Its routes are `GET /api/flow/{*path}` and `GET /api/stats` (lib.rs:236-237).
- Both paths are owned, so the guard's Cloud-cookie check (lib.rs:403) lets `oa_cloud_*` cookies and `sess_` bearer tokens through.

**Impact:** Session credentials reach a separate service that only needs public reads. The pay host is an internal `http://HOST:PORT` sidecar, which limits exposure. The unwraps cannot panic, because both routes are GET-only and always carry the `/api` prefix. The problem is the credentials, not the unwraps.

**Suggested action:**
1. Add a shared `strip_site_credentials(&mut Request)` that removes `COOKIE` and `AUTHORIZATION`, and call it from both `api_proxy` and `pay_proxy`.
2. Replace the unwraps with `let ... else { return StatusCode::BAD_REQUEST }`.
3. To verify, add a test with a fake upstream that records headers, and assert that no `Cookie` or `Authorization` header arrives.

### WEB-05 Generic upstream proxy trusts client-supplied X-Forwarded-For/Host and has no request timeout

**Severity:** Medium · **Category:** security · **Effort:** S

**Locations:** [upstream.rs:190](../../../../crates/openagents-web/src/upstream.rs), [upstream.rs:224](../../../../crates/openagents-web/src/upstream.rs), [upstream.rs:230](../../../../crates/openagents-web/src/upstream.rs), [upstream.rs:235](../../../../crates/openagents-web/src/upstream.rs)

**Evidence:**
- `headers.entry(X_FORWARDED_HOST).or_insert(host)` keeps whatever value the client sent.
- `if !headers.contains_key(X_FORWARDED_FOR)` adds the peer IP only when the client sent no XFF, so a spoofed value passes through unchanged.
- The client built at line 190 with `Client::builder(TokioExecutor::new()).build(connector)` has no timeout, and `self.client.request(...).await` can wait forever.

**Impact:** Upstream services can see client IPs and hosts chosen by an attacker, which can defeat IP-based rate limits and audit trails. A hung upstream holds connections open indefinitely. Whether XFF spoofing is exploitable depends on whether the Cloud Run front end already rewrites XFF, but the proxy itself trusts the client's value.

**Suggested action:**
1. Overwrite `X-Forwarded-Host` with the validated `Host`.
2. Append the peer IP to any existing XFF, or replace XFF unless the request came through a configured trusted front end.
3. Wrap requests that are not upgrades in `tokio::time::timeout`, and return 504 when it expires.
4. To verify, add tests with a fake upstream that echoes headers and another that sleeps.

### WEB-06 openagents-web god modules: chat_store.rs mixes domain model, storage backends, a GCS client, expiry and validation

**Severity:** Medium (lowered from High) · **Category:** maintainability · **Effort:** L

**Locations:** [chat_store.rs:22](../../../../crates/openagents-web/src/chat_store.rs), [chat_store.rs:1168](../../../../crates/openagents-web/src/chat_store.rs), [chat_store.rs:2322](../../../../crates/openagents-web/src/chat_store.rs), [traces.rs:1](../../../../crates/openagents-web/src/traces.rs), [pages/chat.rs:1](../../../../crates/openagents-web/src/pages/chat.rs), [coder_sync.rs:1](../../../../crates/openagents-web/src/coder_sync.rs)

**Evidence:** Re-measured line counts:
- `chat_store.rs` is 2,834 lines. It holds the GCS client constants (lines 25-27), `impl Gcs` with token caching (line 1168) and `spawn_expiry` (line 2322).
- `traces.rs` is 2,434 lines.
- `pages/chat.rs` is 2,275 lines.
- `coder_sync.rs` is 1,764 lines.

**Impact:** These files change often, so concurrent agents hit merge conflicts in them. Domain rules and storage mechanics are tangled together, so a storage change can break validation. No defect is attached to this finding.

**Suggested action:**
1. Split `chat_store` into `chat_store/{model,validate,disk,gcs,expiry,mod}.rs`, keeping the `Store` interface and its `pub use` re-exports.
2. Split `pages/chat.rs` into answer, follow and claude modules.
3. Make each split a pure move in its own commit, and run `cargo test -p openagents-web` after each one.

### WEB-07 Duplicated GCS-over-metadata-server clients

**Severity:** Medium · **Category:** duplication · **Effort:** M

**Locations:** [chat_store.rs:25](../../../../crates/openagents-web/src/chat_store.rs), [chat_store.rs:1168](../../../../crates/openagents-web/src/chat_store.rs), [analytics/store.rs:21](../../../../crates/openagents-web/src/analytics/store.rs), [analytics/store.rs:75](../../../../crates/openagents-web/src/analytics/store.rs)

**Evidence:**
- `chat_store.rs:25-27` and `analytics/store.rs:21-23` define the same `STORAGE_API` and `METADATA_TOKEN` constants.
- The metadata endpoint also appears in `crates/verse-private/src/broker.rs`, `crates/coder-cloud/src/pool.rs` and `crates/inference/src/upstream/google.rs`.
- Timeouts differ: analytics sets connect and request timeouts on two clients (store.rs:75-84), while `chat_store` sets only `connect_timeout` (line 573).

**Impact:** Fixes to token refresh, retries and size limits have to be repeated in each copy.

**Suggested action:**
1. Extract a shared `MetadataToken` and a minimal GCS object client (get, put with generation match, list, delete, explicit timeouts) into one module or crate.
2. Port `chat_store` and `analytics/store` first, then the other users.
3. To verify, run `rg METADATA_TOKEN crates` and expect a single definition.

### WEB-08 Identical process-global auth caches copied between modules, plus an unbounded-bytes avatar cache

**Severity:** Medium · **Category:** duplication · **Effort:** S

**Locations:** [chat_owner.rs:116](../../../../crates/openagents-web/src/chat_owner.rs), [coder_sync.rs:975](../../../../crates/openagents-web/src/coder_sync.rs), [account.rs:31](../../../../crates/openagents-web/src/account.rs), [account.rs:38](../../../../crates/openagents-web/src/account.rs)

**Evidence:**
- `chat_owner.rs:116-138` and `coder_sync.rs:975-995` contain the same code: a static `OnceLock<Mutex<HashMap<[u8;32],(String,Instant)>>>`, the same `remembered` and `remember` functions, and the same clear-all at 4096 entries.
- `account.rs` keeps a global avatar map that is cleared when it exceeds 1024 entries (line 60). Each entry can hold up to 512 KiB (`AVATAR_MAX_BYTES`, line 31).

**Impact:** Revocation behaviour can diverge between the two copies, and the global statics share state across tests. The avatar cache can reach about 512 MiB in the worst case, though real GitHub avatars are much smaller.

**Suggested action:**
1. Create one `SessionCache`, store it in `App`, and use it from both modules.
2. Give the avatar cache a total byte budget, for example 32 MiB with LRU eviction.
3. To verify, add unit tests for TTL expiry and for byte-budget eviction.

### WEB-09 coder-ui re-implements syntect highlighting and includes code-highlight's assets by cross-crate relative path

**Severity:** Medium · **Category:** duplication · **Effort:** S

**Locations:** [coder-ui syntax.rs:12](../../../../crates/coder-ui/src/components/syntax.rs), [syntax.rs:16](../../../../crates/coder-ui/src/components/syntax.rs), [syntax.rs:21](../../../../crates/coder-ui/src/components/syntax.rs), [code-highlight Cargo.toml:9](../../../../crates/code-highlight/Cargo.toml)

**Evidence:** `coder-ui/syntax.rs:12-25` sets up its own syntect and two_face highlighter in a `OnceLock`. It loads `include_bytes!("../../../code-highlight/assets/grok-build/grok-night.tmTheme")` and `include_str!("../../../code-highlight/assets/grok-build/Swift.sublime-syntax")`, while `code-highlight` already provides the same setup behind its `grok` feature.

**Impact:** The two syntect setups can drift apart, and coder-ui stops compiling if code-highlight moves these asset files.

**Suggested action:**
1. Expose `code_highlight::grok::{syntax_set(), theme(...)}`.
2. Make coder-ui depend on code-highlight with `features = ["grok"]`, and delete coder-ui's own setup.
3. To verify, run `rg 'code-highlight/assets' crates --type rust` and expect matches only inside code-highlight.

### WEB-10 rust-native unconditionally pulls 11 tree-sitter C grammars into its 18 dependent manifests

**Severity:** Medium · **Category:** build · **Effort:** S

**Locations:** [rust-native Cargo.toml:8](../../../../crates/rust-native/Cargo.toml), [Cargo.toml:33](../../../../crates/rust-native/Cargo.toml), [Cargo.toml:35](../../../../crates/rust-native/Cargo.toml), [code-highlight Cargo.toml:16](../../../../crates/code-highlight/Cargo.toml)

**Evidence:**
- rust-native depends on `code-highlight` with no feature gate (Cargo.toml:33).
- On non-wasm targets, code-highlight compiles 11 grammar crates plus `tree-sitter-highlight`.
- rust-native's description still says "Experimental", although the shipped apps depend on it.
- rust-native's own `[lints]` table leaves out `unexpected_cfgs` and `linker_messages`, which the workspace lints set.
- About 17-18 `Cargo.toml` files reference rust-native, depending on whether its own manifest is counted.

**Impact:** Every crate that only needs view types pays for compiling the C grammars, and the "experimental" label misrepresents how the crate is used.

**Suggested action:**
1. Add a default-on `syntax` feature that gates `code-highlight` and the `syntax` module.
2. Set `default-features = false` in dependents that never highlight code.
3. Rewrite the description.
4. Inherit `[lints] workspace = true`, or copy the full lint set with a comment pointing at the workspace lints.
5. To verify, run `cargo tree -p <a view-only dependent> -i tree-sitter` and expect nothing.

### WEB-11 Monolithic 3,154-line tests.rs

**Severity:** Low · **Category:** testing · **Effort:** M

**Locations:** [tests.rs:1](../../../../crates/openagents-web/src/tests.rs), [tests.rs:836](../../../../crates/openagents-web/src/tests.rs), [tests.rs:1705](../../../../crates/openagents-web/src/tests.rs)

**Evidence:** `tests.rs` is 3,154 lines. It holds the node test (around line 836) and the connected-backend fake (`impl backend::Backend for Connected`, line 1705), alongside host, CSP and page tests.

**Impact:** The file causes frequent merge conflicts, and each page's tests sit far from the page's code.

**Suggested action:**
1. Move each module's tests next to that module.
2. Keep shared helpers in `test_support.rs`.
3. To verify, confirm the test count is unchanged with `cargo test -p openagents-web -- --list`.

### WEB-12 Config is a 30-field struct and main() is a hand-rolled parser with duplicated GitHub redirect logic

**Severity:** Low (lowered from Medium) · **Category:** maintainability · **Effort:** M

**Locations:** [lib.rs:73](../../../../crates/openagents-web/src/lib.rs), [main.rs:12](../../../../crates/openagents-web/src/main.rs), [main.rs:144](../../../../crates/openagents-web/src/main.rs), [main.rs:166](../../../../crates/openagents-web/src/main.rs), [main.rs:260](../../../../crates/openagents-web/src/main.rs)

**Evidence:**
- `main.rs` is 268 lines in total, of which `main()` is 256.
- The `--github-app` and `--github-oauth` branches each repeat the same match on `(github_redirect, config.cloud)`, with their own error strings (main.rs:140-185).
- The startup line hard-codes "(development backend)" (line 260).

**Impact:** Each new flag makes this one function longer, and the precedence between environment variables and flags is easy to get inconsistent. No defect is attached to this finding.

**Suggested action:**
1. Extract `fn github_redirect(explicit, cloud) -> Result<String>`.
2. Move parsing to a `clap` derive `Cli` with an `into_config()` method, and unit-test its error combinations.
3. Group the `Config` fields into sub-structs.

### WEB-13 String-concatenated HTML beside maud, with four separate HTML escapers

**Severity:** Low · **Category:** security · **Effort:** M

**Locations:** [tasks.rs:154](../../../../crates/openagents-web/src/tasks.rs), [layout.rs:27](../../../../crates/openagents-web/src/layout.rs), [markdown.rs:232](../../../../crates/openagents-web/src/markdown.rs), [openagents-ui actions/html.rs:17](../../../../crates/openagents-ui/src/actions/html.rs), [rust-native-web html.rs:52](../../../../crates/rust-native-web/src/html.rs)

**Evidence:**
- `tasks.rs:154-162` builds its page with `format!` and 26 `escape(` calls, and renders `Queue` and `Execution` with `{:?}` Debug formatting.
- There are four `fn escape` definitions: `layout.rs:27`, `markdown.rs:232`, openagents-ui `actions/html.rs:17` and rust-native-web `html.rs:52`.

**Impact:** In markup built with `format!`, one missed escape is an XSS bug, and Debug output exposes internal enum names. Exposure is limited today: `tasks.rs` is the local-only task browser (the `browser` class in `guard`), and its interpolated values are escaped.

**Suggested action:**
1. Convert `tasks.rs`, and any other page that builds HTML with `format!`, to `maud::html!`.
2. Replace `{:?}` with explicit label functions.
3. Reduce the escapers to one shared function, or to maud alone.
4. To verify, run `rg 'fn escape' crates` and expect one definition.

### WEB-14 Divergent link-safety policies for rendered answers

**Severity:** Low · **Category:** security · **Effort:** S

**Locations:** [openui-lang tree.rs:131](../../../../crates/openui-lang/src/tree.rs), [markdown.rs:350](../../../../crates/openagents-web/src/markdown.rs), [markdown.rs:210](../../../../crates/openagents-web/src/markdown.rs)

**Evidence:**
- `safe_href` (tree.rs:131-143) allows only `https` links and site paths, and rejects whitespace, control characters, quotes and backslashes.
- `markdown::resolve` (markdown.rs:350-355) also allows `http://`, `mailto:` and `#` anchors, and does not filter characters.
- `link_tag` escapes the href afterwards, so the Markdown path cannot be used for injection, and both paths block `javascript:`.

**Impact:** The same answer can produce different links in prose and in a component, and any future policy change has to be made in more than one place. This is an inconsistency, not a vulnerability.

**Suggested action:**
1. Add one shared `link_target(href) -> Option<LinkKind>` in markdown-stream, using `safe_href`'s character filter.
2. Call it from both `resolve` and `safe_href`.
3. To verify, run one shared table test against both call sites.

### WEB-15 rust-native-desktop hard-codes its own theme tokens instead of using oa-tokens

**Severity:** Low · **Category:** duplication · **Effort:** M

**Locations:** [theme.rs:1](../../../../crates/rust-native-desktop/src/theme.rs), [rust-native-desktop Cargo.toml:1](../../../../crates/rust-native-desktop/Cargo.toml)

**Evidence:** `theme.rs:1` describes itself as "the OpenAgents theme tokens (#10022)" and contains about 28 colour literals. `rust-native-desktop/Cargo.toml` does not depend on oa-tokens.

**Impact:** Desktop and web colours can drift apart.

**Suggested action:**
1. Build `Theme` from the oa-tokens constants.
2. To verify, add a test that compares the desktop colour ladder with the oa-tokens values.

### WEB-16 Vestigial Backend abstraction; every deployment logs '(development backend)'

**Severity:** Low · **Category:** dead-code · **Effort:** S

**Locations:** [backend.rs:27](../../../../crates/openagents-web/src/backend.rs), [backend.rs:44](../../../../crates/openagents-web/src/backend.rs), [main.rs:260](../../../../crates/openagents-web/src/main.rs), [tests.rs:1705](../../../../crates/openagents-web/src/tests.rs)

**Evidence:** `trait Backend` has one production implementation, `Development` (backend.rs:44). The only other implementation is the test fake at tests.rs:1705. main.rs:260 always prints "(development backend)".

**Impact:** The log line misleads operators, and the trait implies a data path that does not exist.

**Suggested action:**
1. Either delete the trait and the pages that depend on it, or implement a real backend.
2. Make the startup line print the features that are actually configured.

### WEB-17 Retired pilot page still accepts POST /pilot submissions

**Severity:** Low · **Category:** dead-code · **Effort:** S

**Locations:** [pilot.rs:266](../../../../crates/openagents-web/src/pilot.rs), [pilot.rs:283](../../../../crates/openagents-web/src/pilot.rs), [main.rs:92](../../../../crates/openagents-web/src/main.rs)

**Evidence:**
- `pilot.rs:266-270` routes `.route("/pilot", get(retired).post(submit))`, and `GET /pilot/install` also maps to `retired`.
- `submit` writes to the intake only when `--pilot-config` is set (main.rs:92).
- The archived copy of the page is compiled only under `#[cfg(test)]` (pilot.rs:283).

**Impact:** A form endpoint stays live for an offer nobody can see. It does nothing unless `--pilot-config` is passed.

**Suggested action:**
1. Make `POST /pilot` return the same 404 as `GET`, or remove the module, `Config.pilot` and `--pilot-config`.
2. Move the sales-intake and sales-remote binaries next to `coder::task::sales`.

### WEB-18 rust-native-desktop Shell is a bag of Options unwrapped with expect, with a 360-line window_event

**Severity:** Low · **Category:** maintainability · **Effort:** L

**Locations:** [window.rs:451](../../../../crates/rust-native-desktop/src/window.rs), [window.rs:1474](../../../../crates/rust-native-desktop/src/window.rs)

**Evidence:** `window.rs` is 2,090 lines. The first 25 lines of the `Shell` struct (from line 451) include about 11 `Option` fields. The file has 9 calls to `expect("the gpu" | "a compositor" | "a scene")`.

**Impact:** A bug in lifecycle ordering would panic the app instead of being handled. No such crash has been observed.

**Suggested action:**
1. Add a `Phase` enum whose `Running` variant holds the GPU, compositor and scene state, which are always present while the app runs.
2. Split `window_event` into one handler per event.
3. To verify, run `rg 'expect\("(the gpu|a compositor|a scene)' crates/rust-native-desktop` and expect no hits.

### WEB-19 too_many_arguments suppressed at about 25 sites in the layout and paint engines

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:** [rust-native layout/rows.rs:168](../../../../crates/rust-native/src/layout/rows.rs), [rust-native-desktop layout.rs:930](../../../../crates/rust-native-desktop/src/layout.rs)

**Evidence:** The reviewer counted 25 `#[allow(clippy::too_many_arguments)]` (not re-counted during verification). For example, `docked_pane` takes several positional `bool` arguments.

**Impact:** Positional `bool` and `f32` arguments are easy to swap without any compiler error.

**Suggested action:**
1. Introduce parameter structs such as `PaneSpec`, `TextStyleArgs` and `RowCtx`.
2. Remove each `allow` as its function is converted.
3. To track progress, use the count from `rg -c 'too_many_arguments' crates/rust-native crates/rust-native-desktop`.

### WEB-20 Lint and manifest inconsistencies across the area's crates

**Severity:** Low · **Category:** build · **Effort:** S

**Locations:** [openagents-ui Cargo.toml:1](../../../../crates/openagents-ui/Cargo.toml), [oa-copy Cargo.toml:1](../../../../crates/oa-copy/Cargo.toml), [paper-mono Cargo.toml:3](../../../../crates/paper-mono/Cargo.toml), [openagents-web Cargo.toml:7](../../../../crates/openagents-web/Cargo.toml)

**Evidence:**
- openagents-ui and oa-copy have no `[lints]` section.
- paper-mono hard-codes `version = "0.1.0"` and `edition = "2024"` instead of inheriting them from the workspace.
- openagents-web's description still ends with "the local read-only task browser", which no longer describes what the crate serves.

**Impact:** The workspace lint denies (`dbg_macro`, `todo`, `unimplemented`) do not apply to openagents-ui or oa-copy, and the stale description misleads newcomers.

**Suggested action:**
1. Add `[lints] workspace = true` to openagents-ui and oa-copy.
2. Make paper-mono inherit `version` and `edition` from the workspace.
3. Rewrite the openagents-web description.
4. To verify, run `cargo clippy -p openagents-ui -p oa-copy` and expect a clean result.

### WEB-21 flow.js test silently passes without node; openagents-ui build continues without token files

**Severity:** Low · **Category:** testing · **Effort:** S

**Locations:** [tests.rs:836](../../../../crates/openagents-web/src/tests.rs), [openagents-ui build.rs:98](../../../../crates/openagents-ui/build.rs)

**Evidence:**
- At `tests.rs:836`, `Err(_) => eprintln!("node is not installed; static/flow.test.js did not run")` lets the test pass when node is missing.
- In `build.rs`, `append` only emits a `cargo:warning` and returns when a token file is missing.

**Impact:** Tests can be skipped without anyone noticing, and a build can succeed while producing a broken stylesheet.

**Suggested action:**
1. Fail the flow test when `CI` is set and node is not installed.
2. Panic in `build.rs` when a token file is missing, unless `OPENAGENTS_UI_BLESS` is set.

### WEB-22 bunny-web's largest modules are wasm-only and untested natively

**Severity:** Low · **Category:** testing · **Effort:** M

**Locations:** [bunny-web lib.rs:27](../../../../crates/bunny-web/src/lib.rs)

**Evidence:** `app`, `hud` and `outline` are compiled only under `#[cfg(target_arch = "wasm32")]` (lib.rs:27-32). Tests cover only the native modules.

**Impact:** Input and HUD regressions only show up in a browser.

**Suggested action:**
1. Extract the pure game-state logic from `app.rs` into a module that builds on every target.
2. Add unit tests for that module, runnable with `cargo test -p bunny-web`.

### WEB-23 755 generated icons compiled in, with linear name lookup

**Severity:** Low · **Category:** performance · **Effort:** S

**Locations:** [icons/generated.rs:1](../../../../crates/openagents-ui/src/icons/generated.rs), [icons/mod.rs:71](../../../../crates/openagents-ui/src/icons/mod.rs)

**Evidence:** There are 755 icon variants, and only 56 are referenced outside the icon module. `from_name` searches linearly with `Icon::ALL.into_iter().find(...)`, and its doc comment says it is "for catalogs and tooling".

**Impact:** The compile unit is larger than it needs to be. The lookup cost does not matter much, because only catalogs use it.

**Suggested action:**
1. Put the full icon set behind a `full-icon-set` feature that the `/ui` catalog enables.
2. Optionally, generate a `match` for `from_name`.

## Refuted during verification

- **"code-highlight streaming highlighter has an acknowledged O(lines^2) per-stream clone."** Rejected. `open_code.rs:217-219` documents this as an accepted residual and gives the reason: the clone only copies precomputed spans, the expensive syntect parse is already O(N) in total, and the surrounding render is O(N) per pass. The memo-hit comment at line 117 says the same. The behaviour is intended and documented.
- **WEB-02 "lists disagree and misroute requests."** Narrowed. `/trace` is served correctly: `REMOVED` counts as owned, so the request reaches the router. Only the doc comment is wrong. The other drift example (a literal `"/theme"` instead of the constant) is cosmetic.
- **WEB-03 "libc exists almost entirely for the dead code."** Rejected. `cloud/private.rs` and `cloud/custody.rs` use `libc` in live code, so the dependency stays.
