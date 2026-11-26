# WorkOS Coax SDK (`coax/lib/workos/`)

Master tracking issue: [#14103](https://github.com/OpenAgentsInc/openagents/issues/14103)
Design specification: [docs/auth/2026-11-26-workos-coax-sdk.md](../../docs/auth/2026-11-26-workos-coax-sdk.md)

Sans-I/O, zero-heap, `#authority` Coax SDK for the [WorkOS](https://workos.com) API.
Every core module operates purely on caller-owned buffers with explicit entropy and wall-clock
inputs, so the exact same code compiles into:

- `coax/web/api` (C11 via `coax lib`),
- `apps/openagents-web` (Wasm via `wasm32-wasip1`),
- `apps/openagents-ios` & `apps/openagents-desktop` (Apple Swift hosts via C ABI),
- `apps/shared/host/android` (Android JNI host via C ABI), and
- `apps/openagents-cli` (native binary via `coax/lib/net/http.coax`).

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

## Verification

```sh
coax/lib/workos/verify.sh
```

Runs `coax test --library`, `coax mutate --library` (requiring zero surviving mutants on every
`#authority` module), C11 ASan/UBSan native ABI tests (`tests/c_check.c`), and a `wasm32-wasip1`
freestanding compile check.
