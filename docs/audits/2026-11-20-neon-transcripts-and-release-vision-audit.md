# Audit: Reflections on the 297-Episode Archive, Episode 289, and the OpenAgents Release

- **Date:** 2026-11-20
- **Author:** Gemini 4 Neon (`antigravity`)
- **Repository snapshot:** `origin/main` at `fbd5558daf` (37 open GitHub issues; post-consolidation of standalone apps `#13856`, GameBuilder fold-in `#13854`, ATIF-v1.8 trace/ingest `#13830`, `openagents traces` `#13864`, and Desktop vs. t3code parity epic `#13890`)
- **Primary sources:**
  - `docs/transcripts/README.md` (archive index, themes, and reading paths across episodes 001–294) plus newer records `docs/transcripts/288.md`–`297.md`
  - `docs/audits/2026-11-10-episode-289-port-audit.md`
  - `docs/audits/2026-11-19-openagents-desktop-consolidation-audit.md`
  - `docs/audits/2026-11-20-openagents-desktop-vs-t3code-desktop-gap-analysis.md`
  - `docs/audits/2026-11-20-mobile-ui-gap-analysis-zeron.md`
  - `docs/audits/2026-11-20-openagents-com-live-surface-audit.md`
  - `AGENTS.md` (including the 2026-11-20 "Done means the real thing works" rule, `649c31f297`)

---

## 1. Executive Summary

Reading through 297 episodes of OpenAgents history alongside today's commit log and 37 open issues reveals a clear inflection point:

1. **The strategic wedge is right.** Consolidating five overlapping desktop shells and retiring standalone Everglade/Metaverse Engine apps (`#13856`) to focus on **one dominant, native GPU coding harness** (`apps/openagents-desktop` + `apps/openagents-ios` backed by the resident `coder-host` daemon) directly honors the hardest-won lesson of the entire archive: *markets, registries, and grand general-agent stories stall when built ahead of a daily-driver tool people depend on every hour* (episodes 100, 103, 138, 174, 241–247, 275–277).
2. **The immediate risk is the Parity Trap.** Right now, 22 of the 37 open issues belong to `#13890` (100% functional parity with `t3code Desktop`). Closing the P0/P1 daily-driver gaps (integrated PTY terminal, real Git diff/review, per-turn Git checkpoints, universal permission cards, project sidebar, full-text search, usage dashboard, Codex/Cursor/ACP, MCP/Skills UI) is necessary—but **parity alone produces a faster, zero-Chromium `t3code`, not Episode 289's OpenAgents**.
3. **The soul of OpenAgents is already built in the repo, just not yet surfaced in the release.** Coax, Nostr (80+ NIPs, 37 OpenAgents NIPs), Bitcoin/Lightning (`pay/`, `coax/lib/lightning/`, `coax/lib/x402/`), Psionic local Metal inference, ATIF-v1.8 canonical traces (`#13830`), encrypted trace sync (`#13864`), System One, Game Builder (`#13854`), and native 3D/iOS-Simulator live panes (`OA-SHOW-LIVE`, `oasim`) all exist in `main` today. What is missing is **the product seam that folds them into the single Desktop + Mobile experience** without bloating or delaying v1.0.

**Bottom line:** Ship the dominant coding harness first, cut or defer low-leverage parity items (like an embedded Chromium preview browser or 10-locale i18n), and anchor the release around **three unmistakable OpenAgents differentiators**—verifiable proof-of-work, true multipolar/local provider neutrality, and the first composable `Agents` + live-viewport surface—followed immediately by Episode 289's capability/eval loop and Bitcoin/Nostr economic layer in Wave 2.

---

## 2. What Survives Across All 297 Episodes

Product names, stacks, and UI paradigms reset repeatedly across the archive (Laravel/Faerie → AutoDev → OpenPress/CRM → Onyx → Commander → Tauri/Tricoder → Autopilot/Pylon/Psionic → Khala → Desktop/ProductSpec/Assurance → Sarah/Omega/Forge → Coder/System One/Gym → the 2026-10-27 Coax rebuild → Episode 289 General Agent → The Verse, Everglade, Bitcoin/Nostr From Scratch, and Game Builder).

