# APM (Actions Per Minute) — Full Audit

**Status:** Historical audit · **Date:** 2026-06-29 · **Scope:** every APM-related
artifact across the OpenAgents corpus (video series, code, docs, UI, schema).

> **TL;DR.** APM — *Actions Per Minute*, a StarCraft-inspired agent-velocity metric
> defined as `APM = (messages + tool_calls) / duration_minutes` — was a recurring
> idea from **Episodes 185–188** (July 2025). It was implemented **three separate
> times** (Jul 2025 Tauri/Convex desktop app; Dec 2025 MechaCoder/Electrobun HUD;
> Dec 2025 Rust `autopilot` crate + `wgpui` UI + leaderboard spec), then **pruned**
> out of the active repo in the 2026‑02‑25 restructure (`d7f53fccc`). It survives
> today only as **git history** and as files under `backroom/`. The **live**
> successor concept is the `/trace/{uuid}` ATIF trace system plus the
> "throughput leaderboard (accepted outcomes/week)" direction in the agentic-MMO
> design doc. **No APM code runs in the active product today.** This document is the
> single place that records what was built, why, where it lives, and how the
> definitions diverged.

---

## 1. Origin — the video series (Episodes 185–188, July 2025)

The concept was stated on-camera by Christopher David. Transcripts live in
`docs/transcripts/` (machine-generated; verify wording before quoting).

| Ep | Title | The APM idea, in his words |
| --- | --- | --- |
| [`185`](../transcripts/185.md) | (mobile sync / Convex) | "I want to measure **APM** … what is the **agent actions per minute**" — wants a baseline of his own Claude Code usage, then to "measure our increases," and "maybe even put up a **leaderboard** where we let people show off their APM, **AAPM**." Wants to run overnight and compare *previous usage* vs *overnight orchestration*. |
| [`186`](../transcripts/186.md) | **Actions Per Minute** | The canonical episode. Frames it against a *flawed* "Humanity's Last Exam" benchmark — "developers who may also be gamers … know what to measure." Defines the StarCraft parallel: APM = "**Messages to and from the agent and tool calls**. We'll start with that. We can sophisticate it over time." Demos analyzing "**two months of** [Claude Code] **conversations** … saved on my desktop" via a Tauri app, reports a **lifetime APM of 2.298**, and promises a running spec: "*there'll be something … that says **apm.md** … a running spec of how we're measuring APM.*" |
| [`187`](../transcripts/187.md) | (beta direction) | "measure our **new APM** as I run this on my phone over the weekend … Monday we'll share results about **upgraded APM**." |
| [`188`](../transcripts/188.md) | (HUD workflow) | "I'm tracking the APM pretty soon. That'll be like a **little live ticker** of the APM" in the HUD. |

