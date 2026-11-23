# Transcript Friction Hillclimb: Empirical Audit, Failure Taxonomy & Redesign Playbook

**Date:** 2026-11-22  
**Corpus audited:** Read-only sweep of local Antigravity (`~/.gemini/antigravity/brain/`, `~/.gemini/antigravity-cli/brain/`) and OpenAgents (`~/Library/Application Support/Coder/chats/`, `run-stops.log`, `foreman-audit.jsonl`, `~/.openagents/coder/sessions/*.{ajr,jsonl}`, `~/.openagents/coder/prompt-history.jsonl`).  
**Baseline anchor:** `origin/main` @ `02759077b5` (Nov 22, 2026).

> **Important methodological note on the 48-hour `AGENTS.md` blackout:** Commit `cca95dcf9c` (`2026-11-20 20:48 CST`, `#13845`) accidentally wrote a 0-byte `AGENTS.md` to `main`. It was restored on `origin/main` 48 hours later in `763b29abc1` (`2026-11-22 21:52 CST`), while the local main checkout remained on an older snapshot with 0-byte `AGENTS.md`. Even after restoration, `origin/main:AGENTS.md` measured **37,425 bytes (36.5 KB)** — overflowing Antigravity's **24 KB** per-rule cap (`## Build and test` starts at 23.7 KB; `Wait without polling` at 33.1 KB was silently truncated), while Inky only loaded **~1.9 KB** (`IK_WS_INSTR_MAX = 2560 - 640`) and Microcoder only loaded **256 bytes / 5 lines** (`MC_INSTR_MAX_BYTES = 1024 - 768`). Every finding below distinguishes **blackout/truncation-induced failures** from **structural harness, verification, and workflow failures**.

---

## 1. Quantitative Corpus Baseline

| Store | Scope | Volume | Date Span | Key Headline Metrics |
| --- | --- | --- | --- | --- |
| **Antigravity Desktop/IDE** (`~/.gemini/antigravity/brain/`) | 89 analyzed sessions (37 top-level, 52 subagents) | 110.4 MB (`transcript.jsonl`), 41,023 top-level steps | 2026-07-24 – 2026-11-22 | **320** user turns (median 6/session); **56.8%** of top-level sessions (**67.7%** of multi-turn) had user rework/corrections; **124** compactions; **9,321** truncated tool outputs; **1,240/10,991** (`11.3%`) failed commands; **851** `manage_task(status)` polls; **0** `ask_question` calls vs **49** trailing prose `?` replies |
| **Antigravity CLI** (`~/.gemini/antigravity-cli/brain/`) | 43 sessions (32 non-smoke, 11 eval/smoke) | 1.4 MB | 2026-11-11 – 2026-11-22 | 173 commands, 22 non-zero exits (`12.7%`) |
| **OpenAgents Desktop** (`~/Library/Application Support/Coder/chats/`) | 117–118 chats (109 real, 9 smoke), 386 turns | 29.9 MB JSON, **$578.74** recorded spend | 2026-11-17 – 2026-11-22 | **43** empty-reply turns (**26 with blank `note=""`**); **12.8%** ended bad/incomplete; only **29/109 (26.6%)** used git worktrees; **382/5,375 (7.1%)** failed commands; **58** verify/ship nudges (`21.6%` of follow-ups) |
| **OpenAgents AJR1 Journals** (`~/.openagents/coder/sessions/*.ajr`) | 1,287 `.ajr` files | 852 MB total payload | 2026-11-10 – 2026-11-22 | **740/1,287 (57.5%) are 0-byte empty files** (135 created on Nov 22 alone vs 27 non-zero); **725.1 MB** `model_chunk` vs **15.6 MB** `model_reply` (**46:1 write amplification**); **64/752 (8.5%)** abnormal stops; **308/7,917** Vertex calls corrupted by dangling `model` slice |
| **OpenAgents JSONL + Daemon Logs** | 252 `.jsonl` sessions, `run-stops.log` (185 lines), `foreman-audit.jsonl` (28 lines) | 86 real + 166 `echo` | 2026-11-02 – 2026-11-22 | **141/185 (76.2%)** `run-stops.log` entries are `"left running at launch: busy, though no kept message names it"` — **139 of 141 from just 4 permanent zombie runs**; **7/9** Foreman `create_chat` calls HTTP 500 |