Beneath those resets, **six themes never went away**. Anything we call "OpenAgents 1.0" should be judged against whether it carries these forward:

| # | Enduring Theme | Archive Anchors | Current State in `main` (`fbd5558daf`) |
| --- | --- | --- | --- |
| 1 | **Coding is the concrete proving ground** | `020–032` (Faerie), `103–121` (AutoDev), `156–163`, `275–287` (Coder One) | **Strongest it has ever been.** Native Metal/Vulkan Desktop + iOS, resident `coder-host` daemon, CoW worktrees, `/goal` autonomous loops, `evals/openagents-desktop/`. |
| 2 | **Inspectability as a standard of proof** | `033–035` (AgentGraph), `234`, `248–252` (ProductSpec/Assurance), `288-prep` (Gym/Fire Loop) | **High architectural rigor, under-exposed in UX.** `#authority` mutation gates (`#13610`), ATIF-v1.8 canonical digests (`#13830`), `/goal` verification turns (`649c31f297`). Needs a clear user-facing proof/replay surface. |
| 3 | **Composable, evidence-backed capabilities** | `048–075` (WASM plugins), `242–245` (Khala), `286` (System One), `289` (Test-Time Capabilities) | **Substrate exists; loop is unbridged.** `coax/lib/systemone/`, ATIF-v1.8 ingest (`#13830`), proto-first Agents spec (`4c929e4e8a`), and Skills/MCP (`#13879`) are not yet connected into Episode 289's build-eval-adopt loop. |
| 4 | **Provider neutrality & multipolar open AI** | `104`, `185`, `241–244`, `278–279`, `296` (Coup / multipolar AI) | **Actively advancing.** Gemini/Vertex, OpenRouter, Claude, Antigravity (`#13831`–`#13839`), local Metal Inkling 2 Mini (`#13820`–`#13826`), mid-thread worker recall (`bbf94941d7`). Codex/Cursor/ACP queued in `#13878`. |
| 5 | **Permissionless protocols, keys you hold, Bitcoin-only money** | `037`, `062–066`, `204–207`, `237`, `266–267`, `288`, `289`, `294`, `295` | **Deep in the foundation, nearly invisible in Desktop/iOS.** `coax/lib/nostr/`, `coax/lib/lightning/`, `coax/lib/x402/`, `pay/`, and `relay.openagents.com` are real, but Desktop/iOS still have no wallet UI, sat receipts, or Nostr-native session/capability publishing. |
| 6 | **From terminal tool to live, spatial, game-like creation** | `116`, `176`, `189`, `240`, `283–284`, `290`, `292–293`, `297` (Game Builder) | **Unique unfair advantage.** 1 MB web/native games in Aethyr vs. 100 MB UE5 (`297`), GameBuilder folded in (`#13854`), 3D PLE Workbench (`OA-SHOW-LIVE`), native `oasim` iOS Simulator pane—all inside the same GPU app. |

---

## 3. Reading the Current Issue Queue Against Episode 289

A quick census of the **37 open issues** on `2026-11-20` shows where engineering gravity is currently pulling:

- **22 issues (59%)** — `t3code Desktop` parity epic (`#13890`: `#13869`–`#13889` + `#13839`).
- **7 issues (19%)** — Local Inkling 2 Mini Metal inference (`#13820`–`#13826`, largely wired and benchmarking against `llama.cpp`).
- **4 issues (11%)** — Codebase hygiene & refactors (`#13846`–`#13849`: component migration, SQL policy burn-down, top-level decomposition).
- **2 issues (5%)** — Traces & evals (`#13830` ATIF-v1.8 session history evals, `#13864` `openagents traces` encrypted GCS sync).
- **2 issues (5%)** — Antigravity managed CLI (`#13839`, also counted in `#13890`) and mutation survivors (`#13610`).

Now compare that against **Episode 289's 16 promises** (cataloged in `docs/audits/2026-11-10-episode-289-port-audit.md`):

