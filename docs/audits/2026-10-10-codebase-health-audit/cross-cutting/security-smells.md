# Security smells

Scope: repo-wide security smells across owned code (crates/psionic, vendor and bench excluded). Snapshot `3168c986aa11e18a8bd30f52c270f609e49b3815`, audit date 2026-10-10.

**Health grade: B**

For a codebase this size, the security posture is sound. Owned code never disables TLS verification. SQL binds user values as parameters. String-formatted SQL appears only with compile-time constants or internally minted identifiers. Every secret-shaped string in tracked files was checked without printing it, and each one is a labelled fake or a test fixture. Most `sh -c` call sites pass data as positional `$1`/`$2` arguments rather than interpolating it. The browser-facing auth layer is careful. It uses sealed CSRF tickets bound to viewer and origin, strict `return_to` validation and PKCE flow cookies. Push-gateway uses NIP-98 with replay protection. Stripe signatures are checked with `verify_slice` and a bounded tolerance. The desktop update manifest is Ed25519-signed.

The weaknesses sit at trust boundaries, not in classic injection:

- The Nostr relay trusts the leftmost, client-controlled `X-Forwarded-For` entry.
- openagents-web grants "local" privilege based on the `Host` header.
- The Coder CLI updater on Linux and Windows trusts an unsigned checksum file.
- terminal-core uses a weak scrubber on blocks sent to models.

The second theme is duplication. Security primitives are re-implemented instead of shared: shell quoting, constant-time compare, HTML escaping, redacting Secret types, Bearer parsing, webhook signature verification and credential scrubbing. The copies have already started to drift: `billing.rs` accepts only the last `v1` tag. Nothing scans for secrets automatically on commit or in CI.

## Measurements

| Metric | Value |
|---|---|
| Rust in scope | ~2.34M lines, 6,704 tracked `.rs` files, 184 crates |
| TS/JS files | 476, mostly vendored or static |
| HTTP servers inspected (LOC) | gateway 72,380 (serve.rs 6,008); openagents-web 54,769; coder-host 30,574; nostr-relay 27,795; inference 23,796; x402 13,145; oa-auth 9,426; push-gateway 4,730; laya 3,985; pay-host 2,692 |
| `TcpListener::bind` sites outside tests | 21; only verse_assets hard-codes `0.0.0.0` (Cloud Run, NIP-98) |
| TLS-verification bypass patterns | 0 |
| `Command::new(sh\|bash)` sites | 72 (about 20 outside tests, nearly all positional-arg) |
| Hand-written shell-quote helpers | 17 by the reviewer's count; 13 files match the exact pattern |
| Hand-rolled constant-time compares | 5 (`subtle` is already a dependency and used at 3 sites) |
| HTML escape helpers | 16 definitions outside psionic |
| Redacting Debug impls for Secret/Key/Token types | 20 across about 15 crates |
| Debug-derived structs holding raw secret strings | at least 8 (5 cited below) |
| `strip_prefix("Bearer ")` parsers | 22 non-test sites in 16 files |
| Credential redactors besides secret-screen | at least 6 |
| Unbounded HTTP response body reads | 29 sites outside tests |
| `format!`-built SQL | 11 sites, all internal or constant identifiers |
| Secret-shaped strings in tracked files | about 400, all fakes, fixtures or false positives |
| Committed key material | test TLS keys in `crates/coder-host/tests/fixtures/tls/*.key` |
| CI workflows | none (`.github` holds only `ISSUE_TEMPLATE`) |
| `unsafe` occurrences (excluding psionic and vendor) | 892 |

## Strengths

- Owned code has no TLS-verification bypasses. There are 0 hits for `danger_accept_invalid_certs`, `NoCertificateVerification` and `rejectUnauthorized:false`.
- SQL is parameterized throughout. tenancy and pay-ledger format only internal identifiers or constants into SQL, and user values always go through `$n` or `?` bindings.
- openagents-web CSRF is strong. Sealed tickets bind scope, target, origin and a digest of the viewer credential ([session.rs](../../../../crates/openagents-web/src/cloud/session.rs) session.rs:714-790), and every state-changing form checks them.
- `oa_auth::return_to` ([flow.rs](../../../../crates/oa-auth/src/flow.rs) flow.rs:38-54) is a careful open-redirect guard. It rejects `//`, backslashes, `%5c`, `%2f`, control characters and `/auth/`, and it confirms the joined URL stays on the same host.
- Bounded parsing is the house style:
  - axum `DefaultBodyLimit` on gateway, laya and card-funding
  - `MAX_HTTP_HEAD_BYTES` in the relay
  - bounded websocket frames in coder-reach
  - `MAX_ENTRY` plus a top-level name allowlist in the updater's unpacker
