# Remediation log

Fixes landed on `main` after the audit snapshot (`3168c986aa`). Each commit
names its finding IDs; read the commit for tests and scope notes.

| Finding IDs | Commit | What changed |
| --- | --- | --- |
| CLI-01 (critical), CLI-03 | `0ae48e2124` | `mcp serve` and `x402 mcp-serve` serve only commands declared read-only; wallet, pay, x402, key, ssh, service groups never served; `--yes`/`--show-words`/`--replace` refused before spawn (and before any toll); `x402 mcp-serve` requires `--tool`. `openagents-terminal` 1.0.0 with a version-match test. |
| X-ROUTE-01, X-ROUTE-02, CD-01 | `1a95fbdcca` | Crew requests routed by a typed Jev question set (`questions/agent-request.json`) instead of keyword lists; merges wait for the owner's CONFIRM. Thresholds are provisional; no accuracy eval yet. |
| NO-01, X-SEC-01 | `a8c22711dd` | NEG-OPEN shares REQ's auth and rate-limit admission and refuses incomplete sets; client IP taken from the trusted right-hand XFF hop (`NOSTR_RELAY_TRUSTED_HOPS`, default 1). |
| WEB-01, X-SEC-02 | `5beabb890e` | Graceful shutdown with an 8 s bounded drain; local privilege requires a loopback socket peer, not just the `Host` header. |
| VE-01, X-ERR-02 (verse-world admission only) | `62da4318df` | Broker replay slots consumed only after admission; expired IDs evicted; admission state recovers from poisoned locks. |
| AGT-01 | `d93e955054` | Pylon mainnet daily ceiling fails closed on journal errors and holds a cross-process file lock. |
| AGT-02 | `d8568391c0` | One credential-name policy (`secret_screen::is_credential_name`) for the env scrubbers; no panic on non-UTF-8 env. The agent-launch paths in `coder-new` keep the old rule on purpose (Vertex/Bedrock credentials). |
| PAY-03 | `e4fb8496b7` | One OS-released store lock for all tenancy file stores; a crashed writer no longer blocks writes. |
| X-ERR-01 | `4b2738fcd1` | Typed `NotSent`/`Unknown` pay failures; CLI no longer says nothing was paid when the outcome is unknown and reuses the idempotency key on retry. Mobile Wallet-tab Send still uses a fresh key per tap. |
| CX-01 | `d7abdfb1cf` | Launcher recovery kills a saved process group only when the leader's recorded identity matches. |
| GY-02 | `4f2fb87f76` | eval-runner ledger writes ordered under a writer lock, each via a unique temp file. |
| CS-02 | `76d1977903` | Browser terminal queues keystrokes typed during an in-flight request. |
| GW-01 | `f52e8e7be5` | Receipt appends and fsync run off the async runtime; failures logged and counted on `/healthz`. |
| CD-02, X-LINT-02 | `d765f537cb` | Build scripts no longer watch `.git/index`; dirtiness from `CODER_BUILD_DIRTY`. |
| OPS-02, X-TEST-02 | `816e844714` | `verify-changed.py` handles nested workspaces via `--manifest-path`, checks direct dependents, maps `Cargo.lock` changes. |
| PS4-03, OPS-06 (partial) | `bb81316c75` | Ignore nested Cargo targets and Python bytecode; 2 stray `.pyc` and a `.DS_Store` untracked. The 601 `.pyc` inside terminal-bench traces stay: trace manifests record them by hash (decide with GY-01/OPS-01). |

## Open and blocked

- **X-LINT-01 / X-DEP-01:** the dependency gate needs owner policy calls (7 git sources, license exceptions). The `paste` waiver (RUSTSEC-2024-0436) in `deny.toml` expires 2026-10-20. Tracked in the workspace `NEEDS_OWNER.md`.
- **X-SEC-01 follow-up:** `NOSTR_RELAY_TRUSTED_HOPS=1` assumes Cloud Run appends the client address last; watch relay rate-limit refusals after the next deploy.
- **Pre-existing:** `eval-runner` unit tests do not compile on `main` (`.expose()` on an `Option` in `src/config.rs`).
