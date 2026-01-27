# HISTORY

This narrative is distilled from the git commit history, tags, and branch names. It focuses on visible milestones, pivots, and course corrections.

## Timeline

### 2023-11 to 2023-12: Laravel genesis and early agent UI
- 2023-11-07: Initial commit and Laravel scaffolding, README, Postgres setup.
- Mid November: early Faerie agent experiments, embeddings, GitHub integration, tests, and first chat UI.
- Late November to December: landing pages, login, referrals, dashboards, and chat polish. Many "try" and "fix" commits show fast experimentation.

### 2024-01 to 2024-02: Plugins and agent builder, multiple resets
- Extism WASM plugin support, plugin upload/view, and a builder UI.
- Multiple cleanup waves ("Delete non-MVP code", "Zero-base", "Zerobase") signal scope tightening.
- Early knowledge base and agent chat flows appear, plus Nostr auth exploration.

### 2024-03 to 2024-06: Product hardening, models, payments, marketplace
- Major chat refactors, model selection, multimodal inputs, auth and billing (Cashier).
- Payments and balances added; Lightning/LNURL, payouts, wallet UI, and tool usage tracking.
- Repeated queue locking, race condition, and payment backoff fixes show production pressure.

### 2024-07 to 2024-12: Nostr focus and v1 deprecation
- July to August: Nostr login fixes and Lightning address hardening.
- 2024-10-15: "Deprecate v1" followed by a fresh Laravel Breeze API-only rebuild (2024-10-16).
- New landing pages, chat layouts, Dusk tests, and team/project UI.
- December: Genesis docs and cleanup signal a new direction.

### 2025-01 to 2025-04: Rust pivot and new surfaces (web, mobile, desktop)
- 2025-01-07: Rust app with a nostr-htmx demo, then Axum/Askama site and HTMX chat.
- Early 2025: agent manager services, repo mapping, and solver tools appear.
- March to April: Onyx and Coder apps (Expo, Tauri/Electron), MCP tools, Cloudflare Agents, and shared UI packages.
- OIDC/better-auth and GitHub tool integration become core.

### 2025-05 to 2025-06: Monorepo and Effect-based experiments
- Wallet UX, Spark integration, and web wallet experiments.
- Switch to pnpm workspaces and Effect.js monorepo structure.
- Nostr core and SDK updates mature.

### 2025-07 to 2025-12: Autopilot era, runtime, marketplace, and WGPUI
- Large sprint builds Autopilot, Pylon, and Nexus: NIP-90 marketplace, compute buying, replay bundles, and E2E tests.
- Runtime, containerization, and budget semantics land in late December.
- WGPUI and autopilot UI system stabilize; Commander is renamed to Autopilot.
- Tags v0.1.x and v0.2.x appear in Oct 2025 around bridge/tricoder releases.

### 2026-01: Autopilot Desktop and WGPUI migration
- WGPUI desktop bootstrapping, autopilot_app and autopilot_ui crates, and storybook coverage.
- Full Auto loops, plan-mode optimization, and heavy clippy/testability cleanup.
- Pylon/Onyx/Nexus v0.1.0 tags land in early January.

## Crises and course corrections (visible in commit history)

- Reverts during production churn:
  - 2024-05-23: revert to old prompt after regressions.
  - 2024-06-24: revert exponential backoff for payins.
  - 2025-10 to 2025-11: multiple UI and streaming reverts to restore stability.
- Reliability and race-condition fixes:
  - 2024-05-15: "Fix race condition" and related queue-locking fixes.
  - 2025-12-31: "Fix RefCell borrow panics" in wallet worker.
- Build/quality crises:
  - Frequent "build fix", "fix clippy", and "fix tests" spikes.
  - 2025-02-03: dependency security update for OpenSSL.
- Reset moments:
  - "Zero-base" / "Zerobase" (Feb 2024) and "Delete non-MVP code" commits show repeated scope resets.
  - Oct 2024 "Deprecate v1" marks a major reset and rebuild.

## Branches and side quests

Active and historical branches show parallel tracks:
- `autopilot`, `issues`, `issuetools`, `dogfood`: issue tracking, MCP tooling, and internal usage.
- `distribute`: packaging and platform env fixes.
- `bb-converter`, `blackbox`: converter and documentation experiments.
- `recorder`, `restore-effect-nostr`: logging/replay and Effect-based Nostr work.
- `ui`, `storybook`, `shad`: UI component experiments.
- `cleanup`: maintenance spikes.

## Release tags observed

- 2025-10-24: `v0.1.0`, `v0.1.1` (bridge bootstrap and session hardening).
- 2025-10-29 to 2025-10-30: `v0.2.1` through `v0.2.5` (bridge/tricoder updates).
- 2026-01-06 to 2026-01-07: `onyx-v0.1.0`, `pylon-v0.1.0`, `nexus-v0.1.0`.

---

If you want this to be more granular, I can expand any era into a date-by-date log or add direct links to milestone commits.