- The desktop updater verifies an Ed25519-signed manifest against compiled-in keys, then checks codesign and notarization ([update.rs](../../../../crates/openagents-desktop/src/update.rs) update.rs:1-67).
- push-gateway uses NIP-98 with one attempt per authorization, which gives replay protection. It stores installation tokens only as digests.
- Stripe webhook verification in card_funding.rs:81-135 accepts multiple `v1` tags, bounds tolerance to 1-300 s and uses `Mac::verify_slice`.
- The shared `secret-screen` crate (680 LOC, used by 9 crates) is one rule set for credential shapes.
- `deny.toml` denies unknown registries and git sources, yanked crates and unmaintained advisories. Every ignore carries an owner and a review date.
- A test enumerates the gateway route catalog, so a route cannot be mounted without being listed. oak-mcp-http has an explicit `--allow-origin` allowlist against DNS rebinding.
- Shell call sites mostly pass paths and PIDs as positional arguments, for example openagents-desktop/src/update.rs:1249 and openagents-cli/src/connect/ssh.rs:430.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| X-SEC-01 | High | security | Nostr relay trusts the leftmost, client-controlled XFF: rate-limit bypass and key-table lockout | S |
| X-SEC-02 | Medium | security | openagents-web grants "local" privilege from the Host header | S |
| X-SEC-03 | Medium | security | Coder CLI updater (Linux/Windows) trusts an unsigned SHA256SUMS | M |
| X-SEC-04 | Medium | security | terminal-core scrubber is far weaker than secret-screen | S |
| X-SEC-05 | Medium | security | laya-serve uses `CorsLayer::permissive` on an unauthenticated localhost door | S |
| X-SEC-06 | Medium | security | Gateway cookies lack `Secure`; admin cookie holds the raw admin token | S |
| X-SEC-07 | Medium | security | Cloud pool SSH disables host-key checks and interpolates shell strings | M |
| X-SEC-08 | Medium | security | Debug-derived structs hold raw secret strings | S |
| X-SEC-09 | Low | security | Gateway CORS fails open (`ACAO: *` unless the prefix is listed) | S |
| X-SEC-10 | Low | security | pay_proxy forwards cookies; forward() keeps the client's X-Forwarded-* | S |
| X-SEC-11 | Low | duplication | About 20 separate redacting Secret/Key/Token newtypes | M |
| X-SEC-12 | Low | duplication | POSIX shell-quote helper copy-pasted across crates | S |
| X-SEC-13 | Low | duplication | Five hand-rolled constant-time compares despite `subtle` | S |
| X-SEC-14 | Low | duplication | Two diverging Stripe-style signature verifiers | S |
| X-SEC-15 | Low | maintainability | Duplicated HTML escapers; gateway builds HTML with `format!` | L |
| X-SEC-16 | Low | duplication | Ad-hoc Bearer parsing at 22 sites, case handling differs | S |
| X-SEC-17 | Low | duplication | Several credential redactors alongside secret-screen | M |
| X-SEC-18 | Low | repo-hygiene | No automated secret scanning on commit or in CI | S |
| X-SEC-19 | Low | security | Unbounded HTTP response body reads in outbound clients | S |
| X-SEC-20 | Low | maintainability | tenancy builds SQL literals with manual quote doubling | S |

### X-SEC-01 Nostr relay trusts the client-controlled leftmost X-Forwarded-For

Severity: High · Category: security · Effort: S

**Locations**
- [socket.rs](../../../../crates/nostr-relay/src/gateway/socket.rs) socket.rs:142
- [rate.rs](../../../../crates/nostr-relay/src/gateway/rate.rs) rate.rs:305
- [nostr-relay.conf](../../../../deploy/nginx/nostr-relay.conf) nostr-relay.conf:19
- [nostr-relay.env.example](../../../../deploy/nostr-relay.env.example) nostr-relay.env.example:7

**Evidence.** `effective_ip()` (socket.rs:142-157) returns the first entry of `x-forwarded-for.split(',')`. It reads `x-real-ip` only when XFF is missing or does not parse. The shipped nginx config sets `X-Forwarded-For $proxy_add_x_forwarded_for`, which appends the real peer after any value the client sent, and sets `X-Real-IP $remote_addr`. The env example sets `NOSTR_RELAY_TRUST_PROXY=true`. `allow_ip_for()` (rate.rs:305-314) refuses every new key once `map.len() >= MAX_RATE_KEYS`.

