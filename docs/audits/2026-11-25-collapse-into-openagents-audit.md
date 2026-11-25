# Audit: Collapsing "Talking to an Agent Harness" into Talking to OpenAgents

**Date:** 2026-11-25  
**Audited at:** `origin/main` (`32ee5b59d1`)  
**Context:** Re-reading [Episode 289: OpenAgents](../transcripts/289.md) against the 15 days of rapid shipping since the [2026-11-10 Episode 289 Port Audit](2026-11-10-episode-289-port-audit.md), evaluating the strategic choice between **smartly routing** vs. **hillclimbing one core**.

---

## 1. Executive Summary & Recommendation

**Bottom line:** Your instinct is spot on. Today's codebase has an architectural irony: **nearly every building block Episode 289 promised now exists in checked Coax, yet the product UX drifted in the opposite direction—forcing the user to pick and talk to an agent harness** (`Flawed`, `Microcoder`, `Inky`, `Claude Code`, `Antigravity`, `Codex`, `Cursor`, `ACP`).

On the core question—**smartly route vs. hillclimb the core**—the codebase evidence points cleanly to a **two-layer synthesis**, not a binary choice:

1. **Collapse into ONE core and hillclimb it relentlessly (Priority #1).**
   - `flawed/` (all 24 chapters complete, `#authority;` zero-survivor mutation boundaries) + `coder/tui/flawedworker.coax` just proved it can **double Antigravity's coding pass rate on the exact same Gemini 4 Neon model** (`8/16 = 50.0%` vs `4/16 = 25.0%`), run **41.2% faster**, use **35–46% fewer prompt tokens**, and achieve **0/41 edit failures and 0 wire retries** ([benchmark writeup](../verification/2026-11-25-flawed-neon-vs-antigravity-neon-bench.md)).
   - That engine **is** the OpenAgents core. Rename the user-facing identity from "Flawed / Microcoder / Inky" to **OpenAgents**, and concentrate engineering effort on making this one core loop extraordinary on both local Inkling 2 Mini and cloud frontier models.
2. **Put a razor-thin System One Typed Front in front of that ONE core—not to scatter users across 5 chat harnesses, but to compose capabilities and compute cleanly.**
   - We *already have* a battle-tested System One router (`coax/lib/agent/router/` + `coax/lib/systemone/gemini.coax`) that scores **66/66 (100%) on dev and 30/31 (96.8%) on held-out** at ~$0.0001/decision—it just isn't wired into Desktop's main chat path or general-agent intents yet.
   - Instead of routing the user to 5 incompatible harnesses with separate transcripts, the Front does **three surgical jobs** before each OpenAgents Core turn:
     1. **Progressive capability/member mounting:** Decide *which* tool pack and context to load into the **same** OpenAgents conversation (`general/KB`, `coding + intel`, `wallet` with mandatory spending approval, `cli`, `metaverse/gamebuilder`, `gym/eval`).
     2. **Compute & model selection (local vs. cloud vs. remote station):** Send fast/private/light turns to local Inkling 2 Mini ($0, instant) and deep/multi-file/long-context turns to cloud frontier models (e.g., Gemini 4 Neon) or remote stations—while honoring explicit overrides (`"use Claude"`, `"do it with Codex"`, `"keep it local"`).
     3. **Missing-capability handoff:** When OpenAgents cannot yet do a task (`"turn my meeting notes into Linear tickets"`), invoke the **already-built** authoring interview & two-arm ATIF eval (`coax/lib/agent/desktop/capability-eval.coax`) right inside chat.
3. **Demote external harnesses (`claudeworker`, `agyworker`, `codex`, `cursor`) from primary identities to delegatable sub-engines.**
   - You never "switch to a harness." You talk to **OpenAgents**. When you explicitly say `"do it with Codex"` or `"have Claude Opus review this"`—or when the Front detects an external engine has unique auth/quota/capability—OpenAgents delegates as a subagent and returns the result into your single OpenAgents thread.

---

## 2. The Paradox: Episode 289 vs. What Exists Today (2026-11-25)

Episode 289 (`2026-10-02`) gave a crystal-clear definition of OpenAgents:

> *"288 videos and almost three years later, we can finally define what OpenAgents is. It's a **composable general agent**. ... Our whole CLI, just like everything, everything, everything behind this single agent, accessible through anything."*

In the 15 days since the `2026-11-10` port audit, the team shipped an extraordinary amount of real Coax infrastructure—`flawed/`, `coax/lib/agent/router/`, `capability-eval.coax`, `agents-catalog.coax`, `metaverse-route.coax`, `gamebuilder`, `pay/`, `psionic-inkling-serve`, and native iOS/macOS Tailscale apps.

**Yet the user-facing architecture fractured into "harness picking":**

| Surface / Subsystem | What the user sees today | Why it breaks Episode 289 |
|---|---|---|
| **Desktop Model Picker** (`apps/openagents-desktop/page/models.coax`) | 11 distinct `CP_MK_*` harness/model kinds (`FLAWED`, `INKLING`, `GEMINI`, `OPENROUTER`, `AGY`, `HYBRID`, `INKLING_NEON`, `CODEX`, `CURSOR`, `ACP`, `CLAUDE`) | User thinks in harnesses & providers, not OpenAgents. |
| **Desktop Worker Zoo** (`coder/tui/`) | 5 separate chat worker binaries (`flawedworker`, `mcworker`, `inky`, `claudeworker`, `agyworker`) | Divergent prompts, tool schemas, compaction, permissions, and failure modes. |
| **Desktop Settings & Flags** (`FlawedSettings.swift`, `coax/lib/agent/desktop/flags.coax`) | 20+ Claude-Code-parity "Flawed" settings + 30 feature flags (`DC_FLAG_*`) gating capabilities like `wallet`, `capabilities`, `agents`, `metaverse`, `gamebuilder`, `gym` | Capabilities are isolated UI panes/flags instead of composable members behind one chat. |
| **System Identity** (`coder/tui/flawedworker.coax`) | Default worker introduces itself as *"Flawed, an autonomous software engineering coding agent running natively inside OpenAgents Desktop"* | Literally tells the user they are talking to a coding harness named Flawed. |
| **iOS & macOS Companion Apps** (`apps/openagents-ios/`, `apps/openagents-macos/`) | Built as dedicated **Antigravity-only** Tailscale clients (`#13993`, `#authority` Antigravity-only policy) | Hard-wires mobile/companion surfaces to a single third-party harness instead of OpenAgents. |
| **CLI Chat** (`apps/openagents-cli/chat.coax`) | Talks to hosted `/api/chat/*/v2alpha1`; 30+ other CLI commands (`wallet`, `nostr`, `pylon`, `realm`, `project`, `dictate`, `labs`, `skill`, `vault`) sit beside it unbridged | The CLI is *beside* the agent, not *behind* it (Episode 289 P8). |
| **The OpenAgents Book** (`openagents/book/chapters.json`) | 78 chapters total: **13 `partly implemented`** (UI Ch. 1–9 + Ch. 54–55, 69–70), **62 `design draft`**, **3 `planned`**. Zero of Ch. 12–53 (the general agent front, members, KB, plugins, test-time capabilities) are implemented under `openagents/`. | The spec for Episode 289 stayed as drafts in `openagents/book/` while real code landed in `flawed/`, `coax/lib/agent/`, and `apps/openagents-desktop/`. |

---

## 3. Episode 289 Promise-by-Promise Delta (`2026-11-10` → `2026-11-25`)

Most of Episode 289 is **much closer than it looks**—the primitives exist, they are just disconnected or hidden behind flags/harnesses.

| # | Episode 289 Promise | Status on Nov 10 | Status Today (Nov 25) & Relevant Code | Gap to Collapse |
|---|---|---|---|---|
| **P1** | One agent behind web, terminal, desktop, mobile | Partial shells; no shared runtime | **Fragmented.** Desktop runs `flawedworker`/`inky`/`mcworker`; iOS/macOS (`#13993`) run Antigravity-only; CLI `chat.coax` hits web API. | Make one `OpenAgents` runtime (on `flawed/core.coax` + `coder-host`) the single execution owner across all surfaces. |
| **P2** | Shared knowledge base | Missing (`NIP-KB` only) | **Still missing as a runtime.** `flawed/memory.coax` handles per-workspace memory; `coax/lib/agent/intel/` handles compiler code queries (`ix_defines`, `ix_uses_of`); docs/books/transcripts exist on disk, but **no semantic KB retrieval runtime** sits behind chat. | Build one lightweight KB index/retrieval member over docs, books, transcripts, and workspace notes (Ch. 21–22). |
| **P3** | Loads Coder for repos; delegates to other engines (`"do it with Codex"`) | Coder book + drivers | **Very strong.** `flawed/` (24/24 chapters) + `flawedworker.coax` (2× Antigravity Neon pass rate) + `coax/lib/agent/{claude,codex,cursor}` + `engine/antigravity.coax` + `flawed/subagent.coax` & `swarm.coax`. | Make `flawed` the default Coder member inside OpenAgents and demote Claude/Codex/Cursor/Agy to subagent delegates. |
| **P4** | Visible tools and subagents | Partial | **Strong.** Desktop renders tool rows, thoughts, and live subagent sidebars; `coax/lib/agent/desktop/agents-catalog.coax` defines proto-first `AgentDefinition` (`coder`, `simulated-app-tester`, `game-builder`) with `#authority` target-lease guards (`coax/lib/agent/registry/policy.coax`). | Unify tool/member visibility so every mounted capability (Coder, Wallet, GameBuilder, CLI, Skills) appears cleanly in one member drawer. |
| **P5** | Plugins & eval system (adapted from Claude Code) | Partial | **Strong local foundation.** `flawed/extensions.coax` (skills/plugins), `flawed/hooks.coax`, `flawed/mcp.coax`, `coax/lib/agent/desktop/capability-eval.coax` + `capability-gate.coax` (draft `SKILL.md` + real two-arm `/bin/sh` runs + ATIF-v1.8 logs + `#authority` promotion gate), `evals/{openagents-desktop,session-history,terminal-bench}/`. | Promote out from behind `DC_FLAG_CAPABILITIES` and connect directly to the OpenAgents Core loop. |
| **P6** | Routes beyond coding (general, policy, small talk, Gym, Verse) | Missing | **Partial pieces, unrouted.** `coax/lib/agent/desktop/metaverse-route.coax` (`#authority` 3D/GameBuilder routing), `gamebuilder` CLI/MCP bundled in workers (`#14039`), `gym.coax` / `gbench-run.coax`, `Window+WalletRPC.swift`. **No general-intent front routes between them.** | Extend `coax/lib/agent/router/` beyond coding engines to route domain/member packs. |
| **P7** | Typed System One front (`Jev` / Decisions API) | Partial (`systemone` only) | **Built & proven for coding delegation, not wired to general chat.** `coax/lib/agent/router/` (`decide.coax`, `ask.coax`, `policy.coax`, `station.coax`, `eval.coax`) scores **66/66 dev, 30/31 held-out** on Gemini Decision 1 (~$0.0001/call), with local `elephant-1` fallback (`coax/lib/agent/engine/route.coax`). Only exposed as a preview in `coax/web/api/router.c`. | Generalize `rtr_ask_body` to classify **member pack + model tier/station + continuation + missing-capability**, and run it before every OpenAgents turn. |
| **P8** | Whole CLI behind the agent (`"send 1,000 sats"`, `"show deck"`) | CLI existed; no wallet | **Wallet & CLI both exist, still disconnected.** `#authority` `pay/` (`wallet.coax`, `wallet-policy.coax`, `lnaddr-policy.coax`, `makerpay-policy.coax`), `SparkWallet.swift`, `Window+WalletRPC.swift`, `apps/openagents-cli/wallet.coax` + 30 CLI subcommands. | Expose safe CLI subcommands & Wallet RPC as typed OpenAgents members, with mandatory UI approval cards for money/destructive actions. |
| **P9** | Build missing plugin from chat (interview → tests → eval → run) | Missing | **Already prototyped offline!** `coax/lib/agent/desktop/capability-eval.coax` (`draft` + `run`) and `CapabilityStudio.swift` implement the exact Episode 289 flow (interview → `SKILL.md` + pos/neg cases → two-arm ATIF run → `#authority` gate), gated behind `DC_FLAG_CAPABILITIES`. | Trigger this flow **conversationally** from OpenAgents chat when a requested capability is missing, rather than requiring a hidden studio button. |
| **P10** | Registry; everyone gets passing plugins | Missing (`NIP-REG` only) | **Early.** `openagents/examples/signed-module-registry.coax` + `pinned-defaults-rollout.coax` exist (Ch. 54–55 partly implemented); no live plugin registry relay wired to Desktop yet. | Phase 4: publish promoted `capability-eval` artifacts + ATIF digests as signed `NIP-REG` events. |
| **P11** | Authors paid Lightning micropayments | Missing | **Primitives ready, economy unwired.** `pay/`, `coax/lib/lightning/`, `coax/lib/x402/`, `pay/makerpay-policy.coax`, `apps/openagents-cli/pylon-settle.coax` all exist and pass mutation gates. | Phase 4: attach per-call metering receipts + scheduled Lightning payout splits to registry calls. |
| **P12–P14** | Open protocols (Nostr), OpenAgents API, reputation/identity | Strong protocol; partial API | **Strong base.** `coax/lib/nostr/`, 37+ OpenAgents NIPs, `proto/` wire registry, `coax/web/api/`, NIP-44 Tailscale transport in `apps/shared/antigravity/`. | Unify API & Tailscale transport around **OpenAgents Core** instead of Antigravity-only RPCs. |
| **P15** | Test-time capabilities (candidate → evidence → reproduction → adoption) | Essay only | **Local half built.** `capability-eval.coax` + `capability-gate.coax` + `ATIF-v1.8` logs + `evals/session-history/` friction hillclimb. Missing multi-key reproduction & network adoption policy. | Wire local promotion now; add multi-key reproduction & `NIP-EVAL` claims in Phase 4. |
| **P16** | 1.0 across Mac, Windows, Linux, iOS, Android, web, terminal | No Windows host | **Major progress.** Desktop has macOS + Linux/Windows Vulkan & key-repeat parity (`0bfb0a18dd`, `52a39db9ff`), web Coax Native works, iOS native SwiftUI exists. | Converge all platforms onto the single OpenAgents Core contract. |

---

## 4. Deep Assessment: Smart Routing vs. Hillclimbing the Core

You framed the exact strategic fork:
> *"Basically, just gonna smartly route to whatever needs to be done, or just hill climb the core and have the core be really good."*

Here is how the three architectural options actually compare given what is in this repo today:

### Option A: Pure "Smart Router" across existing harnesses (`mcworker`, `claudeworker`, `agyworker`, `codex`, `cursor`)
- **How it works:** System One reads every message and dispatches the turn to whichever full external/internal harness seems best.
- **Why it fails on its own:**
  - **Broken continuity:** Switching harnesses mid-thread (`Inkling` → `Claude Code` → `Antigravity` → `Codex`) breaks prompt caching, leaks divergent transcript formats, and forces lossy context summaries across turns.
  - **5× maintenance tax:** You keep maintaining 5 workers, 11 picker kinds, and external CLI quirks (`agy` startup contention, `<end_of_image>` glitches, Claude CLI auth drift).
  - **Zero compounding moat:** If you route primary coding work to third-party harnesses, you cannot hillclimb the loop. Today's benchmark already proved that **our own pure-Coax harness (`flawed`) beats Antigravity 2× on the exact same Gemini 4 Neon model** (`50%` vs `25%`, `-41%` latency, `-35%` to `-46%` prompt tokens). Routing away from our own core would *lower* quality.

### Option B: Pure "Hillclimb One Core" with NO front router
- **How it works:** Everything goes straight into one monolithic `flawedworker` loop with one big system prompt and all tools permanently attached.
- **Why it hits a ceiling:**
  - **Prompt & token bloat:** Loading coding tools, git snapshots, GameBuilder MCP, Wallet RPC, CLI schemas, KB search, and Gym tools into *every* turn destroys local Inkling 2 Mini's 32K context budget and slows trivial questions (`"what's my sats balance?"`, `"what did we say in ep 289?"`).
  - **Safety & approval boundaries:** Coding wants `Allow All` (`flawed` default since `8ec09656ce`); spending sats (`pay/`) or publishing to relays requires strict, deterministic `#authority` approval gates *before* execution.
  - **Local vs. Cloud economics:** Always hitting cloud Neon costs ~$0.58–$0.71/task even for simple lookups; always hitting local Inkling 2 Mini fails hard multi-file tasks where Neon succeeds.
  - **No clean trigger for Episode 289's killer feature (P9):** Without a lightweight decision step, a general coding model tends to hack around missing capabilities with brittle bash one-liners instead of cleanly recognizing *"I don't have a Linear capability yet—want me to build and eval it?"*

### Option C (Recommended): One Hillclimbed Core + Razor-Thin Typed Front (Progressive Capability Composition)
- **Mental model:** There is **one agent: OpenAgents** (powered by `flawed/core.coax` → `openagents-core`). There is **one conversation, one transcript DAG (`flawed/transcript_dag.coax`), one compaction pipeline (`flawed/compaction.coax`), one memory system (`flawed/memory.coax`)**.
- **What the Thin Front (`coax/lib/agent/router/` extended) actually decides (~150ms, ~$0.0001 on Gemini Decision 1, or free local Elephant-1):**
  1. **Capability Pack(s) to mount for this turn** (keeps base prompt tiny and cache-hot):
     - `general` (conversation + lightweight KB retrieval)
     - `coder` (workspace tools + session git snapshot + `coax/lib/agent/intel/` compiler queries)
     - `wallet` (`pay/` + `Window+WalletRPC` with mandatory `#authority` spending approval)
     - `metaverse` (`metaverse-route.coax` + `gamebuilder` CLI/MCP + 3D viewport)
     - `cli` (typed `openagents` CLI commands: `nostr`, `pylon`, `project`, `dictate`, `vault`, etc.)
     - `capability_author` (triggers `capability-eval.coax` interview + two-arm ATIF eval)
  2. **Compute & Model Tier** (transparent to the user, overridable via simple preference `Auto | Local | Cloud` or explicit wording):
     - `local` (Psionic Metal Inkling 2 Mini — instant, $0, private)
     - `cloud_fast` / `cloud_deep` (Gemini 4 Hydrogen/Carbon/Neon or user's configured provider)
     - `remote_station` (`coderos` / GPU host via existing `StationReport` eligibility rules in `coax/lib/agent/router/policy.coax`)
  3. **Explicit External Delegate Override (rare escape hatch):**
     - Only when the user explicitly asks (`"do it with Codex"`, `"have Claude Opus review this"`) or a task strictly requires an external CLI, OpenAgents Core invokes `claudeworker`/`codex`/`agyworker` **as a subagent tool call**, never replacing the OpenAgents conversation itself.

**Why Option C wins:**
- **80% of your engineering effort stays focused on hillclimbing the ONE Core** (prompt/tool ergonomics, compaction, Inkling 2 Mini + Neon pass rates, latency, zero edit/wire failures)—every improvement lifts *every* capability simultaneously.
- **20% goes to the thin typed Front & Member mounting**—delivering the real Episode 289 "composable general agent" without duplicating loops or fragmenting UX.
- **Zero "harness" jargon in the UI.** The user opens OpenAgents and talks to OpenAgents.

---

## 5. Concrete Collapse Roadmap (4 Phases)

### Phase 1: Collapse the Brand & Kill the Harness Zoo (Fastest Win, Lowest Risk)
1. **Unify identity in the core worker:** Rename `flawedworker` user-facing strings & system prompt from `"Flawed, an autonomous software engineering coding agent..."` to **`OpenAgents`** (`coder/tui/flawedworker.coax`, `apps/openagents-desktop/Window+Runs.swift`, `apps/openagents-desktop/page/`). Keep `flawed/` as the internal library/module name initially if desired (avoiding gratuitous repo churn), or alias it cleanly to `openagents-core`.
2. **Collapse the 11-kind model/harness picker (`apps/openagents-desktop/page/models.coax`):**
   - Default UX: **No harness picker at all.** One clean `Auto` mode (or a simple 3-way pill: `Auto` · `Local (Inkling)` · `Cloud (Neon)` + Effort), with `show_other_models` / external harnesses tucked strictly under Developer Settings or invoked via delegation (`"use Claude"`).
   - Fold `FlawedSettings.swift`'s 20+ harness knobs down to plain OpenAgents settings (Permissions/Safety, Compute/Local Model, Memory, Developer).
3. **Retire redundant primary workers:**
   - Stop adding features to `mcworker` and `inky` as separate desktop chat loops; converge local Inkling 2 Mini + cloud Neon + OpenRouter onto the single `flawed/core.coax`-backed worker (which already supports all three and already outperforms both).
   - Keep `claudeworker` and `agyworker` solely as optional delegated subagent executors (and align `apps/openagents-ios` / `apps/openagents-macos` to talk to the OpenAgents Core daemon over Tailscale instead of being permanently Antigravity-only).

### Phase 2: Keep Hillclimbing the One Core (Compounding Quality)
1. **Continue the proven Flawed v4 → v5 hillclimb** on `evals/session-history/` (`dev10` + `heldout6`) and `evals/openagents-desktop/` + `gBench`:
   - Current state: `6/10` dev10, `2/6` heldout6 on Neon (`50%` combined, `2×` Antigravity); Inkling 2 Mini local also improving rapidly (`+42.9pp` in `882ad943e2`).
   - High-leverage next core upgrades:
     - Mount `coax/lib/agent/intel/` (`ix_defines`, `ix_uses_of`, `ix_check`, `ix_replace`) directly as first-class Coax-aware tools in the core worker so OpenAgents stops grepping blindly in Coax repos.
     - Add a **lightweight general/non-coding prompt profile** so conversational/KB/wallet/CLI turns don't pay the full software-engineering system prompt tax.
     - Tighten local Inkling 2 Mini tool budgets & structured JSON tool calling so local turns handle 60–70% of everyday requests cleanly at $0 cost.

### Phase 3: Wire the Thin System One Front & Composable Members
1. **Extend `coax/lib/agent/router/`** (`ask.coax`, `policy.coax`, `decide.coax`) with a new schema version (`openagents.router.v2alpha1` in `proto/`):
   - Add typed questions: `domain` (`general`, `knowledge`, `coding`, `wallet`, `cli`, `metaverse`, `gym`, `missing_capability`), `compute` (`local_inkling`, `cloud_fast`, `cloud_deep`, `remote_station`), `explicit_delegate` (`none`, `claude`, `codex`, `antigravity`), and `continue` (keep active pack/context in multi-turn conversations).
   - Keep the exact `#authority` discipline already proven in `coax/lib/agent/router/policy.coax`: code decides eligibility, grants, and safety walls; System One only classifies intent & difficulty; labelled eval sets (`labels.tsv` + `labels-held.tsv`) gate every prompt edit.
   - Door order: local `elephant-1` (when resident, ~ms, $0, private) or `gemini-decision-1` (~$0.0001), with deterministic fallback to the core default if offline.
2. **Mount existing subsystems as typed OpenAgents members (no new UI chrome required):**
   - **Coder member:** workspace tools + `intel` + git worktree (`flawed/core.coax`).
   - **Wallet member:** `#authority` `pay/` + `Window+WalletRPC.swift` / `apps/openagents-cli/wallet.coax`, always gated by an interactive spending-approval card (`"Send 1,000 sats to ...?"`).
   - **Metaverse / GameBuilder member:** `coax/lib/agent/desktop/metaverse-route.coax` + bundled `gamebuilder` CLI/MCP (`#14039–#14046`).
   - **CLI member:** typed manifest over `apps/openagents-cli/` commands (`nostr`, `pylon`, `project`, `dictate`, `transcribe`, `realm`, `vault`).
   - **Agents Catalog member:** `coax/lib/agent/desktop/agents-catalog.coax` (`simulated-app-tester`, `game-builder`, `coder`) with target-lease enforcement.

### Phase 4: Close the Full Episode 289 Loop (Knowledge, Authoring, Registry, Payouts)
1. **Shared Knowledge Base member (P2):** Index `docs/`, `*/book/`, `docs/transcripts/`, and workspace docs with hybrid semantic/lexical retrieval so *"what did we decide in episode 289?"* or *"how does Coax `#authority` work?"* answers accurately in one fast turn without spinning up a repo grep loop.
2. **Conversational Capability Authoring (P5/P9/P15):** When System One or Core hits `missing_capability`, run `coax/lib/agent/desktop/capability-eval.coax` right inside the chat transcript: short interview → draft `SKILL.md` + positive/held-out cases → live two-arm ATIF-v1.8 eval → `#authority` `capability-gate.coax` verdict → immediate local activation.
3. **Open Registry & Lightning Payouts (P10–P11, P14):** Publish passing capability packages + ATIF digests as signed `NIP-REG` / `NIP-EVAL` events (building on `openagents/examples/signed-module-registry.coax`), allow hash-pinned installs across all OpenAgents clients, and meter paid calls onto `#authority` `pay/` + `coax/lib/lightning/` splits.
4. **Update `openagents/book/` to match reality:** Reconcile `openagents/book/PLAN.md` and `chapters.json` so Chapters 12–53 point at the real `flawed/`, `coax/lib/agent/router/`, `capability-eval.coax`, and `pay/` implementations rather than duplicating them as separate unbuilt drafts.

---

## 6. Five Concrete Decisions for the Owner

Before implementing Phase 1–3, five quick calls will lock the direction cleanly:

1. **Retire "Flawed" as a user-facing name?** Keep `flawed/` as the internal pure-Coax core engine directory (or rename later), while every user-facing prompt, window title, badge, and setting says **OpenAgents**? *(Recommended: Yes.)*
2. **Default compute routing policy:** Should **Auto** prefer **Local Inkling 2 Mini first** (escalating to Cloud Neon when System One scores difficulty ≥ Moderate or context exceeds local budget), or **Cloud Neon first** with Local as opt-in/offline fallback? *(Recommended: Local-first for General/KB/CLI/Trivial, Cloud Neon for Hard/Multi-file coding—with a 1-click `Auto | Local | Cloud` override.)*
3. **External harnesses (`Claude Code`, `Antigravity`, `Codex`, `Cursor`):** Demote all of them from top-level chat pickers to **delegated sub-engines** behind OpenAgents (invoked when you ask `"do it with Codex"` or via Developer flag)? *(Recommended: Yes.)*
4. **Mobile/macOS companion convergence (`apps/openagents-ios/`, `apps/openagents-macos/`):** Keep them Antigravity-only for now while we ship Phase 1–3 on Desktop, or prioritize switching their Tailscale RPC target from `agyworker` to the unified OpenAgents Core daemon next? *(Recommended: Switch to OpenAgents Core daemon right after Phase 1 lands so iOS + Mac talk to the exact same OpenAgents agent.)*
5. **Phase ordering:** Execute **Phase 1 (Collapse UI/Identity) + Phase 2 (Core Hillclimb)** immediately, followed by **Phase 3 (System One General Front + Wallet/CLI/Metaverse members)**, then **Phase 4 (KB + Chat Authoring + Registry/Lightning)**? *(Recommended: Yes—Phase 1 removes friction immediately, Phase 2 keeps compounding the benchmark lead, and Phase 3 unlocks Episode 289's general-agent magic on top of a rock-solid core.)*