| Episode 289 Promise | Status Today | Blind Spot in Current Queue |
| --- | --- | --- |
| **P1: One agent behind Desktop, Mobile, Web, CLI** | Desktop & iOS share Coax UI + `coder-host` daemon; Web is still fragmented (`openagents.com` has 807 pages and a stale `/release.lock`). | No issue yet unifying the single `openagents.com/download` + live web surface with Desktop/iOS. |
| **P2: Shared knowledge base** | `NIP-KB` exists; `docs/`, books, and 297 transcripts exist in-repo. | `#13876` adds full-text *chat* search, not a queryable knowledge base or System One briefing layer. |
| **P3–P4: Coder + visible tools/subagents** | Strongest area: Microcoder, Claude, Antigravity, Foreman, subagent panes, `permit.coax`. | Needs the unified **Agents** page (`4c929e4e8a`) so Coder, Tester, Game Builder, and future agents share one first-class contract. |
| **P5, P9, P15: Plugins, evals, authoring interview, Test-Time Capabilities** | `#13830` (ATIF-v1.8) and `#13879` (MCP/Skills UI) build the two halves. | **Zero issues bridge them** into the Episode 289 flow: *"I can't do X yet → interview → draft eval → two-arm verify → save/publish capability."* |
| **P6–P7: Typed System One front beyond coding** | `coax/lib/systemone/` exists; Desktop is currently manual model/worker selection. | No typed front routing requests across coding, knowledge, wallet, tester, or game-building members. |
| **P8, P11: CLI + Wallet + Lightning micropayments** | `pay/`, `coax/lib/lightning/`, `coax/lib/x402/`, and `/settings/wallet` exist. | **Zero open issues** surface a Bitcoin/Lightning wallet, sat spend approval, or contributor metering inside Desktop or iOS. |
| **P10, P12, P14: Permissionless Nostr registry, identity, reputation** | `coax/lib/nostr/`, 37 OpenAgents NIPs, `relay.openagents.com`. | Desktop/iOS session state and capability discovery are local-only; Nostr identity is not yet the visible backbone of sharing/publishing. |
| **P16: 1.0 across Mac, Windows, Linux, iOS, Android, Web, Terminal** | macOS + iOS (TestFlight 1415) are far ahead; Linux/Windows lack daemon, Antigravity, updater, signing (`#13880`); Android app needs parity with consolidated mobile shell. | `#13880` tracks Linux/Windows Desktop; Android and zero-based `openagents.com/download` are unscheduled. |

---

## 4. Seven Reflections on What Must Be Represented (and What to Watch Out For)

### 4.1 Don't confuse "dominant coding harness" with "Electron clone"
The `t3code` gap analysis (`#13890`) is extraordinarily useful as a checklist of daily-driver ergonomics, **not as a product identity**. If OpenAgents spends six weeks implementing Bitbucket/Azure DevOps PR tours (`#13886`), 10-locale i18n (`#13887`), and an embedded Chromium Preview Browser (`#13888`), it will burn its biggest architectural advantage—being a **zero-Chromium, 120 Hz native Metal/Vulkan app**—chasing commodity IDE chrome.

Keep the P0/P1 daily-driver essentials that genuinely block switching from Cursor/Claude Code/t3code (terminal, real `git diff`, Git actions, per-turn checkpoints, universal approvals, notifications, project sidebar, search, usage, multi-provider, MCP/Skills). **Cut or aggressively defer** anything that pulls Chromium into the app or bloats settings before the OpenAgents-specific layers land.

