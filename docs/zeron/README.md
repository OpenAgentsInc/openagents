# Zeron — Codebase Overview

> Strictly read-only audit and reference documentation for `zeronsh/zeron` (workspace version `0.3.9`, Rust edition `2024`).

## What Zeron Is

**Zeron** is a local-first, native multi-device controller for AI coding agents — supporting **Claude Code**, **Codex**, **Cursor**, **Devin**, **Grok**, **Hermes**, **Pi**, **Antigravity**, **OpenCode**, **Muse**, and ACP-compatible harnesses. It runs locally by default via a background Rust engine (`zeron-engine`) and optionally syncs sessions, workspaces, terminals, previews, and voice sessions across desktop (macOS, Linux, Windows), mobile (iOS, Android), and Zeron Cloud sandboxes through a Cloudflare Workers / Durable Objects edge layer.

### Mission & Core Principles

- **Local-first by default:** All agent execution, transcript persistence, git/worktree management, terminals, and previews run on the user's machine with zero cloud requirement.
- **Viewport / Engine separation:** Desktop (`apps/zgx`), iOS (`apps/ios`), Android (`apps/android`), and CLI (`apps/zeron`) are thin viewports over a single typed RPC + CRDT/log-sync engine (`crates/engine`).
- **Multi-device mesh with optional Cloud:** Signed-in users connect devices through Cloudflare Durable Object rooms (`DeviceRoom`, `SessionRoom`, `ChatRoom`, `RegistryRoom`, `PreviewRoom`) and can optionally provision managed Cloud runner sandboxes backed by an isolated credential vault (`edge/vault`).
- **Zero-trust boundaries by design:** Separate full vs. agent-scoped localhost IPC tokens (`crates/rpc/src/ipc_auth.rs`), sandboxed plugin execution (`crates/plugin-guard`), and an HTTP-less credential vault worker (`edge/vault`).

## Documentation Index

| Document | Description |
| --- | --- |
| [`README.md`](./README.md) | Project overview, mission, tech stack, repository layout, and build/test/lint commands. |
| [`features.md`](./features.md) | Exhaustive catalog of Zeron capabilities across apps, Rust crates, and edge services, with crate/file references. |
| [`architecture.md`](./architecture.md) | Technical architecture deep dive: 33-crate workspace dependency map, apps & edge topology, storage, sync/RPC protocols, security boundaries, and CI/release pipelines. |
| [`audit.md`](./audit.md) | Senior-advisor codebase audit: strengths, design decisions, and concrete findings across reliability, security, performance, testing/CI, and tech debt with `file:line` evidence. |

## Tech Stack

| Layer | Technologies |
| --- | --- |
| **Core Engine & Shared Crates** | Rust (`stable`, edition `2024`, 33 workspace members), `tokio`, `loro` CRDT, `tokio-tungstenite`, `serde`/`serde_json`, `tree-sitter` (`crates/syntax`), `wgpu` (`crates/visual-gpu`), `zatex` (LaTeX math layout) |
| **Desktop App (`apps/zgx`)** | Rust host + **zgx** native GPU UI framework (`github.com/zeronsh/zgx`), hot-reloaded **Luau** UI (`apps/zgx/ui/`, ~296k LOC), Ghostty terminal (macOS), WebKitGTK (Linux), WebView2 (Windows), `mimalloc` |
| **CLI & Headless Engine (`apps/zeron`)** | Rust binary (`zeron`) linking `zeron-engine`, `zeron-cli`, `zeron-mcp`, `zeron-rpc`, `zeron-sync`, `zeron-update`, `zeron-notify` |
| **iOS App (`apps/ios`)** | Swift / UIKit + Live Activities (`ZeronLiveActivity`), UniFFI bindings generated from `crates/mobile` (`zeron_core.swift`) |
| **Android App (`apps/android`)** | Kotlin / Android + UniFFI bindings from `crates/mobile`, custom `VoiceOrbView`, `VoicePeer`, native transcript renderer |
| **Edge Control Plane (`edge/`)** | TypeScript (`^6.0.3`), Cloudflare Workers, 8 Durable Object classes, 4 Cloudflare Workflows, 3 R2 buckets, Cloudflare Browser Rendering (`@cloudflare/puppeteer`), `jose` (WorkOS AuthKit JWTs), `loro-crdt` |
| **Credential Vault (`edge/vault/`)** | Isolated Cloudflare Worker (`VaultApi` WorkerEntrypoint, zero HTTP surface) + Durable Objects (`VaultAccount`, `VaultDevices`), reachable only via Workers Service Binding |
| **Web Surfaces (`apps/landing`, `apps/card-web`)** | Static web landing page (`zeron.sh`) and web member card renderer sharing design tokens from `apps/zgx/ui/ds/themes/zeron-dark.json` |

