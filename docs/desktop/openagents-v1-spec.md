# OpenAgents v1.0 Specification (Inkling-Only Minimal Release)

**Date:** 2026-11-21  
**Status:** Proposed specification  
**Baseline:** `origin/main` `7a85209a15`  
**Target Artifact:** `OpenAgents.app` v1.0.0 (`com.openagents.desktop`, macOS 14+, Apple Silicon ≥ 32 GB unified memory)  
**Scope:** `apps/openagents-desktop/`, `coax/lib/agent/desktop/`, `psionic/platform/inkling-*`, `evals/openagents-desktop/`, `evals/session-history/`  
**Related docs:**
- [OpenAgents Desktop: audit for an Inkling-only first release](../audits/2026-11-21-openagents-desktop-inkling-first-release-audit.md)
- [Coax Desktop](coax-desktop.md)
- [Desktop Evals Specification](../coder/desktop-evals-spec.md)
- [Ch. 269: Inkling 2 Mini on Metal](../../psionic/book/ch269-inkling-2-mini-on-metal.md)
- [Ch. 270: Tool Calls and Responses for Inkling 2 Mini](../../psionic/book/ch270-tool-calls-and-responses-for-inkling-2-mini.md)

---

## 1. Purpose and Product Definition

OpenAgents v1.0 is a **minimal, single-model, local-only coding desktop app for Apple Silicon Macs** running **Inkling 2 Mini** directly on Psionic's native Metal 4 runtime (`psionic-inkling-serve`).

Everything outside that core loop—cloud models, accounts, Bitcoin wallet, Gym, Foreman, iOS Simulator, integrated terminal, web preview, voice, MCP/Skills, remote environments, Tailscale pairing, 3D scenes, and more—**stays in the monorepo** for future releases and developer builds, but is **excluded from the shipped v1.0 app** at both runtime and build time.

### Core Principles

1. **One model, zero model selector.** v1.0 runs only Inkling 2 Mini (26.4B looped MoE, 3.4B active, ~17.6 GB `IQ4_NL` GGUF) on this Mac. There is no model picker, no cloud API key setup, no account login, and no dollar metering.
2. **Minimal surface area.** Keep only the features required to open a repo, talk to Inkling 2 Mini, inspect/edit files, approve actions, run `/goal`, keep worktrees clean, and update the app.
3. **Code stays in the codebase; only v1.0 ships.** All non-v1.0 code remains tracked, compiled, and tested in full developer builds (`build.sh --full`). The v1.0 release build (`build.sh --minimal`) strips excluded frameworks, helper CLIs, workers, Swift files, and Coax modules out of `OpenAgents.app` so dead weight is never shipped, signed, or loaded by `dyld`.
4. **Honest hardware and quality bars.** Inkling 2 Mini requires Apple Silicon with ≥ 32 GB unified memory. Unsupported Macs are told immediately on first run. Linux and Windows are out of scope for v1.0 (Metal-only runtime). Release quality is measured solely on the real shipped `OpenAgents.app` and `psionic-inkling-serve` binary against real weights.

---

## 2. Exact Feature Scope

### 2.1 Included in OpenAgents v1.0

