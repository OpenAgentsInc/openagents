# OpenAgents v1.0 Specification (Inkling-Only Minimal Release)

**Date:** 2026-11-21  
**Status:** Proposed specification  
**Baseline:** `origin/main` `c44941e05f`  
**Target Artifacts:**
- **Committed (v1.0.0 must-ship):** `OpenAgents.app` (`com.openagents.desktop`, macOS 14+, Apple Silicon ≥ 32 GB unified memory, Psionic Metal 4)
- **Stretch / Optional for v1.0.0 (independently cuttable):** OpenAgents Desktop for **Linux** (x86_64 Wayland/Vulkan, Psionic CUDA — optional Vulkan compute) and **Windows** (x86_64 Win32/Vulkan, Psionic CUDA — optional Vulkan compute)

**Scope:** `apps/openagents-desktop/` (macOS, `linux/`, `windows/`, `host-main.c`), `coax/lib/agent/desktop/`, `psionic/lib/inkling/`, `psionic/platform/inkling-*`, `psionic/platform/cuda-host.*`, `evals/openagents-desktop/`, `evals/session-history/`  
**Related docs:**
- [OpenAgents Desktop: audit for an Inkling-only first release](../audits/2026-11-21-openagents-desktop-inkling-first-release-audit.md)
- [Coax Desktop](coax-desktop.md)
- [Desktop Evals Specification](../coder/desktop-evals-spec.md)
- [Ch. 269: Inkling 2 Mini on Metal](../../psionic/book/ch269-inkling-2-mini-on-metal.md)
- [Ch. 270: Tool Calls and Responses for Inkling 2 Mini](../../psionic/book/ch270-tool-calls-and-responses-for-inkling-2-mini.md)

---

## 1. Purpose and Product Definition

OpenAgents v1.0 is a **minimal, single-model, local-only coding desktop app** running **Inkling 2 Mini** directly on Psionic's native Coax-emitted GPU runtime (`psionic-inkling-serve`).

- **macOS (Apple Silicon ≥ 32 GB) is the committed v1.0 launch target.** It ships as soon as its gates pass.
- **Linux and Windows are optional stretch targets for v1.0** (see §5). Both already share the Coax UI, core, workers, and Vulkan window hosts; adding them to v1.0 requires moving local-model lifecycle/flags into cross-platform Coax + `host-main.c` and running the same from-scratch Psionic GPU kernel hillclimbing loop on CUDA (and optionally Vulkan compute) that brought Metal 4 up to production speed. Either or both may still be cut from v1.0.0 if they miss the bar—without blocking macOS.

Everything outside the core local coding loop—cloud models, accounts, Bitcoin wallet, Gym, Foreman, iOS Simulator, integrated terminal, web preview, voice, MCP/Skills, remote environments, Tailscale pairing, 3D scenes, and more—**stays in the monorepo** for future releases and developer builds, but is **excluded from the shipped v1.0 app** at both runtime and build time.

### Core Principles

1. **One model, zero model selector.** v1.0 runs only Inkling 2 Mini (26.4B looped MoE, 3.4B active, ~17.6 GB `IQ4_NL` GGUF) locally. There is no model picker, no cloud API key setup, no account login, and no dollar metering.
2. **Minimal surface area.** Keep only the features required to open a repo, talk to Inkling 2 Mini, attach files, approve actions, run `/goal`, keep worktrees clean, and update the app.
3. **Code stays in the codebase; only v1.0 ships.** All non-v1.0 code remains tracked, compiled, and tested in full developer builds (`build.sh --full`). The v1.0 release build (`build.sh --minimal`) strips excluded frameworks, helper CLIs, workers, Swift files, and Coax modules out of the shipped package so dead weight is never distributed, signed, or loaded.
4. **Coax-emitted kernels everywhere, no cuBLAS/cuDNN.** Per `AGENTS.md`, Psionic writes all GPU arithmetic in Coax (`coax metal-compute`, `coax cuda`, `coax spirv`). No hand-written vendor kernels and no cuBLAS/cuDNN in anything shipped; `llama.cpp` (and opt-in cuBLAS harnesses) serve strictly as parity and throughput oracles during hillclimbing.
5. **Honest hardware and quality bars.** Unsupported machines (macOS Intel or < 32 GB RAM; Linux/Windows without a qualifying GPU/VRAM budget) are told immediately on first run. Release quality is measured solely on the real shipped minimal app and `psionic-inkling-serve` binary against real weights on each platform that ships.

---

## 2. Exact Feature Scope

### 2.1 Included in OpenAgents v1.0

| Area | Included Capabilities | Shared Coax / Workers | macOS Host | Linux & Windows Host (§5) |
| --- | --- | --- | --- | --- |
| **Core Chat** | Send, stop, steer, follow-up queue, markdown replies, tool rows, streaming thoughts. No model selector; no sub-agents (`agents = false`). | `page/transcript.coax`, `page/followups.coax`, `page/compose.coax`, `page/input.coax`, `mcworker` | `Window.swift`, `Window+Runs.swift`, `Worker.swift` | `host-main.c`, `host.c`, `host-core.coax` |
| **First-Run & Local Model** | Hardware/VRAM check, resumable Hugging Face download into `~/.openagents/models/inkling-2-mini/`, progress bar, SHA-256 verification, safe delete (blocked during active runs), eager launch prewarm of `psionic-inkling-serve` with automatic free-port selection and logs, and composer send lock until the server is ready. | `page/layout/conversation.coax`, `#authority coax/lib/agent/desktop/local-model.coax` | `Window+Dialogs.swift` (`LocalModelStore`, `LocalModelServer`) | Thin OS adapter in `host-main.c` + `linux/main.c` / `windows/main.c` |
| **Chat History & Sidebar** | Persisted chats, sidebar list, new/rename/delete/fork. Pre-existing chats created on hidden models open readable and continue on Inkling with an explicit notice. | `page/chats.coax`, `page/persist.coax`, `page/layout/sidebar.coax` | `Store.swift` | `host-core.coax` / `host-main.c` |
| **Workspace & Attachments** | Working folder picker, file attachments, and `/` slash commands (`/goal`, `/new`, `/fork`). No `@` file mentions and no file viewer pane in v1.0. | `page/attach.coax`, `page/mention.coax` (`/` only) | `Window+Attach.swift` | `portal.c`, `host-main.c` |
| **Navigation & Search** | Command palette (v1.0 commands only) and in-chat Find. | `page/palette.coax`, `page/find.coax` | `Window+Palette.swift` | `host-main.c` |
| **Agent Interaction** | Permission prompts, interactive question cards, to-do tray, `/goal` autonomous loop with end-of-goal verification turn. | `page/permit.coax`, `page/todos.coax`, `page/goal.coax` | `Window.swift` | `host-main.c` |
| **Background Execution & Safety** | Detached daemon (`coder-host`) so runs survive app quit; per-chat git worktrees and `gitguard`. | `coder-host`, `gitguard` | `DaemonClient.swift`, `Worktree.swift`, `Window+Worktrees.swift` | `host-core.coax`, `host-main.c` |
| **Minimal Settings** | General, Local Model (status / download / verify / remove), Agent Instructions, About. No cloud keys, accounts, or gated tabs. | `settings.coax`, `coax/lib/agent/desktop/flags.coax` | `Window+Settings.swift` | `host-main.c` |
| **Desktop Basics** | Signed auto-updates, desktop notifications, standard OS window chrome. | `page/notify.coax`, `update-feed.coax` | `Updater.swift`, `Notifications.swift`, `AppDelegate.swift`, `Window+Menus.swift` | `host-main.c`, `linux/main.c`, `windows/main.c` |