**Impact.** A client chooses its own rate-limit identity, which bypasses every per-IP limit. Spoofing enough distinct addresses within one window fills the key table, after which every new legitimate IP is refused.

**Suggested action**
1. In `effective_ip`, prefer `X-Real-IP` (nginx sets it from `$remote_addr`). Otherwise take the rightmost XFF entry after skipping `NOSTR_RELAY_TRUSTED_HOPS` (default 1). Never use the first entry.
2. Add a unit test: with `trust_proxy=true`, `X-Forwarded-For: 1.2.3.4, 10.0.0.9` resolves to `10.0.0.9`.
3. Document the hop count for Cloud Run fronts in the env example.
4. Verify with the new test, plus a test that a flood of distinct spoofed XFF values on one socket maps to a single key.

### X-SEC-02 openagents-web grants 'local' privilege from the Host header, which bypasses the agent-work sign-in gate

Severity: Medium · Category: security · Effort: S

**Locations**
- [lib.rs](../../../../crates/openagents-web/src/lib.rs) lib.rs:327, lib.rs:336, lib.rs:422
- [agent_work.rs](../../../../crates/openagents-web/src/agent_work.rs) agent_work.rs:88, agent_work.rs:111
- [main.rs](../../../../crates/openagents-web/src/main.rs) main.rs:264
- [upstream.rs](../../../../crates/openagents-web/src/upstream.rs) upstream.rs:205
- [web.sh](../../../../deploy/staging/web.sh) web.sh:43

**Evidence.** `Hosts::local()` compares the raw `Host` header with `127.0.0.1:{port}` and `localhost:{port}`, and `guard()` then sets `LOCAL_HEADER` (lib.rs:422-427). `agent_work::gate` passes every `local_request()`. When sign-in is unavailable, `scope()` returns `Scope{account:None, legacy:true}` for local requests. The server binds `0.0.0.0:8080` (web.sh). The socket peer is already available, because main.rs:264 uses `into_make_service_with_connect_info::<SocketAddr>()` and upstream.rs:205 reads `ConnectInfo`.

**Impact.** Authorization for routes that drive machines rests on a header the client controls. Cloud Run's host routing blocks this, but any front that forwards a foreign `Host` unchanged lets a remote client run agent work without signing in.

**Suggested action**
1. In `guard()`, require `request.extensions().get::<ConnectInfo<SocketAddr>>()` to have a loopback `ip()`, in addition to a local `Host`, before setting `LOCAL_HEADER`.
2. Add a test in `crates/openagents-web/src/tests.rs`: a non-loopback peer sending `Host: 127.0.0.1:PORT` gets no `LOCAL_HEADER` and is gated.
3. Verify that the existing local-dev tests still pass with a loopback peer.

### X-SEC-03 The Coder CLI self-updater on Linux and Windows trusts an unsigned SHA256SUMS from the same bucket as the archive

Severity: Medium · Category: security · Effort: M

**Locations**
- [coder-new update.rs](../../../../crates/coder-new/src/update.rs) update.rs:30, update.rs:469, update.rs:884, update.rs:920, update.rs:1004
- [openagents-desktop update.rs](../../../../crates/openagents-desktop/src/update.rs) update.rs:67, update.rs:357

**Evidence.** `DEFAULT_BASE_URL` points to the GCS release bucket, and the checksum file `SHA256SUMS-coder-{version}` comes from the same base (line 469). The only authenticity check is `check_signature()`. It runs only inside `if let Some(team) = &context.signing_team` (line 920) and uses `codesign`, so it works on macOS only. openagents-desktop already verifies an Ed25519 signature with `UnparsedPublicKey::new(&ED25519, key.public)` (line 357).

**Impact.** Anyone who can write to the release bucket can push code to every auto-updating Linux and Windows Coder install. That includes someone exploiting a bucket ACL mistake.

**Suggested action**
1. Sign `SHA256SUMS-coder-<v>` with an Ed25519 release key in the Coder release script.
2. Compile the public keys into coder-new and verify the signature before trusting any digest. Reuse the openagents-desktop verify code through a small shared module.
3. Add tests that refuse an archive whose checksum matches but whose signature is bad or missing.
4. Verify end to end with a staged release signed by a test key.

### X-SEC-04 terminal-core sends terminal blocks to the model through a keyword scrubber far weaker than secret-screen