### OpenAgents Desktop Breakdown by Model/Harness (109 Real Chats)

| Harness / Model | Chats | Turns | Empty Replies (%) | Non-Done States | Cmd Fail Rate | Rework Turns | Verify/Ship Nudges | Recorded Spend | p50 TTFT / tok/s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `gemini-4-neon` (Microcoder) | 37 | 118 | 12 (`10.2%`) | 12 | `117/1934` (`6.0%`) | 19 | 21 | $262.62 | — |
| `unknown` (pre-persist model field) | 44 | 156 | 13 (`8.3%`) | 12 | `77/1271` (`6.1%`) | 9 | 27 | $255.69 | — |
| `antigravity` (legacy) | 4 | 41 | 7 (`17.1%`) | 5 | `74/808` (`9.2%`) | 5 | 6 | $60.23 | — |
| `inkling-2-mini` (Inky local) | 17 | 28 | 5 (`17.9%`) | 5 | `10/333` (`3.0%`) | 2 | 2 | $0.00 | `754 ms` / `49.2 tok/s` |
| `agy:gemini-4-neon` | 5 | 11 | 2 (`18.2%`) | 2 | `78/741` (`10.5%`) | 1 | 2 | $0.00 | `15,550 ms` |
| `openrouter:deepseek-v4.1-flash` | 2 | 10 | 0 (`0.0%`) | 0 | `26/288` (`9.0%`) | 1 | 0 | $0.21 | — |

### Antigravity Command Failure & Wall-Time Hotspots (89 Sessions, 10,991 `run_command` Calls)

| Category | Calls | Fails (%) | Sum Time | Dominant Root Cause |
| --- | ---: | ---: | ---: | --- |
| `shell-read/search` | 4,071 | 171 (`4.2%`) | 78.7m | `1,423` truncated outputs (unbounded `cat`/`rg`/`find`); `128` grep exit-1s; `106` `no_such_file` |
| `coax-cli` (`coax test/check/mutate`) | 1,372 | **508 (`37.0%`)** | 71.2m | Top error codes: `E0315` (244x), `E0406` (168x), `E0702`/`E0700` (137x), `E0317` (87x), `E0351` (75x), `E0366` (58x), `E0400`/`E0404` (67x), `E0941`/`E0942` (86x), `E0216` (`..<`) |
| `coax-tool.sh` | 703 | **151 (`21.5%`)** | 41.2m | Missing `./coax/bin/coax` in fresh worktrees; compile-time checks |
| `git-write/sync` | 643 | 44 (`6.8%`) | 34.2m | **43** merge/rebase conflicts; **28** `prepush` rejections (missing `Agent:` trailer, non-fast-forward) |
| `python-script` | 919 | 108 (`11.8%`) | 29.0m | Quote-escaping `SyntaxError`s, timeout-less live test scripts |
| `desktop-build/install` | 415 | 33 (`8.0%`) | 10.0m | `--full` builds taking 2.5–3m+, silent `install.sh` exit-0 deferrals |
| `sleep/poll` | 81 | 11 (`13.6%`) | 2.6m | Plus **851** `manage_task(status)` polling calls across Antigravity sessions |

---

## 2. Top 10 Quantified Failure Modes (Backed by Transcript & Code Evidence)