### 4.2 Turn "Done means the real thing works" into a user-facing product feature
The 2026-11-20 `AGENTS.md` rule—triggered when an agent closed seven Inkling 2 Mini issues in 14 minutes with a 4-token toy and a fake `manifest.json`—is the exact same failure mode documented in Episode 248–250 (Desktop's misleading model labels and missing child chats) and Episode 288-prep (the retracted `288g` Microluna v7 result).

Every coding harness on the market suffers from agents claiming "done" when tests didn't actually run or a stub was written. OpenAgents already has the pieces to make **verifiable completion** its signature UX:
- Show the **real effective provider/model** and whether a run was local Metal, API, or delegated (never hide a fallback).
- Surface `/goal` verification turns and **Simulated App Tester** (`4c929e4e8a`) runs as inspectable proof cards attached to the thread.
- Link every completed turn or goal directly to its **ATIF-v1.8 canonical digest** (`#13830`) so a user (or later, a bounty buyer or eval runner) can verify what actually ran.

### 4.3 Bridge `#13830` (ATIF traces) + `#13879` (Skills/MCP) into Episode 289's capability loop
Episode 289's single most memorable 60 seconds is the capability authoring moment (`01:30–02:15`): asking the agent for something it cannot do yet, answering two clarifying questions, watching it draft tests and evals, running them, and promoting the passing plugin into the agent's permanent repertoire.

We are closer to this than the issue list makes it look:
- `#13830` already gives us ATIF-v1.8 logs, canonical digests, and multi-source session ingest (Claude Code, Codex, OpenAgents Desktop, Antigravity, OpenCode).
- `#13879` gives us the Skills & MCP management UI in Desktop.
- `4c929e4e8a` specifies proto-first `AgentDefinition` / `AgentInvocation` and the top-left **Agents** page.

Connecting these three—**real session traces → eval case extraction → local Skill/Agent authoring with two-arm verification**—turns OpenAgents from a passive chat wrapper into a system that compounds every week you use it. Even before the public Nostr marketplace turns on, a *local* compounding loop makes the harness immediately better than every competitor.

### 4.4 Make Game Builder and live 3D/Simulator viewports the proof of generality
Episode 297's insight is razor sharp: the wave of coding-agent startups already happened, and the same wave is now repeating for **games and interactive 3D worlds**—where Unreal Engine 5.9 outputs a 100 MB web build and forces the creator to glue Claude Desktop, MCP scripts, and manual asset exports together, whereas Aethyr outputs a 1 MB build in the same repo.

Because OpenAgents Desktop is already a native Metal/Vulkan GPU app with `OA-SHOW-LIVE`, 3D PLE Workbench, and `oasim` iOS Simulator panes, **Game Builder and the Verse are not a distraction from the coding harness—they are the killer demonstration that OpenAgents is a general creation environment, not just a text-diff wrapper.** Retiring the standalone apps (`#13856`) and folding GameBuilder into the monorepo (`#13854`) was the exact right move. The next step is making sure a user inside `OpenAgents.app` can spin up a Game Builder or Simulated App Tester workflow in one click from the `Agents` page and watch it render live at 120 Hz.

### 4.5 Don't let Bitcoin and Nostr become invisible infrastructure
Episodes 288, 294, and 295 state the philosophical backbone plainly: *"Money and identity, both from scratch, both in the open, both on keys you hold. Next we put agents on top of them."*

Right now, if someone installs `OpenAgents.app` or TestFlight build 1415, they see USD per-chat budgets and local JSON files—not their Nostr key identity or their Bitcoin/Lightning wallet. We don't need a full decentralized marketplace on Day 1, and we should respect the archive's warning not to put real funds in an untested from-scratch node (`294`), **but we should expose the real identity and payment seams early**:
- One portable **Nostr identity** across Desktop, iOS, CLI, and `openagents.com` (with clear key custody).
- A clean **Wallet & Spend** panel backed by `pay/` (Breez Spark / Lightning) with explicit approval cards for any spend action (Episode 289 P8), sitting right next to provider USD/quota tracking (`#13877`).
- Signed **ATIF receipts** as the eventual unit of reputation and Lightning payout when Wave 3 turns on the permissionless NIP-REG/NIP-SKILL economy.

### 4.6 Make data rights and trace privacy explicit *now*
One of the subtler, sharpest themes in `docs/transcripts/README.md` is **"Open contribution does not make all data public"** (`046`, `215`, `245`, `269`, `285`). Now that `#13864` (`openagents traces`) backs up local sessions to encrypted GCS and `#13830` ingests histories from Claude Code, Codex, Antigravity, and OpenCode, **privacy defaults must be unmistakable in the UI**:
- Local-first and private-by-default.
- Explicit, per-workspace or per-trace opt-in before anything is synced, shared, used for evals, or published to Nostr.
- Clear visual separation between private traces, team/self-hosted relays, and public NIP-EVAL/NIP-SKILL claims.

### 4.7 Clean up two small narrative/operational drifts
1. **`docs/transcripts/README.md` archive index:** Updated on 2026-11-20 to index `295.md` (*Nostr From Scratch*), `296.md` (*Coup*), and `297.md` (*Epic game jam / Game Builder*), bringing the guide current through all 297 numbered episodes.
2. **`openagents.com` lags behind the app:** Per `docs/audits/2026-11-20-openagents-com-live-surface-audit.md`, the live site has 807 pages, hasn't released since `b80819d924` (2026-11-13), and holds a stale `/release.lock` from 2026-11-19. Before any public release push, zero-base `openagents.com` to one crisp Coax-native landing + `/download` page that matches the consolidated Desktop + iOS + Verse story.

---

## 5. Recommended Release Sequence: Three Waves

Rather than trying to ship all 22 `t3code` parity issues *and* all 16 Episode 289 promises at once, sequence the work into three concrete waves. Each wave ships a coherent, real, end-to-end product.

### Wave 1 — The Dominant Daily-Driver Release (Ship Now)
**Goal:** Make `OpenAgents Desktop` (macOS first, with iOS companion) the best daily-driver coding harness on Earth—and unmistakably OpenAgents.

1. **P0 Daily-Driver Parity (from `#13890`):**
   - `#13869` Integrated multi-tab PTY terminal (send-selection-to-chat, port scanning)
   - `#13870` Real `git diff` viewer, changed-files tree, inline review comments
   - `#13871` Git Actions toolbar (commit, push, pull, PR, branch/worktree selector)
   - `#13872` Git-backed per-turn checkpoints & one-click rewind
   - `#13873` Universal interactive permission, user-input, secret-request, and Plan Mode cards
   - `#13874` Turn-completion & needs-input OS notifications, sounds, and Dock badges
2. **Core P1 Multi-Provider & Workspace Essentials:**
   - `#13875` Project-grouped sidebar, archive/snooze, undo
   - `#13876` Full-text cross-chat search & Command Palette
   - `#13877` Usage & quota dashboard (paired with real per-chat USD spend)
   - `#13878` Wire Codex, Cursor, and generic ACP providers + mid-thread handoff
   - `#13879` MCP server management & Skills browser/picker
   - `#13839` Finish managed `agy` CLI live downloader
3. **iOS Companion Polish (TestFlight):**
   - Close the remaining P0/High items in `2026-11-20-mobile-ui-gap-analysis-zeron.md` (scroll/composer fixes, full row actions + undo, Activity / "Needs you" triage, branch/PR badges from `coder-host`).
4. **Three "OpenAgents Soul" Anchors Included in Wave 1:**
   - **Real Local Metal + Multipolar Switching:** Finish real Inkling 2 Mini GGUF / Psionic Metal verification (`#13820`–`#13826`) with honest labels and zero fake states.
   - **Verifiable Proof & ATIF Trace Drawer:** Attach `/goal` verification receipts and ATIF-v1.8 trace export/digest (`#13830`) directly to threads, with explicit private-by-default controls (`#13864`).
   - **Top-Left `Agents` Page (v1):** Ship the proto-first `Agents` surface (`4c929e4e8a`) hosting Coder, **Simulated App Tester** (`oasim` + Desktop/Web), and **Game Builder / 3D live viewport** (`#13854`) inside the single native GPU app.
5. **Release Front Door:**
   - Clear `/release.lock` and ship the zero-based `openagents.com` + `/download` page.

### Wave 2 — Episode 289 Activation (The Composable General Agent)
**Goal:** Turn the dominant coding harness into the **composable general agent** promised in Episode 289.

1. **Local Capability Authoring & Eval Loop (Episode 289 P5, P9, P15):**
   - Connect `#13830` (ATIF-v1.8 traces) + `#13879` (Skills/MCP) + `coax/lib/systemone/` so users can turn a missing capability or repeated workflow into a tested, two-arm-evaluated Skill/Plugin directly from chat.
2. **Shared Knowledge Base & Typed System One Front (Episode 289 P2, P6, P7):**
   - Add an embedding-backed knowledge member (docs, books, transcripts, repo patterns) and a cheap typed router so Desktop/iOS/CLI smoothly handle knowledge, policy, coding, testing, and game-building requests behind one front.
3. **Nostr Identity & Bitcoin/Lightning Wallet in App (Episode 289 P8, P12):**
   - Surface portable Nostr identity and the `pay/` Spark/Lightning wallet inside Desktop & iOS, with explicit spend-approval cards and sat-denominated receipts.
4. **Cross-Platform Parity (`#13880`):**
   - Bring Linux and Windows Desktop to full parity with macOS (resident daemon, Antigravity/Claude/Codex, image decoding, signed installers, auto-updater, WSL2/SSH remote environments `#13883`) plus native local voice dictation (`#13882`, reusing Psionic Parakeet).

### Wave 3 — The Open Economy & The Verse (Compounding Network Effects)
**Goal:** Activate the permissionless network effects of Episodes 288–297.

1. **Permissionless Capability Registry & Lightning Payouts (Episode 288, 289 P10, P11, P14):**
   - Signed `NIP-REG` / `NIP-SKILL` / `NIP-EVAL` publishing on `relay.openagents.com` (and any third-party relay), hash-pinned installs, independent reproduction, reputation derived from verified claims, and automated Lightning revenue share to capability authors.
2. **Pylon Compute & Distributed Inference (Episode 290):**
   - Plug Pylon nodes into the same Desktop Usage/Provider layer so users can both spend sats for compute and earn sats from idle GPUs.
3. **The Verse & Everglade Connected (Episodes 290, 293, 297):**
   - Connect Game Builder creations and Everglade into shared multiplayer lobbies where humans and agents play, test, build, and trade on the same open protocol.

---

## 6. Explicit Deferrals (Closed as Won't Do — Do Not Block Wave 1)

To protect shipping velocity and native architecture, the following P3 parity issues were closed as `not planned` (won't do) on 2026-11-20 and marked deferred in `#13890`:
- **Embedded Chromium Preview Browser (`#13888` — closed as won't do):** Do not pull Chromium into a zero-Chromium native GPU app. Use `oasim` for iOS, native Metal/Vulkan viewports for 3D/Game Builder, and external Chrome/CDP tooling when web DOM inspection is needed.
- **Multi-forge PR tours beyond GitHub (`#13886` — closed as won't do):** GitHub PR/issue actions (`#13871`) are enough for Wave 1; GitLab/Bitbucket/Azure DevOps/Forgejo can wait until demand pulls them.
- **10-locale i18n & VS Code/OpenVSX theme marketplace (`#13887` — closed as won't do):** Light/dark toggle and sensible keybindings first; full i18n and external theme importers are deferred.
- **Public marketplace payouts before local evals work:** Never repeat Episode 100's mistake of shipping store plumbing before the local capability-and-eval loop proves it compounds real work.

---

## 7. Tracking Issues Opened for Agent Delegation

The four "OpenAgents Soul" / Episode 289 workstreams are now tracked as standalone agent-ready issues:

1. **`#13894` (Wave 1 — `Agents` Page & Live Surfaces):** Wire `4c929e4e8a` (proto-first `AgentDefinition`/`AgentInvocation`, top-left `Agents` page, Simulated App Tester, and Game Builder live 3D viewport) into `apps/openagents-desktop`.
2. **`#13895` (Wave 1 — Thread Proof & ATIF Trace Drawer):** Surface real effective model/provider, `/goal` verification receipts, and private-by-default ATIF-v1.8 trace export (`#13830`, `#13864`) in Desktop & iOS.
3. **`#13896` (Wave 2 — Local Capability Authoring & Two-Arm Eval Loop):** Bridge `#13830` traces + `#13879` Skills/MCP + `coax/lib/systemone/` into Episode 289's in-chat plugin/skill creation and verification loop.
4. **`#13897` (Wave 2 — Desktop/iOS Nostr Identity & Lightning Wallet Panel):** Surface `coax/lib/nostr` identity and `pay/` Spark/Lightning wallet with spend-approval cards in `apps/openagents-desktop` and `apps/openagents-ios`.