Severity: Medium · Category: security · Effort: S

**Locations**
- [smart.rs](../../../../crates/terminal-core/src/smart.rs) smart.rs:99, smart.rs:190
- [context.rs](../../../../crates/terminal-core/src/context.rs) context.rs:54
- [excerpt.rs](../../../../crates/terminal-core/src/excerpt.rs) excerpt.rs:117
- [helpers.rs](../../../../crates/terminal-gfx/src/helpers.rs) helpers.rs:7

**Evidence.** `smart::scrub` redacts a whole line only when it contains one of `authorization:`, `api_key`, `api-key`, `access_token`, `password=` or `secret=`. Otherwise it redacts a word only when, after edge punctuation is trimmed, the word starts with `sk-`, `oak_`, `sess_`, `nsec1`, `ghp_`, `github_pat_`, `gho_` or `ghs_`. These get through:

- `export GITHUB_TOKEN=ghp_x`, because the core word starts with `GITHUB_TOKEN`
- `aws_secret_access_key = …`
- `AKIA…`
- `sk_live_…`
- `xox…`

The scrubber is used by `context.attach` (smart.rs:190, paper.rs:1725), by excerpt (excerpt.rs:117/122) and by terminal-gfx. terminal-core does not depend on secret-screen.

**Impact.** Credentials in attached terminal blocks can reach model providers. Exposure is limited because the user attaches blocks explicitly and sees a preview first.

**Suggested action**
1. Add `secret-screen` to `crates/terminal-core/Cargo.toml`.
2. In `smart::scrub`, call `secret_screen::redact` on each line after the PEM-block state machine runs.
3. Add regression tests for `export GITHUB_TOKEN=ghp_…`, `aws_secret_access_key = …`, `sk_live_…` and `AKIA…`.

### X-SEC-05 laya-serve applies CorsLayer::permissive to an unauthenticated localhost inference door

Severity: Medium · Category: security · Effort: S

**Locations**
- [serve.rs](../../../../crates/laya/src/serve.rs) serve.rs:656
- [laya_serve.rs](../../../../crates/laya/src/bin/laya_serve.rs) laya_serve.rs:44
- [kev Cargo.toml](../../../../crates/kev/Cargo.toml) Cargo.toml:29
- [lev Cargo.toml](../../../../crates/lev/Cargo.toml) Cargo.toml:31

**Evidence.** `router()` mounts `/v1/systemone`, `/v1/models`, `/api/info` and `/healthz` with `.layer(tower_http::cors::CorsLayer::permissive())` and no auth. It defaults to 127.0.0.1:8010. kev and lev both declare the optional tower-http dependency with the `cors` feature, but neither uses it.

**Impact.** Any website the user visits can drive the local model door and read its responses.

**Suggested action**
1. Remove `CorsLayer::permissive`, or replace it with an `--allow-origin` allowlist that defaults to empty, and reject non-allowlisted `Origin`.
2. Remove tower-http from the `serve` features in kev and lev.
3. Add a test that `Origin: https://evil.example` receives no `Access-Control-Allow-Origin`.

### X-SEC-06 Gateway browser cookies lack Secure, and the inference admin cookie stores the raw admin token

Severity: Medium · Category: security · Effort: S

**Locations**
- [dashboard.rs](../../../../crates/gateway/src/dashboard.rs) dashboard.rs:32, dashboard.rs:340
- [inference_status.rs](../../../../crates/gateway/src/inference_status.rs) inference_status.rs:173, inference_status.rs:192

**Evidence.** dashboard.rs documents "no `Secure` flag so a localhost deployment still works" and builds `oa_session={token}; Path=/; HttpOnly; SameSite=Lax`. `inference_status::session` sets `oa_inference_admin={token}; HttpOnly; SameSite=Strict; Path=/admin/inference`. That token is the admin token from the environment, as the check `same(token, &expected)` shows.

**Impact.** Session tokens and the master admin token can travel in cleartext over HTTP. The admin cookie cannot be revoked without rotating the admin token.

**Suggested action**
1. Add `secure_cookies: bool` to the gateway `Config`, defaulting to true when the listen address is not loopback, and append `; Secure`.
2. In inference_status, issue `HMAC(admin_token, issued_at||expiry)` as the cookie value and verify it on each request.
3. Add tests that `Secure` is present and that the cookie value differs from the admin token.

### X-SEC-07 The Cloud pool SSH disables host-key checking and builds ProxyCommand and remote shell strings by interpolation