Echoes later in [`199`](../transcripts/199.md)/[`200`](../transcripts/200.md)/[`206`](../transcripts/206.md):
"I had the **APM storage** and the **trajectory collector**…" — i.e. APM was paired
from the start with a *trajectory collector* (the precursor to today's trace store).

**The original thesis (verbatim spirit):**
1. Frontier benchmarks are gameable/bogus; trust *practitioner-gamer* metrics instead.
2. Borrow StarCraft's **APM** as the "primary metric of mechanical skill."
3. Agent APM = **messages + tool calls per minute**, computed from saved Claude Code
   conversation logs.
4. Establish a personal **baseline**, then prove the orchestration layer **raises it**
   (interactive vs overnight-autonomous).
5. Make it social: a **leaderboard** ("AAPM"), a **live ticker** in the HUD, a
   public **apm.md** spec.

Named transcript copy also exists: `…/transcripts/oa-186-actions-per-minute.md`
(committed `d50d8db22a "APM doc"`).

---

## 2. The StarCraft framing (design rationale)

Captured in `backroom/reference/openagents-docs/inspiration/starcraft.md` — "The
Performance-Driven Power User Interface." Core mapping:

| StarCraft | OpenAgents equivalent |
| --- | --- |
| **APM (Actions Per Minute)** | **Jobs/hour · agent efficiency metrics** |
| Hotkeys / control groups (1‑9) | Agent management shortcuts / agent groups |
| Minimap | Dashboard / fleet overview |
| Resource counters (minerals, supply) | Token usage, sats balance, API limits |
| Build orders | Workflow templates / blueprints |
| Replay system · match stats | Job history · production dashboard |
| Leaderboards / tournaments | Efficiency leaderboards |

Thesis: managing agents is *performance work*, not casual browsing; a tool that is
**measurable** rewards mastery and self-selects power users. This is the design DNA
behind every later APM gauge, the leaderboard spec, and the agentic-MMO HUD.

---

## 3. Implementation history — three waves, then pruned

APM was built three times. Below is the chronology (commit hashes are in the
`openagents` repo history unless noted; the **code itself** no longer exists in the
working tree — recover via `git show <hash>:<path>` or read the mirrors under
`backroom/`).

### Wave 1 — Tauri/Convex desktop app (Jul 2025, Episodes 185–188 era)

The on-stream implementation. Issue **#1167**, PR **#1170**
(`d93d0af21e "Implement Comprehensive APM Analysis System"`).

- `tauri/src-tauri/src/lib.rs` — Rust commands that parse saved **Claude Code
  conversation JSONL** from the user's desktop and compute APM over time windows.
- `tauri/src/panes/StatsPane.tsx` (+ `Hotbar`, `PaneManager`, pane stores) — the
  **Stats pane** that showed the lifetime/1h/6h/1d/1w/1m numbers from Ep 186.
- `docs/apm.md` (191 lines) and `docs/apm-analysis-methodology.md` (214 lines) —
  the first written spec promised on-camera.

Follow-ups (Jul 24–29, 2025), all in the desktop/mobile Convex app:
- `c6cd86977d` historical APM **chart** (time-series visualization).
- `dd053c5f2e` (#1197) extend APM to **SDK/Convex** conversations, not just local logs.
- `4a1db4f078` (#1239) **Effect-TS APM service** (Phase 2); `528bca4b02` Confect/Convex.
- `a084571db5` (#1225) **multi-client APM aggregation** across **desktop, mobile, GitHub**.
- `3713e5262c` (#1283) **realtime APM component** (Effect-TS + Convex).
- `debb90b051` APM services + mobile sync.

### Wave 2 — MechaCoder / Electrobun HUD (early Dec 2025)

The "live ticker" from Ep 188, rebuilt for the Electrobun desktop HUD.

- `72c4f9e9f4` APM measurement system for **MechaCoder**.
- `0a1cf888ef` integrate APM tracking into **orchestrator + HUD**.
- `b1c8fc5da8` APM **SVG widget** in the Electrobun mainview; `47dd1e0b0c` (Phase 2)
  APM widget + keyboard shortcuts; `5816102a23` ported into the "effuse" widget set.
- `d50d8db22a "APM doc"` — the named Ep‑186 transcript.
- HUD WebSocket protocol: `backroom/reference/openagents-docs/hud/APM.md` defines
  `apm_update`, `apm_snapshot`, `apm_tool_usage` messages (see §5).

### Wave 3 — Rust `autopilot` crate + `wgpui` UI + leaderboard (late Dec 2025)

The most complete implementation, tied to the "directives" program (d‑012…d‑022,
"Phase 5 — Gamification"). This is the version most worth studying.

**Core (`crates/autopilot*/src/`, mirrored at `backroom/autopilot-old/src/`):**
- `apm.rs` (~666 lines) — canonical module. `calculate_apm()`,
  `calculate_apm_from_timestamps()`, `APMStats` (session/1h/6h/1d/1w/1m/lifetime),
  `APMBaseline`, `APMSource` (Autopilot | ClaudeCode | Combined), `APMWindow`,
  `APMTier`, `APMMetrics` (real-time aggregator), `SessionData`.
  (`8bd8eb3b5d` tracker core; `8496683636` calculation module.)
- `apm_parser.rs` (~447 lines) — JSONL parser: `parse_claude_code_session()` reads
  `~/.claude/projects/<encoded-path>/*.jsonl` and autopilot `docs/logs/**/*.jsonl`,
  counting `user`/`assistant`/`tool_use` and first/last timestamps.
  (`80bc895639`.)
- `apm_storage.rs` (~1158 lines) — **SQLite** persistence (schema v2). Tables:
  `apm_schema_version`, `apm_sessions`, `apm_events` (event_type:
  message/tool_call/git_command/file_operation/other), `apm_snapshots`,
  `apm_baselines`. (`fac29a325e` data model; `332f1af17d` snapshots table in
  `metrics.db`.)
- `apm_telemetry_bridge.rs` (~251 lines) — `spawn_telemetry_consumer()` drains
  `acp_adapter::ActionEvent`s into the APM DB during a run. (`52b8b20d7c` ACP
  telemetry E2E test.)
- `trajectory.rs` — `Trajectory`/`Step`/`StepType`; computes APM from
  `(messages + tool_calls)/duration`. `c0007c6b4a` integrated APM storage into the
  **TrajectoryCollector** (the Ep‑199/200 "APM storage + trajectory collector").
- `0ef6521251` auto-track APM on session completion; `197c94141f` / `BackfillApm`
  CLI backfilled APM from existing trajectory logs; `eafe02d131` APM in benchmark
  results.

**CLI (`autopilot apm …` / unified `openagents` binary):**
`stats`, `sessions`, `show <id>`, `breakdown`, `export <file>` (JSON for external
analysis), `watch [--interval]` (live dashboard), `baseline`, plus a top-level
`--no-apm` run flag and `apm regenerate` snapshot command (#964). (`bd276ad061`,
`7aa84117d9`, `a5f1a291b5`, `840c6d5e64`, `62ca436dce`, `7d9ebc3353`.)

**UI (`crates/wgpui/src/components/`, in
`backroom/openagents-rust-deprecation-2026-02-11/`):**
- `atoms/apm_gauge.rs` — `ApmGauge` dial + `ApmLevel` enum.
- `molecules/apm_session_row.rs` — ranked session row (score, tier, status badge).
- `molecules/apm_comparison_card.rs` — side-by-side Claude Code vs Autopilot.
- `organisms/apm_leaderboard.rs` — ranked leaderboard.
- Storybook **Section 23 "APM Metrics"** (`3472560821`); GUI **gauge + thinking
  blocks** (`180db69ea9`); dashboard indicators (`bf6b6a734b`, `d7b7867f81`),
  real-time WebSocket updates (`b842d13e07`), comparison view (`199ea0ced2`).

**Docs (this wave):**
- `docs/apm.md` — spec + CLI quickstart + tiers (see §4).
- `docs/apm/methodology.md` — data sources, JSONL counting rules, caveats.
- `docs/apm/leaderboard-design.md` (531 lines) — full **d‑016** leaderboard spec:
  Personal / Project / Global scopes, `LeaderboardEntry` data model, Nostr-relay
  aggregation, time-decay ranking. (`faab6e0fd9`.)
- `docs/apm/report-20251222.md` — first multi-machine report, marked **"PENDING
  RECOLLECTION"** after a methodology bug was found (see §6).
- Weekly automated trend reports (`e51259bc35`, `0bb65e08ec`) and
  auto-issue-from-anomaly (`89b393b7ca`).

**Nostr projection:** `nip_sa_trajectory.rs` mapped TrajectoryCollector events to
NIP‑SA Nostr trajectory events (kinds 38030/38031) — `1097ae4433`, `8b1a4e7ed1`,
`a31e22c7ae`, `dfac9d21a2`, `411b71f683`.

### The prune

Most of this was removed from the active repo in the **2026‑02‑25** restructure
(commit `d7f53fccc`, per the workspace `CLAUDE.md`), alongside DSPy/Adjutant/RLM/FRLM.
`1f4ed67e23 "Archive outdated autopilot docs to backroom"` moved the surviving
material out. **As of this audit the active `openagents` tree contains no `apm.*`
code and no `docs/apm/` other than this file** — only the transcripts and the
agentic-MMO design-doc reference (§7).

---

## 4. Definition drift — ⚠️ the metric was specified inconsistently

The **formula is stable** everywhere:

```
APM = total_actions / duration_minutes
total_actions = messages (user + assistant) + tool_calls
```

But the **tier bands diverged across implementations** — the same APM number means a
different "tier" depending on which file you read. Anyone reviving APM must pick one.

| Source | Bands |
| --- | --- |
| `backroom/openagents/docs/apm.md` (Wave 3 spec) | Baseline 0–5 · Active 5–10 · Productive 10–15 · Elite 15–20 · **Superhuman 20+** |
| `docs/apm/methodology.md` **and** `apm.rs::APMTier` | Baseline 0–5 · Active 5–15 · Productive 15–30 · High Performance 30–50 · **Elite 50+** |
| `wgpui …/apm_gauge.rs::ApmLevel` | Idle <1 · Low <10 · Normal 10–30 · High 30–60 · **Intense 60+** |

Note the **"Elite" collision**: in one scale Elite is 15–20; in another it's 50+.
The gauge uses an entirely different vocabulary (Idle/Low/Normal/High/Intense). This
is the single biggest correctness hazard in the corpus.

**Calibration anchors observed** (useful sanity checks, not authoritative):
- Ep 186 on-stream host **lifetime APM ≈ 2.298** (interactive, including idle time).
- `apm.rs` header + `docs/apm.md` example: interactive **Claude Code ≈ 4.5 APM**,
  autonomous **Autopilot ≈ 19 APM** → the whole point: orchestration ~4× velocity.

Note the **window divisor subtlety** (original Tauri `docs/apm.md`): windowed APM
divided by the *full window length* (e.g. last-hour actions ÷ 60), so wall-clock APM
over long windows is tiny (0.0x–0.5x) because idle hours count. The Wave‑3 session
APM divides by *active session* duration, giving the larger 4.5/19 figures. **These
two conventions are not comparable** — another reason the report was reset (§6).

---

## 5. HUD / real-time protocol (Wave 2)

From `backroom/reference/openagents-docs/hud/APM.md`. The "live ticker" Ep 188 wanted:

```typescript
// periodic, during a run
interface APMUpdateMessage {
  type: "apm_update";
  sessionId: string;
  sessionAPM: number;     // current session
  recentAPM: number;      // last 5 minutes
  totalActions: number;   // messages + tool calls this session
  durationMinutes: number;
}

// at session start/end, with history + comparison
interface APMSnapshotMessage {
  type: "apm_snapshot";
  combined: { apm1h; apm6h; apm1d; apm1w; apm1m; apmLifetime; totalSessions; totalActions };
  comparison: { claudeCodeAPM; mechaCoderAPM; efficiencyRatio /* mecha/claude */ };
}

// tool-mix breakdown
interface APMToolUsageMessage { type: "apm_tool_usage"; tools: { name; count; percentage; category }[] }
```

Event flow: `APMCollector.recordAction("tool_call","Edit")` → throttled `apm_update`
(every ~30s or on significant change) → HUD widget.

---

## 6. Known data-quality issues (documented at the time)

`docs/apm/report-20251222.md` was published **"PENDING RECOLLECTION"** after the team
found the collection pipeline was producing garbage. The recorded failure modes:

1. **`.rlog` truncation** — human-readable logs cap messages at 200 chars, tool output
   at 100; counting from them undercounts. **Fix:** always count from full `.jsonl`,
   never `.rlog` (now the loud rule in `methodology.md`).
2. **Subagent activity uncounted** — the `x:` subagent-spawn line type existed in the
   format spec but was never emitted; `.sub-*.jsonl` files weren't linked. **Fix:**
   dual-format logging + `parent_session` linkage.
3. **Fake durations** — early stats assumed "15 min/session" instead of reading
   first/last timestamps, producing meaningless numbers. **Fix:** real timestamps only.
4. **Record-type confusion** — older code grepped `"type":"message"` (deprecated);
   correct counting is `"type":"user"` + `"type":"assistant"` + nested
   `"type":"tool_use"`.
5. **Idle inflation / error sessions / multi-machine double-counting** — listed as
   open caveats; combining machines must sum actions and durations separately and
   avoid counting agent-subprocess files twice against rlog.

The upshot: **every published APM number from before 2025‑12‑22 should be treated as
unreliable**, and the host's own Ep‑186 figure (2.298) predates these fixes.

---

## 7. What APM is *today* (the live successors)

APM-the-feature is dormant, but its three jobs (capture, compare, rank) live on under
different names in the active product:

- **Capture → ATIF traces / `/trace/{uuid}`.** The TrajectoryCollector lineage became
  the trace store: `POST /api/traces`, D1 + R2 refs, public shareable
  `/trace/{uuid}` render, Claude Code/Codex → ATIF converters, N-way trace comparison
  (#6209, #6211, #6223; default-on redacted free-tier capture). This is the modern,
  shipped version of "save and analyze every agent conversation."
- **Rank → "throughput leaderboard."**
  [`docs/game/2026-06-16-spatial-hud-agentic-mmo-wow-direction.md`](../game/2026-06-16-spatial-hud-agentic-mmo-wow-direction.md)
  explicitly carries the Ep‑185 idea forward but **re-bases the metric on honesty**:
  > "AAPM + leaderboard (`185`) → **Throughput leaderboard**: accepted outcomes/week
  > (the Linear 50–70 fixes/wk benchmark), per contributor/agent → a rank → that
  > contributor's receipts."
  i.e. rank by **accepted outcomes** (with receipt click-through), **not** by raw
  action count.
- **Compare → receipts / accepted-outcome accounting.** The Ep‑237 "accepted outcome
  as the atomic unit of the economy" reframes velocity around *verified delivered
  work*, addressing the core weakness below.

### Why the pivot away from raw APM is correct

Raw "actions per minute" is **Goodhart-fragile**: it rewards chattiness and tool
spam, not delivered value (a verbose agent that re-reads files looks "Elite"). The
StarCraft analogy breaks because in StarCraft every action is *intentional input by a
skilled human*; for an autonomous agent, more actions can mean *worse* behavior
(thrashing, re-planning, redundant reads). The honest metric is **accepted outcomes
per unit time**, which is exactly where the live product landed. APM remains useful as
a **secondary, diagnostic** signal (detecting stalls/thrash within a single run, or
A/B-ing interactive vs orchestrated velocity), not as a headline leaderboard score.

---

## 8. If we revive APM — recommendations

1. **Pick one tier scale and delete the others.** Recommend the `apm.rs`/methodology
   scale (Baseline 0–5 / Active 5–15 / Productive 15–30 / High 30–50 / Elite 50+) and
   retire the conflicting "Superhuman 20+" and gauge "Idle/Low/Normal/High/Intense"
   bands, or map all UIs onto the one scale.
2. **Treat APM as diagnostic, rank on accepted outcomes.** Keep the throughput
   leaderboard receipt-backed; surface APM only as an in-run velocity ticker.
3. **Source from ATIF traces, not bespoke JSONL parsing.** The trace store already
   captures messages + tool calls with timestamps and redaction; compute APM as a
   derived view over `/api/traces` rather than re-reading `~/.claude`.
4. **Always count from full data + real timestamps.** Re-apply the 2025‑12‑22 fixes
   (no `.rlog`, count `user`+`assistant`+`tool_use`, first/last timestamp duration,
   include linked subagents once).
5. **State the window convention explicitly.** "Session APM" (÷ active duration) and
   "windowed APM" (÷ full window) are different metrics; never show them on the same
   axis without labels.

---

## Appendix A — Artifact index (where the bodies are)

> The active `openagents` working tree contains **none** of the code below. Recover
> code via `git show <hash>:<path>` in `openagents`, or read the mirrors in
> `backroom/`. Paths are as-committed.

**Active repo (live):**
- `docs/transcripts/185.md`,`186.md`,`187.md`,`188.md`,`199.md`,`200.md`,`206.md` — origin.
- `docs/game/2026-06-16-spatial-hud-agentic-mmo-wow-direction.md` — live successor.
- `docs/apm/AUDIT.md` — this file.

**Recoverable from `openagents` git history:**
- `tauri/src-tauri/src/lib.rs`, `tauri/src/panes/StatsPane.tsx` — Wave 1 (`d93d0af21e`).
- `docs/apm.md`, `docs/apm-analysis-methodology.md` — Wave 1 spec (`d93d0af21e`).
- `crates/autopilot/src/apm.rs`,`apm_storage.rs` — Wave 3 (`8bd8eb3b5d`,`fac29a325e`).
- `docs/apm/leaderboard-design.md` — Wave 3 (`faab6e0fd9`).

**Mirrored under `backroom/` (readable directly):**
- `backroom/autopilot-old/src/{apm,apm_parser,apm_storage,apm_telemetry_bridge,trajectory,nip_sa_trajectory}.rs`
- `backroom/autopilot-old/src/cli.rs` — `ApmCommands`, `BackfillApm`, `--no-apm`.
- `backroom/openagents/docs/apm.md`, `backroom/openagents/docs/apm/{methodology,leaderboard-design,report-20251222}.md`
- `backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/wgpui/src/components/{atoms/apm_gauge,molecules/apm_session_row,molecules/apm_comparison_card,organisms/apm_leaderboard}.rs`
- `backroom/reference/openagents-docs/{apm.md,hud/APM.md,inspiration/starcraft.md,claude/plans/apm.md}`
- `backroom/reference/openagents-docs/transcripts/oa-186-actions-per-minute.md`
- `backroom/openagents-examples/workflows/analyze_trajectories.py` — pandas trajectory analyzer.

## Appendix B — Key commits (chronological)

```
2025-07-25  d93d0af21e  Comprehensive APM Analysis System — Issue #1167 (#1170)   [Wave 1]
2025-07-26  c6cd86977d  Historical APM chart (time-series)
2025-07-26  dd053c5f2e  Extend APM to SDK/Convex conversations (#1197)
2025-07-27  4a1db4f078  Phase 2 Effect-TS APM + Auth services (#1239)
2025-07-27  a084571db5  Multi-client APM aggregation: desktop/mobile/GitHub (#1225)
2025-07-29  3713e5262c  Realtime APM component (Effect-TS + Convex) (#1283)
2025-12-03  72c4f9e9f4  APM measurement system for MechaCoder                       [Wave 2]
2025-12-03  0a1cf888ef  Integrate APM into orchestrator + HUD
2025-12-03  b1c8fc5da8  APM SVG widget in Electrobun mainview
2025-12-03  d50d8db22a  "APM doc" (oa-186 transcript)
2025-12-22  8496683636  APM calculation module                                      [Wave 3]
2025-12-22  80bc895639  Claude Code JSONL parser for APM extraction
2025-12-22  332f1af17d  APM fields + snapshots table in metrics.db
2025-12-22  197c94141f  Backfill APM from existing trajectory logs
2025-12-22  (report)     report-20251222.md — "PENDING RECOLLECTION" (methodology fix)
2025-12-23  8bd8eb3b5d  APM tracker core (ActionEvent + APMMetrics)
2025-12-23  c0007c6b4a  Integrate APM storage into TrajectoryCollector
2025-12-23  840c6d5e64  APM export for external analysis
2025-12-23  199ea0ced2  APM comparison view (Claude Code vs Autopilot)
2025-12-23  e51259bc35  Automated weekly trend reports
2025-12-24  faab6e0fd9  APM leaderboard design spec (d-016)
2025-12-25  180db69ea9  Autopilot GUI: APM gauge + thinking blocks
2025-12-25  3472560821  Storybook Section 23 "APM Metrics"
2026-02-25  d7f53fccc   Restructure/prune (APM removed from active repo)
2026-02-(xx) 1f4ed67e23  Archive outdated autopilot docs to backroom
2026-06-16  (doc)        spatial-hud doc reframes AAPM → throughput leaderboard
2026-06-(xx) (#6209…)    ATIF /trace/{uuid} trace store — live successor to capture
```
