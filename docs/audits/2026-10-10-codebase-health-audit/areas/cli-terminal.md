# CLI and terminal stack

**Scope:** `crates/{openagents-cli, openagents-terminal, openagents-login, openagents-connect, openagents-deck, terminal-core, terminal-gfx, terminal-mux, terminal-app, terminal-remote, terminal-tty, terminal-control, terminal-studio}`, plus `coder-terminal`, `coder-vt` and `coder-pty` for the overlap check. Snapshot `3168c986aa`, 2026-10-10.

**Health grade: C**

The stack is about 176k lines of Rust across 16 crates with 1,258 tests. The code within each file is good. Production code rarely unwraps, locks tolerate poisoning, wire messages have size limits, and every unsafe block in coder-pty and the CLI has a SAFETY comment. Module docs are thorough. The problems are in architecture and policy.

`openagents-cli` is an 89k-line crate with only a binary target. It depends on 66 workspace crates and resolves to 845 crates in total, including wgpu, wasmtime, ldk-node, breez and iroh. Command metadata is kept by hand in several places, and `main.rs` changed 105 times in 30 days. The money-handling code (x402, pay) lives in the binary and runs paid commands without the `supervise` subprocess contract the rest of the workspace uses. There are three copies of that executor, and none of them has a deadline. The most serious finding: `openagents mcp serve` and `x402 mcp-serve` expose every command group by default, including wallet and payment commands, and an argument can supply the `--yes` confirmation. A connected agent, or a buyer who pays the toll, could spend from the wallet. There is also a release blocker: the release script will refuse to cut 1.0.0 because `openagents-terminal` still says `1.0.0-rc.2`.

On the terminal side, the coder-* crates are reasonable leaf crates: coder-vt is the emulator, coder-pty is the NIP-TERM transport and coder-terminal holds the ratatui widgets. The terminal-* layer has split into 8 crates. Several are under 900 lines, have few tests and have no consumers. terminal-core is a full product application built around a public god struct. terminal-gfx (the glyph renderer) pulls in remote transport by default. terminal-core decides whether Enter runs a line in the shell or sends it as a question by scoring keyword lists. Workspace policy forbids that kind of routing, and the code acts on the score even when it marks the result as unsure. The name "OpenAgents Terminal" refers to two different products.

## Measurements

| Metric | Value |
|---|---|
| LOC incl. tests | openagents-cli 89,408 (168 files, about 60.6k non-test); terminal-core 19,527; coder-pty 13,327; coder-terminal 11,163; openagents-terminal 9,463; coder-vt 8,326; terminal-gfx 6,311; openagents-connect 5,950; terminal-studio 3,520; openagents-deck 3,173; terminal-mux 1,656; terminal-app 1,561; terminal-remote 1,267; terminal-tty 1,077; openagents-login 591; terminal-control 485. Total about 176k |
| Tests (`#[test]` / `tokio::test`) | 1,258 total: cli 480, coder-terminal 177, coder-vt 101, coder-pty 95, openagents-terminal 79, terminal-core 68, connect 56, terminal-gfx 44, deck 28, studio 21, mux 10, tty 8, login 5, app 3, remote 2, control 1 |
| CLI integration tests | 43 files; 31 spawn the binary via `CARGO_BIN_EXE` |
| CLI unwraps in non-test code | about 14 (out of 2,666 total, nearly all in tests) |
| CLI unsafe / SAFETY | 37 / 37 |
| CLI `#[path]` attributes | 28 |
| CLI `USAGE` constants | 92 |
| CLI `Args::parse` files | 77, of which 44 never check for unknown options |
| CLI subprocess sites | 45 `Command::new` vs 12 `supervise` uses |
| CLI ad hoc Tokio runtimes | 51 sites in 32 files (verifier count) |
| CLI dependencies | 65-66 workspace path deps; `cargo tree` 845 crates (not re-verified), 55 at duplicate versions (axum 0.7/0.8, reqwest 0.12/0.13, sha2 0.10/0.11, secp256k1 0.29/0.31, rand 0.8/0.9/0.10) |
| Churn (30 days) | cli `main.rs` 105 commits, cli `Cargo.toml` 59, openagents-terminal `app.rs` 42; CLI 656 commits since 2026-01-27 |
| Largest files | terminal-core `application_tests.rs` 3,039; cli `agent.rs` 2,947; terminal-core `paper.rs` 2,793; coder-pty `host/mod.rs` 2,752; cli `x402.rs` 2,491; coder-pty `ext.rs` 2,132; cli `connect/ssh.rs` 2,055; openagents-terminal `app.rs` 2,032 |
| Longest functions | `x402_native::serve` about 508 lines; `Application::paper_key` about 435 lines |
| Dependents | terminal-mux 0, terminal-app 0 (binary), terminal-control 2, terminal-remote 2, terminal-tty 1, terminal-core 14, coder-vt 13, coder-pty 15 |
| TODO/FIXME; `#[allow]` | 9; 26 |
| Release versions | openagents-cli 1.0.0, microcoder 1.0.0, openagents-terminal 1.0.0-rc.2, terminal-app 1.0.0-rc.2 |

## Strengths