Severity: Medium · Category: security · Effort: M

**Locations**
- [pool.rs](../../../../crates/coder-cloud/src/pool.rs) pool.rs:337, pool.rs:341, pool.rs:357, pool.rs:520, pool.rs:527

**Evidence.** `ssh()` always passes `StrictHostKeyChecking=no` and `UserKnownHostsFile=/dev/null`. Under `OA_POOL_SSH=internal` it connects directly to the VPC address. `ProxyCommand` is built with `format!("gcloud compute start-iap-tunnel {} 22 ... --project {project} --zone {}", host.name, host.zone)`. `run_dir()` returns `$HOME/.oa-pool/runs/{run}`, which `start_remote` embeds unquoted as `d={dir}`.

**Impact.** In internal mode, an attacker inside the VPC can intercept the session. IAP mode authenticates the tunnel. If a caller ever influences a host, zone or run ID, these strings become shell injection.

**Suggested action**
1. Pin host keys from GCE guest attributes into a per-pool `known_hosts` file and set `StrictHostKeyChecking=yes`.
2. Before formatting, validate `name`, `zone` and `project` against `^[a-z0-9-]{1,63}$` and `run` against `^[A-Za-z0-9_-]{1,64}$`.
3. Add tests that refuse a run ID containing `;`.

### X-SEC-08 Debug-derived config and domain structs hold raw secret strings

Severity: Medium · Category: security · Effort: S

**Locations**
- [relay_worker.rs](../../../../crates/gateway/src/relay_worker.rs) relay_worker.rs:94
- [advertise.rs](../../../../crates/gateway/src/advertise.rs) advertise.rs:49
- [wallet.rs](../../../../crates/nostr/src/domain/wallet.rs) wallet.rs:39
- [oak lib.rs](../../../../crates/oak/src/lib.rs) lib.rs:130
- [config.rs](../../../../crates/wallet/src/config.rs) config.rs:110

**Evidence**

| Struct | Derives | Secret field |
|---|---|---|
| `WorkerConfig` | `Debug, Deserialize` | `worker_secret: Option<String>` |
| `AdvertiseConfig` | `Debug, Deserialize` | `service_secret: Option<String>` |
| `WalletSecrets` | `Clone, Debug, PartialEq, Eq` | `private_key: String` |
| `FileConfig` | `Debug, Default, Deserialize` | `api_key: Option<String>` |
| `Lsp` | `Clone, Debug, ..` | `token: Option<String>` |

**Impact.** A single `{:?}` in an error or log path prints a signing key or an API key.

**Suggested action**
1. Hand-write a redacting `Debug` for these five types, or switch the fields to the shared Secret type from X-SEC-11.
2. Add one test per type asserting that the `{:?}` output does not contain the secret.
3. Then audit the remaining structs in the "at least 8" count, such as those in wallet_connect.rs and remote_sign.rs, which were not re-opened during verification.

### X-SEC-09 Gateway CORS fails open: any GET not on a hand-kept prefix list answers Access-Control-Allow-Origin: *

Severity: Low · Category: security · Effort: S

**Locations**
- [serve.rs](../../../../crates/gateway/src/serve.rs) serve.rs:594, serve.rs:617, serve.rs:684
- [inference_status.rs](../../../../crates/gateway/src/inference_status.rs) inference_status.rs:30
- [inference_public.rs](../../../../crates/gateway/src/inference_public.rs) inference_public.rs:59
- [inference_rates.rs](../../../../crates/gateway/src/inference_rates.rs) inference_rates.rs:17

**Evidence.** `CREDENTIALED_GET` lists 19 prefixes, and `credentialed()` is a `starts_with` check. These authenticated routes are missing from the list and receive `ACAO: *`:

- `/v1/admin/inference/status`
- `/v1/usage/{request_id}`
- `/v1/usage/tokens-served`
- `/v1/rates`
- `/v1/key`
- the cookie-authenticated `/admin/inference` page

None of this is exploitable today, for three reasons:

- `ACAO: *` is never sent together with Allow-Credentials.
- The admin cookie is `SameSite=Strict`.
- The public preflight omits allow-headers, so a cross-origin request cannot send `Authorization`.

**Impact.** The policy fails open, so every new authenticated route gets public CORS by default.

**Suggested action**
1. Invert the check. Keep a `PUBLIC_GET` allowlist (the discovery routes, `/healthz`, `/skills`) and treat everything else as credentialed.
2. Add a test that walks `api_routes()` and asserts that no non-public route emits `access-control-allow-origin: *`.