| Area | Included Capabilities | Primary Files |
| --- | --- | --- |
| **Core Chat** | Send, stop, steer, follow-up queue, markdown replies, tool execution rows, streaming thoughts. No model selector; no sub-agents (`agents = false`). | `page/transcript.coax`, `page/followups.coax`, `page/compose.coax`, `page/input.coax`, `Window.swift`, `Window+Runs.swift`, `Worker.swift` |
| **First-Run & Local Model** | Apple Silicon + ≥ 32 GB RAM check, one-click resumable Hugging Face download into `~/.openagents/models/inkling-2-mini/`, progress bar, SHA-256 verification, safe delete (blocked while runs are active), auto-managed `psionic-inkling-serve` with error logs and port-collision diagnostics. | `Window+Dialogs.swift` (`LocalModelStore`, `LocalModelServer`), `page/layout/conversation.coax` |
| **Chat History & Sidebar** | Persisted chats, sidebar list, new/rename/delete/fork. Pre-existing chats created on hidden models open readable and continue on Inkling with an explicit notice. | `Store.swift`, `page/chats.coax`, `page/persist.coax`, `page/layout/sidebar.coax` |
| **Workspace & Files** | Working folder picker, file attachments, `@` file mentions, read-only file pane. | `page/attach.coax`, `page/mention.coax`, `page/filepane.coax`, `page/filetree.coax`, `Window+Attach.swift`, `Window+Files.swift` |
| **Navigation & Search** | Command palette (v1.0 commands only) and in-chat Find. | `page/palette.coax`, `page/find.coax`, `Window+Palette.swift` |
| **Agent Interaction** | Permission prompts, interactive question cards, to-do tray, `/goal` autonomous loop with end-of-goal verification turn. | `page/permit.coax`, `page/todos.coax`, `page/goal.coax` |
| **Background Execution & Safety** | Detached daemon (`coder-host`) so runs survive app quit; per-chat git worktrees and `gitguard`. | `DaemonClient.swift`, `Worktree.swift`, `Window+Worktrees.swift`, `coder-host`, `gitguard` |
| **Minimal Settings** | General, Local Model (status / download / verify / remove), Agent Instructions, About. No cloud keys, accounts, or gated tabs. | `settings.coax`, `Window+Settings.swift` |
| **Desktop Basics** | Signed auto-updates, desktop notifications, standard macOS window/menu chrome. | `Updater.swift`, `Window+Update.swift`, `Notifications.swift`, `page/notify.coax`, `AppDelegate.swift`, `Window+Menus.swift` |

> **Candidate first post-v1.0 addition (`git`):** The audit notes that a coding agent without an inline diff viewer is lean. Keep `git` (diff viewer, git actions toolbar, checkpoints/rewind) excluded from v1.0 so the initial surface stays minimal, and designate it as the first flag to turn on in **v1.0.1**.

### 2.2 Excluded from OpenAgents v1.0 (28 Flags, All Off & Stripped at Build Time)

Every row below stays in the repository and compiles under `--full` developer builds, is controlled by the Coax flag registry (`coax/lib/agent/desktop/flags.coax`) with default `false`, and is **excluded from the shipped v1.0 `.app` bundle** under `--minimal`.

**Excluded Models (7 flags):**
- `models.gemini` — Gemini 4 Neon/Carbon/Hydrogen/Argon, Gemini API key, Vertex/ADC/env sign-in probe, Gemini sub-agent levels.
- `models.claude` — Claude Fable/Opus/Sonnet/Haiku, `claudeworker`, `claude` CLI detection, Claude Key, Claude sub-agents.
- `models.openrouter` — OpenRouter models, key settings, and sub-agent level.
- `models.antigravity` — Antigravity model entry, `agyworker`, `agy` CLI detection, and settings rows.
- `models.external` — Codex, Cursor, ACP Registry, multi-model handoff and fan-out.
- `models.executor` — Hybrid Executor, weight modules, Local Pylon, settled sats, shadow rollout.
- `models.inklingNeon` — Inkling + Neon hybrid routing and Inkling as a cloud sub-agent.

**Excluded Features (21 flags):**
- `account` — OpenAgents login, account profile, passkey/device-key linking (`lab-sign.o`, `Account.swift`, `AppAuth.swift`, `DeviceKey.swift`, `KitPasskey.swift`).
- `foreman` — Foreman chat, menu, permission tiers, control server, `foreman-ctl`.
- `gym` — Benchmarks, Terminal-Bench, PLE 3D, Lean · RH, 3D RH Flythrough (`RhFlythroughView.swift`, `rh-flythrough.o`), `lean-run`, `tbench-run`, `ple-run`, `gym-evals/`.
- `wallet` — Breez Spark Bitcoin/Lightning wallet (`breez_sdk_sparkFFI.framework`, `libBreezSdkSpark.a`, `libBigNumber.a`, `pay-wallet*.o`, `bip39-english.txt`), wallet RPC, Nostr identity.
- `forge` — PRs & Issues sidebar view and review tours.
- `git` — Git actions toolbar, inline diff viewer, checkpoints/rewind (first candidate for v1.0.1).
- `simulator` — iOS Simulator pane and `oasim`.
- `terminal` — Integrated multi-tab terminal pane.
- `preview` — Web preview pane (`WebKit`) and SnapShot Region.
- `voice` — Dictation (`Speech`, `AVFoundation`), mic button, and Voice settings.
- `mcp` — MCP servers, OAuth, and Skills catalog.
- `scheduled` — Scheduled tasks, webhooks, and Update Everything.
- `environments` — Remote SSH, Boats, server exposure, keep awake.
- `pairing` — Tailscale mobile device pairing listener (`Tailnet.swift`, `AppTailnet.swift`).
- `usage` — Usage/quota dashboard, price overrides, per-chat cost/budget displays.
- `customize` — Themes, VS Code themes, keybindings, languages.
- `proof` — Thread Proof, ATIF trace export (`proof-export`), sync opt-in.
- `capabilities` — New Local Capability studio and `capability-eval`.
- `agents` — Agents page (Coder, Tester, Game Builder), `agents-catalog`, `tester-cli`.
- `scenes` — Open 3D Scene and 3D scene side panel (`SceneModel.swift`, `SceneView.swift`, `ScenePanelView.swift`).
- `rich` — Prompt stash, prompt history, side chat, and extended composer widgets (`page/rich.coax`).

