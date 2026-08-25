# Architecture Evaluation: Migrating OpenAgents CLI to Rust

**Date:** 2026-08-25
**Author:** OpenAgents Core Team
**Scope:** `packages/openagents-cli` (TypeScript / Effect-TS) $\rightarrow$ Full Rust rewrite (`crates/openagents-cli`)

---

## Executive Summary

The `openagents` CLI is currently implemented in TypeScript (`packages/openagents-cli`) using `@effect/platform-node` and `effect` for async concurrency, typed errors, and streaming, alongside cryptographic libraries (`@noble/curves`, `@noble/hashes`, `@scure/bip39`) and terminal interface rendering (`coder-ui.ts`).

Migrating the CLI from TypeScript/Effect-TS to a native Rust binary presents architectural opportunities and trade-offs. This document outlines the technical, operational, and maintenance trade-offs to guide the migration decision.

---

## Current Architecture

- **Language & Runtime:** TypeScript / Node.js 20+ ESM via `@effect/platform-node`.
- **Concurrency & Control Flow:** Effect-TS pipelines for effects, environment management, structured error handling, and terminal lifecycle management.
- **Subsystems in Scope:**
  - **Coder & Fleet Orchestration:** `coder-*.ts` (multi-lane delegation across Devin, Ox Alpha, Claude Code, Codex, local Ollama, headless subprocess execution, ACP protocol).
  - **Interactive TUI Engine:** Custom terminal raw-mode UI, diff renderer, keybinding handlers, box frame layouts (`coder-ui.ts`, `box-*.ts`).
  - **Sovereign Identity & Auth:** BIP-39 mnemonic generation, secp256k1 key derivation, token management in OS keychain (`identity-*.ts`, `auth-*.ts`).
  - **Forge & API Gateway:** REST/JSON API bindings for issues, pull requests, projects, repositories, forum, and WebSocket event subscribers (`api-*.ts`).

---

## Pros: Benefits of Converting to Rust

### 1. Zero Runtime Dependencies & Instant Cold Startup
- **Native Binary:** Eliminates the requirement for users to have Node.js 20+ installed.
- **Sub-10ms Startup:** Eliminates V8 engine initialization, JIT warm-up, and module resolution overhead. Essential for CLI commands invoked frequently in scripts, git hooks, and child agents (`openagents auth status`, `openagents issue view`).
- **Streamlined `curl | bash` Distribution:** Directly yields static binaries (`x86_64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`) with zero packaging friction.

### 2. Predictable Memory Footprint & Resource Efficiency
- **Low Memory Overhead:** A compiled Rust binary operates in 5–20 MB of RAM compared to Node.js / V8's baseline 50–150 MB RSS.
- **Parallel Fan-out Scaling:** In fleet delegation (`coder-delegate.ts`), orchestrating multiple subprocesses and telemetry streams consumes a fraction of host resources.

### 3. Mature Systems & Terminal Ecosystem
- **Rich CLI & TUI Libraries:** First-class ecosystems for CLI interfaces:
  - Argument parsing: `clap` (derive macro, automated shell completions, typed flags).
  - TUI Rendering: `ratatui` / `crossterm` for terminal user interfaces and diffing.
  - Async I/O: `tokio` for async scheduling and process handling.
- **Process & Signal Control:** Exact control over process groups, pseudo-terminals (pty), child process termination, and signal handling (`SIGINT`, `SIGTERM`, `SIGWINCH`) without cross-platform Node.js pty discrepancies.

### 4. Direct Monorepo Rust Synergy
- **Code Sharing with Existing Crates:** The monorepo already maintains Rust crates under `crates/` (`oa-node`, `all-work-contract`, `oa-workroomd`, `openagents-cloud-contract`). A Rust CLI can directly import shared schema types, cryptographic utilities, and protocol contracts without intermediate TypeScript bindings or WASM wrappers.

### 5. Memory Safety & Strict Concurrency Guarantees
- Compiler-enforced concurrency guarantees avoid data races during heavy multi-agent event multiplexing and streaming.

---

## Cons: Drawbacks and Costs of Converting to Rust

### 1. Significant Migration & Rewrite Overhead
- **Substantial Code Surface:** `packages/openagents-cli` contains over 30,000+ lines of TypeScript across ~90 files (coder harnesses, ACP protocol integration, forum claims, repository sync, provider settlements, box run pipelines).
- **Time to Feature Parity:** A full port requires rewriting all API callers, client state machines, formatting outputs, and comprehensive test suites (`test/*.test.ts`).

### 2. Loss of Effect-TS Ecosystem Ergonomics
- Effect-TS provides unified effect tracking, algebraic data types, layer composition, and runtime fiber management in TypeScript.
- While Rust provides `Result<T, E>` and `Option<T>`, mimicking Effect's functional dependency injection and managed service layers requires bespoke architectural patterns in Rust.

### 3. Developer Friction & Contribution Velocity
- Modifying CLI tools in TypeScript allows fast iteration, immediate dynamic inspection, and broad accessibility across JavaScript/TypeScript fullstack developers.
- Rust requires stricter type bookkeeping, handling lifetimes/ownership, and dealing with longer compilation/link cycles during CI/CD test runs.

### 4. Cross-Platform Compilation & CI Release Matrix
- Cross-compiling Rust binaries (especially with OpenSSL or C dependencies) requires dedicated cross-compilation runners, musl toolchains, macOS code notarization, and Windows build pipelines.
- Node/TypeScript packages publish with minimal build steps to npm.

---

## Comparison Matrix

| Dimension | TypeScript (Current / Effect-TS) | Rust (`crates/openagents-cli`) |
| :--- | :--- | :--- |
| **Startup Latency** | ~150ms – 350ms (Node boot) | ~3ms – 10ms (Native) |
| **Runtime Requirement** | Node.js $\ge$ 20.0.0 | None (Statically linked binary) |
| **Memory Consumption** | ~50MB – 120MB RSS | ~5MB – 25MB RSS |
| **Distribution Method** | `npm install -g`, `npx` | `curl \| bash`, GitHub Release, `cargo install` |
| **Development Velocity**| Rapid iteration, hot TS execution | Strict borrow checker, longer compile times |
| **Crate Interop** | Requires WASM / JSON RPC bridges | Direct native crate imports |
| **TUI / Subprocess Control** | Node `child_process` / `node-pty` | `crossterm`, `tokio::process`, native pty |

---

## Recommended Strategy

1. **Near-Term (Current Release):**
   - Retain TypeScript codebase for immediate velocity.
   - Package standalone binaries using Bun single-file executable compiler or Node Single Executable Applications (SEA) to enable the `curl -fsSL https://openagents.com/cli/install.sh | bash` distribution pipeline without requiring Node on user machines.

2. **Medium-Term (Targeted Rust Migration):**
   - Extract performance-critical / system-level components into Rust crates (e.g. `coder` execution engine, credential vaults, pty supervisors).
   - Evaluate a full Rust CLI binary under `crates/openagents-cli` once protocol schemas and agent harness APIs stabilize.