### 1. `AGENTS.md` Blackout + Silent Truncation Across All Three Harnesses
- **Evidence:** Commit `cca95dcf9c` wiped `AGENTS.md` to 0 bytes for 48 hours (`2026-11-20` → `2026-11-22`); `coax/web/prepush.coax` allowed it because `pp_only_docs` skipped checks, same-agent commits naming `AGENTS.md` bypassed `pp_scan`, and `pp_judge` lacked any file-size floor guard. Even restored at `763b29abc1`, `AGENTS.md` was **37,425 bytes** — silently truncated at **24,000 bytes** by Antigravity (`## Build and test` and `Wait without polling` lost), at **1,920 bytes** by Inky (`coax/lib/inky/instructions.coax:13`), and at **256 bytes** by Microcoder (`coax/lib/microcoder/instructions.coax:27`, diagnosed in chat `b6aeea54` / `#13808`).
- **Fallout:** 4+ sessions on Nov 22 (`00f1bf5a` step 685, `95fa0f93` step 479, `1386dad6` step 500, `fbf4f695` step 111) bounced on `prepush` missing `Agent: ID`; 3 concurrent `/goal` hillclimbs (`767d5084`, `12d58a9e`, `1447d7a2`) edited the shared main checkout (causing 60+ manual 3-way merge steps in `12d58a9e`); agents repeatedly built `--full` instead of `--minimal`.
- **Fix shipped:** Critical-file wipe/shrinkage guard in `coax/web/prepush.coax` & `contributor-check.coax`; restructured `AGENTS.md` (<22 KB with highest-ROI rules in the top 2 KB + `docs/REPO-MAP.md`); progressive-disclosure `.agents/rules/` + `.agents/skills/`; bumped `IK_WS_INSTR_MAX` and `MC_INSTR_MAX_BYTES`.

### 2. "Fake Done" — Skipping Live E2E Verification on the Installed `--minimal` Artifact (~50% of Wasted Turns)
- **Evidence (44 wasted/friction turns out of 132 [33.3%] across the 5 highest-friction Antigravity sessions):**
  - `1386dad6` T06 (`step 678`): `"what the FUCK is going on? you didnt load models., and what the fuck is this login bullshit?????"` — never ran live auth + model list.
  - `1386dad6` T09 (`step 979`): `"im trying to click on gcp project and cant fucking click on it... did you fuckign TEST ITS A REAL INPUT?"` — unit tests called `cp_press` directly, missing host `CP_ASK_FOCUS` clobber.
  - `1386dad6` T11–T13 (`steps 1165, 1245, 1340`): never ran a live `agyworker` turn executing an escalated command (`git push`), shipping a 1 µs `cu_settle` auto-decline that killed multiple chats.
  - `00f1bf5a` T07–T08 (`steps 596, 605`): checked only offscreen Metal PNG (`--shot-fileviewer`), never clicking a `file://` link or a File Explorer item in a worktree.
  - `95fa0f93` T03/T05/T07 (`steps 528, 749, 833`): tested only `DescribeEnvironment`, missing `ListSessions` exceeding `JCS_MAX_KEYS=1024` on 96 real sessions, `NWListener.newConnectionLimit=4`, and Desktop's `sweepRuns()` deleting active phone chats.
  - `b5774a4f` T14–T16/T39 (`steps 272, 291, 302, 2747`): never launched `/Applications/OpenAgents.app` (port collision, empty-state copy button) and patched `mcworker` (`--full`) instead of `--minimal`'s `inky`!
  - `3809461a` T13/T14/T32 (`steps 1521, 1571, 2218`): raised `INK_PREFILL_B` without testing >2K-tok SWA ring-wrap (corrupting system prompt), hardcoded `max_tokens=640` in demo video without inspecting frames, and declared Gym Benchmarks done without bundling `gym-evals/` (`"Clearly you did not test this"`).
- **Fix shipped:** Explicit live E2E verification gates in `AGENTS.md`, `.agents/rules/desktop-app.md`, and `.agents/skills/desktop-ship-verify/SKILL.md`, plus mandatory unflagged core-flow and paired client+host deploy rules.

### 3. Stale Checkouts, Uncommitted Local Installs & Broken Desktop Install/Build Defaults (~25% of Wasted Turns)
- **Evidence:**
  - `00f1bf5a` T03–T06, T09 (5 of 7 wasted turns): worked on a 31-commit-stale checkout, misread `ls -ld OpenAgents.app` mtime as `"Nov 19 build"`, and hand-copied uncommitted local builds into `/Applications/OpenAgents.app` — getting overwritten **twice in 10 minutes** by the 15-min LaunchAgent and a concurrent agent's `install.sh --minimal`.
  - `d9596343` (1-turn clean fix): still lost **5 of 11.7 minutes (43%)** debugging a stale local checkout before inspecting `Info.plist:OACommit`.
  - `apps/openagents-desktop/{build.sh,install.sh,check.sh,release.sh}` still defaulted to `--full`; `install.sh:331` permanently froze `CFBundleVersion` at `28830` while `Window+Settings.swift:358` hid `OACommit`/`OAProfile`; `install.sh` silently exited `0` when deferred (`replies_running`, lock contention); `build.sh --minimal` ran sequentially for **2m 41s – 3m 06s**; `psionic/platform/inkling-build.sh` failed on line 15 in every fresh worktree missing `coax/bin/coax` (`3809461a` steps 1025, 4317, 7623).
