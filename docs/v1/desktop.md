# OpenAgents v1.0.0 — Desktop & CLI Specification

**Primary Ship Artifact:** `OpenAgents.app` (`com.openagents.desktop`, macOS 14+ Apple Silicon, signed + notarized DMG at `https://openagents.com/downloads/OpenAgents.dmg`, signed auto-updates)  
**Secondary / Source Builds:** Linux (`apps/openagents-desktop/linux/build.sh`, x86_64 Wayland/Vulkan) & Windows (`apps/openagents-desktop/windows/build.sh`, x86_64 Win32/Vulkan)  
**CLI:** `openagents` (`apps/openagents-cli/`)

---

## 1. Unified Desktop Build Architecture (`#14116`)

Commit `685bffd3c3` collapsed the earlier `--minimal` / `--full` split into **one unified build** across macOS, Linux, and Windows (`profile=unified`, `inputs_schema=7`, `--minimal` and `--full` kept as no-op aliases):

- **Single Coax UI & Host Stack:** `apps/openagents-desktop/app.coax` / `page.coax` compiled to C (`gallery.o`) driving shared Coax GPU shaders (`coax metal` on macOS, `coax spirv` on Linux/Windows Vulkan).
- **Bundle Integrity Policy (`apps/openagents-desktop/v1-bundle-policy.coax` & `v1-bundle-check.coax`):** Clean staging `.app` assembly before atomic swap (never mutating an existing `.app` in place), strict size budgets, allowlisted binaries/resources, and `Info.plist` provenance (`OACommit`, `OAProfile=unified`, `CFBundleVersion`).
- **Auto-Updates & LaunchAgent:** Signed Sparkle/Ed25519 update feed (`Updater.swift`) and local `apps/openagents-desktop/install.sh` / `com.openagents.desktop-update` ownership of `/Applications/OpenAgents.app`.
- **Runtime Flags (`coax/lib/agent/desktop/flags.coax`):** `account` (`default_on = true`) and `git` (`default_on = true`) are enabled out of the box; experimental panes (`metaverse`, `gamebuilder`, `browser`, `gym`, `foreman`, etc.) remain cleanly flag-gated.

---

## 2. Core Desktop Capabilities in v1.0.0

### 2.1 Unified Agent Harness (`oa-worker` + `coder-host` + `gitguard`)
- **Detached Daemon Execution (`coder-host`):** Agent runs survive closing or quitting the Desktop window; re-opening reconnects seamlessly to the live event stream.
- **Git Worktree Isolation (`#14121`):** Canonical git-root resolution, `.worktreeinclude` support, per-chat isolated worktrees, `gitguard` safety checks (no global/TCC scans), cache invalidation, stale-worktree sweeper, and background cleanup.
- **Inline Git Actions & Diffs (`git` flag ON):** Inspect unified diffs, review changes, copy chat ID/path, and keep active human direction over every edit.
- **Paper-Lab & Compaction Upgrades (`#14131`–`#14134`, `59f705073c`, `fd030ccc1b`):**
  - **Compact-Bench** context compaction for long sessions
  - **DepSync** (dependency sync on Write), **ToolPulse** (3x repetition detection), **FaithCheck**, and break-even **ObsMask** (observation masking)
- **`/goal` Autonomous Loop:** Multi-turn goal execution with an automatic final verification turn that re-checks acceptance criteria against real commands/files before declaring `[GOAL_COMPLETE]`.

### 2.2 Local + Cloud + BYOK Models
1. **Local Inkling 2 Mini (Offline-First, Zero API Cost):**
   - 26.4B looped MoE (3.4B active, ~17.6 GB `IQ4_NL` GGUF, pinned SHA-256 `cbdf3fa9dc50a4124977980f37da4f1fa764b0cfe800d275cd43d26ce2a81af4`) running on **Psionic Metal 4** (`psionic-inkling-serve` + Coax-emitted `inkling-kernels.metal`, `--ctx 32768`, Apple Silicon ≥ 32 GB).
   - Resumable download into `~/.openagents/models/inkling-2-mini/`, hardware floor check, eager prewarm on launch, free-port scan (`18794..18857`), detached `--idle-exit-s 300`, safe delete blocked during active runs.
   - Native RFC 8259 JSON-schema constrained token sampling (`psionic/lib/inkling-responses.coax`).