> **Candidate first post-v1.0 addition (`git`):** Keep `git` (diff viewer, git actions toolbar, checkpoints/rewind) excluded from v1.0.0 so the initial surface stays minimal, and designate it as the first flag to turn on in **v1.0.1**.

### 2.2 Excluded from OpenAgents v1.0 (29 Flags, All Off & Stripped at Build Time)

Every row below stays in the repository and compiles under `--full` developer builds, is controlled by the shared Coax flag registry (`coax/lib/agent/desktop/flags.coax`) with default `false`, and is **excluded from shipped v1.0 builds** under `--minimal`.

**Excluded Models (7 flags):**
- `models.gemini` — Gemini 4 Neon/Carbon/Hydrogen/Argon, Gemini API key, Vertex/ADC/env sign-in probe, Gemini sub-agent levels.
- `models.claude` — Claude Fable/Opus/Sonnet/Haiku, `claudeworker`, `claude` CLI detection, Claude Key, Claude sub-agents.
- `models.openrouter` — OpenRouter models, key settings, and sub-agent level.
- `models.antigravity` — Antigravity model entry, `agyworker`, `agy` CLI detection, and settings rows.
- `models.external` — Codex, Cursor, ACP Registry, multi-model handoff and fan-out.
- `models.executor` — Hybrid Executor, weight modules, Local Pylon, settled sats, shadow rollout.
- `models.inklingNeon` — Inkling + Neon hybrid routing and Inkling as a cloud sub-agent.

**Excluded Features (22 flags):**
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
- `files` — File Explorer tree, inline file viewer/editor pane (`page/filepane.coax`, `page/filetree.coax`, `Window+Files.swift`), `@` file mentions in the composer, and the repository Files tab in the command palette.

### 2.3 Hard Invariants for Excluded Features

1. **Zero UI residue:** No sidebar row, menu item, palette command, settings tab, settings search hit, keyboard shortcut, slash command, or model selector.
2. **Zero background activity:** No CLI probing (`claude`, `agy`, `codex`, `cursor`), no credential probing (`mcworker --auth`), no Tailscale listener, no Foreman control server, no scheduled-task daemon, and no wallet initialization.
3. **Zero shipped artifacts in `--minimal`:** Excluded frameworks, CLIs, workers, fixtures, and (in Stage 2) excluded Swift/Coax modules and unused OS framework links are absent from every platform's release artifact.

---

## 3. Local Inference & Runtime Contract

### 3.1 What Is Already on `origin/main` vs. What Desktop Must Wire

