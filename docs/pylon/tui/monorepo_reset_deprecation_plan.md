# Monorepo Reset and Deprecation Plan

## Current State Analysis

Our workspace at `/Users/christopherdavid/work` is currently a multi-repo federation consisting of separate Git clones (including `openagents`, `probe`, `psionic`, `control`, `vortex`, and `deprecated/openagents.com`). This leads to several architectural challenges:

1. **Dependency Drift**: We maintain separate package registries, Cargo locks, and Bun locks across different repository clones.
2. **Framework Fragmentation**: We mix Rust/Ratatui TUIs, React/Tauri desktop shells, and Bun-based TypeScript runtimes.
3. **Outdated Codebases**: The `openagents` repository contains legacy experimental code (such as Rust `pylon`, `pylon-tui`, and the Tauri-based `autopilot`) that is out of parity with our modern `probe` runtime and active product layers.

---

## Active Monorepo Cleanup (Completed Actions)

To begin the monorepo reset, we have completed the following structural deprecations within the `openagents` repository:

1. **Relocated Outdated Applications**:
   * Moved `openagents/apps/pylon` to `openagents/apps/deprecated/pylon`
   * Moved `openagents/apps/pylon-tui` to `openagents/apps/deprecated/pylon-tui`
   * Moved `openagents/apps/autopilot` to `openagents/apps/deprecated/autopilot` (renamed and deprecated)
2. **Updated Cargo Workspace**:
   * Modified `openagents/Cargo.toml` to map workspace members to their new `apps/deprecated/*` paths.
3. **Resolved Dependency Paths**:
   * Adjusted all relative manifest paths (`../../crates/*`) in the moved Cargo crates to maintain cargo build, syntax checking, and linting integrity.

---

## Proposed Unified Monorepo Architecture: Bun Workspaces

We propose unifying the workspace as a single **Bun Workspace Monorepo**, bringing all TypeScript/Bun services under a unified dependency graph.

### 1. Root Workspace Definition

We will establish a root-level `package.json` at the workspace root (`/Users/christopherdavid/work/package.json`) to declare workspaces:

```json
{
  "name": "openagents-monorepo",
  "private": true,
  "workspaces": [
    "probe",
    "openagents/packages/*",
    "openagents/apps/deprecated/autopilot"
  ],
  "devDependencies": {
    "bun-types": "latest"
  }
}
```

### 2. Standardized Directory Layout

Under a fully reset monorepo, we will segregate codebases by runtime engine:

```
/Users/christopherdavid/work/
├── package.json               # Root Bun Workspace
├── bun.lock                   # Single unified lockfile
├── apps/                      # TypeScript/Bun applications
│   ├── probe/                 # Standalone probe coding agent
│   └── pylon-v0.3/            # Modern TS/Effect/OpenTUI Pylon earning node
├── rust/                      # Shared Rust workspace (cargo-driven)
│   ├── psionic/               # Standalone psionic ML runtime
│   └── crates/                # Shared kernel/nostr Rust crates
└── deprecated/                # Frozen/Archived projects (read-only)
    ├── vortex/                # Stale Next.js product surface
    └── openagents.com/        # Old Laravel public portal
```

---

## Phased Migration and Reset Path

### Phase 1: Prune and Archive Legacy Repos (Immediate)
* Move the stale Convex/Next.js `vortex` and Laravel `deprecated/openagents.com` clones completely out of the active workspace.
* Relocate their contents to `/Users/christopherdavid/work/backroom/` as read-only historical source material.

### Phase 2: Initialize Root Bun Workspace (Short-Term)
* Create `/Users/christopherdavid/work/package.json` and generate a single unified `bun.lock` for all active TS projects (`probe`, `@opentui/core`).
* Link internal packages together using standard npm/Bun workspace resolution rather than ad-hoc copy operations.

### Phase 3: Implement Pylon v0.3 (Mid-Term)
* Build Pylon v0.3 within `/Users/christopherdavid/work/apps/pylon-v0.3` using TypeScript.
* Consume the unified `@opentui/core` bindings.
* Use `effect` to manage background MDK wallet and presence telemetry.
* Completely delete `openagents/apps/deprecated/pylon` and `openagents/apps/deprecated/pylon-tui` once Pylon v0.3 achieves parity.