### X-SEC-10 openagents-web pay_proxy forwards site cookies upstream, and upstream.forward keeps client-supplied X-Forwarded-* headers

Severity: Low · Category: security · Effort: S

**Locations**
- [lib.rs](../../../../crates/openagents-web/src/lib.rs) lib.rs:584, lib.rs:606, lib.rs:610
- [upstream.rs](../../../../crates/openagents-web/src/upstream.rs) upstream.rs:222

**Evidence.** `api_proxy` removes `COOKIE` (lib.rs:606), but `pay_proxy` does not. `forward()` uses `entry(X_FORWARDED_HOST).or_insert(host)` and inserts `X-Forwarded-For` only `if !headers.contains_key`, so a client-supplied value is kept.

**Impact.** Session cookies reach a second first-party service that does not need them. Upstreams that trust the first XFF entry can be spoofed.

**Suggested action**
1. Remove `COOKIE` and `AUTHORIZATION` in `pay_proxy`, as `api_proxy` already does.
2. In `Upstream::forward`, append the `ConnectInfo` peer to `X-Forwarded-For` rather than keeping the client's value, and set `X-Forwarded-Host` from the validated Host.
3. Add a test that a request to `/api/flow` carrying `oa_cloud_session` reaches the fake upstream without a `Cookie` header.

### X-SEC-11 About 20 separate redacting Secret/Key/Token newtypes instead of one shared type

Severity: Low · Category: duplication · Effort: M

**Locations**
- [credentials.rs](../../../../crates/coder-delegate/src/credentials.rs) credentials.rs:40
- [secret.rs](../../../../crates/inference/src/upstream/secret.rs) secret.rs:34
- [github.rs](../../../../crates/oa-auth/src/github.rs) github.rs:375
- [config.rs](../../../../crates/push-gateway/src/server/config.rs) config.rs:39
- [model-access lib.rs](../../../../crates/model-access/src/lib.rs) lib.rs:188

**Evidence.** Outside psionic there are 20 implementations of `impl (fmt::)Debug for (Secret|Key|Keys|Credentials|Keychain|ApiKey)`. Only 5 `Cargo.toml` files depend on `zeroize`.

**Impact.** Redaction and zeroization behavior differ from type to type, and new code has no obvious shared type to reach for.

**Suggested action**
1. Add one `Secret<T>` type, either in secret-screen or in a small `oa-secret` crate. It should have redacting Debug and Display, zeroize on drop, transparent serde and an `expose()` accessor.
2. Migrate the server paths first: inference, oa-auth, push-gateway.
3. Verify with a test that `format!("{:?}", Secret::new("x"))` does not contain `x`. Track the remaining count with the same rg query.

### X-SEC-12 The POSIX shell-quote helper is copy-pasted across many crates

Severity: Low · Category: duplication · Effort: S

**Locations**
- [follow.rs](../../../../crates/boat/src/follow.rs) follow.rs:64
- [ssh.rs (openagents-cli)](../../../../crates/openagents-cli/src/connect/ssh.rs) ssh.rs:425
- [ssh.rs (coder-ssh)](../../../../crates/coder-ssh/src/ssh.rs) ssh.rs:318
- [local_checks.rs](../../../../crates/coder/src/task/local_checks.rs) local_checks.rs:37
- [sanitize.rs](../../../../crates/coder-environment-build/src/sanitize.rs) sanitize.rs:75

**Evidence.** An exact rg for `replace('\'', "'\\''")` matches 13 non-psionic files. The reviewer's figure of 17 also counts variant spellings and closures.

**Impact.** A fix applied to one copy, such as NUL rejection or `~` handling, does not reach the others.

**Suggested action**
1. Add a shared `shell::quote(&str) -> Result<String, NulError>`, plus `quote_home`.
2. Replace the copies.
3. Add a round-trip property test seeded from the existing tests in openagents-cli/src/connect/ssh.rs.

### X-SEC-13 Five hand-rolled constant-time comparisons even though `subtle` is already a dependency

Severity: Low · Category: duplication · Effort: S

**Locations**
- [accounts.rs](../../../../crates/gateway/src/accounts.rs) accounts.rs:1073
- [inference_status.rs](../../../../crates/gateway/src/inference_status.rs) inference_status.rs:137
- [payment_scheme.rs](../../../../crates/x402/src/payment_scheme.rs) payment_scheme.rs:390
- [remote.rs](../../../../crates/coder/src/task/sales/remote.rs) remote.rs:1278
- [analytics/mod.rs](../../../../crates/openagents-web/src/analytics/mod.rs) mod.rs:525
- [oa-auth Cargo.toml](../../../../crates/oa-auth/Cargo.toml)