- **Fix shipped:** Default `--minimal` across all desktop scripts, monotonic `CFBundleVersion` + `OACommit`/`OAProfile` in Settings > About, non-zero exit `4` on interactive `install.sh` deferral + stale-lock detection + post-launch commit verification, parallelized `build.sh` with C `.o` caching & `--fast` flag, and auto-resolved `COAX` in `inkling-build.sh`.

### 4. Verify/Ship Nudge Tax & Unasked Feature Flags (98 Wasted User Turns)
- **Evidence:** **40** follow-up turns in Antigravity (`14.1%`) and **58** in OpenAgents Desktop (`21.6%`) were pure ship/verify/install nudges (`"did you reinstall it"`, `"merge that to main yourself and rebuild/reinstall now"`, `"commit and push if u havent"`, `"move that from behind the feature flag"` — e.g. `1386dad6` T04–T05, `00f1bf5a` T03–T05, `95fa0f93` T02, `3d791ea4` T9, `61263eac` T11–T12, `b6aeea54` T0).
- **Fix shipped:** Codified default-ship contract and no-unasked-flag-gating rule in `AGENTS.md` top 2 KB + `.agents/rules/00-core-hygiene.md` & `desktop-app.md`.

### 5. Critical Production Bugs in `coax/lib/inky/` (Infinite Loop, `[step 80/30]` Spiral, Silent Empty `CP_DONE`)
- **Evidence:**
  - `coax/lib/inky/loop.coax:297-306`: reset `l[0].retried_stream = false` **4 lines before** checking `&& !l[0].retried_stream` on `done_r == "length"` while suppressing `step += 1` (`retrying_same_step = true`) — infinite loop burning GPU until Stop/kill.
  - `coax/lib/inky/loop.coax:319`: executed model-emitted `<|call:...|>` tool calls even after `step >= max_steps` and with no consecutive duplicate-guard breaker — trapping `worker-1795378556768708.ajr` (`68678b25` T3, 5.13 MB) from `[step 80/30]` through step 86 repeating the exact same blocked `grep` 45+ times, and `worker-1795385915531890.ajr` (`af74c6e1` T3, 3.21 MB) repeating the same blocked `cat`.
  - `coax/lib/inky/loop.coax:337` + `apps/openagents-desktop/page/host-api.coax:304-309`: empty `nreply == 0` saved silently as `state = CP_DONE (1), reply = "", note = ""`.
  - `apps/openagents-desktop/page/persist.coax:172-173`: coerced crashed/abandoned `CP_RUNNING (0)` turns to `CP_STOPPED (3)` with blank `note=""` (10 Desktop turns disguised as manual user stops).
- **Fix shipped:** Fixed `retried_length` infinite loop, enforced `max_steps` hard cap + consecutive duplicate-block termination + duplicate `run_command` breaker in `coax/lib/inky/{loop,guard}.coax`, prevented silent empty `CP_DONE` in `host-api.coax`, and added an explicit interruption note on `CP_RUNNING -> CP_STOPPED` coercion in `persist.coax`.

### 6. Daemon 12-Worker Slot Zombie Leak (`141` `run-stops.log` Entries from 4 Zombies)
- **Evidence:** `141/185` lines in `run-stops.log` (`"left running at launch: busy, though no kept message names it"`) came from just 4 zombie runs (`3d9660d3` 63 launches, `237ea00d` 62 launches, `8965d0a2` 7 launches, `556dc1c4` 7 launches). Cause: `Worker.swift:278-284` and `coder/tui/host-runs.coax:311-319` treated `{"kind":"ready"}` / `journal` / `aware` / 0-byte `.out` as permanently busy; `Window+Runs.swift:294` leaked a newly started worker on cancelled async start; `Window+Runs.swift:347-349` orphaned runs of deleted chats; `Worker.swift:272-276` 64 KiB tail seek broke on `ended` lines >64 KiB; `coax/lib/agent/desktop/worker.coax:170` leaked 22–25 zero-byte `ID.in` files. Exhausted `DR_MAX = 12` (`eb5b603e` T18–T19, `61263eac` T10) and blocked `install.sh`.
- **Fix shipped:** Fixed idle classifier (`ready`/`journal`/`aware`), cancelled async start leak, deleted-chat stop, >64 KiB tail reading, `ID.in` cleanup, and conservative stale-orphan reaper.