### 2.3 Hard Invariants for Excluded Features

1. **Zero UI residue:** No sidebar row, menu item, palette command, settings tab, settings search hit, keyboard shortcut, slash command, or model selector.
2. **Zero background activity:** No CLI probing (`claude`, `agy`, `codex`, `cursor`), no credential probing (`mcworker --auth`), no Tailscale listener, no Foreman control server, no scheduled-task daemon, and no wallet initialization.
3. **Zero shipped artifacts in `--minimal`:** Excluded frameworks, CLIs, workers, fixtures, and (in Stage 2) Swift/Coax modules and Apple system framework links are not present in the release `.app` bundle.

---

## 3. Local Inference & Runtime Contract

### 3.1 What Is Already on `origin/main` vs. What Desktop Must Wire

Two of the core Psionic engine blockers from the audit have already landed on `origin/main`:
- **32 GB residency & 32K context guardrails (`f009098a5a`, #13823):** `psionic/platform/inkling-metal.c` (`ink_open_ex`) enforces Apple Silicon ≥ 32 GB unified memory and fits 32,768 context on a 32 GB Mac (9,334 MiB headroom, 2.78 GiB peak RSS).
- **Native parser & strict JSON-schema sampler (`7a85209a15`, #13824):** `psionic/platform/inkling-serve.c` and `#authority psionic/lib/inkling-responses.coax` enforce `response_format` (`json_object` / `json_schema`) and `/v1/responses` (`text.format`) via RFC 8259 constrained token sampling.

Desktop (`#13825`) must lock onto this exact runtime contract:

| Requirement | Contract |
| --- | --- |
| **Server binary** | Ship and run `Contents/Resources/psionic-inkling-serve` + `Contents/Resources/inkling-kernels.metal`. Never measure one server (`llama-server`) and ship another. |
| **Context length** | `--ctx 32768` in `LocalModelServer`, `coax/lib/gateway/card.coax`, and all eval harnesses. |
| **Weights path & integrity** | `~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf` (shared path per `AGENTS.md`). Download is resumable (`curl -fSL -C -`) and **must** pass SHA-256 verification against pinned digest `cbdf3fa9dc50a4124977980f37da4f1fa764b0cfe800d275cd43d26ce2a81af4` before writing the manifest or reporting "Ready". Existing weights on disk are verified before first use. |
| **Logging & port handling** | Never discard server or download output to `/dev/null`. Write logs under `~/.openagents/logs/` (`inkling-download.log`, `psionic-inkling-serve.log`). Default port `18794`, overridable via `OA_INKLING_PORT`. On startup conflict, inspect the listener (`lsof`) and surface the conflicting process name/PID immediately instead of hanging 45s. |
| **Server lifetime on quit** | Start `psionic-inkling-serve` with `--idle-exit-s 300`. If the user quits `OpenAgents.app` while a daemon-owned (`coder-host`) Inkling run is active, leave the server running so the turn finishes cleanly; terminate immediately on quit only when idle. |
| **Safe removal** | `LocalModelStore.removeInkling` refuses to delete weights while any run is active. |
| **First-run onboarding** | On launch with no verified weights: (a) if Intel or < 32 GB RAM, show a clear hardware-requirement screen (no broken chat input); (b) if Apple Silicon ≥ 32 GB, show a single-action download card with progress, byte count, SHA-256 verification state, and automatic transition into a ready chat on completion. |

---

## 4. Paring Down the Shipped App (Build-Time Exclusion Plan)

Runtime flags alone hide features from users, but **still ship ~81% dead weight by disk size and ~62% dead weight by LOC** inside `OpenAgents.app`.

### 4.1 Measured Baseline (Current Full Bundle)

Measured from an actual `OpenAgents.app` build (`apps/openagents-desktop/build.sh`):

- **Total app size today:** **60.5 MB on disk** (**25 MB** zipped via `ditto`).
- **Total UI + Mac host source compiled into `OpenAgents` (excluding tests & external CLIs):** **59,731 LOC** (31,816 Swift + 27,915 Coax; plus 4,687 C glue).

| Bundle Component | Disk Size | Zipped | LOC (if in `OpenAgents`) | Needed for v1.0? |
| --- | ---: | ---: | ---: | --- |
| `Contents/Frameworks/breez_sdk_sparkFFI.framework` | **37.1 MB** | **17 MB** | Rust FFI dylib | **No** (`wallet`) |
| 17 flagged items in `Contents/Resources/` (`lean-run`, `tbench-run`, `oasim`, `claudeworker`, `ple-run`, `agyworker`, `tester-cli`, `proof-export`, `capability-eval`, `agents-catalog`, `gym-evals/`, `fixtures/`, `foreman-ctl`, `bip39-english.txt`, `tbench-harbor/`, `tbench-policies/`, `lean-checker/`) | **3.7 MB** | ~1.5 MB | Separate CLIs | **No** (various flags) |
| `Contents/MacOS/OpenAgents` — Breez static libs (`libBreezSdkSpark.a` 8.5 MB + `libBigNumber.a` 0.3 MB, ~29% of `__text`) | **~4.0 MB** | ~1.5 MB | Static archives | **No** (`wallet`) |
| `Contents/MacOS/OpenAgents` — Other flagged Swift (46 files), Coax modules (19 files), and C objs (`lab-sign.o` 558 KB, `rh-flythrough.o` 62 KB, `pay-wallet*.o` 12 KB) | **~4.5 MB** | ~1.5 MB | **36,866 LOC** (22,998 Swift + 13,868 Coax) | **No** (28 flags) |
| `Contents/MacOS/OpenAgents` — v1.0 Core (23 Swift files, 35 Coax files, `carrier.o`, `ui-host.o`, `desktop-core.o`) | **~5.1 MB** | ~1.5 MB | **22,865 LOC** (8,818 Swift + 14,047 Coax) | **Yes** |
| Core v1.0 `Contents/Resources/` (`coder-host` 2.5 MB, `inkling-kernels.metal` 1.2 MB, `mcworker` 0.9 MB, `Components/` 0.8 MB, `psionic-inkling-serve` 0.2 MB, `OpenAgents.icns` 0.2 MB, `gitguard` 0.1 MB, metadata) | **6.1 MB** | ~2.0 MB | Core runtime | **Yes** |
| **Total today** | **60.5 MB** | **25 MB** | **59,731 LOC** | **~49 MB (81%) removable** |

### 4.2 Build Profiles in `apps/openagents-desktop/build.sh`

Introduce an explicit build mode while keeping all source code intact in the repository:

- **Developer / Full build (default for local dev & CI):** `./build.sh` or `./build.sh --full`
  - Defines `-DOA_FULL -DOA_WALLET`.
  - Compiles and bundles everything; all 28 runtime flags still default to `false` and remain toggleable via `defaults`, `OA_FLAGS`, `--flag`, or `experimentalFeatures`.
  - Full test suites continue to cover all features.
- **v1.0 Minimal Release build:** `./build.sh --minimal` (or `OA_RELEASE_MINIMAL=1`)
  - Used by `install.sh --minimal` and v1.0 release DMG/update packaging.
  - Compiles and bundles only the v1.0 slice in two stages below.

### 4.3 Stage 1 — Bundle, Worker & Spark Strip (High-Leverage Cut)

**Target outcome:** **60.5 MB → ~16 MB on disk (−73%)**, **25 MB → ~5–6 MB zipped (−76%)**.

1. **Skip copying all 17 non-v1 `Contents/Resources/` artifacts** in `build.sh` when `--minimal` is set. Ship only:
   - `coder-host`
   - `mcworker`
   - `psionic-inkling-serve`
   - `inkling-kernels.metal`
   - `gitguard`
   - `Components/` (`components.metal`, `composition.metal`, `fonts/`, `icons/`, `NOTICE`)
   - `OpenAgents.icns`
2. **Exclude Breez Spark from `--minimal`:**
   - Do not invoke `spark-sdk.sh` or copy `breez_sdk_sparkFFI.framework` into `Contents/Frameworks/` (leaving `Contents/Frameworks/` absent/empty).
   - Do not compile or link `pay-wallet.o`, `pay-wallet-backup.o`, `pay-wallet-rpc.o`, `libBreezSdkSpark.a`, or `libBigNumber.a`.
   - Omit `Window+Wallet.swift`, `Window+WalletRPC.swift`, `BreezKey.swift`, `BIP39.swift`, `WalletKeychain.swift`, and `SparkWallet.swift` from `swiftc`, guarding the handful of wallet call sites in `Window.swift` / `AppDelegate.swift` behind `#if OA_WALLET`.
3. **Remove duplicate Inkling build step** from `build.sh` and remove hardcoded personal home-directory fallback paths from `GymBenchmarkView.swift`.

### 4.4 Stage 2 — Full Swift, Coax & Framework Strip (Minimal Binary)

**Target outcome:** **~16 MB → ~11–12 MB on disk (−81% vs. today)**, **~5–6 MB → ~4 MB zipped (−84% vs. today)**, **−62% compiled UI/host LOC**.

1. **Split `swiftc` inputs into `CORE_SWIFT` and `FULL_SWIFT`:**
   - **`CORE_SWIFT` (23 files, ~8,818 LOC):** `Activation.swift`, `AppDelegate.swift`, `DaemonClient.swift`, `DefaultsMigration.swift`, `DesktopCore.swift`, `Notifications.swift`, `Store.swift`, `Updater.swift`, `Window.swift`, `Window+Attach.swift`, `Window+Chats.swift`, `Window+Dialogs.swift`, `Window+Files.swift`, `Window+FollowUps.swift`, `Window+Menus.swift`, `Window+Palette.swift`, `Window+Runs.swift`, `Window+Settings.swift`, `Window+TitleBar.swift`, `Window+Update.swift`, `Window+Worktrees.swift`, `Worker.swift`, `Worktree.swift`.
   - **`FULL_SWIFT` (46 files, ~22,998 LOC, omitted under `--minimal`):** All Gym/Benchmarks/Terminal-Bench/RhFlythrough files (7), Scenes (3), Foreman (5), Account/Auth/Passkey (4), Wallet/Spark (6), Git actions/diff/checkpoints (3), Tailnet (2), Scheduled (2), plus `AgentsPage.swift`, `CapabilityStudio.swift`, `ThreadProof.swift`, `Window+Customize.swift`, `Window+Environments.swift`, `Window+Forge.swift`, `Window+McpSkills.swift`, `Window+Preview.swift`, `Window+Providers.swift`, `Window+Rich.swift`, `Window+Simulator.swift`, `Window+Terminal.swift`, `Window+Usage.swift`, `Window+Voice.swift`.
   - Guard cross-file references inside `CORE_SWIFT` with `#if OA_FULL` so `--minimal` compiles cleanly with zero dead symbols.
2. **Exclude non-v1 C/Coax link objects:**
   - Skip `lab-sign.o` (558 KB) and `rh-flythrough.o` (62 KB) in `--minimal`.
3. **Slim Coax page build (`app-v1.coax`, ~13,868 LOC excluded):**
   - Provide a minimal Coax root (`apps/openagents-desktop/app-v1.coax` + stub module for excluded panels) so `--minimal` omits the 19 flagged-off Coax page modules (`rich`, `gym-lean*`, `termpane`, `customize`, `forge`, `simpane`, `environments`, `usage`, `git*`, `mcpskills`, `providers`, `voice`, `preview`, `scheduled`, `layout/review`) when emitting `gallery.c` / `gallery.o` (shrinking `gallery.o` from ~3.2 MB to ~1.5 MB).
   - Keep `app.coax` untouched for `--full` builds so all 846+ existing page checks continue running unchanged.
4. **Unlink unused Apple system frameworks in `--minimal`:**
   - Drop `-framework Speech`, `-framework AVFoundation`, `-framework CoreMedia`, `-framework CoreVideo`, `-framework WebKit`, `-framework AuthenticationServices`, and `-framework breez_sdk_sparkFFI`.
   - Keep only the frameworks v1.0 actually uses (`AppKit`, `Metal`, `QuartzCore`, `CoreText`, `ImageIO`, `UniformTypeIdentifiers`, `CoreGraphics`, `CryptoKit`, `Security`, `UserNotifications`, `IOKit`).
   - Result: cleaner privacy/entitlement posture for Apple notarization, faster cold launch (`dyld` loads zero third-party frameworks), and faster release builds.

### 4.5 Automated Bundle Verification Gate (`v1-bundle-check`)

Add a Coax tool (`apps/openagents-desktop/v1-bundle-check.coax`, run via `coax/web/tool.sh`) that verifies any `--minimal` `OpenAgents.app` build and fails loudly if anything leaks back in:

1. **Size budget:** Total `.app` ≤ 18 MB (Stage 1) and ≤ 14 MB (Stage 2); zipped archive ≤ 7 MB (Stage 1) and ≤ 5 MB (Stage 2).
2. **Zero bundled frameworks:** `Contents/Frameworks/` does not exist or is empty.
3. **Strict `Contents/Resources/` allowlist:** Only `coder-host`, `mcworker`, `psionic-inkling-serve`, `inkling-kernels.metal`, `gitguard`, `Components/`, and `OpenAgents.icns` may exist.
4. **Strict `otool -L` denylist on `Contents/MacOS/OpenAgents`:** Must **not** link `breez_sdk_sparkFFI`, `WebKit`, `Speech`, `AVFoundation`, or `AuthenticationServices`.
5. **Runtime launch check:** Running the minimal bundle with default environment shows no model selector, no gated sidebar/menu/settings items, and resolves Inkling 2 Mini as the sole model.

---

## 5. Benchmarks, Evals & Release Qualification Gate

Per `AGENTS.md` ("Done means the real thing works"), v1.0 does not ship until the **actual minimal `OpenAgents.app`** and **shipped `psionic-inkling-serve --ctx 32768`** pass real end-to-end checks on real Inkling 2 Mini weights.

### 5.1 Eval Harness Alignment

1. **Default model & server:** `evals/openagents-desktop/tier2/run_tasks.py` and `evals/openagents-desktop/run.sh` default to `inkling-2-mini` against the shipped `psionic-inkling-serve --ctx 32768`, auto-starting and logging the server.
2. **Applicable task filtering:** Mark sub-agent tasks (`c1`, `c2`, `c3`) and USD-budget tasks (`d2`, `d3`) as `N/A` on local Inkling runs; replace dollar-cost assertions with wall time, tok/s, TTFT, and peak RSS.
3. **Honest naming:** Ensure synthetic checks (`psionic/tools/executor-h7-qual.coax`) never report under a Terminal-Bench name.
4. **Keep `llama.cpp` only as a developer reference** inside the gated Gym Benchmarks tab (`--full` builds).

### 5.2 Release Scorecard (4 Published Numbers)

Measured on the final `--minimal` release build with `psionic-inkling-serve --ctx 32768`, reported per Apple Silicon memory tier (**32 GB**, **48 GB**, **64+ GB**):

1. **Session-history pass rate:** All **27 tasks** (9 dev + 18 held-out) in `evals/session-history/tasks/`.
2. **Tier 2 applicable pass rate:** All **10 applicable tasks** (`a1–a3`, `b1–b3`, `d1`, `e1–e3`; 5 marked `N/A`).
3. **Throughput:** Median decode tok/s and prefill tok/s.
4. **Responsiveness & footprint:** Median TTFT (ms), server boot time (ms), and peak RSS (MB).

> **Known failures to resolve before v1.0 sign-off:** An initial live Tier 2 run on `psionic-inkling-serve --ctx 32768` (on the `inkling-first-release` branch) passed **8/10 applicable tasks** (`a1–a3`, `b1–b3`, `e2–e3` passed at $0.00; `c1–c3`, `d2–d3` marked `N/A`), with two real failures:
> - `d1_goal_todo` timed out at 600s (`goal_status=4` after 28 attempts / 54 commands).
> - `e1_refuse_rm_parent` attempted `../` twice (tripping the guard instead of refusing cleanly on turn 1).
>
> Both must be investigated and fixed (prompting, tool loop, or `/goal` budget/exit behavior) before cutting v1.0.

### 5.3 End-to-End App Smoke Matrix

Before tagging v1.0.0, run on the actual signed minimal `OpenAgents.app`:
1. **Fresh 32 GB+ Mac (no weights):** Launches cleanly → shows Inkling onboarding card → downloads weights → verifies SHA-256 → opens chat → real multi-turn edit + command run succeeds.
2. **Unsupported Mac (< 32 GB or simulated RAM floor):** Shows clear unsupported-hardware message; does not offer a broken download or hang.
3. **App quit during active run:** Start a multi-step Inkling turn, quit `OpenAgents.app`, verify `coder-host` + `psionic-inkling-serve` finish the run and `psionic-inkling-serve` exits after idle timeout; reopen app and confirm completed reply is persisted.
4. **Legacy saved chat:** Open a chat saved with `gemini-4-neon` or `claude-sonnet-5-5` → history displays cleanly → next message executes on Inkling 2 Mini and says so.
5. **All automated gates pass:** `v1-bundle-check`, Tier 0 + Tier 1 + Compaction (8/8 suites on `--full` and core suites on `--minimal`), `page-slots.sh`, `flags.coax` (0 surviving mutants), `tools-check`, `wire-check`, `layer-check`.

---

## 6. Phased Execution Plan

| Phase | Deliverable | Current Status (as of 2026-11-21) |
| --- | --- | --- |
| **Phase 0: Engine Readiness** | 32 GB residency guardrails (#13823) and strict JSON-schema sampler + `/v1/responses` (#13824) in `psionic-inkling-serve`. | **Done on `origin/main`** (`f009098a5a`, `7a85209a15`). |
| **Phase 1: Runtime Flags, Single-Model UX & Eval Wiring** | Single model list, model selector removed in release mode, 28-flag Coax registry, SHA-256 download + first-run onboarding + port diagnostics + quit lifetime, Tier 2/docs updated for Inkling. | **In progress** on `inkling-first-release` branch; rebase onto `origin/main` (reconciling `inkling-serve.c` with #13824), verify, and land. |
| **Phase 2: Stage 1 Bundle & Spark Strip** | Add `--minimal` to `build.sh`, guard Wallet/Spark with `#if OA_WALLET`, drop `breez_sdk_sparkFFI.framework` + static Spark libs + 17 non-v1 `Resources/` items, add `v1-bundle-check`. (**60.5 MB → ~16 MB**; **25 MB → ~5–6 MB** zipped.) | **Not started.** |
| **Phase 3: Stage 2 Swift, Coax & Framework Strip** | Split `CORE_SWIFT` vs. `FULL_SWIFT` (`#if OA_FULL`), skip `lab-sign.o` / `rh-flythrough.o`, add `app-v1.coax` slim page entrypoint, drop unused Apple frameworks (`WebKit`, `Speech`, `AVFoundation`, `AuthenticationServices`). (**~16 MB → ~11–12 MB**; **~4 MB** zipped; **−62% compiled LOC**.) | **Not started.** |
| **Phase 4: Qualification, Fixes & Packaging** | Fix Tier 2 `d1_goal_todo` & `e1_refuse_rm_parent`, run all **27 session-history tasks** on `--minimal` + `psionic-inkling-serve --ctx 32768`, test on 32 GB Mac, notarize DMG & update feed, publish 4-number scorecard, close #13825. | **Not started.** |

---

## 7. Explicit Non-Goals for v1.0

- **No deleting feature code from the repo:** All gated subsystems stay maintained in `main` and exercised by `--full` builds and CI.
- **No Linux or Windows v1.0 release:** Psionic's Inkling runtime is Metal-only today; cross-platform releases follow once a non-Metal backend exists.
- **No renaming legacy data paths in v1.0:** `~/Library/Application Support/Coder/` and `CODER_*` environment variables continue working so existing chats aren't orphaned; renaming is a separate migration decision.
- **No cloud fallback or sub-agents:** v1.0 is strictly local single-model Inkling 2 Mini.
- **No `git` diff/checkpoints in v1.0.0:** Deferred to **v1.0.1** as the first incremental capability turned back on after v1.0.0 ships cleanly.
