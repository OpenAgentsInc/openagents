# WorkOS Coax SDK (`coax/lib/workos/`)

Master tracking issue: [#14103](https://github.com/OpenAgentsInc/openagents/issues/14103)
Design specification: [docs/auth/2026-11-26-workos-coax-sdk.md](../../docs/auth/2026-11-26-workos-coax-sdk.md)

Sans-I/O, zero-heap, `#authority` Coax SDK for the [WorkOS](https://workos.com) API.
Every core module operates purely on caller-owned buffers with explicit entropy and wall-clock
inputs, so the exact same code compiles into:

- `coax/web/api` (C11 via `coax lib`, `adapters/c_unit.coax`),
- `apps/openagents-web` (Wasm via `wasm32-wasip1`, `adapters/app_unit.coax`),
- `apps/openagents-ios` & `apps/openagents-desktop` (Apple Swift hosts via C ABI, `adapters/app_unit.coax`),
- `apps/shared/host/android` (Android JNI host via C ABI, `adapters/app_unit.coax`), and
- `apps/openagents-cli` (native binary via `adapters/http_native.coax`).

## Phase 1 modules (`#14098`)

| Module | Authority surface | Verification |
| --- | --- | --- |
| `pkce.coax` | Canonical unpadded `base64url_encode`, PKCE `S256` verifier/challenge (RFC 7636), constant-time `state` compare | `coax test` + `coax mutate` (0 survivors) |
| `url.coax` | RFC 3986 percent-encoding, strict dot-segment path-traversal guard, AuthKit/OAuth/SSO authorize, logout, and JWKS URL builders | `coax test` + `coax mutate` (0 survivors) |
| `webhook.coax` | `WorkOS-Signature` (`t=<ms>, v1=<hex>`) strict parser rejecting unknown components, HMAC-SHA256 verification (±180s), synchronous AuthKit Actions verifier (±30s) & signed verdict builder | `coax test` + `coax mutate` (0 survivors) |
| `seal.coax` | AES-256-GCM session sealing (`workos-rust` raw compat + `workos-ios` `wos2.` PBKDF2-HMAC-SHA256 + HKDF-SHA256 + AAD `"wos2."`) with zeroized key schedules | `coax test` + `coax mutate` (0 survivors) |
| `jwt.coax` | Duplicate-key-rejecting JSON header/payload inspection, JWKS unique-`kid` RSA matcher (`e=AQAB`), RS256 verification (`rsa_verify.coax`), zero-leeway `exp`/`nbf`/`iss`, and mandatory human-session token-confusion guard (`sid`, `auth_time`, `sub_profile`, `sub_type`) | `coax test` + `coax mutate` (0 survivors) |
| `error.coax` | Dual WorkOS error normalizer (`{code,message}` + OAuth `{error,error_description}`), `organization_selection_required`/`sso_required` detection, `Retry-After` parser, refresh error classification | `coax test` + `coax mutate` (0 survivors) |
| `retry.coax` | Safe-method / `Idempotency-Key` retry eligibility and capped exponential backoff with equal jitter | `coax test` + `coax mutate` (0 survivors) |
| `phase1_unit.coax` | `#export_c` C ABI boundary + native ASan/UBSan & Wasm verification unit | `verify.sh` |

## Phase 2 modules (`#14099`)

| Path | Role | Verification |
| --- | --- | --- |
| `tools/oagen-coax/` | `@workos/oagen` + `@workos/openapi-spec` emitter generating deterministic checked-in Coax files | `bun run typecheck && bun run check` |
| `gen/` | 38 checked-in files covering all 269 WorkOS OpenAPI operations across 24 services, 12 typed authenticate/create wrappers, 844 borrowed-`Jv` model views, and 216 enums | `coax test gen/all.coax --library` |
| `flow.coax` | Pure `#authority` Sans-I/O auth state machine (PKCE, Magic Auth, multi-org selection, RFC 8628 device flow, single-flight refresh) | `coax test` + `coax mutate` (0 survivors) |
| `adapters/http_native.coax` | Native HTTP/TLS adapter over `coax/lib/net/http.coax` (retry/backoff, `Retry-After`, `Idempotency-Key`, zero-redirect Auth safety) | `tests/emulate_e2e.sh` |
| `adapters/c_unit.coax` | Server `#export_c` C ABI unit compiled into `coax/web/api` | `coax test` + C11 compile |
| `adapters/app_unit.coax` | Public-client `#export_c` C ABI unit for iOS/macOS/Android/Wasm (strictly omits `client_secret`) | `coax test` + C11 + `wasm32-wasip1` |
| `tests/emulate_e2e.{sh,coax,yaml}` | Offline deterministic E2E test suite against `workos/emulate` | `tests/emulate_e2e.sh` |

## Verification

```sh
coax/lib/workos/verify.sh
coax/lib/workos/tests/emulate_e2e.sh
```

To regenerate checked-in `gen/` files after bumping `@workos/openapi-spec`:

```sh
cd tools/oagen-coax && bun install && bun run generate && bun run check
```