2. **OpenAgents Cloud & Frontier Models:**
   - Sign in with your OpenAgents account to run hosted **Gemini 4 Neon** (and OpenAgents Auto router) funded by your Promo Credit or Lightning Paid Credit — zero external API key needed.
   - **BYOK Fallback:** Direct Claude, Gemini, and OpenRouter keys remain first-class for full user sovereignty.

### 2.3 Tools, MCP & Subagents
- **Exa WebSearch & WebFetch (`59f705073c`):** Native web search and page fetch inside `oa-worker` (metered cleanly via `0222_exa_search_billing.sql` when using OpenAgents Cloud).
- **Optional X API (`XSearch`, `#14124`):** Gated by API key and Desktop Settings.
- **Full MCP Support (Phases 0–5, `coax/lib/mcp/`):**
  - Phase 0–1 pure `#authority` Sans-I/O protocol core
  - Phase 2 stdio transports & conformance harness
  - Phase 3–4 Streamable HTTP + SSE server and client
  - Phase 5 OAuth 2.1 client & resource server
- **Subagents (`#14104`, `#14120`, `#14122`, `#14123`):**
  - Truly cache-exact `FORK_AGENT`
  - 6-tier loader with tool filtering, memory scopes, `permissionMode`, and `maxTurns`
  - Session-scoped append-only DAG JSONL transcripts, disk-backed `SendMessage` resume, and blocking `TaskOutput`
  - Explicit per-delegation git rules (`none`, `commit`, `push`) configurable in Settings and preserved on resume
- **Programmatic iOS Simulator Pane (`oasim`, `#14116`):** Passive subscription to `simhost.sock` so agents and users can attach and inspect iOS Simulator sessions directly inside Desktop.

### 2.4 OpenAgents Account & Cloud Computer Enrollment
- **WorkOS Account Sign-In (`Account.swift`, `account` flag ON by default):** Magic Auth, Google, GitHub, Apple, Email+Password, Enterprise SSO, B2B `switchOrganization()`, `0600` credential storage under `~/.openagents/desktop/`, and Passkey/Nostr `L2` linking.
- **Enrolled Workstation Daemon (Zero Tailscale):** Enroll the Mac/Linux machine with OpenAgents Cloud (`0220_cloud_projects_environments.sql`). Desktop / `openagents-environment` heartbeats available git repos to `oa-api` and accepts durable `turns_v2` jobs from Web and iOS over outbound HTTPS/SSE — no inbound ports, no Tailscale, no firewall changes.

---

## 3. OpenAgents CLI (`apps/openagents-cli/`)

The `openagents` CLI ships alongside Desktop and covers headless workflows, Linux servers, and admin operations:

| Command Group | Capabilities in v1.0.0 |
| --- | --- |
| `openagents login --workos` / `whoami` / `logout` | RFC 8628 WorkOS device authorization flow, refresh rotation, and NIP-98 key auth |
| `openagents chat` | Cloud & local terminal chat against `/api/v1/inference` and `chat_v2` |
| `openagents environments` | `enroll`, `list`, `run` — enroll any Mac or Linux workstation into OpenAgents Cloud |
| `openagents projects` | List, create, and inspect repo-matched Cloud projects |
| `openagents referral` / `referral redeem CODE` | View personal/vanity referral link, stats, promo balance, and redeem codes |
| `openagents admin referral ...` | Campaign `new|list|revoke`, grant `hold|void|clawback`, `cap`, `flag`, `config` |
| `openagents wallet` / `mcp` / `pylon` / `coder` | Spark wallet, MCP tooling, Pylon onboarding, and headless coding agent runs |

---

## 4. Platform Tiering for Monday Launch

| Platform | v1.0.0 Status | Distribution | Notes |
| --- | --- | --- | --- |
| **macOS (Apple Silicon, macOS 14+)** | **Committed v1.0.0** | Signed + notarized `/downloads/OpenAgents.dmg` + auto-update | Full unified app + local Metal 4 Inkling 2 Mini (≥ 32 GB RAM) + Cloud/BYOK |
| **Linux (x86_64 Wayland/Vulkan)** | **Build from source** | `apps/openagents-desktop/linux/build.sh` (linked on `openagents.com`) | Full Coax UI + Cloud/BYOK + MCP + Subagents + Cloud enrollment; CUDA Inkling hillclimbing & packaged `.AppImage`/`.deb` post-v1.0.0 |
| **Windows (x86_64 Win32/Vulkan)** | **Build from source** | `apps/openagents-desktop/windows/build.sh` (linked on `openagents.com`) | Shared Coax UI + Win32/Vulkan host; signed installer & CUDA Inkling post-v1.0.0 |