**Evidence.** An rg for `fold(0u8` finds exactly these 5 sites. oa-auth and nostr already declare `subtle`, and `ConstantTimeEq` is used in oa-auth/src/flow.rs, oa-auth/src/repos/broker.rs and one nostr test.

**Impact.** A security primitive is duplicated in five places. The early length check in these copies also leaks the token's length.

**Suggested action**
1. Add one `token_eq(a, b)` helper in oa-auth or a small shared crate. It should hash both inputs with SHA-256, then compare with `subtle::ct_eq`.
2. Replace all 5 copies.
3. Verify that `rg 'fold\(0u8'` returns 0 hits outside the helper.

### X-SEC-14 Two diverging implementations of the Stripe-style `t=…,v1=…` signature verifier

Severity: Low · Category: duplication · Effort: S

**Locations**
- [billing.rs](../../../../crates/gateway/src/billing.rs) billing.rs:457, billing.rs:481
- [card_funding.rs](../../../../crates/gateway/src/card_funding.rs) card_funding.rs:81
- [subscriptions.rs](../../../../crates/gateway/src/subscriptions.rs) subscriptions.rs:201

**Evidence.** `card_funding::verify_signature` accepts up to 16 `v1` tags, rejects a duplicate `t=` and requires a tolerance in `1..=300`. subscriptions.rs:201 reuses it. billing.rs parses the header by hand instead, and differs in four ways:

- A later `v1` tag overwrites earlier ones.
- It does not reject a duplicate `t=`.
- It MACs `timestamp.to_string()` rather than the raw value.
- It compares against `config.webhook_skew_secs` with no bound.

**Impact.** Secret rotation that sends multiple `v1` tags can fail on the billing webhook. A misconfigured skew value widens the replay window.

**Suggested action**
1. Move `verify_signature` to `gateway/src/webhook_sig.rs` and make billing.rs call it.
2. Clamp `webhook_skew_secs` to `1..=300` at config load.
3. Add a test where only the first of two `v1` tags matches and the billing webhook accepts it.

### X-SEC-15 HTML escaping is duplicated, and the gateway builds HTML with format! plus manual esc()

Severity: Low · Category: maintainability · Effort: L

**Locations**
- [dashboard.rs](../../../../crates/gateway/src/dashboard.rs) dashboard.rs:70
- [site.rs](../../../../crates/discovery/src/site.rs) site.rs:149
- [layout.rs](../../../../crates/openagents-web/src/layout.rs) layout.rs:27
- [html.rs](../../../../crates/rust-native-web/src/html.rs) html.rs:52

**Evidence.** Outside psionic there are 16 definitions matching `fn esc|escape|html_escape|escape_html`. openagents-web already uses maud, which escapes automatically.

**Impact.** A single forgotten `esc()` call in a gateway page becomes XSS.

**Suggested action**
1. Move the gateway pages to maud, or to one `Html` newtype whose constructors escape.
2. Delete the duplicate escapers.
3. Verify with a test per page that renders a `<script>` payload and asserts it comes out escaped.

### X-SEC-16 Bearer-token parsing is ad hoc at 22 sites and disagrees on case

Severity: Low · Category: duplication · Effort: S

**Locations**
- [inference_status.rs](../../../../crates/gateway/src/inference_status.rs) inference_status.rs:147
- [accounts.rs](../../../../crates/gateway/src/accounts.rs) accounts.rs:1068
- [lib.rs](../../../../crates/openagents-web/src/lib.rs) lib.rs:391

**Evidence.** There are 22 non-test `strip_prefix("Bearer ")` sites, all case-sensitive. openagents-web compares the scheme with `eq_ignore_ascii_case`.

**Impact.** Clients get inconsistent 401s, and every route repeats the parsing logic.

**Suggested action**
1. Add one `bearer(&HeaderMap)` helper: case-insensitive scheme, exactly one header, length cap.
2. Replace the 22 sites.
3. Add helper tests for `bearer`, `BEARER`, a duplicated header and an oversized token.

### X-SEC-17 Several credential redactors exist alongside secret-screen with different coverage

Severity: Low · Category: duplication · Effort: M