- Production error handling is disciplined. About 60k non-test CLI lines contain about a dozen unwraps. terminal-core, terminal-gfx, terminal-mux, terminal-remote, terminal-control and coder-terminal have essentially none outside tests. Errors are plain sentences returned through `Output::usage` and `Output::fail`, with consistent exit codes (0, 1, 64).
- coder-pty locks tolerate poisoning throughout (`lock().unwrap_or_else(PoisonError::into_inner)`, `host/mod.rs:898, 2310, 2344, 2581`). PTYs run as process groups and are cleaned up with SIGHUP, then SIGTERM, then SIGKILL.
- Wire framing has size limits: the openagents-connect control channel caps messages at 256 KiB (`control.rs:28`) and enroll at 32 KiB (`enroll.rs:31`), both checked before allocation (`control/client.rs:50`). MCP `tools/call` caps argument count and size and rejects NUL bytes (`mcp.rs:397-416`).
- Unsafe code is documented: coder-pty 47 blocks and 47 SAFETY comments, the CLI 37 and 37, terminal-gfx 4 and 4. The workspace denies `unsafe_op_in_unsafe_fn`.
- Module docs (`//!`) name the NIP, issue and design doc behind each module (for example coder-pty `lib.rs`, openagents-connect `lib.rs`, terminal-control `unix.rs`).
- The lower layers are sound. coder-vt is a pure emulator over vte. coder-pty puts the portable wire/client half and the host half behind separate features, and its consumers correctly use `default-features = false`. terminal-core contains no transport or rendering code and is shared by Grid (web), the native window and the phone renderer.
- Test coverage is broad: golden-style app tests (openagents-terminal `app_tests.rs` 2,208 lines, terminal-core `application_tests.rs` 3,039 lines) and real-PTY tests in coder-pty, coder-vt and terminal-tty. Large untested CLI modules are rare (`studio.rs` 650, `provider_key.rs` 519, `plugin_team.rs` 493).
- The release script refuses mismatched crate versions and versions that are already published (`scripts/release/terminal.sh:473-478`). That check catches the drift in CLI-03.

## Findings

| ID | Severity | Category | Finding | Effort |
|---|---|---|---|---|
| CLI-01 | Critical | security | MCP surfaces expose every command group, wallet spend included, by default | M |
| CLI-02 | High | concurrency | Paid command executors bypass supervise: no deadline, no process group, deadlock risk, unbounded output (x3) | M |
| CLI-03 | High | build | openagents-terminal 1.0.0-rc.2 vs CLI 1.0.0 blocks release and shows the wrong version | S |
| CLI-04 | High | architecture | openagents-cli is binary-only and pulls GPU, wasm and Lightning stacks | XL |
| CLI-05 | Medium | policy | Smart-terminal Enter routing uses keyword scoring and acts on unsure scores | M |
| CLI-06 | Medium | duplication | `connect --ssh` reimplements coder-ssh's install-and-start | L |
| CLI-07 | Medium | maintainability | CLI command metadata kept by hand in several places; main.rs merge hotspot | L |
| CLI-08 | Medium | error-handling | Shared Args parser accepts unknown options and swallows the next word | M |
| CLI-09 | Medium | security | Private credential files written several ways; non-atomic, loose modes kept, default ACLs on Windows | M |
| CLI-10 | Medium | security | terminal-control socket: no peer UID check, chmod after bind, unbounded lines, thread per peer | S |
| CLI-11 | Medium | maintainability | "OpenAgents Terminal" names two products; CLI screen.rs/terminal.rs misnamed | M |
| CLI-12 | Medium | architecture | terminal-* fragmented into small, weakly tested crates; terminal-gfx default pulls remote/host | M |
| CLI-13 | Medium | architecture | terminal-core is the whole app: all-public god struct plus product pages | L |
| CLI-14 | Medium | duplication | RFC 8628 device-code login implemented twice | M |
| CLI-15 | Low | architecture | coder-vt depends on coder-pty for wire types; coder-pty default `host` is a footgun | L |
| CLI-16 | Low | duplication | Hidden-input/raw-mode termios copied; wallet copy lacks restore guard; Windows key entry echoes | S |
| CLI-17 | Low | maintainability | God functions/files in payment and PTY host paths | M |
| CLI-18 | Low | performance | New multi-thread Tokio runtime per call, including per loop item | S |
| CLI-19 | Low | maintainability | 28 `#[path]` attributes fake a directory layout | S |
| CLI-20 | Low | docs | AGENTS.md omits terminal-* crates; 9 in-scope crates lack a README | S |
| CLI-21 | Low | duplication | Duplicated hex helpers and split sha2 versions | S |

### CLI-01 MCP surfaces (`mcp serve`, paid `x402 mcp-serve`) expose every command group, including wallet spend, by default

**Severity:** Critical · **Category:** security · **Effort:** M

**Locations:**
- [mcp.rs:46-50](../../../../crates/openagents-cli/src/mcp.rs)
- [mcp.rs:92-107](../../../../crates/openagents-cli/src/mcp.rs)
- [mcp.rs:375-435](../../../../crates/openagents-cli/src/mcp.rs)
- [x402.rs:81-88](../../../../crates/openagents-cli/src/x402.rs)
- [x402.rs:1062-1073](../../../../crates/openagents-cli/src/x402.rs)
- [wallet.rs:374-379](../../../../crates/openagents-cli/src/wallet.rs)

**Evidence:** `callable()` is a deny-list that excludes only `"host" | "terminal" | "mcp" | "completions"` (mcp.rs:99-103). Every other USAGE group, including wallet, pay, x402, key, ssh and service, becomes a tool. `tools/call` checks only that the name is in that list and that the args are strings without NUL (mcp.rs:383-428), then runs `openagents --json GROUP ARGS...` with stdin closed. `wallet send` refuses non-interactive use only when `!args.switch("yes")` (wallet.rs:374), so `wallet {"args":["send","lnbc...","--yes"]}` pays with no human confirmation. The mcp USAGE says "Serve every command group as an MCP tool" (mcp.rs:46). `x402 mcp-serve` treats `--tool` as optional (x402.rs:1066-1073; usage line 81 shows `[--tool GROUP]...`, "narrows the served groups"). `x402` itself is callable, so a tool call can trigger a nested paid `x402 call`. `tests/mcp_serve.rs` has no test asserting that wallet or pay are refused.

**Impact:** Any agent connected to `openagents mcp serve` can spend from the owner's wallet, run ssh and install services. With `x402 mcp-serve` and no `--tool`, whoever the operator connects to that stdio server gets the same access for the price of the toll. Closing stdin was meant to block prompts, but an explicit `--yes` gets around it.