## Repository Layout

```text
zeron/
├── Cargo.toml                 # Root Rust workspace (33 members, v0.3.9, edition 2024)
├── rust-toolchain.toml        # stable channel + rustfmt, clippy
├── README.md                  # User-facing overview & install instructions
├── ARCHITECTURE.md            # Original native rewrite architecture doc (partially predates zgx)
├── CONTEXT.md                 # Domain glossary (theme vocabulary, surface treatment)
├── apps/
│   ├── zeron/                 # CLI & headless engine binary (no GUI window)
│   ├── zgx/                   # Native desktop app (separate Cargo workspace + Luau UI)
│   ├── ios/                   # Swift/UIKit iOS app + Live Activity extension
│   ├── android/               # Kotlin Android app
│   ├── landing/               # zeron.sh marketing/landing site
│   └── card-web/              # Web member card renderer & preview tooling
├── crates/                    # 32 Rust library/tool crates (engine, harness, sync, doc, rpc, mobile, etc.)
├── edge/
│   ├── src/                   # Main Cloudflare Worker (auth, rooms, cloud control plane, cards, releases)
│   ├── vault/                 # Isolated credential vault Worker (no public HTTP routes)
│   ├── wrangler.jsonc         # Production + staging bindings (DOs, Workflows, R2, Browser, Service Binding)
│   └── package.json           # Vitest (unit + workerd pool) and wrangler scripts
├── dist/                      # Packaging assets (icons, macOS/Windows/voice assets, zeron.desktop)
├── docs/                      # Extensive design specs, research notes, and reliability docs (~50+ files)
├── scripts/                   # Build, packaging, e2e (cloud-e2e, transport-proxy), SDK update, and installer test scripts
└── .github/workflows/         # 19 GitHub Actions workflows (CI router, multi-platform builds, deploy, release, mobile)
```

## Build, Test, and Lint Commands

### Rust Workspace (Root)

```bash
# Build workspace
cargo build --workspace

# Run unit & integration tests
cargo test --workspace

# Format & lint
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# Run headless engine locally
cargo run -p zeron -- headless
```

### Desktop App (`apps/zgx` — separate workspace)

```bash
# Build & test zgx host + Luau UI test suite
cargo test --manifest-path apps/zgx/Cargo.toml
cargo clippy --manifest-path apps/zgx/Cargo.toml --all-targets

# Run desktop app in development (hot-reloads apps/zgx/ui/*.luau)
cargo run --manifest-path apps/zgx/Cargo.toml
```

### Edge Worker (`edge/`) & Credential Vault (`edge/vault/`)

```bash
# Edge Worker
cd edge
npm ci
npm run typecheck      # tsc --noEmit
npm run test           # vitest unit + workerd Durable Object/Worker tests
npm run dev            # wrangler dev --port 27640 --var AUTH_MODE:dev
npm run build          # wrangler deploy --dry-run

# Credential Vault
cd edge/vault
npm ci
npm run typecheck      # tsc --noEmit
npm run test           # vitest unit + workerd tests
```

### Mobile Apps (`apps/ios`, `apps/android`)

- **iOS:** Built via Xcode (`apps/ios/Zeron.xcodeproj`) against the Rust `crates/mobile` static library (`cargo build --profile mobile-dist`); CI/TestFlight automated in `.github/workflows/testflight.yml`.
- **Android:** Built via Gradle (`apps/android/`) with JNI/UniFFI bindings from `crates/mobile`; CI automated in `.github/workflows/android.yml`.