**Locations**
- [secret-screen lib.rs](../../../../crates/secret-screen/src/lib.rs) lib.rs:171
- [router.rs](../../../../crates/coder/src/router.rs) router.rs:1629
- [smart.rs](../../../../crates/terminal-core/src/smart.rs) smart.rs:99

**Evidence.** terminal-core's `smart::scrub` keeps its own keyword list and does not depend on secret-screen (see X-SEC-04). Coder's router and tracker keep their own redactors.

**Impact.** Coverage gaps between the copies, and fixes that land in only one of them.

**Suggested action**
1. Make secret-screen the single source of credential-shape rules.
2. Run `secret_screen::redact` inside `coder::router::redact` and inside the exact-value scrubbers.
3. Share one corpus of credential samples across the redactor tests.

### X-SEC-18 No automated secret scanning on commit or CI

Severity: Low · Category: repo-hygiene · Effort: S

**Locations**
- [.githooks/pre-commit](../../../../.githooks/pre-commit)
- [.github/ISSUE_TEMPLATE/playtest-report.yml](../../../../.github/ISSUE_TEMPLATE/playtest-report.yml)
- [secret-screen lib.rs](../../../../crates/secret-screen/src/lib.rs) lib.rs:1

**Evidence.** `.github` contains only `ISSUE_TEMPLATE`. The repo hook `.githooks/pre-commit` is enabled through `core.hooksPath`, but it only runs `scripts/dev/check-large-files.sh`. No hook or script invokes gitleaks, trufflehog or secret-screen.

**Impact.** A secret leaked in bulk-committed bench or trace artifacts is caught only if a reviewer happens to notice it.

**Suggested action**
1. Add `scripts/dev/check-secrets.sh`. It runs a secret-screen CLI over `git diff --cached`, with an allowlist for the known fixtures, including `crates/coder-host/tests/fixtures/tls/*.key`.
2. Call it from `.githooks/pre-commit` next to `check-large-files.sh`.
3. Verify by staging a file containing a fake `ghp_…` token and checking that the commit is refused.

### X-SEC-19 Unbounded HTTP response body reads in outbound clients

Severity: Low · Category: security · Effort: S

**Locations**
- [apns.rs](../../../../crates/push-gateway/src/server/apns.rs) apns.rs:115
- [psionic.rs](../../../../crates/inference/src/upstream/psionic.rs) psionic.rs:238
- [card_funding.rs](../../../../crates/gateway/src/card_funding.rs) card_funding.rs:330

**Evidence.** `response.json().await` at apns.rs:115 and `reply.json().await` at psionic.rs:238 read the body with no size cap. card_funding.rs:330-342 already has a capped read: a `content_length` check plus a `MAX_BODY` guard on accumulated chunks. The reviewer counted 29 such unbounded sites outside tests; verification spot-checked 2.

**Impact.** A misbehaving upstream can force large allocations.

**Suggested action**
1. Extract the card_funding chunk loop into a shared `read_capped(response, max)`.
2. Use it with a 1 MiB cap for control and error responses.
3. Add a test against a fake upstream that streams more than the cap and expect a bounded error.

### X-SEC-20 tenancy builds SQL literals with manual quote doubling

Severity: Low · Category: maintainability · Effort: S

**Locations**
- [docs.rs](../../../../crates/tenancy/src/db/docs.rs) docs.rs:34, docs.rs:199, docs.rs:444, docs.rs:469

**Evidence.** Three sites inline `value.replace('\'', "''")`. The values come from `fixed: &'static [(&'static str, &'static str)]`.

**Impact.** The code is safe today, but it becomes injectable if `fixed` is ever made data-driven.

**Suggested action**
1. Bind the fixed values as extra `$n` parameters.
2. Add a `debug_assert` that identifiers match `^[a-z_.]+$`.
3. Verify with the existing tenancy db tests.

## Refuted during verification

- **Inconsistent randomness sources and time-based IDs.** Rejected. The IDs in question are documented as non-credentials. inference/src/seal.rs:177-178 describes the clock fallback as being for "an id that is never a secret". `coder-access` `studio_intents::mint` is documented as a request/command identity that is unique per process, not an authority token. The reviewer found no exploitable token, and the suggested action was speculative hardening.
- **Verification corrections (not new findings).** excerpt.rs:267 in terminal-core is a `#[cfg(test)]` helper, not a production scrubber, so it does not need deleting. The `unwrap`/`expect` calls in openagents-web `pay_proxy` cannot fail because routing guarantees the path prefix. The claim that the repo has no pre-commit hook was wrong: `.githooks/pre-commit` exists.