**Suggested action:**
1. Replace the deny-list with an allowlist derived from the `tree.rs` Effect declarations. By default admit only read-only effects. Never admit spend, secret-output (`--show-words`) or long-running effects.
2. Reject `--yes`, `--show-words` and `--replace` in MCP-supplied args before spawning.
3. Make `x402 mcp-serve` require at least one `--tool`, and refuse spend-class groups (wallet, pay, x402) even when named.
4. Add tests in `crates/openagents-cli/tests/mcp_serve.rs` asserting that `wallet send --yes`, `pay`, `ssh`, `service` and `x402` return `unknown tool` or a refusal, for both `mcp serve` and `x402 mcp-serve`.
5. Add an `INVARIANTS.md` row for MCP exposure policy that points at those tests.

### CLI-02 Paid command executors spawn processes without supervise: no deadline, no process group, write-then-read deadlock risk, unbounded output (three copies)

**Severity:** High · **Category:** concurrency · **Effort:** M

**Locations:**
- [x402_native.rs:212-233](../../../../crates/openagents-cli/src/x402_native.rs)
- [x402_native.rs:262-300](../../../../crates/openagents-cli/src/x402_native.rs)
- [x402.rs:318-339](../../../../crates/openagents-cli/src/x402.rs)
- [pay.rs:505-540](../../../../crates/openagents-cli/src/pay.rs)
- [AGENTS.md:673-679](../../../../AGENTS.md)

**Evidence:** There are three near-identical executors. Each runs `Command::new(...).stdin(piped).stdout(piped)`, calls `stdin.write_all(input)` before any read, then calls `wait_with_output()` (x402_native.rs:215-227, x402.rs:321-333, pay.rs:527-532). None has a timeout. x402_native's Command arm calls `run_command(program, args, input)` and ignores `execute_until`, which only the Job path uses (via `window_left`). The child gets no process group, and stdout is unbounded. AGENTS.md:673 describes `crates/supervise` as the subprocess supervisor coder and coderbench use, with process groups, deadlines and output caps. AGENTS.md scopes it to coder and coderbench, so this is a gap against the workspace's own standard rather than a violation of a written CLI-wide rule.

**Impact:** The serve loop deadlocks when the input is larger than the pipe buffer and the seller command writes before it finishes reading stdin. A hanging command is never killed after the paid window closes, and very large output can exhaust memory. In each case the buyer has paid and the provider is stuck. Every fix has to be made three times.

**Suggested action:**
1. Add one `paid_exec(program, args, env, input, deadline, cap)`, preferably in `crates/x402` as a seller module, built on `supervise::Job`. Set the deadline from `window_left(execute_until)`, apply an output cap, and feed stdin from a separate thread.
2. Replace the three executor bodies with calls to it.
3. Add tests: a command that echoes 1 MiB of stdin back must complete without deadlock, and `sleep 999` must be killed at the deadline and recorded as a failed purchase.

### CLI-03 Release version drift: openagents-terminal is 1.0.0-rc.2 while openagents-cli and microcoder are 1.0.0, so the release script refuses and the welcome card shows the wrong version

**Severity:** High · **Category:** build · **Effort:** S

**Locations:**
- [openagents-terminal/Cargo.toml:3-7](../../../../crates/openagents-terminal/Cargo.toml)
- [openagents-cli/Cargo.toml:5](../../../../crates/openagents-cli/Cargo.toml)
- [microcoder/Cargo.toml:5](../../../../crates/microcoder/Cargo.toml)
- [terminal-app/Cargo.toml:3](../../../../crates/terminal-app/Cargo.toml)
- [terminal.sh:470-478](../../../../scripts/release/terminal.sh)
- [app.rs:2015](../../../../crates/openagents-terminal/src/app.rs)