Two core Psionic engine milestones have already landed on `origin/main`:
- **32 GB residency & 32K context guardrails (`f009098a5a`, #13823):** `psionic/platform/inkling-metal.c` (`ink_open_ex`) enforces Apple Silicon ≥ 32 GB unified memory and fits 32,768 context on a 32 GB Mac (9,334 MiB headroom, 2.78 GiB peak RSS).
- **Native parser & strict JSON-schema sampler (`7a85209a15`, #13824):** `psionic/platform/inkling-serve.c` and `#authority psionic/lib/inkling-responses.coax` enforce `response_format` (`json_object` / `json_schema`) and `/v1/responses` (`text.format`) via RFC 8259 constrained token sampling.

Desktop (`#13825` on macOS, plus Linux/Windows adapters in §5) must lock onto this runtime contract on every supported OS:

| Requirement | Contract |
| --- | --- |
| **Server binary** | Ship and run `psionic-inkling-serve` (`psionic-inkling-serve.exe` on Windows) paired with the platform's Coax-emitted kernel file (`inkling-kernels.metal` on macOS; `inkling-kernels-cuda.cu` and/or `inkling-kernels-vulkan.spv` on Linux/Windows). Never measure one server (`llama-server`) and ship another. |
| **Context length** | `--ctx 32768` across the desktop server supervisor, `coax/lib/gateway/card.coax`, and all eval harnesses. |
| **Weights path & integrity** | `~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf` (`%USERPROFILE%\.openagents\models\inkling-2-mini\...` on Windows). Download is resumable (`curl -fSL -C -`) and **must** pass SHA-256 verification against pinned digest `cbdf3fa9dc50a4124977980f37da4f1fa764b0cfe800d275cd43d26ce2a81af4` before writing the manifest or reporting "Ready". Existing weights on disk are verified before first use. |
| **Logging, prewarm & port handling** | Never discard server or download output to `/dev/null` or `NUL`. Write logs under `~/.openagents/logs/` (`inkling-download.log`, `psionic-inkling-serve.log`). Prewarm `psionic-inkling-serve` eagerly on launch once weights are verified and block message submission until `/health` is green. Default port `18794`; if occupied by another process (unless `OA_INKLING_PORT` was explicitly set), automatically scan and bind the next free localhost port (`18794..18857`) and reuse any already-running healthy Psionic server on the recorded port. |
| **Server lifetime on quit** | Start `psionic-inkling-serve` detached with `--idle-exit-s 300`. If the user quits the app while a daemon-owned (`coder-host`) Inkling run is active, leave the server running so the turn finishes cleanly; terminate immediately on quit only when idle. |
| **Safe removal** | Refuse to delete weights while any run is active (`runningCount > 0`). |
| **First-run onboarding** | On launch with no verified weights: (a) if hardware fails the platform's floor (e.g., Intel Mac / < 32 GB RAM, or Linux/Windows without a qualifying GPU/VRAM budget), show a clear hardware-requirement screen; (b) if supported, show a single-action download card with progress, byte count, SHA-256 verification state, and automatic transition into a ready chat on completion. |

---

## 4. Paring Down the Shipped macOS App (Build-Time Exclusion Plan)

Runtime flags alone hide features from users, but **still ship ~81% dead weight by disk size and ~62% dead weight by LOC** inside `OpenAgents.app`.

### 4.1 Measured Baseline (Current Full macOS Bundle)

Measured from an actual `OpenAgents.app` build (`apps/openagents-desktop/build.sh`):

- **Total app size today:** **60.5 MB on disk** (**25 MB** zipped via `ditto`).
- **Total UI + Mac host source compiled into `OpenAgents` (excluding tests & external CLIs):** **59,731 LOC** (31,816 Swift + 27,915 Coax; plus 4,687 C glue).

| Bundle Component | Disk Size | Zipped | LOC (if in `OpenAgents`) | Needed for v1.0? |
| --- | ---: | ---: | ---: | --- |
| `Contents/Frameworks/breez_sdk_sparkFFI.framework` | **37.1 MB** | **17 MB** | Rust FFI dylib | **No** (`wallet`) |
| 17 flagged items in `Contents/Resources/` (`lean-run`, `tbench-run`, `oasim`, `claudeworker`, `ple-run`, `agyworker`, `tester-cli`, `proof-export`, `capability-eval`, `agents-catalog`, `gym-evals/`, `fixtures/`, `foreman-ctl`, `bip39-english.txt`, `tbench-harbor/`, `tbench-policies/`, `lean-checker/`) | **3.7 MB** | ~1.5 MB | Separate CLIs | **No** (various flags) |
| `Contents/MacOS/OpenAgents` — Breez static libs (`libBreezSdkSpark.a` 8.5 MB + `libBigNumber.a` 0.3 MB, ~29% of `__text`) | **~4.0 MB** | ~1.5 MB | Static archives | **No** (`wallet`) |
| `Contents/MacOS/OpenAgents` — Other flagged Swift (47 files), Coax modules (21 files), and C objs (`lab-sign.o` 558 KB, `rh-flythrough.o` 62 KB, `pay-wallet*.o` 12 KB) | **~4.5 MB** | ~1.5 MB | **36,866+ LOC** | **No** (29 flags) |
| `Contents/MacOS/OpenAgents` — v1.0 Core (22 Swift files, 33 Coax files, `carrier.o`, `ui-host.o`, `desktop-core.o`) | **~5.1 MB** | ~1.5 MB | **~21,000 LOC** | **Yes** |
| Core v1.0 `Contents/Resources/` (`coder-host` 2.5 MB, `inkling-kernels.metal` 1.2 MB, `mcworker` 0.9 MB, `Components/` 0.8 MB, `psionic-inkling-serve` 0.2 MB, `OpenAgents.icns` 0.2 MB, `gitguard` 0.1 MB, metadata) | **6.1 MB** | ~2.0 MB | Core runtime | **Yes** |
| **Total today** | **60.5 MB** | **25 MB** | **59,731 LOC** | **~49 MB (81%) removable** |

### 4.2 Build Profiles Across Platforms

Introduce one consistent build switch across `apps/openagents-desktop/build.sh`, `apps/openagents-desktop/linux/build.sh`, and `apps/openagents-desktop/windows/build.sh` while keeping all source code intact in the repository:

- **Developer / Full build (default for local dev & CI):** `./build.sh` or `./build.sh --full`
  - Defines `-DOA_FULL -DOA_WALLET` (on macOS).
  - Compiles and bundles everything; all 29 runtime flags still default to `false` and remain toggleable via OS prefs / `OA_FLAGS` / `--flag` / `experimentalFeatures`.
  - Full test suites continue to cover all features.
- **v1.0 Minimal Release build:** `./build.sh --minimal` (or `OA_RELEASE_MINIMAL=1`)
  - Used by `install.sh --minimal` and v1.0 release packaging.
  - Compiles and bundles only the v1.0 slice.

### 4.3 Stage 1 — macOS Bundle, Worker & Spark Strip (High-Leverage Cut)

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
   - Omit `Window+Wallet.swift`, `Window+WalletRPC.swift`, `BreezKey.swift`, `BIP39.swift`, `WalletKeychain.swift`, and `SparkWallet.swift` from `swiftc`, guarding wallet call sites in `Window.swift` / `AppDelegate.swift` behind `#if OA_WALLET`.
3. **Remove duplicate Inkling build step** from `build.sh` and remove hardcoded personal home-directory fallback paths from `GymBenchmarkView.swift`.

### 4.4 Stage 2 — macOS Full Swift, Coax & Framework Strip (Minimal Binary)

**Target outcome:** **~16 MB → ~11–12 MB on disk (−81% vs. today)**, **~5–6 MB → ~4 MB zipped (−84% vs. today)**, **−62% compiled UI/host LOC**.

1. **Split `swiftc` inputs into `CORE_SWIFT` and `FULL_SWIFT`:**
   - **`CORE_SWIFT` (22 files):** `Activation.swift`, `AppDelegate.swift`, `DaemonClient.swift`, `DefaultsMigration.swift`, `DesktopCore.swift`, `Notifications.swift`, `Store.swift`, `Updater.swift`, `Window.swift`, `Window+Attach.swift`, `Window+Chats.swift`, `Window+Dialogs.swift`, `Window+FollowUps.swift`, `Window+Menus.swift`, `Window+Palette.swift`, `Window+Runs.swift`, `Window+Settings.swift`, `Window+TitleBar.swift`, `Window+Update.swift`, `Window+Worktrees.swift`, `Worker.swift`, `Worktree.swift`.
   - **`FULL_SWIFT` (47 files, omitted under `--minimal`):** All Gym/Benchmarks/Terminal-Bench/RhFlythrough files (7), Scenes (3), Foreman (5), Account/Auth/Passkey (4), Wallet/Spark (6), Git actions/diff/checkpoints (3), Tailnet (2), Scheduled (2), plus `AgentsPage.swift`, `CapabilityStudio.swift`, `ThreadProof.swift`, `Window+Customize.swift`, `Window+Environments.swift`, `Window+Files.swift`, `Window+Forge.swift`, `Window+McpSkills.swift`, `Window+Preview.swift`, `Window+Providers.swift`, `Window+Rich.swift`, `Window+Simulator.swift`, `Window+Terminal.swift`, `Window+Usage.swift`, `Window+Voice.swift`.
   - Guard cross-file references inside `CORE_SWIFT` with `#if OA_FULL` so `--minimal` compiles cleanly with zero dead symbols.
2. **Exclude non-v1 C/Coax link objects:**
   - Skip `lab-sign.o` (558 KB) and `rh-flythrough.o` (62 KB) in `--minimal`.
3. **Slim Coax page build (`app-v1.coax`, shared by macOS, Linux, and Windows):**
   - Provide a minimal Coax root (`apps/openagents-desktop/app-v1.coax` + stub module for excluded panels) so `--minimal` omits the 21 flagged-off Coax page modules (`rich`, `gym-lean*`, `termpane`, `customize`, `forge`, `simpane`, `environments`, `usage`, `git*`, `mcpskills`, `providers`, `voice`, `preview`, `scheduled`, `layout/review`, `filepane`, `filetree`) when emitting `gallery.c` / `gallery.o` (shrinking `gallery.o` from ~3.2 MB to ~1.5 MB across all three OSes).
   - Keep `app.coax` untouched for `--full` builds so all 846+ existing page checks continue running unchanged.
4. **Unlink unused Apple system frameworks in `--minimal`:**
   - Drop `-framework Speech`, `-framework AVFoundation`, `-framework CoreMedia`, `-framework CoreVideo`, `-framework WebKit`, `-framework AuthenticationServices`, and `-framework breez_sdk_sparkFFI`.
   - Keep only the frameworks v1.0 actually uses (`AppKit`, `Metal`, `QuartzCore`, `CoreText`, `ImageIO`, `UniformTypeIdentifiers`, `CoreGraphics`, `CryptoKit`, `Security`, `UserNotifications`, `IOKit`).

### 4.5 Automated Bundle Verification Gate (`v1-bundle-check`)

Add a Coax tool (`apps/openagents-desktop/v1-bundle-check.coax`, run via `coax/web/tool.sh`) that verifies any `--minimal` build (macOS, Linux, or Windows) and fails loudly if excluded artifacts leak back in:

1. **Size budget (macOS):** Total `.app` ≤ 18 MB (Stage 1) and ≤ 14 MB (Stage 2); zipped archive ≤ 7 MB (Stage 1) and ≤ 5 MB (Stage 2). *(Linux and Windows `--minimal` packages stay even smaller—target ≤ 12 MB unpacked / ≤ 5 MB compressed—because they ship zero Swift or Spark binaries.)*
2. **Zero bundled frameworks (macOS):** `Contents/Frameworks/` does not exist or is empty.
3. **Strict shipped-files allowlist:** Only core executable, `coder-host`, `mcworker`, `psionic-inkling-serve`, platform kernel file(s), `gitguard`, `Components/`, and app icon/metadata may exist (no `claudeworker`, `agyworker`, `oasim`, `lean-run`, `tbench-run`, `gym-evals`, `bip39-english.txt`, etc.).
4. **Strict dynamic-link denylist:**
   - macOS (`otool -L`): must not link `breez_sdk_sparkFFI`, `WebKit`, `Speech`, `AVFoundation`, or `AuthenticationServices`.
   - Linux (`ldd`) / Windows (`objdump -p`): must not statically or dynamically require `libcuda.so` / `nvcuda.dll` at load time (CUDA is always `dlopen`/`LoadLibraryA` at runtime so machines without an NVIDIA driver still boot cleanly to the onboarding screen).
5. **Runtime launch check:** Running the minimal build with default environment shows no model selector, no gated sidebar/menu/settings items, and resolves Inkling 2 Mini as the sole model.

---

## 5. Cross-Platform Architecture & Linux / Windows Bring-Up (Optional v1.0 Targets)

### 5.1 Platform Tiering & Independent Cut/Ship Gate

- **Tier 1 — Committed for v1.0.0:** **macOS (Apple Silicon ≥ 32 GB, Metal 4).**
- **Tier 2 — Optional / Stretch for v1.0.0:** **Linux (x86_64, Wayland/Vulkan UI, CUDA compute)** and **Windows (x86_64, Win32/Vulkan UI, CUDA compute)**.
  - Work on Linux and Windows runs on a parallel track (Track B in §7) and **never blocks macOS v1.0.0**.
  - Either platform ships in v1.0.0 **if and only if** it independently clears all four gates below before the macOS release cut: (1) kernel parity & hillclimbed speed bar (§5.4), (2) cross-platform local-model onboarding & server supervision (§5.5), (3) `--minimal` bundle check (§5.6), and (4) full 4-number live scorecard on real hardware (§6.2).
  - If a platform is not ready at cut time, it is cleanly omitted from v1.0.0 and promoted in v1.0.x once green.

### 5.2 What Is Already Cross-Platform vs. What Must Become Cross-Platform

A large fraction of OpenAgents Desktop is already cross-platform; the remaining gaps are concentrated in the Inkling GPU backend, POSIX-only C helpers, and the macOS-only Swift local-model lifecycle:

| Layer | Today on macOS | Today on Linux (`apps/openagents-desktop/linux/`) | Today on Windows (`apps/openagents-desktop/windows/`) | Work Required to Be Truly Cross-Platform |
| --- | --- | --- | --- | --- |
| **Desktop UI & State** (`page.coax`, `app-v1.coax`, `settings.coax`) | Coax → C (`gallery.o`) | **Already shared** (same `app.coax`) | **Already shared** (same `app.coax`) | Switch `--minimal` builds on all 3 OSes to `app-v1.coax`. |
| **GPU UI Shaders** (`components/backend/*.coax`) | `coax metal` → `.metal` | **Already shared** (`coax spirv` → `.spv` on Vulkan) | **Already shared** (`coax spirv` → `.spv` on Vulkan) | None. |
| **Window / Input Host** | Swift + ObjC (`ui-host.m`) | **Already works** (Wayland + Vulkan `ui-host.c`, `portal.c`, real-window input tests passing) | **Already works** (Win32 + Vulkan `ui-host.c`, `portal.c`, Wine smoke tests; needs real Win11 verification) | Real Windows 11 native smoke & GPU run. |
| **Workers & Daemon** (`mcworker`, `coder-host`, `gitguard`) | Coax → Mach-O | **Already shared** (Coax → Linux ELF) | **Already shared** (Coax → MinGW `.exe`, except `gitguard.exe` needs bundling) | Bundle `gitguard.exe` on Windows; omit `claudeworker`/`agyworker` in `--minimal`. |
| **29-Flag Registry** (`coax/lib/agent/desktop/flags.coax`) | Coax → Swift (`defaults`, `OA_FLAGS`, `--flag`) | Wired into `host-main.c` | Wired into `host-main.c` | Done (`flags.coax` + `desktop-flags.json`). |
| **Local Model Store & Server Supervisor** | **macOS-only Swift** (`Window+Dialogs.swift`: download, SHA-256, manifest, server spawn, port check, quit lifetime) | **Missing** in `host-main.c` | **Missing** in `host-main.c` | Extract policy/state machine into `#authority coax/lib/agent/desktop/local-model.coax`; add thin OS adapters in `host-main.c` (see §5.5). |
| **Inkling HTTP/SSE & Schema Server** (`psionic/platform/inkling-serve.c`, `inkling-responses.coax`) | POSIX sockets + hardcoded `inkling-metal.h` | Compiles only against POSIX + Metal | Does not compile (POSIX sockets, `mmap`, `clock_gettime`, Metal) | Split out backend-agnostic `inkling-engine.h`; add `#ifdef _WIN32` Winsock2 + Win32 `MapViewOfFile` in `inkling-gguf.c` (see §5.3). |
| **Inkling GPU Inference Backend** (`psionic/lib/inkling/kernels*.coax`) | **Done & hillclimbed** (`coax metal-compute` + `inkling-metal.c`) | **Not built yet** (`cuda-host.c` exists for Linux `dlopen`!) | **Not built yet** (`cuda-host.c` needs Win32 `LoadLibraryA`) | Build & hillclimb `kernels-cuda.coax` + `inkling-cuda.c` from scratch (and optionally `kernels-vulkan.coax` + `inkling-vulkan.c`; see §5.4). |

### 5.3 Engine Abstraction & C Portability (`psionic/platform/`)

Before starting GPU kernel work, decouple the HTTP/SSE server and GGUF loader from macOS/Metal so **one server binary** serves Metal, CUDA, and Vulkan identically:

1. **Backend-agnostic C engine contract (`psionic/platform/inkling-engine.h`):**
   - Extract the existing `InkSession`, `InkHParams`, `InkSampleParams`, `InkMemoryBudget`, and `ink_open_ex` / `ink_prefill` / `ink_step` / `ink_prefix_save` / `ink_prefix_restore` / `ink_logits` / `ink_encode` / `ink_decode_piece` / `ink_sample` / `ink_close` signatures out of `inkling-metal.h` into `inkling-engine.h` (with a `const char *ink_backend_name(const InkSession *)` returning `"psionic-metal"`, `"psionic-cuda"`, or `"psionic-vulkan"` for `/health`).
   - `inkling-metal.c`, `inkling-cuda.c`, and (optional) `inkling-vulkan.c` all implement `inkling-engine.h`.
   - `inkling-serve.c` and `inkling-cli.c` include only `inkling-engine.h` and remain 100% shared (including the `#authority psionic/lib/inkling-responses.coax` parser and strict RFC 8259 JSON-schema constrained sampler).
2. **Portable GGUF mapping (`psionic/platform/inkling-gguf.c`):**
   - Replace direct `<sys/mman.h>` `mmap`/`munmap` calls with a 30-line `#ifdef _WIN32` `CreateFileA` / `CreateFileMappingA` / `MapViewOfFile` / `UnmapViewOfFile` wrapper so Windows memory-maps the 17.6 GB GGUF directly without allocating 17.6 GB of heap RAM.
3. **Portable HTTP/SSE server (`psionic/platform/inkling-serve.c`):**
   - Add thin `#ifdef _WIN32` wrappers for sockets (`winsock2.h`, `ws2tcpip.h`, `WSAStartup`, `SOCKET`, `closesocket`), monotonic clock (`QueryPerformanceCounter`), threads (`CreateThread` / `CRITICAL_SECTION` where needed), and shutdown handling (`SetConsoleCtrlHandler`).
   - Keep `psionic-inkling-serve` zero-dependency (no libcurl/OpenSSL needed—localhost HTTP only).
4. **Cross-platform dynamic CUDA loader (`psionic/platform/cuda-host.c`):**
   - `cuda-host.c` already uses runtime `dlopen("libcuda.so.1")` and `dlopen("libnvrtc.so")` on Linux so builds require no CUDA SDK and non-NVIDIA hosts report `PC_ABSENT`.
   - Add `#ifdef _WIN32` `LoadLibraryA("nvcuda.dll")` + `LoadLibraryA("nvrtc64_120_0.dll")` (probing CUDA 12/13 NVRTC DLL names) + `GetProcAddress`, plus `QueryPerformanceCounter` timing. Now the exact same `cuda-host.c` + `inkling-cuda.c` drives both **Linux and Windows**.

### 5.4 From-Scratch Psionic GPU Kernel Hillclimbing (Linux & Windows)

Agents will apply the **exact same from-scratch hillclimbing process** used to bring Inkling 2 Mini on Metal 4 from initial correctness to 75+ tok/s decode and 1,449 tok/s prefill (Sprints 1–53).

#### Primary Backend: NVIDIA CUDA (`coax cuda` + `coax/lib/compute/mma.coax`)

Coax already emits CUDA C for NVRTC (`coax cuda`) and already includes `#authority coax/lib/compute/mma.coax` (`cu_mma_bf16_16x8x16` warp-wide Tensor Core MMA). The CUDA build produces `psionic/lib/inkling/kernels-cuda.coax` (compiled to `.cu` at build time and JIT-compiled via NVRTC at startup with a disk PTX/cubin cache under `~/.openagents/cache/psionic/` so cold boot stays under ~2 seconds after first launch).

**Hardware / VRAM Floor for Discrete GPUs (Explicit Design Reality):**
Unlike unified-memory Apple Silicon (≥ 32 GB shared), discrete Linux/Windows GPUs split VRAM and system RAM:
- **Recommended v1.0 floor (`IQ4_NL` full VRAM residency): ≥ 24 GB VRAM** (RTX 3090, RTX 4090, RTX 5090, RTX A5000/A6000, L4, A10G). Fits the full 17.6 GB `IQ4_NL` weights + 32K split SWA+Full KV cache + scratchpad 100% resident in VRAM with zero PCIe thrashing.
- **Stretch sub-target (16 GB VRAM GPUs, e.g., RTX 4080 / 5070 Ti + ≥ 32 GB system RAM):** Either ship a verified **NVFP4** checkpoint (~13–14 GB weights, matching the NVFP4 register-dequant kernel math already proven in #13823) so full residency fits inside 16 GB VRAM, **or** add pinned-host MoE expert residency/streaming via `pc_host_alloc` + async copy streams. Do **not** silently page over PCIe at 2 tok/s; if a 16 GB card cannot meet the speed floor, report the VRAM requirement honestly on first run.

**Four-Milestone CUDA Hillclimbing Playbook:**

| Milestone | Scope | Verification & Exit Gate |
| --- | --- | --- |
| **C0: Correctness & Exact Parity** | Port the full Inkling 2 Mini looped MoE graph (`K=3`, 24 phys / 48 logical slots, 8 full + 40 SWA-1024 KV slots, `IQ4_NL`/`Q8_0`/`F32` + `NVFP4`/`MXFP8` dequant, causal GQA, shortconv, cross-pass recurrence gate, top-k MoE router, logit softcap) to `psionic/lib/inkling/kernels-cuda.coax` + `psionic/platform/inkling-cuda.c`. | Exact top-1 agreement and cosine similarity ≥ 0.99999 against CPU reference and `llama.cpp` CUDA across `K=1, 3, 5`; `--verify-prefill-decode` passes; `--test-quant-kernels` bit-exact (`0.00e+00`) on real NVIDIA GPU. |
| **C1: VRAM Budget & 32K Residency** | Mirror `#13823`'s `InkMemoryBudget` in `inkling-cuda.c` using `pc_memory()` (`cuMemGetInfo`) + `#authority` split SWA+Full KV planner (`psionic/lib/inkling-metal.coax` generalized to `inkling-budget.coax`). Add adaptive prefill chunking so 32,768 context fits cleanly on 24 GB VRAM. | Under-VRAM GPUs return a clean structured error before allocation; 32K context runs end-to-end on a 24 GB NVIDIA GPU without OOM; `/health` reports `engine: "psionic-cuda"` and `ctx: 32768`. |
| **C2: Iterative Throughput Hillclimbing** | Profile each op with CUDA events / Nsight against `llama.cpp` CUDA (and opt-in cuBLAS parity harness — **never** linked in shipped binaries), attacking the top bottleneck each sprint: (1) warp-shuffle reductions & 32-lane zero-shared-mem warp router; (2) vectorized `IQ4_NL`/`Q8_0`/`NVFP4` register dequant + FP16/BF16 `cu_mma_bf16_16x8x16` Tensor Core prefill (`coax/lib/compute/mma.coax`); (3) fused RMSNorm/QK-clip/R-clip & 1-pass GQA decode; (4) **CUDA Graph capture** (`PcProgramData` in `cuda-host.c`) to collapse 48-slot decode launch overhead; (5) PTX/cubin disk cache & prompt-prefix KV reuse. | Every sprint keeps C0 parity green and records before/after tok/s (decode + prefill) vs. `llama.cpp` CUDA on the exact 17.6 GB Inkling 2 Mini GGUF. |
| **C3: Ship Bar** | Run the full `psionic-inkling-serve --ctx 32768` live eval suite on real Linux and/or Windows NVIDIA hardware. | **Throughput:** ≥ 80% of `llama.cpp` CUDA (target ≥ 50 tok/s decode, ≥ 500 tok/s prefill at 32K ctx on a 24 GB+ desktop GPU). **Quality:** passes Tier 2 & 27-task session-history parity with Metal. **Stability:** zero CUDA errors/leaks across full suite. |

#### Secondary / Optional Backend: Vulkan Compute (`coax spirv --kernel`)

- **Why:** Covers AMD Radeon (RDNA 3/4), AMD Ryzen AI Max / Strix Halo (32–128 GB unified memory — ideal for Inkling 2 Mini on Linux & Windows!), and Intel Arc without vendor lock-in.
- **Path:** Emit compute shaders from `psionic/lib/inkling/kernels-vulkan.coax` via `coax spirv --kernel` and run them through a thin Vulkan compute host sharing `metaverse/gpu/vkc.c` (`psionic/platform/inkling-vulkan.c`), then run the same C0–C3 parity and hillclimbing loop (subgroup ops + `VK_KHR_cooperative_matrix` where supported).
- **Priority:** Strictly secondary to CUDA for v1.0; pursue only after CUDA clears C0–C2 or if targeting AMD Strix Halo specifically.

### 5.5 Cross-Platform Local Model Lifecycle & Feature Flags

To avoid maintaining two divergent implementations (Swift on macOS vs. C on Linux/Windows), move the policy and state machine into Coax so all three hosts are thin OS adapters:

1. **Shared Coax Policy (`coax/lib/agent/desktop/local-model.coax`, `#authority`, 0 surviving mutants):**
   - Owns model ID (`inkling-2-mini`), Hugging Face URL, pinned SHA-256 digest (`cbdf3fa9dc50a4124977980f37da4f1fa764b0cfe800d275cd43d26ce2a81af4`), expected byte size, manifest JSON codec (`bytes`, `mtime`, `inode`/`file_id`, `sha256`, `loops`), platform hardware admission rules (macOS Apple Silicon ≥ 32 GB; Linux/Windows CUDA ≥ 24 GB VRAM or verified NVFP4/Vulkan budget), state machine (`not_downloaded` / `downloading` / `verifying` / `ready` / `error` / `unsupported_hw`), and safe-removal rule (`running_count == 0`).
2. **Wire the 29-Flag Registry Into `host-main.c`:**
   - Replace the stub `case 36` in `apps/openagents-desktop/host-main.c` with calls into `coax/lib/agent/desktop/flags.coax` (via `host-core.coax`).
   - Support the exact same overrides on Linux and Windows as macOS: `--flag NAME`, `--experimental`, `OA_FLAGS=name,name`, `OA_EXPERIMENTAL=1`, plus a simple persistent config file (`~/.openagents/desktop-flags.json` on Linux; `%USERPROFILE%\.openagents\desktop-flags.json` on Windows).
   - Default in all builds: all 29 flags `false` (Inkling-only, no model selector, no file viewer, no `@` file mentions, no CLI/auth auto-probing).
3. **Thin OS Adapters in `host-main.c` (Linux & Windows) Matching macOS `Window+Dialogs.swift`:**

| Adapter Concern | macOS (`Window+Dialogs.swift`) | Linux (`host-main.c` + `linux/main.c`) | Windows (`host-main.c` + `windows/main.c`) |
| --- | --- | --- | --- |
| **Shared data & models dir** | `$HOME/.openagents/` | `$HOME/.openagents/` | `%USERPROFILE%\.openagents\` |
| **Hardware probe** | `sysctl` (`arm64` + `hw.memsize >= 32 GB`) | `pc_open()` CUDA VRAM + `/proc/meminfo` (optional Vulkan memory heap query) | `pc_open()` CUDA VRAM (`nvcuda.dll`) + `GlobalMemoryStatusEx` (optional Vulkan query) |
| **Resumable download** | `/usr/bin/curl -fSL -C -` | `curl -fSL -C -` (or Coax HTTP client) | `curl.exe -fSL -C -` (built into Windows 10/11) |
| **SHA-256 verification** | `CryptoKit` (or shared Coax SHA-256) | Shared Coax SHA-256 (`coax/lib/crypto/`) or `sha256sum` streaming | Shared Coax SHA-256 (`coax/lib/crypto/`) or `BCrypt` streaming |
| **Server supervisor** | Spawn `psionic-inkling-serve --ctx 32768 --idle-exit-s 300`, log to `~/.openagents/logs/` | Same POSIX spawn (`setsid`), log to `~/.openagents/logs/` | `CreateProcessA` (`DETACHED_PROCESS \| CREATE_NO_WINDOW`), log to `%USERPROFILE%\.openagents\logs\` |
| **Port conflict check** | `lsof -nP -iTCP:<port> -sTCP:LISTEN` | `ss -tlnp` / `lsof` | `GetExtendedTcpTable` (`iphlpapi.dll`) |
| **Server quit lifetime** | `stopIfIdle(runsOutlive:)` | Same: leave detached server alive if `coder-host` has active runs | Same: leave detached `psionic-inkling-serve.exe` alive if `coder-host.exe` has active runs |

### 5.6 Minimal Build Profiles & Packaging on Linux and Windows

Linux and Windows are already much lighter than macOS because they never link Swift or `breez_sdk_sparkFFI.framework`, **but** their build scripts still compile and bundle several excluded features today.

1. **`apps/openagents-desktop/linux/build.sh --minimal` & `install.sh --minimal`:**
   - Compile `app-v1.coax` instead of `app.coax`.
   - Omit `pay/wallet.coax`, `pay/wallet-backup.coax`, `pay/wallet-rpc.coax`, `claudeworker`, and `agyworker`.
   - Build and bundle `psionic-inkling-serve` + `inkling-kernels-cuda.cu` (and optional `inkling-kernels-vulkan.spv`) alongside `OpenAgents`, `mcworker`, `coder-host`, `gitguard`, `components.spv`, `composition.spv`, fonts, icons, and `.desktop`.
   - Ship as a self-contained tarball (or AppImage) validated by `v1-bundle-check`.
2. **`apps/openagents-desktop/windows/build.sh --minimal`:**
   - Compile `app-v1.coax` instead of `app.coax`.
   - Omit `pay/wallet*.coax`, `claudeworker.exe`, and `agyworker.exe`.
   - Build and bundle `psionic-inkling-serve.exe` + `inkling-kernels-cuda.cu` (and optional `.spv`) + `gitguard.exe` alongside `OpenAgents.exe`, `mcworker.exe`, `coder-host.exe`, `res/` (`components.spv`, `composition.spv`, fonts, icons, `OpenAgents.ico`).
   - Ship as a signed portable `.zip` / installer validated by `v1-bundle-check`.

---

## 6. Benchmarks, Evals & Release Qualification Gate

Per `AGENTS.md` ("Done means the real thing works"), no platform ships in v1.0 until its **actual `--minimal` release build** and **shipped `psionic-inkling-serve --ctx 32768`** pass real end-to-end checks on real Inkling 2 Mini weights.

### 6.1 Eval Harness Alignment

1. **Default model & server:** `evals/openagents-desktop/tier2/run_tasks.py` and `evals/openagents-desktop/run.sh` default to `inkling-2-mini` against the shipped `psionic-inkling-serve --ctx 32768`, auto-starting and logging the server on macOS, Linux, and Windows.
2. **Applicable task filtering:** Mark sub-agent tasks (`c1`, `c2`, `c3`) and USD-budget tasks (`d2`, `d3`) as `N/A` on local Inkling runs; replace dollar-cost assertions with wall time, tok/s, TTFT, and peak memory/VRAM.
3. **Honest naming:** Ensure synthetic checks (`psionic/tools/executor-h7-qual.coax`) never report under a Terminal-Bench name.
4. **Keep `llama.cpp` strictly as a developer oracle** for Psionic Metal/CUDA/Vulkan hillclimbing and the gated Gym Benchmarks tab (`--full` builds).

### 6.2 Release Scorecard (4 Published Numbers per Shipping Platform)

Measured on the final `--minimal` release build with `psionic-inkling-serve --ctx 32768`, reported per hardware tier (macOS: **32 GB / 48 GB / 64+ GB** Apple Silicon; Linux & Windows if shipped: **24 GB / 32+ GB** NVIDIA VRAM, plus 16 GB VRAM if NVFP4/offload ships):

1. **Session-history pass rate:** All **27 tasks** (9 dev + 18 held-out) in `evals/session-history/tasks/`.
2. **Tier 2 applicable pass rate:** All **10 applicable tasks** (`a1–a3`, `b1–b3`, `d1`, `e1–e3`; 5 marked `N/A`).
3. **Throughput:** Median decode tok/s and prefill tok/s (and % of `llama.cpp` oracle on that GPU).
4. **Responsiveness & footprint:** Median TTFT (ms), server boot time (ms), and peak RSS / VRAM (MB).

> **Known failures to resolve before v1.0 sign-off:** An initial live Tier 2 run on `psionic-inkling-serve --ctx 32768` (Metal, on the `inkling-first-release` branch) passed **8/10 applicable tasks** (`a1–a3`, `b1–b3`, `e2–e3` passed at $0.00; `c1–c3`, `d2–d3` marked `N/A`), with two real failures:
> - `d1_goal_todo` timed out at 600s (`goal_status=4` after 28 attempts / 54 commands).
> - `e1_refuse_rm_parent` attempted `../` twice (tripping the guard instead of refusing cleanly on turn 1).
>
> Both must be investigated and fixed (prompting, tool loop, or `/goal` budget/exit behavior) before cutting v1.0.

### 6.3 End-to-End App Smoke Matrix (Run per Shipping Platform)

Before tagging v1.0.0, run on the actual signed minimal build for macOS (and for Linux / Windows if included):
1. **Fresh qualifying machine (no weights):** Launches cleanly → shows Inkling onboarding card → downloads weights → verifies SHA-256 → opens chat → real multi-turn edit + command run succeeds.
2. **Unsupported machine (Intel / < 32 GB Mac; or Linux/Windows without qualifying GPU/VRAM):** Shows clear unsupported-hardware message; does not offer a broken download, crash on missing CUDA DLL/SO, or hang.
3. **App quit during active run:** Start a multi-step Inkling turn, quit the app, verify `coder-host` + `psionic-inkling-serve` finish the run and `psionic-inkling-serve` exits after idle timeout; reopen app and confirm completed reply is persisted.
4. **Legacy saved chat:** Open a chat saved with `gemini-4-neon` or `claude-sonnet-5-5` → history displays cleanly → next message executes on Inkling 2 Mini and says so.
5. **All automated gates pass:** `v1-bundle-check`, Tier 0 + Tier 1 + Compaction (8/8 suites on `--full` and core suites on `--minimal`), `page-slots.sh`, `flags.coax` & `local-model.coax` (0 surviving mutants), `tools-check`, `wire-check`, `layer-check`. On Windows, smoke must run on **real Windows 11 with a real GPU** (Wine validates launch/UI only, not CUDA inference).

---

## 7. Phased Execution Plan

### Track A — Committed macOS v1.0 Release Path

| Phase | Deliverable | Current Status (as of 2026-11-21) |
| --- | --- | --- |
| **A0: Metal Engine Readiness** | 32 GB residency guardrails (#13823) and strict JSON-schema sampler + `/v1/responses` (#13824) in `psionic-inkling-serve`. | **Done on `origin/main`** (`f009098a5a`, `7a85209a15`, `c44941e05f`). |
| **A1: Runtime Flags, Single-Model UX & Eval Wiring** | Single model list, model selector removed in release mode, 28-flag Coax registry, SHA-256 download + onboarding + port diagnostics + quit lifetime, Tier 2/docs updated for Inkling. | **In progress** on `inkling-first-release` branch; rebase onto `origin/main` (reconciling `inkling-serve.c` with #13824), verify, and land. |
| **A2: Stage 1 Bundle & Spark Strip** | Add `--minimal` to `build.sh`, guard Wallet/Spark with `#if OA_WALLET`, drop `breez_sdk_sparkFFI.framework` + static Spark libs + 17 non-v1 `Resources/` items, add `v1-bundle-check`. (**60.5 MB → ~16 MB**; **25 MB → ~5–6 MB** zipped.) | **Not started.** |
| **A3: Stage 2 Swift, Coax & Framework Strip** | Split `CORE_SWIFT` vs. `FULL_SWIFT` (`#if OA_FULL`), skip `lab-sign.o` / `rh-flythrough.o`, add shared `app-v1.coax` slim page entrypoint, drop unused Apple frameworks (`WebKit`, `Speech`, `AVFoundation`, `AuthenticationServices`). (**~16 MB → ~11–12 MB**; **~4 MB** zipped; **−62% compiled LOC**.) | **Not started.** |
| **A4: macOS Qualification, Fixes & Packaging** | Fix Tier 2 `d1_goal_todo` & `e1_refuse_rm_parent`, run all **27 session-history tasks** on `--minimal` + `psionic-inkling-serve --ctx 32768`, test on 32 GB Mac, notarize DMG & update feed, publish 4-number scorecard, close #13825. | **Not started.** |

### Track B — Optional Linux & Windows v1.0 Stretch Track (Parallel, Non-Blocking)

| Phase | Deliverable | Gate to Include in v1.0.0 |
| --- | --- | --- |
| **B1: Portable Engine & Server Boundary** | Extract `inkling-engine.h`; add Win32 `MapViewOfFile` to `inkling-gguf.c`, Winsock2 to `inkling-serve.c`, and Win32 `LoadLibraryA` to `cuda-host.c`. | `psionic-inkling-serve` compiles cleanly on macOS, Linux, and Windows (`MinGW`/MSVC) against stub/metal engines. |
| **B2: CUDA Kernel Bring-Up & Hillclimbing (C0–C3)** | Write `psionic/lib/inkling/kernels-cuda.coax` (`coax cuda` + `mma.coax`) + `inkling-cuda.c`; hillclimb sprints against `llama.cpp` CUDA oracle (no cuBLAS/cuDNN) with CUDA Graphs and 24 GB VRAM residency guardrails (plus optional 16 GB NVFP4/offload). | Exact parity (`cos >= 0.99999`, top-1 match across `K=1, 3, 5`), ≥ 80% of `llama.cpp` CUDA speed (target ≥ 50 tok/s decode, ≥ 500 tok/s prefill at 32K ctx). |
| **B3: Cross-Platform Local Model & Flags (`host-main.c`)** | Move local-model policy into `#authority coax/lib/agent/desktop/local-model.coax`; wire `flags.coax` + hardware probe + `curl`/SHA-256 download + `psionic-inkling-serve` supervisor + port checks into `host-main.c` for Linux and Windows. | Onboarding, download, SHA-256 check, chat, and quit-lifetime flows work identically on Linux and Windows. |
| **B4: Linux & Windows `--minimal` Builds & Qualification** | Add `--minimal` (using `app-v1.coax`, omitting `pay/wallet*.coax`, `claudeworker`, `agyworker`) to `linux/build.sh` and `windows/build.sh`; extend `v1-bundle-check`; run full 27 session-history + 10 Tier 2 tasks & §6.3 smoke matrix on real Linux and real Windows 11 GPUs. | Independent go/no-go per OS at macOS A4 cut time; ship if green, otherwise cut from v1.0.0 and land in v1.0.x. |
| **B5 (Optional): Vulkan Compute Backend** | `psionic/lib/inkling/kernels-vulkan.coax` (`coax spirv --kernel`) + `inkling-vulkan.c` for AMD Radeon / Strix Halo / Intel Arc. | Same C0–C3 parity & speed bar as CUDA; never blocks CUDA or macOS. |

---

## 8. Explicit Non-Goals for v1.0

- **No deleting feature code from the repo:** All gated subsystems stay maintained in `main` and exercised by `--full` builds and CI.
- **No delaying macOS v1.0 for Linux or Windows:** Linux and Windows ship in v1.0 only if they independently clear Track B before the macOS release cut.
- **No CPU-only fallback release:** Inkling 2 Mini (26.4B MoE) is a GPU-first release; machines without qualifying Apple Silicon unified memory or supported discrete/unified GPU acceleration are told clearly rather than given an unusable slow mode.
- **No renaming legacy data paths in v1.0:** `~/Library/Application Support/Coder/` and `CODER_*` environment variables continue working so existing chats aren't orphaned; renaming is a separate migration decision.
- **No cloud fallback or sub-agents:** v1.0 is strictly local single-model Inkling 2 Mini.
- **No `git` diff/checkpoints in v1.0.0:** Deferred to **v1.0.1** as the first incremental capability turned back on after v1.0.0 ships cleanly.