### 7. 740 Zero-Byte `worker-*.ajr` Files + Dangling Vertex `model` Slice Use-After-Free (`308` Calls / `101` Journals)
- **Evidence:**
  - `coder/tui/inky.coax:323-343` & `mcworker.coax:376-386` opened+fsynced `worker-*.ajr` before reading stdin, and `apps/openagents-desktop/build.sh` ran `tool.sh inky.coax < /dev/null` on every build — creating **740** zero-byte `.ajr` files (**135 on Nov 22 alone**).
  - `coder/tui/microcoder_keys.coax:253` passed `vm := mk_vertex_model(model)` (a slice into `mcworker`'s per-stdin-line arena `ab`) directly into `gw_plan_add` without copying. On the next stdin line, `ab` was reused, corrupting `a.model` into 13-byte garbage across **308 Vertex calls in 101 `.ajr` journals**, causing `gw_price` to return `-1` and silently zeroing USD spend tracking!
- **Fix shipped:** Lazy `jf_open` on first real turn + unlink 0-sequence journals on exit + `COAX_TOOL_BUILD_ONLY=1` in `tool.sh`/`build.sh`; copied Vertex model string into owned `MkKeys` buffer in `microcoder_keys.coax` with regression `#test`.

### 8. Missing Commit-Time `Agent:` Hooks & Silent Worktree Fallbacks
- **Evidence:** **28** `prepush` rejections (4 on Nov 22 alone) caught missing `Agent: <id>` only after commit/build/rebase/push. Only **29/109 (26.6%)** Desktop chats used worktrees, and `Window+Worktrees.swift:195-199, 219-232` silently fell back to the shared main checkout even when worktrees were enabled (`8500374c` autostash collision; 144 dirty files on main).
- **Fix shipped:** Added `coax/remote/hooks/prepare-commit-msg` (auto-append from `OA_AGENT_ID` / agent env) + `coax/remote/hooks/commit-msg` (<10ms commit-time check), exported `OA_AGENT_ID` from Desktop workers, and removed silent main-checkout fallbacks in `Window+Worktrees.swift`.

### 9. Multi-Hour Silent Hangs (`5h` + `2h 32m`), `manage_task(status)` Polling (`851`x) & Reckless Live Probes
- **Evidence:**
  - `e9088a00` (`steps 520–521`: **5 hours** — `"status????? 5 hours ago what the fuck???"`) and `b5774a4f` T39→T40 (`steps 2953–2956`: **2h 32m** — `"what in the absolute fuck are you doing, sitting ther efor 3 hours?"`) both hung on timeout-less Python verification scripts sending the wrong JSON key with zero read timeout and zero `schedule` watchdog.
  - **851** `manage_task(status)` polling calls + **81** `sleep N` calls across Antigravity sessions.
  - `1386dad6` T07 (`steps 716–733`): running `agy acp` without `SSH_CONNECTION=1` hijacked Chrome with a dead OAuth callback.
- **Fix shipped:** Hard rules in `.agents/rules/00-core-hygiene.md` and `AGENTS.md`: mandatory OS/script timeout + `schedule` watchdog on background verification runs, strict ban on `manage_task(status)`/`sleep` polling, and mandatory `SSH_CONNECTION=1` for `agy`/OAuth probes.

### 10. Context Bloat, Coax Trial-and-Error (`37%` Fail Rate) & Blind Telemetry
- **Evidence:** **9,321** truncated tool outputs (`6,288` `read_file`, `3,020` `run_command` — `1,423` unbounded shell reads/searches); **1,355** redundant same-window overlapping/exact re-reads + **1,009** post-compaction re-reads; **28** compactions in one thread (`3809461a`, 186 post-compaction re-reads); **508** `coax-cli` failures (`37.0%`) concentrated in 10 repeatable error codes; `evals/session-history/` invisible to all 119 Desktop `Coder/chats/*.json` files and lacking a friction scorecard.
- **Fix shipped:** Glob-scoped `.agents/rules/coax-language.md` + `.agents/skills/coax-pitfalls/SKILL.md` + `.agents/skills/session-handoff/SKILL.md`; upgraded `evals/session-history/tools/ingest.coax` (Desktop chats + full Antigravity exit/empty-reply fidelity) and added pure-Coax `evals/session-history/tools/friction.coax` (`evals/session-history/run.sh friction`).

---

## 3. What Made Our Best Sessions Fly (Empirical Patterns)

Deep-diving our 12 cleanest high-output sessions (`767d5084`, `5f3c51f4`, `12d58a9e`, `1447d7a2`, `a76663b7`, `d9596343`, `4707bf52`, `cf574134`, `b648eff9`, `b6aeea54`, `970310fc`, `48c12f81`) revealed 6 concrete differentiators:

1. **Prompt Specificity Halved Rework (Typos Didn't Matter At All):**
   - Prompts `<25` words (`n=18`): **66.7%** rework session rate, **1.94** avg rework turns, **10.7** turns, **1,323** steps.
   - Prompts `>=25` words (`n=19`): **52.6%** rework session rate, **0.89** avg rework turns (**-54%**), **6.7** turns (**-37%**), **907** steps (**-31%**).
   - Prompts containing a file path (`n=14`): **872** avg steps vs **1,255** (**+44%**) without one.
   - Exact quoted UI strings (`"show all lines"` in `b648eff9` even when screenshot couldn't be viewed) and crisp observable thresholds (`≤ 6 lines inline`, `start highlighted` in `970310fc`) enabled 1-shot delivery even with casual typos.
2. **External Memory Ledgers Neutralized Compaction Amnesia:** `767d5084` survived **8 compactions across 5.7 hours (2,734 steps)** with **0 rework** because every sprint logged baseline + delta numbers into persistent `*_hillclimb_log.md` artifacts and `scratch/*.json`, which it immediately re-read after each checkpoint.
3. **Aggressive Parallel Read Fan-Out & Subagent Offloading:** `a76663b7` read 19 docs in 45s via 3–4-wide parallel batches and finished Turn 1 in 50s; `767d5084` and `12d58a9e` offloaded 40-chapter `coax/book/` and `psionic/book/` invariant extraction to `research` subagents while building scratch benchmarks.
4. **Harness-First / Red-Test-First Discipline:** `d9596343` wrote a failing test first, saw red, fixed, saw green; `767d5084`, `12d58a9e`, and `1447d7a2` froze baseline binaries/JSON metrics before Iteration 1; `a76663b7` script-verified every cited path existed on `origin/main` before committing.
5. **Active Multi-Agent Awareness & Disposable Worktrees:** `5f3c51f4` checked `origin/main`, active worktree mtimes, and issue comments first, then burned down **22 issues in 40 commits** with **0 rework** using 1 disposable worktree per issue and APFS-cloned `coax/bin` (`cp -c -R`).
6. **Ground-Truth Inspection on `origin/main` & `Info.plist:OACommit`:** `a76663b7` read `origin/main` directly via `git show` (zero stale-tree friction); `d9596343` resolved its bug the moment it checked installed `Info.plist:OACommit`.

---

## 4. Prompting Playbook (Tailored to Your Real Workflows)

Typos and casual phrasing have **zero** negative effect. Four structural anchors move the needle dramatically: **(a) exact target/quoted UI string or path**, **(b) build flavor & scope (`--minimal`, desktop+mobile)**, **(c) observable done/verification rule**, and **(d) ship action (`commit, push to main, install --minimal` or `open PR`)**.

### Template 1: `/goal` Performance / Quality Hillclimb
> `/goal Hillclimb [metrics: latency, bytes, lag, rework, ...] of [exact target + flavor, e.g. OpenAgents Desktop --minimal / Psionic Metal 4]. Work in a fresh git worktree off origin/main (never edit the main checkout). First delegate reading [canonical book/spec dir] to a research subagent for invariants, build a reproducible baseline harness in scratch/, and track every iteration in a persistent hillclimb_log.md artifact. Feel free to redesign core infrastructure while preserving exact parity. Commit and push each verified milestone to origin/main (with Agent: trailer), and report a before/after scorecard.`

### Template 2: 1-Turn Surgical UI / Bug Fix
> `[Screenshot if visual] "[exact visible UI string]" — [exact observable behavior / threshold, e.g. ≤6 lines inline; focus & select all on open]. Scope: [Desktop --minimal / iOS / both; unflagged core flow]. [Optional ref: ~/work/projects/repos/t3code]. Verify live in the installed app (not just unit tests), add a regression check, commit, push to main, and run install.sh --minimal.`

### Template 3: Architectural Audit & Decision Doc
> `Read all [topic] audits/specs in [dirs] and verify live status against origin/main (not local working tree). Write [doc path] with: (1) shipped today, (2) principled take vs skip criteria, (3) prioritized next steps. Verify every cited file path exists on origin/main, then commit and push via a fresh worktree.`

### Template 4: Multi-Agent Backlog Burn-Down
> `/goal Burn down [issue query / list] to zero. Inspect git log origin/main, active worktrees, and recent issue comments first so you start on untouched items. Use one disposable worktree per issue off origin/main (APFS-clone coax/bin), verify each fix end-to-end against real acceptance criteria, rebase+push to main, close with commit SHA + proof, and delete the worktree.`

### Fast Rule of Thumb for Long Threads
- **Split chats at milestone boundaries or after ~4–6 compactions:** Ask `"Write a 10-line handoff (SHAs, verified checks, open items, pitfalls) and stop"` and start fresh. Single threads past 8–28 compactions (`3809461a`, `3d791ea4`) burn massive cost/latency on re-reads and stale edits.

---

## 5. Friction Hillclimb Tracking Scorecard

Run `evals/session-history/run.sh friction` (introduced alongside this audit) to re-measure these KPIs after every workflow/harness change:

| KPI | 2026-11-22 Baseline | Target | Primary Lever |
| --- | ---: | ---: | --- |
| Antigravity multi-turn rework session rate | `67.7%` (`21/31`) | `< 20%` | Live E2E gate, `AGENTS.md` <24 KB, `.agents/` rules/skills, prompt templates |
| OpenAgents Desktop empty-reply rate | `11.1%` (`43/386`) | `< 1%` | Fix `inky` infinite/step loops, disallow silent empty `CP_DONE`, explicit crash notes |
| Verify/ship user nudge rate (AGY / Desktop) | `14.1%` / `21.6%` | `< 3%` | Top-2KB ship contract, `--minimal` default, non-silent `install.sh` exit `4` |
| Zero-byte `worker-*.ajr` creation rate | `57.5%` (`740/1287`) | `0%` | Lazy `jf_open` + `COAX_TOOL_BUILD_ONLY=1` in `build.sh` |
| Orphaned busy daemon runs (`run-stops.log`) | `141` (`76.2%` of log) | `0` | Fix `isIdleLine`/`dr_idle_line`, async start leak, stale-orphan reaper |
| Missing `Agent:` `prepush` rejections | `28` (4 on Nov 22) | `0` | `prepare-commit-msg` + `commit-msg` git hooks + worker `OA_AGENT_ID` |
| `coax-cli` non-zero command rate | `37.0%` (`508/1372`) | `< 15%` | Glob-scoped `.agents/rules/coax-language.md` + `coax-pitfalls` skill |
| `manage_task(status)` + `sleep` poll calls | `851` + `81` | `0` | Always-on `.agents/rules/00-core-hygiene.md` + `AGENTS.md` top section |
| Desktop `--minimal` incremental rebuild time | `161–186s` | `< 60s` | Cached C `.o` units + parallel tool/swift builds + `--fast` dev mode |
| Multi-hour silent verification hangs | `2` (`5h`, `2.5h`) | `0` | Mandatory OS/script timeouts + `schedule` watchdogs |