**Evidence:** openagents-terminal/Cargo.toml:7 has `version = "1.0.0-rc.2"`. Its own comment (lines 3-6) calls it the release line of OpenAgents Terminal (the `openagents` program) and says `scripts/release/terminal.sh` refuses a release whose version differs. openagents-cli and microcoder were bumped to 1.0.0 in `f882905192` (#11091), which did not touch openagents-terminal. terminal.sh:473-477 loops over all three manifests and exits with "commit the bump first" on a mismatch. app.rs:2015 renders `OpenAgents v{CARGO_PKG_VERSION}`. terminal-app is also at 1.0.0-rc.2.

**Impact:** `scripts/release/terminal.sh --version 1.0.0` fails. A HEAD build shows rc.2 on the welcome card while `openagents --version` reports 1.0.0.

**Suggested action:**
1. Bump `crates/openagents-terminal/Cargo.toml` to 1.0.0 now.
2. Define the release version in one place, for example by passing the CLI's `version_line()` into openagents-terminal at launch, or by having a shared build.rs read a single `[workspace.metadata.openagents] release` key.
3. Add a unit test asserting the two versions match.
4. Decide whether terminal-app follows this release line and record the decision in `docs/release/terminal.md`.
5. Verify with a dry run of `scripts/release/terminal.sh --version 1.0.0`.

### CLI-04 openagents-cli is a binary-only crate that pulls a GPU, wasm and Lightning stack

**Severity:** High · **Category:** architecture · **Effort:** XL

**Locations:**
- [openagents-cli/Cargo.toml:11](../../../../crates/openagents-cli/Cargo.toml)
- [openagents-cli/Cargo.toml:99-105](../../../../crates/openagents-cli/Cargo.toml)
- [verse/Cargo.toml:67](../../../../crates/verse/Cargo.toml)
- [verse/Cargo.toml:149](../../../../crates/verse/Cargo.toml)
- [main.rs:13-126](../../../../crates/openagents-cli/src/main.rs)

**Evidence:** The manifest declares only `[[bin]]` (line 11) and no `[lib]`, with 65 `path = "../` workspace dependencies. It depends on `verse` with `default-features = false` (line 99), but verse lists `verse-gfx` (line 67) and `wgpu = "29.0.4"` (line 149) as unconditional dependencies. The `x402*.rs` files total 5,719 lines inside the binary. 31 of 43 test files spawn the binary via `CARGO_BIN_EXE`. The 845-crate figure from `cargo tree` was not re-run during verification.

**Impact:** Builds and links are slow and the binary is large. Payment logic and other domain code inside a binary cannot be reused by desktop, mobile or host, and cannot be tested in-process.

**Suggested action:**
1. Make `wgpu` and `verse-gfx` optional in `crates/verse` behind a `render` feature, so the CLI's `xp-host`, `remote-chamber` and `studio-host` features compile without them. Verify that `cargo tree -p openagents-cli -i wgpu` is empty.
2. Add `src/lib.rs` and reduce `main.rs` to a thin shim, so tests can call command groups in-process.
3. Move seller-side x402 and pay code into `crates/x402` (`openagents_x402::seller`).
4. Track the resolved crate count as a metric in CI and watch it fall.

### CLI-05 Smart-terminal Enter routing chooses shell or ask by keyword scoring, against workspace routing policy, and Enter acts on unsure scores

**Severity:** Medium · **Category:** policy · **Effort:** M

**Locations:**
- [route.rs:1-8](../../../../crates/terminal-core/src/route.rs)
- [route.rs:270-307](../../../../crates/terminal-core/src/route.rs)
- [route.rs:455-496](../../../../crates/terminal-core/src/route.rs)
- [smart.rs:432-441](../../../../crates/terminal-core/src/smart.rs)
- [paper.rs:388-393](../../../../crates/terminal-core/src/paper.rs)
- [smart-terminal.md:294-314](../../../../docs/terminal/smart-terminal.md)

**Evidence:** Rule 4 scores a line against hard-coded word lists: QUESTION (26 words), STRONG and WEAK, as `ask += 2 * strong.min(2) + weak.min(2)` (route.rs:474). The module doc says no local decision model answers rule 4 yet, so the score decides on its own. `sure: ask.abs_diff(shell) >= 3` (route.rs:495) only affects `label()`, which shows "ask?". `route_enter` acts on `decision.route == Route::Ask` regardless of `sure` (smart.rs:433-436), and `paper_route` returns only `.route` (paper.rs:392). The workspace CLAUDE.md says: "Do not add ad hoc string or keyword matching for user-facing intent routing." This behavior is documented in smart-terminal.md ("How Enter decides"), which says a model will replace the score. An Ask goes to the hook's ask widget (`ASK_KEY`), not straight off the machine.

**Impact:** For borderline lines, a score-only guess decides whether a typed line runs as a shell command or becomes a request. Misroutes will happen, and the word lists will keep growing ad hoc.

**Suggested action:**
1. Keep rules 1-3 (explicit prefix, path or assignment, the shell's `whence` result) as bounded deterministic parsing.
2. Put rule 4 behind a trait implemented by the central typed selector or a bounded local classifier (the doc already names Lev/Laya/Kev).
3. Until that exists, use the existing `offer_ask` affordance when `sure == false` instead of acting, in both `route_enter` and `paper_route`.
4. Add an `INVARIANTS.md` row for Enter routing, and keep the route.rs table test as a regression suite. Add cases asserting that unsure lines produce an offer rather than an action.

### CLI-06 Two SSH install-and-start implementations: `openagents connect --ssh` reimplements what coder-ssh does

**Severity:** Medium · **Category:** duplication · **Effort:** L

**Locations:**
- [connect/ssh.rs:1-30](../../../../crates/openagents-cli/src/connect/ssh.rs)
- [connect/ssh.rs:440-450](../../../../crates/openagents-cli/src/connect/ssh.rs)
- [connect/ssh.rs:675-712](../../../../crates/openagents-cli/src/connect/ssh.rs)
- [remote.sh:50-80](../../../../crates/coder-ssh/src/remote.sh)
- [ssh.rs:1-8](../../../../crates/openagents-cli/src/ssh.rs)

**Evidence:** `connect/ssh.rs` is 2,055 lines and never references `coder_ssh`. It carries its own inline PROBE and INSTALL shell scripts (a `uname` case, sha256sum/shasum, stream, verify, chmod, mv, run once) and spawns the system `ssh` itself (line 447). coder-ssh (a 663-line launcher plus the 416-line remote.sh) already does probe, SHA verification, install and start-or-adopt, with an openssl fallback and `private_dir` checks. The CLI already uses coder-ssh for `openagents ssh add` (7 `coder_ssh` references in ssh.rs). The two paths install different binaries for different protocols (openagents host plus enroll, versus the pinned coder release plus NIP-ENV). The main divergence between the scripts is that the CLI's copy lacks the openssl fallback.

**Impact:** Security-sensitive remote install logic (hash verification, detached host start, adopt-or-refuse) is maintained in two places, and the scripts, tool fallbacks and error messages are drifting apart.

**Suggested action:**
1. Extend coder-ssh with an option to install an arbitrary binary at a given SHA-256, and add a stdio-bridge mode for enrollment.
2. Rewrite `connect/ssh.rs` as a thin front end over `coder_ssh::launcher`, and delete its PROBE and INSTALL scripts.
3. Run both `openagents connect --ssh` and `openagents ssh add` against one fake-ssh fixture in tests.

### CLI-07 CLI command metadata is hand-kept in several places, and main.rs is the merge hotspot

**Severity:** Medium · **Category:** maintainability · **Effort:** L

**Locations:**
- [main.rs:17-126](../../../../crates/openagents-cli/src/main.rs)
- [main.rs:131-260](../../../../crates/openagents-cli/src/main.rs)
- [main.rs:340-460](../../../../crates/openagents-cli/src/main.rs)
- [argv.rs:28-49](../../../../crates/openagents-cli/src/argv.rs)
- [mcp.rs:62-89](../../../../crates/openagents-cli/src/mcp.rs)
- [tree.json](../../../../crates/coder/src/cli_route/tree.json)

**Evidence:** Each command group appears as a `mod` declaration (with `cfg(unix)` variants), a USAGE row and a dispatch arm (for example `"terminal" => screen::run` or `#[cfg(unix)] "connect" => ...`). Some groups also appear as a help-depth tuple in `argv::normalize_help` (argv.rs:30-45), in the `tree.rs` Effect declarations and in a bundled `tree.json`. `mcp::groups` and completions get the group list by parsing USAGE text (two-space indent, ASCII-lowercase name; mcp.rs:64-89). `main.rs` is 721 lines and has had 105 commits since 2026-09-10.

**Impact:** Adding or renaming a command needs coordinated edits in several places, which causes frequent main.rs merge conflicts. Reformatting a USAGE line can silently drop a group from MCP and completions.

**Suggested action:**
1. Introduce `struct Group { name, aliases, summary, section, platforms, effect, run }` and a `static GROUPS: &[Group]`.
2. Generate USAGE, dispatch, non-unix refusals, MCP tools (allowlisted by effect, see CLI-01) and completions from that table.
3. Move the `normalize_help` depth rules into the group declarations.
4. Verify with the existing help, completions, mcp_serve and tree-drift tests. Target main.rs under 250 lines.

### CLI-08 The shared Args parser silently accepts unknown options and swallows the next word

**Severity:** Medium · **Category:** error-handling · **Effort:** M

**Locations:**
- [coder/src/argv.rs:24-56](../../../../crates/coder/src/argv.rs)
- [wallet.rs:113-116](../../../../crates/openagents-cli/src/wallet.rs)
- [relay.rs:57-60](../../../../crates/openagents-cli/src/relay.rs)
- [pay.rs](../../../../crates/openagents-cli/src/pay.rs)

**Evidence:** `Args::parse(words, switches)` treats any `--name` that is not a known switch as an option and consumes the next word as its value (argv.rs:40-50). It has no list of known options. 77 CLI files call `Args::parse`, and 44 of them never call `option_names()`, including wallet.rs, relay.rs and pay.rs. relay.rs parses with `Args::parse(rest, &[])`.

**Impact:** A typo changes what the command does instead of exiting with code 64. A misspelled flag consumes the next positional argument, which can leave a command running against the default relay, wallet or route.

**Suggested action:**
1. Add `Args::parse_strict(words, switches, options) -> Result<Args, String>` in `crates/coder/src/argv.rs` that rejects unknown `--names`.
2. Migrate the 44 unchecked CLI call sites, starting with the money and relay commands.
3. Add a help.rs test per group asserting that `--bogus` exits 64.

### CLI-09 Private credential files are written several different ways; some non-atomic, some keep loose modes, Windows gets default ACLs

**Severity:** Medium · **Category:** security · **Effort:** M

**Locations:**
- [studio_up.rs:1393-1405](../../../../crates/openagents-cli/src/studio_up.rs)
- [session.rs:501-514](../../../../crates/openagents-cli/src/session.rs)
- [gym.rs:221-240](../../../../crates/openagents-cli/src/gym.rs)
- [openagents-login/src/lib.rs:215-225](../../../../crates/openagents-login/src/lib.rs)

**Evidence:** `studio_up::write_private` writes a temp file and renames it, with mode 0600. `session::write_private` truncates in place with `options.mode(0o600)`, which only applies when the file is created, so an existing file with looser permissions keeps them. `gym::write_private` falls back to `std::fs::write` on non-Unix platforms (gym.rs:236-238). openagents-login sets mode 0600 (lib.rs:221) on Unix only and has no private-fs dependency.

**Impact:** A crash can leave credential and grant files half-written, and a pre-existing file can stay group- or world-readable. On Windows, tokens and connection codes get default ACLs.

**Suggested action:**
1. Add `private_fs::write_atomic(path, bytes)`. On Unix: create a 0600 temp file with O_NOFOLLOW, fsync, rename, and verify the mode with fchmod. On Windows: apply an owner-only DACL.
2. Replace the ad hoc writers in studio_up.rs, session.rs, gym.rs and openagents-login with it.
3. Add a test that pre-creates a 0644 file and asserts it is 0600 after a save.

### CLI-10 terminal-control's same-user socket skips the peer UID check, chmods after bind, and reads unbounded lines with a thread per peer

**Severity:** Medium · **Category:** security · **Effort:** S

**Locations:**
- [terminal-control/src/unix.rs:66-115](../../../../crates/terminal-control/src/unix.rs)
- [terminal-control/src/unix.rs:156-185](../../../../crates/terminal-control/src/unix.rs)
- [coder-host/src/control/socket.rs:197](../../../../crates/coder-host/src/control/socket.rs)

**Evidence:** `DirBuilder.recursive(true).mode(0o700)` only affects directories it creates. The socket is created with `UnixListener::bind` and only afterwards restricted with `set_permissions(path, 0o600)` (lines 80-84). There is no `peer_cred` check, whereas coder-host does `stream.peer_cred().ok().map(|cred| cred.uid())` (socket.rs:197). `serve_peer` reads with `reader.lines()` and no length bound, and every accept spawns a new thread with no cap (lines 98-102). Any existing file at the socket path that does not accept a connection is deleted with `remove_file`, even if it is not a socket (lines 78-79).

**Impact:** The socket is less protected than the host's own control socket. If the parent directory is permissive (for example `VERSE_TERMINAL_SOCKET` in /tmp), another user could inject keystrokes into panes. Any peer can exhaust memory or threads.

**Suggested action:**
1. Check that the parent directory is owned by the current user with mode 0700, and bind under a restrictive umask or in a fresh 0700 directory.
2. Reject peers whose uid differs from `geteuid()`.
3. Wrap each line read in `take(64 KiB)` and cap the number of concurrent peers.
4. Remove a pre-existing path only if it is a socket.
5. Reuse coder-host's socket helper if possible.
6. Add Unix tests for oversized lines, a permissive parent directory and a non-socket file at the socket path.

### CLI-11 'OpenAgents Terminal' names two products, and CLI modules screen.rs/terminal.rs are misnamed

**Severity:** Medium · **Category:** maintainability · **Effort:** M

**Locations:**
- [openagents-terminal/Cargo.toml:2-11](../../../../crates/openagents-terminal/Cargo.toml)
- [terminal-app/Cargo.toml:7-11](../../../../crates/terminal-app/Cargo.toml)
- [README.md:43](../../../../README.md)
- [design-principles.md:3](../../../../docs/terminal/design-principles.md)
- [screen.rs:1-3](../../../../crates/openagents-cli/src/screen.rs)
- [terminal.rs:1-2](../../../../crates/openagents-cli/src/terminal.rs)

**Evidence:** openagents-terminal describes itself as "OpenAgents Terminal: a full-screen chat ... drawn with coder-terminal". terminal-app describes itself as "OpenAgents Terminal: the shared smart terminal in a native window" and builds `[[bin]] name = "openagents-terminal"`. design-principles.md:3 says the name means the standalone window, while README.md:43 says `openagents terminal` opens the full-screen chat. In the CLI, `screen.rs` implements `openagents terminal` and `terminal.rs` implements `computer exec` / `computer shell`.

**Impact:** Contributors edit the wrong crate or module. The terminal-app binary has the same name as a different crate, which causes collisions in packaging.

**Suggested action:**
1. Give each surface its own product name, for example by renaming the terminal-app binary to `openagents-window` or the chat crate to `openagents-chat-tui`.
2. Rename CLI `screen.rs` to `terminal_cmd.rs` and `terminal.rs` to `computer_shell.rs`.
3. Update `docs/terminal/*.md`, `docs/ui/inventory.md` and the release docs in the same change.

### CLI-12 The terminal-* layer has fragmented into small crates with weak tests; terminal-gfx's default pulls remote/host code

**Severity:** Medium · **Category:** architecture · **Effort:** M

**Locations:**
- [terminal-control/src/lib.rs:1-9](../../../../crates/terminal-control/src/lib.rs)
- [terminal-mux/Cargo.toml:1-40](../../../../crates/terminal-mux/Cargo.toml)
- [terminal-gfx/Cargo.toml:7-22](../../../../crates/terminal-gfx/Cargo.toml)
- [terminal-gfx/src/lib.rs:23](../../../../crates/terminal-gfx/src/lib.rs)
- [verse/Cargo.toml:94](../../../../crates/verse/Cargo.toml)

**Evidence:** terminal-mux has 0 dependents and 2 commits, both on 2026-10-06. terminal-app has 0 dependents (it is a binary). terminal-gfx sets `default = ["native"]`, where `native` pulls in terminal-remote, terminal-control, winit, arboard, libc and `coder-pty/host`. lib.rs:23 has `pub use terminal_remote as remote`. Web and mobile consumers (coder-browser-web, everglade-web, coder-mobile) must remember to set `default-features = false`. The crate description does mention "native local-session adapters", so the default is documented, but it is still easy to get wrong. verse depends on terminal-control directly and unconditionally (verse/Cargo.toml:94).

**Impact:** Each crate adds manifest, feature and release overhead. A renderer whose default drags host and network code into glyph-only consumers. An unused multiplexer that will decay.

**Suggested action:**
1. Change terminal-gfx to `default = ["fonts"]`, enable `native` explicitly in terminal-app and verse, and drop the `remote` re-export.
2. Fold terminal-control into terminal-core behind a `control-socket` feature. It cannot go into terminal-gfx's `native` module because verse uses it directly.
3. Either wire terminal-mux into a real entry point with a smoke test, or delete it and keep `docs/terminal/multiplexing-lessons.md`.
4. Add a layer map (coder-vt, coder-pty -> terminal-core -> front ends) to `docs/terminal/README.md`.
5. Verify with `cargo tree -p coder-browser-web -i terminal-remote` (expect empty) after step 1.

### CLI-13 terminal-core is the whole smart-terminal application: an all-public god struct plus product pages

**Severity:** Medium · **Category:** architecture · **Effort:** L

**Locations:**
- [terminal-core/src/lib.rs:1-46](../../../../crates/terminal-core/src/lib.rs)
- [application.rs:60-114](../../../../crates/terminal-core/src/application.rs)
- [paper.rs:396-831](../../../../crates/terminal-core/src/paper.rs)

**Evidence:** lib.rs has 38 `pub mod` lines, including product pages (gym, knowledge, run, files, rules, thread, studio, paper), and `pub use application::Application as Overlay` (line 42). `pub struct Application` (ending at line 114) has 34 `pub` fields. About 7 `impl Application` blocks are spread across 5 files (application.rs has 3; paper.rs, paste.rs, resources.rs and sharing.rs have the rest). `paper_key` runs from line 396 to 830, about 435 lines. 14 crates depend on terminal-core.

**Impact:** Any mount can change focus, tab or sharing state directly and break the struct's invariants. A change to any page recompiles 14 dependents. Product concepts leak into every terminal surface.

**Suggested action:**
1. Split the crate into `terminal-core` (panes, blocks, select, copy, input, keys, mouse, layout, integration, route) and `terminal-pages` (paper, gym, knowledge, run, files, rules, thread, studio, sharing).
2. Make the `Application` fields `pub(crate)` and expose accessor and command methods instead. Remove the `Overlay` alias.
3. Break `paper_key` into per-page handlers dispatched on `self.paper.page`.
4. Use `application_tests.rs` as the safety net; it must pass unchanged apart from field-access syntax.

### CLI-14 The RFC 8628 device-code login exists twice (openagents-login and openagents-mobile account_link)

**Severity:** Medium · **Category:** duplication · **Effort:** M

**Locations:**
- [openagents-login/src/lib.rs:1-15](../../../../crates/openagents-login/src/lib.rs)
- [account_link.rs:4-5](../../../../crates/openagents-mobile/src/account_link.rs)
- [account_link.rs:834](../../../../crates/openagents-mobile/src/account_link.rs)
- [account_link.rs:1286-1390](../../../../crates/openagents-mobile/src/account_link.rs)

**Evidence:** openagents-login, used by coder-new and coder-sync, requests `/device/code` and then polls `/device/token`, honoring `slow_down`. account_link.rs implements the same flow again: `format!("{origin}/device/code")` (834), polling `{origin}/device/token` at 1286-1330, and its own `"authorization_pending"` / `"slow_down"` handling at 1385-1386. openagents-mobile is its own excluded workspace, but a path dependency can still reach openagents-login.

**Impact:** Polling intervals, error mapping and token handling can drift between phone and desktop. A server-side change has to be fixed in two places.

**Suggested action:**
1. Split openagents-login into a transport-agnostic core (`start`, `poll_once`, `wait`) with a pluggable token store.
2. Have openagents-mobile use it through a path dependency, delete its own request and polling code, and keep only the UI glue.
3. Share the test fixture in `openagents-login/src/tests.rs` between both callers.

### CLI-15 coder-vt depends on coder-pty for wire types, and coder-pty's default `host` feature is a footgun for 13 consumers

**Severity:** Low · **Category:** architecture · **Effort:** L

**Locations:**
- [coder-vt/Cargo.toml:14-22](../../../../crates/coder-vt/Cargo.toml)
- [coder-pty/Cargo.toml:9-15](../../../../crates/coder-pty/Cargo.toml)
- [coder-pty/Cargo.toml:37-39](../../../../crates/coder-pty/Cargo.toml)

**Evidence:** coder-vt depends on `coder-pty = { default-features = false }` for the NIP-TERM records. Its Unix dev-dependencies enable `coder-pty/host`, and coder-pty dev-depends on coder-vt. coder-pty sets `default = ["host"]`, which brings in libc, supervise, coder-boundary and windows-sys. 13 manifests depend on coder-pty with `default-features = false`. Changing the emulator does not rebuild the PTY host: coder-vt depends on coder-pty, and the reverse dependency is dev-only.

**Impact:** A new consumer that forgets `default-features = false` links PTY host code, which is a problem for wasm and mobile. The real shared contract, the NIP-TERM wire format, is hidden inside the transport crate.

**Suggested action:**
1. Cheap option: set coder-pty to `default = []` and opt into `host` explicitly in the host consumers.
2. Fuller option: extract the wire, ext, proposal and client types into a `nip-term` crate that both coder-vt and coder-pty depend on.
3. Verify with the coder-pty wire/ext tests and the coder-vt snapshot/authority tests.

### CLI-16 Hidden-input and raw-mode termios code is copied several times; the wallet copy has no restore guard and non-Unix provider-key entry echoes

**Severity:** Low · **Category:** duplication · **Effort:** S

**Locations:**
- [wallet.rs:609-632](../../../../crates/openagents-cli/src/wallet.rs)
- [provider_key.rs:78-120](../../../../crates/openagents-cli/src/provider_key.rs)
- [terminal.rs:316-347](../../../../crates/openagents-cli/src/terminal.rs)
- [terminal-tty/src/runner.rs:45-75](../../../../crates/terminal-tty/src/runner.rs)

**Evidence:** `wallet::read_hidden` calls `tcsetattr` directly and restores the terminal only after `read_line` returns (line 630); it has no Drop guard. `provider_key::NoEcho` has a Drop impl, but its `#[cfg(not(unix))]` branch does nothing. `terminal-tty/src/runner.rs` has 7 `unsafe` occurrences and 1 SAFETY comment. In practice `read_line` almost never panics, and a Drop guard would not cover SIGINT, so the wallet risk is small. The Windows echo case is real but narrow.

**Impact:** An interrupt while reading recovery words can leave echo off. On Windows a typed API key is echoed to the screen. There are several unsafe sites to audit instead of one.

**Suggested action:**
1. Add one `HiddenLine` / `RawMode` guard in a shared module: restore on Drop on Unix, use `SetConsoleMode` on Windows.
2. Replace the copies in wallet.rs, provider_key.rs and terminal.rs with it.
3. Add a SAFETY comment to every unsafe block in `terminal-tty/src/runner.rs`.

### CLI-17 God functions and god files in payment and PTY host paths

**Severity:** Low · **Category:** maintainability · **Effort:** M

**Locations:**
- [x402_native.rs:361-869](../../../../crates/openagents-cli/src/x402_native.rs)
- [coder-pty/src/host/mod.rs:1222-2252](../../../../crates/coder-pty/src/host/mod.rs)

**Evidence:** `pub fn serve` starts at x402_native.rs:361, and the next item (`fn resolve_capability`) starts at 869, so serve is about 508 lines. coder-pty `host/mod.rs` is 2,752 lines, and its `impl Host` block starts at 1222 and runs about 1,030 lines.

**Impact:** The payment path is hard to review and cannot be unit-tested one phase at a time. Both files are frequent conflict sites.

**Suggested action:**
1. Split `x402_native::serve` into `parse_config`, `open_inbox`, `admit`, `execute` and `settle_and_publish`, with unit tests for each phase using the existing fakes.
2. Split `host/mod.rs` into attach, frames, lifecycle and rights submodules behind the `Host` facade.
3. Target: no non-test function over 150 lines in these files.

### CLI-18 Ad hoc Tokio runtimes: a new multi-thread runtime per call, including per item in a loop

**Severity:** Low · **Category:** performance · **Effort:** S

**Locations:**
- [main.rs:498-503](../../../../crates/openagents-cli/src/main.rs)
- [labor.rs:399-410](../../../../crates/openagents-cli/src/labor.rs)

**Evidence:** `pub fn runtime()` builds a new `new_multi_thread()` runtime on every call, with `.expect("a Tokio runtime builds")` (main.rs:498-503). labor.rs:404-409 calls `crate::runtime().block_on(...)` once per 64-hex source inside a loop. There are 51 `Builder::new_*_thread` / `runtime().block_on` sites across 32 CLI files.

**Impact:** Each item pays for thread creation and teardown, which adds latency. Calling `block_on` from inside an async context would panic.

**Suggested action:**
1. Replace `runtime()` with a `OnceLock<Runtime>` accessor, or create a single runtime at dispatch.
2. In `labor::load_events`, fetch all hex IDs concurrently inside one `block_on`.
3. Verify with grep that `Builder::new_` appears only in the accessor.

### CLI-19 28 #[path] module attributes fake a directory layout in the CLI

**Severity:** Low · **Category:** maintainability · **Effort:** S

**Locations:**
- [sales.rs:9-36](../../../../crates/openagents-cli/src/sales.rs)
- [chat.rs:36-47](../../../../crates/openagents-cli/src/chat.rs)
- [catalog.rs:20-21](../../../../crates/openagents-cli/src/catalog.rs)

**Evidence:** `crates/openagents-cli/src` contains 28 `#[path` attributes. For example, sales.rs mounts the `sales_*.rs` files and chat.rs mounts the `chat_*.rs` files. Other modules (`pay_plugin/`, `plugin_purchase/`, `mcp/`) already use real directories.

**Impact:** File names and module paths do not match, which makes the code harder to navigate and grep.

**Suggested action:**
1. `git mv` the files into `src/sales/*.rs`, `src/chat/*.rs` and `src/catalog/purchase.rs`, and delete the `#[path]` attributes.
2. Verify with `rg -c '#\[path' crates/openagents-cli/src` that the count is zero.

### CLI-20 Docs gap: AGENTS.md crate map omits all terminal-* crates, and 9 in-scope crates have no README

**Severity:** Low · **Category:** docs · **Effort:** S

**Locations:**
- [AGENTS.md](../../../../AGENTS.md)
- [terminal-core/Cargo.toml](../../../../crates/terminal-core/Cargo.toml)
- [terminal-gfx/Cargo.toml](../../../../crates/terminal-gfx/Cargo.toml)
- [openagents-cli/Cargo.toml](../../../../crates/openagents-cli/Cargo.toml)

**Evidence:** `rg crates/terminal- AGENTS.md` returns nothing. These crates have no README.md: openagents-cli, openagents-terminal, openagents-login, openagents-connect, terminal-core, terminal-gfx, terminal-app, terminal-control and coder-terminal. terminal-mux, terminal-remote, terminal-tty, terminal-studio, coder-vt, coder-pty and openagents-deck each have one.

**Impact:** Agents routing terminal work cannot find the crate that owns it.

**Suggested action:**
1. Add one AGENTS.md entry per terminal-* crate that points at `docs/terminal/smart-terminal.md`.
2. Add short READMEs for terminal-core and terminal-gfx that state the layer map (see CLI-12).
3. Verify that `rg crates/terminal- AGENTS.md` lists every terminal-* crate.

### CLI-21 Small duplicated helpers and split dependency versions in scope

**Severity:** Low · **Category:** duplication · **Effort:** S

**Locations:**
- [sov_host.rs:81](../../../../crates/openagents-cli/src/sov_host.rs)
- [x402_spark.rs:86](../../../../crates/openagents-cli/src/x402_spark.rs)
- [pay_hosted.rs:47](../../../../crates/openagents-cli/src/pay_hosted.rs)
- [terminal-core/Cargo.toml:16](../../../../crates/terminal-core/Cargo.toml)
- [terminal-studio/Cargo.toml:27](../../../../crates/terminal-studio/Cargo.toml)
- [openagents-cli/Cargo.toml:94](../../../../crates/openagents-cli/Cargo.toml)

**Evidence:** The CLI has three private hex encoders: `sov_host::hex`, `x402_spark::hex` and `pay_hosted::to_hex`. terminal-core and terminal-studio use `sha2 = "0.10"`, while openagents-cli uses `sha2 = "0.11.0"`. The reviewer's count of 55 duplicate-version crates was not re-checked.

**Impact:** Slightly longer compile times and a slightly larger binary, plus room for drift.

**Suggested action:**
1. Use one shared hex helper, or the `hex` crate.
2. Bump terminal-core and terminal-studio to sha2 0.11.
3. Turn on duplicate-version warnings in `deny.toml` for the CLI tree. Verify with `cargo tree -p openagents-cli -d | rg sha2`, which should show a single version.

## Refuted during verification

- **"coder-terminal::native (549 lines) only serves an example yet forces rust-native onto every TUI."** Rejected. coder-terminal uses `rust_native` outside `native.rs` (`ladder.rs:168`, `components/diff.rs:130` and `components/turn.rs:234`, for colors), so putting `native.rs` behind a feature would not remove the dependency, and the proposed check (`cargo tree -p openagents-terminal -i rust-native` returning empty) would fail. The module is documented in `lib.rs:27-28`, and coder-computers uses it in a test and an example. It is not dead code.
- Partial corrections already applied above. CLI-05: confidence is shown in the label, just not used to gate Enter, and an Ask stays in the local ask widget rather than going off the machine. CLI-06: the CLI install script maps `uname` into a fixed case set, so it is not an unsanitized-input defect. CLI-07: the MCP deny-list is a separate design choice and was not caused by the metadata duplication. CLI-15: changing the emulator does not rebuild the PTY host.
