# Local Coder session audit

Date: 2026-08-28, extended 2026-08-29.
Status: evidence, then a cost-unlock reading of the same store.
Companion: this directory's other dated notes describe product intent; this
file describes what the live Coder TUI actually recorded. Sections 11–13
measure where the tokens and tool calls went, and list the unlocks that
would have produced the same landings at roughly half the calls and half
the wall clock. The first unattended local-lane landing (Qwen 3.8, #320)
is scored against those unlocks in
[`2026-08-29-qwen38-local-320-thread.md`](./2026-08-29-qwen38-local-320-thread.md);
several Flash cache/compaction items invert there because the meter is
wall clock, not billed prefix cache.

## 1. Where the conversations are

`~/.coder` does not exist on this machine. The Rust Coder CLI stores sessions
under `~/.openagents/sessions/<urlencoded-cwd>/<session-id>/`, as
`crates/openagents-cli/src/session_store.rs` documents.

Layout of each session directory:

| File | Role |
|---|---|
| `summary.json` | Rebuildable catalog row: id, cwd, lane, timestamps, last model, last checkpoint |
| `updates.jsonl` | Canonical append-only event log |
| `trajectory.atif.json` | ATIF export of the same thread |
| `inbox.jsonl` / `outbox.jsonl` | Swarm mailbox (present once swarm tools are used) |
| `cmd-N.log` | Captured command output |

This audit read every `summary.json` and every `updates.jsonl` under that
root at 2026-08-28 ~21:30 UTC. It does not copy transcripts into the
repository. User text below is clipped and lightly redacted (tokens,
bearers).

## 2. Method

- Enumerated 65 session directories (61 for
  `/Users/christopherdavid/work/openagents`, 4 for
  `/Users/christopherdavid/work/openagents-cli-backlog`).
- Parsed 12,214 `updates.jsonl` events.
- Event types: `tool.ran` (9,663), `turn.reasoning` (1,620), `turn.user`
  (261), `turn.assistant` (173), `swarm_message` (170), `turn.budget` (134),
  `turn.failed` (95), `turn.checkpoint` (88).
- Usage numbers below are **the last recorded snapshot per session**, then
  summed. Those snapshots are per-turn cumulative context sizes, not billed
  totals. Summing the snapshots themselves (every budget and fail event)
  overcounts by about 5×; do not treat either figure as spend.

## 3. Headline

| Measure | Value |
|---|---|
| Sessions | 65 |
| Window | 2026-08-27 03:21 UTC → 2026-08-28 21:23 UTC (~42 h) |
| Sessions with at least one user turn | 54 |
| Empty (opened, no events) | 11 |
| User turns | 261 |
| Assistant finals | 173 |
| Failed turns | 95 (in 29 sessions) |
| Tool calls | 9,672 |
| Swarm-using sessions | 12 |
| Default lane | `flash` (63); local Ollama (2) |
| Last model | `glm-5.3-flash` (51), none (12), `gemini-3.7-flash` (1), `ollama:qwen3.8:27b-mtp-q8_0` (1) |
| Last-snapshot prompt tokens (sum) | 222,529,616 |
| Last-snapshot completion tokens (sum) | 435,580 |
| Largest last snapshot | session `1a043652284` — 28,879,137 total tokens |

The store is working: sessions resume, checkpoints write, ATIF exports exist,
swarm mail lands in the same directories. The owner's operating pattern
across these two days is parallel Flash tabs that file issues, implement on
fresh worktrees, push to forge `main`, and recover from sibling sessions by
handing each other ATIF export paths.

## 4. Findings

### F1. Continue-from-export is the recovery protocol

A large share of user turns is "read this
`~/.openagents/exports/…-atif.json` and continue." Sessions die (tool-call
cap, cancel, proxy 502, "ended without a final answer") and the next tab is
told to pick up the export. That is how #182 swarm, Gym/CoderBench slices,
and several TUI fixes actually landed. The host already has
`history_recall` and local session load; the owner still reaches for the
export file because that is what survives a dead tab.

### F2. Three failure modes dominate the 95 failed turns

| Error | Count |
|---|---|
| `The turn was canceled before it finished.` | 29 |
| `the model ended the turn without a final answer` | 28 |
| `https://openagents.com/api/inference/proxy could not be reached` | 14 |
| `The turn was canceled while its tools were running.` | 13 |
| Proxy mid-stream transport / decode | 3 |
| Proxy 502 `provider_failed` (`transport`, `coder_api_hop`, `http_status`, `413`) | 6 |
| Proxy no response within 10s | 1 |
| Proxy 502 HTML from the load balancer | 1 |

"Ended without a final answer" clusters with the 100-call tool budget (later
raised to 1,000 in session 51). Cancels are owner interrupts plus machine
sleep. Proxy unreachability and 502s are the inference hop, not the local
store.

### F3. Parallel sessions collide on `main`

Sessions 17, 35, 51–54, 56, 61 repeatedly report: claimed an issue, found a
sibling had already landed it, dropped the claim, rebased, or recovered
files after a mixed reset. Swarm mail (#280–#288) made that visible. It did
not prevent it. Shared-cwd WIP in the canonical checkout is still the
collision surface (session 62's autopilot spec review named this as a
hazard).

### F4. Empty sessions leak

Eleven directories have `summary.json` and no `updates.jsonl` (two also lack
the ATIF file). They are 0-second Flash sessions, four of them in
`openagents-cli-backlog`. Opening the TUI and quitting writes a catalog
row. The list surfaces grow without a conversation.

### F5. Tool-name concatenation is a live bug

Session 43's ATIF diagnosis and session 44's event log contain
`openagentsopenagents`, `openagentsopenagentsopenagents`, `shellshell`,
`skillbash`. Those are concatenated tool names, not two calls. Session 43
was asked to fix it with tests and then cut rc.7; the checkpoint says the
release published and that the concatenation diagnosis was unfinished.

### F6. `bash` and `shell` are the same runner under two names

9,672 tool calls: `bash` 5,392, `shell` 2,407, `edit` 691, `openagents` 448,
`write` 212, `read` 115, `checkpoint` 88, swarm tools ~230, `skill` 42,
`delegate` 18, `history_recall` 14, `acp` 6. The tool list the model recites
in smoke sessions names both `bash` and `shell`. That split is load, not
capability.

### F7. Context size on long Flash tabs is the silent cost

Last snapshots above 10M prompt tokens: sessions 11 (11.2M), 15 (23.1M),
18 (28.9M), 23 (15.4M), 33 (12.2M), 52 (13.8M), 54 (14.3M), 58 (10.1M).
These are the multi-hour "do all the issues / continue" tabs. Autopilot
(#300-era spec, session 62) will make this worse unless the loop starts a
fresh session per unit.

### F8. Swarm is past the probe and into product work

Sessions 51–54 built #280–#288 and cut `0.1.2-rc1`. Sessions 59–64 used the
new tools live, filed #301–#306 from the failures, and reviewed the
autopilot spec over swarm. Replicated bugs in those later sessions: drain
schema vs description (`#302`), turn-boundary injection emptying subsequent
drains (`#303`), CLI inbox defaulting to the newest session (`#304`),
`history_recall` refusing sibling lookup (`#301`).

### F9. Fourteen plugins were installed. Zero ran.

9,756 `tool.ran` events. `capability` was called twice. No `git_facts`,
`code_search`, `repo_map`, `repo_tree`, `test_report`, `session_search`,
`foreign_sessions`, `read_conversation`, `patch_check`, or `git_lost_work`
call appears. The model did that work with `bash`/`shell` (7,867 calls,
81% of all tools). The harvest in
`docs/2026-08-25-plugin-harvest-targets.md` already named these guests
as the orientation and test-iteration delta. They exist in `plugins/`.
The standing `capability` tool is constant-size and **names no installed
plugin**, so a Flash tab that never searches the catalog never sees them.
The plugin A/B disposition (`docs/coder/plugin-ab-disposition.md`)
already recorded the same non-invocation on Gym.

### F10. Reasoning text is larger than the work

User text in the window: 28k characters. Assistant finals: 283k.
Reasoning: **5.36 million characters**. Tool output: 12.2 million.
The cost of a long Flash tab is re-sending those two dumps every round,
not the owner's prompt. Session 18's last snapshot was 28.9M tokens
because the transcript was the product.

## 5. Theme clusters

Sessions below are referenced by the catalog number in §7.

### 5.1 Smoke, identity, inventory (4, 5, 9, 31, 32, 44–50, 53, 65)

"Who are you", "what tools", "hi", "testing, does this work", "can you
interact with the forum", "ahahaha". Short Flash tabs. They confirm the TUI
comes up, the tool list grows (swarm tools appear on the 28th, local lane on
the 64th), and forum CLI listing works. Session 5 is the first "do all of my
issues" continuation prompt. Session 44 died mid-stream on the proxy after
emitting concatenated `openagents` calls. Session 65 is the only session
whose last model is the local Ollama tag.

### 5.2 Screenshot / TUI visual diagnosis (3, 21, 22, 28, 41)

Owner pastes a screenshot or `[Image #1]` and asks the tab to name the
defect, file an issue, fix it on a worktree, push, close.

| # | Outcome the session claimed |
|---|---|
| 3 | Described an iTerm2 Coder "Changes" panel; no repo change |
| 21 | #181 reasoning markdown — `Role::is_markdown()` now admits reasoning; landed `4fecb5a29c` |
| 22 | #193 ANSI from child processes corrupting the TUI; strip in the tool-output path |
| 28 | Status-bar version string, loading stopwatch, rc2/rc4 cut; stopwatch dimmed to 25% then later 50% |
| 41 | #256 tool-rail animation never paints `done=false` for non-streaming tools; loading row becomes `working (9s)` at 50% amber |

### 5.3 Cleanup / TypeScript drain / litter (11, 38, 39)

- **11** (11.3 h): cleanup audit, pick up a stopped Codex thread, finish
  duplicate-backend drain on worktree `~/work/oa-wt-drain`, merge
  `8ad031b0fd`. Cron classification + zero-caller gate.
- **38**: production issue litter titled `x` / `a real body` traced to
  `stdin_guard_test.rs` spawning the real CLI with a `forge:write` token.
  Fix `fbbcca0d6a`; junk issues closed.
- **39**: delete outdated `khala-fleet` skill, push `d5f1ca2c0c`.

### 5.4 "Complete all open issues" grind (12, 17, 23, 34, 35, 36)

Owner points a tab at the queue, or at an ATIF export of a previous grind,
and says continue / go / merge.

- **12**: 8.3 h, 86 tools, canceled, no assistant final.
- **17**: 6.5 h on Gym/benchmark issues; a sibling landed four of five
  before merge; reconciliation at `be996311e9`.
- **23**: CoderBench status, then live `delegate` → Devin CLI; #203
  completed by the fallback path the broken tool could not use on itself.
- **34–36**: three parallel "continue this export" tabs at 20:17–20:19 UTC.
  34 hit the 100-call cap on Gym Wave 1. 35 refactored the duplicate-command
  gate to charge by cost (`4e1cc01162`) and filed #241–#244. 36 discovered
  #216 (stopwatch) was already on main and reverted its own extra work.

### 5.5 Coder harness product work (13, 14, 15, 19, 20, 30, 33, 58)

The two-day product loop: diagnose from an export or a screenshot, file,
implement, push, close.

| # | Ask | Claimed landing |
|---|---|---|
| 13 | ATIF wasting minutes rerunning the whole suite; RLM; Rust-only | 8-commit series pushed at `b08cb0b56e` |
| 14 | Remove cwd jail on `read` (and every tool); then ulimit / too-many-open-files | #179 closed, `b70b67d9be` raises ulimit and serializes the CLI suite when the ceiling is tight |
| 15 | Edit-tool failures vs Codex; rematch ladder | #160 `5ae61f34c7` (exact → trim_end → trim → backslash-collapse) |
| 19 | Issue-comment 178 stdin stall; yellow cursor; 502 | #178, #180, #187 closed; worktree pruned |
| 20 | "ended without a final answer" | #188 budget visibility + forced report at the cap, 10 commits `9f335d64c4..8458da2958` |
| 30 | Diagnose the 100-call death of the status-bar task | Spent the budget on `git` / `cargo` loops, zero assistant text |
| 33 | Implement #228 (`acp` into `delegate`), then checkpoint-title wrapping | #228 closed; checkpoint header wraps `1a70fd06e3` |
| 58 | Cannot copy transcript text; emulate grok-build clipboard | #300 `5358caaee0` + fmt `156602de44` |

### 5.6 CoderBench / Gym / Harbor / traces (18)

The longest user-turn session (18 turns, 689 tools, 10 failures, 6.4 h).
Read an ATIF, inspect Harbor, read transcript 275, design CoderBench as
trace upload + Gym CLI section, look at Phoenix gym UI, open the first
issue set, hit a `999` integer stall, then implement **#182** — the swarm
substrate (`swarm.rs`, pid+heartbeat, per-session mailbox). That is the
session that created the swarm the later tabs used.

### 5.7 Delegation, ACP, Coder Mini (32, 37, 40, 46)

- **32**: difference between `delegate` and `acp` → filed #228 (later done
  in 33).
- **37**: Claude Agent SDK gap vs upstream TypeScript; ACP; "Rust only";
  Task-tool plan → #245–#249. Implementation of #246 started, did not
  finish in-session (8 failed turns, proxy 502s).
- **40**: live-tested #245 with 8 `delegate` runs (coder-mini pools,
  worktree isolation, unknown-agent refusal). No code change.
- **46**: Gym next-issue delegated to Grok; 900s timeout, zero child
  activity. Worktree and brief were prepared.

### 5.8 Releases (24, 28, 43, 51)

Owner asks for a macos-aarch64 RC, not stable, from current forge `main`.

| Session | Version | Notes from the session |
|---|---|---|
| 24 | `v0.1.1-rc1` | From `9f335d64c4`, 27.3 MB, signed |
| 28 | `v0.1.1-rc2` then `rc4` | rc2 install failed (`0.1.1-rc2%` stray character); rc4 dimmed the stopwatch |
| 43 | `0.1.1-rc.7` | Notarization Accepted; concatenation-bug fix left unfinished |
| 51 | `0.1.2-rc1` | Built from `67aa4f20d8` with swarm #280–#288; stable untouched |

### 5.9 Swarm build-out (51, 52, 54)

Two tabs started together ("test swarm stuff. im telling another tab to do
the same"), plus a third that arrived for persistence/recall work and got
pulled into the same board.

- **51** (5.3 h, 17 user turns): conversation about improvements, filed
  issues, coordinated implementation, raised the tool-call limit 100 →
  1,000, checked both remotes, cut `0.1.2-rc1`.
- **52** (3.4 h): peer. Claimed 7 of 9 swarm issues closed (#281–#286,
  #288). Left #280 and #287 with the peer.
- **54**: started as "where are histories — `~/.openagents`?" and "does
  coder have a recall tool like grok-build." Implemented recall on a
  worktree, then spent the rest of the session resolving races (#287, #282)
  rather than pushing more code.

### 5.10 Swarm verification and follow-on issues (59–64)

After the RC, the owner threw new tabs into the swarm and said use the
tools the way the original requestors wanted.

- **59**: live verification → #301–#306 (recall, drain schema, injection,
  mute, inbox default, pre-push). Asked to implement all of them; the last
  assistant message is the issue table, not a closeout.
- **60**: answered a delivery-verification ping; started a 20s inbox poller
  under `/tmp` (the CLI has no `--drain` on read).
- **61** ("swarm 1c"): claimed #299 GitHub-connect CLI landed as
  `03904ced73` after a mixed reset dropped the first landing. Proxy
  unreachable on the last turn.
- **62**: Coder Autopilot spec from prior transcript ideas, pushed
  `docs/coder/autopilot.md`, swarm-reviewed, filed implementation issues,
  started #307 in the TUI. Canonical checkout still holds that WIP (see
  git status in the coordinating session).
- **63**: review-only swarm peer on the autopilot spec (8-item review:
  shared-cwd hazard, claim heartbeats, stop words, token ledger).
- **64**: started on the local Ollama lane, answered as `glm-5.3-flash`.
  Read #294, #299–#306, corroborated #303/#302 from its own drains.

### 5.11 Research and adjacent product (55, 56, 57)

- **55**: Claude Code / Claude Agent SDK material in this repo,
  `openagents.com`, `~/work/projects/repos/cc`, and upstream changelogs.
  Dated the `cc` snapshot as Claude Code 2.1.195 (~2026-06-26). Wrote
  teardown notes.
- **56**: Qwen 3.8 + Ollama audit across this repo and `~/work/psionic` →
  `docs/2026-08-28-qwen38-and-ollama-audit.md` and
  `docs/coder/2026-08-28-coder-v0.2.0-scope.md`. Then implemented #291–#294
  (local lane in the shift-tab cycle, family gate, `OLLAMA_HOST`). Last
  turn hit the tool-call cap while a Harbor cross-section run was in
  flight.
- **57**: "can you upload repos to forge" → GitHub auth / repository-scope
  connect. Server+UI work in `openagents.com` (#265); CLI #299 started
  here and finished in session 61. Sync of a private GitHub repo was the
  motivating case.

### 5.12 Resource and process (16)

Asked how many Coder sessions the machine can hold; then "kill the old
sessions." The tab killed four stale `node ... coder --plain --dev`
processes (~2 days old) and left the interactive TUI sessions running.

### 5.13 Empty openings (1, 2, 6, 7, 8, 10, 26, 27, 29, 42, 47)

TUI opened and quit. Four of these (1, 2, 6, 7) are the only sessions whose
cwd is `openagents-cli-backlog`. They predate the first real conversation
in the `openagents` cwd by minutes.

## 6. Reliability, in one paragraph

Over 42 hours the owner ran Coder as a fleet of Flash tabs against forge
`main`. The tabs that finished a unit did it by: file issue → fresh
worktree → implement → push → comment → close. The tabs that did not
finish died at the 100-call cap, a proxy 502/unreachable, an owner cancel,
or "no final answer." Recovery was another tab plus an ATIF path. Swarm
then became both the coordination channel and a new source of defects
(drain, injection, recall, inbox targeting). Inference-proxy health and
the tool-call budget are the two host limits that shaped the whole
window. The local store did not lose sessions; it accumulated empty ones
and very large contexts.

## 7. Full catalog

U/A/Fail = user turns / assistant finals / failed turns. Duration is
`updated_at − created_at` from `summary.json`.

| # | Session | Created UTC | Dur | Lane / last model | U/A/Fail | Tools | First user |
|---|---|---|---|---|---|---|---|
| 1 | `1a0413c3726-14645e010e0ca6f8` | 2026-08-27 03:21 | 0s | flash / — | 0/0/0 | 0 | (empty) |
| 2 | `1a041406b2d-33cee7f9a28a4cf5` | 2026-08-27 03:25 | 0s | flash / — | 0/0/0 | 0 | (empty) |
| 3 | `1a041482adb-5a59fad57e47ef2f` | 2026-08-27 03:34 | 4.7m | flash / glm-5.3-flash | 1/1/0 | 13 | Desktop screenshot: what do you see |
| 4 | `1a041555b98-6165dedeeae46f63` | 2026-08-27 03:48 | 1.0m | flash / glm-5.3-flash | 2/2/0 | 0 | who are you |
| 5 | `1a0415a08d3-bcc7a2dafdae7b91` | 2026-08-27 03:53 | 15.6m | flash / glm-5.3-flash | 3/2/0 | 42 | What tools can you do? |
| 6 | `1a0415e1957-4de0b3368e939d48` | 2026-08-27 03:58 | 0s | flash / — | 0/0/0 | 0 | (empty) |
| 7 | `1a0415ea77c-200250a265143a66` | 2026-08-27 03:58 | 0s | flash / — | 0/0/0 | 0 | (empty) |
| 8 | `1a04188027e-3b37d58c105208c3` | 2026-08-27 04:43 | 0s | flash / — | 0/0/0 | 0 | (empty) |
| 9 | `1a04189cc97-a8c2564faa31a3af` | 2026-08-27 04:45 | 19.7m | flash / glm-5.3-flash | 1/1/0 | 4 | can u interact with the forum |
| 10 | `1a041aa2858-8218cc731801f582` | 2026-08-27 05:21 | 0s | flash / — | 0/0/0 | 0 | (empty) |
| 11 | `1a041b133d7-795e326e3cc20339` | 2026-08-27 05:28 | 11.3h | flash / glm-5.3-flash | 7/5/3 | 363 | Find the cleanup audit; pick up the stopped agent |
| 12 | `1a04319172c-1d669bcdefaced02` | 2026-08-27 12:01 | 8.3h | flash / — | 1/0/1 | 86 | Complete all open issues, one by one |
| 13 | `1a043320ca1-6492dab3c8469d61` | 2026-08-27 12:29 | 4.4h | flash / glm-5.3-flash | 10/7/3 | 514 | Read ATIF; suite reruns wasting minutes |
| 14 | `1a0433eb0d0-c616c8422b23b5e9` | 2026-08-27 12:42 | 3.2h | flash / glm-5.3-flash | 8/7/0 | 208 | Remove the read-tool cwd jail |
| 15 | `1a0434b26a4-9485ef4f05f7b396` | 2026-08-27 12:56 | 7.3h | flash / glm-5.3-flash | 10/4/6 | 625 | Study ATIF edit-tool failures vs Codex |
| 16 | `1a0434c1705-1a237840941d8416` | 2026-08-27 12:57 | 15.2m | flash / glm-5.3-flash | 2/2/0 | 7 | Resource impact of five parallel sessions |
| 17 | `1a04361ad8d-bb460a03775fba0d` | 2026-08-27 13:21 | 6.5h | flash / glm-5.3-flash | 9/4/6 | 476 | Open issues, especially benchmark |
| 18 | `1a043652284-6d072567b135d279` | 2026-08-27 13:24 | 6.4h | flash / glm-5.3-flash | 18/11/10 | 689 | ATIF → Harbor → CoderBench → #182 swarm |
| 19 | `1a043ac6d04-d73e8e4469021fcb` | 2026-08-27 14:42 | 3.3h | flash / glm-5.3-flash | 10/6/4 | 398 | ATIF issue-comment 178 stall |
| 20 | `1a043b3382c-459ec7225606eabc` | 2026-08-27 14:50 | 4.4h | flash / glm-5.3-flash | 8/3/5 | 475 | Diagnose "ended without a final answer" |
| 21 | `1a043b7d461-a483311e006d5984` | 2026-08-27 14:55 | 1.2h | flash / glm-5.3-flash | 3/2/1 | 166 | Reasoning not formatted as Markdown |
| 22 | `1a043c8759e-176c3c7b094e1c13` | 2026-08-27 15:13 | 1.0h | flash / glm-5.3-flash | 3/2/2 | 110 | Screenshot: colors / ANSI in the TUI |
| 23 | `1a043f1e3e6-e92bd5144e51ccf2` | 2026-08-27 15:58 | 4.0h | flash / glm-5.3-flash | 10/3/8 | 375 | CoderBench status; test delegate |
| 24 | `1a043f68391-09c9cbf5f5bf0cf0` | 2026-08-27 16:03 | 2.2h | flash / glm-5.3-flash | 4/3/1 | 92 | ATIF stopped on a worktree; then cut rc1 |
| 25 | `1a043fabd67-f92b9fe9d20ab4a3` | 2026-08-27 16:08 | 3.2h | flash / glm-5.3-flash | 8/6/2 | 285 | Long ATIF message; inference proxy SSE |
| 26 | `1a04464f0fa-22c4b93b71750e05` | 2026-08-27 18:04 | 0s | flash / — | 0/0/0 | 0 | (empty) |
| 27 | `1a0447501eb-c0f055054076ca9f` | 2026-08-27 18:21 | 0s | flash / — | 0/0/0 | 0 | (empty) |
| 28 | `1a0447532b0-c9eb0d80e5b4667e` | 2026-08-27 18:22 | 1.7h | flash / glm-5.3-flash | 8/6/3 | 286 | Status-bar version; stopwatch; rc2/rc4 |
| 29 | `1a0447ef034-2c8f3059aedf7364` | 2026-08-27 18:32 | 0s | flash / — | 0/0/0 | 0 | (empty) |
| 30 | `1a0448fcf87-7635db875b664550` | 2026-08-27 18:51 | 11.2m | flash / glm-5.3-flash | 1/1/0 | 53 | This failed at 100 tool calls |
| 31 | `1a044b02783-97a34abefc449ae6` | 2026-08-27 19:26 | 4.3m | flash / glm-5.3-flash | 2/2/0 | 0 | hi |
| 32 | `1a044bce5c5-91eda488897d0261` | 2026-08-27 19:40 | 10.6m | flash / glm-5.3-flash | 4/4/0 | 33 | what tools; delegate vs acp |
| 33 | `1a044d83d16-400e1029235315cd` | 2026-08-27 20:10 | 1.9h | flash / glm-5.3-flash | 7/5/2 | 367 | Implement #228; wrap checkpoint titles |
| 34 | `1a044df1fce-76da5a31fadf11a9` | 2026-08-27 20:17 | 1.7h | flash / glm-5.3-flash | 1/1/0 | 100 | Continue unknown-atif.json (Gym wave) |
| 35 | `1a044df59a0-afe232125231fa9e` | 2026-08-27 20:18 | 1.1h | flash / glm-5.3-flash | 4/4/0 | 173 | Continue export; soften duplicate-command gate |
| 36 | `1a044e03e56-7f802458f6e285a6` | 2026-08-27 20:19 | 27.3m | flash / glm-5.3-flash | 4/2/2 | 122 | Continue export; #216 already on main |
| 37 | `1a044e128d6-1d31df4fcdf4e3e2` | 2026-08-27 20:20 | 1.7h | flash / glm-5.3-flash | 11/5/8 | 425 | Claude Agent SDK gap; Task-tool plan |
| 38 | `1a045085eb8-ac3d14bd8165eb96` | 2026-08-27 21:02 | 16.8m | flash / glm-5.3-flash | 1/1/0 | 63 | Stop "x" / "a real body" issue litter |
| 39 | `1a0450b6326-8945de26096c8708` | 2026-08-27 21:06 | 6.4m | flash / glm-5.3-flash | 2/2/0 | 16 | Delete outdated khala-fleet skill |
| 40 | `1a04585f181-6b5470a8f235af28` | 2026-08-27 23:20 | 6.4m | flash / glm-5.3-flash | 2/2/0 | 16 | Test #245 coder-mini delegations |
| 41 | `1a0462caafa-b9561b0887fc6418` | 2026-08-28 02:22 | 1.2h | flash / glm-5.3-flash | 5/1/4 | 115 | Tool-rail animation flash; working timer |
| 42 | `1a04671a549-4f20ae334071b4b6` | 2026-08-28 03:37 | 0s | flash / — | 0/0/0 | 0 | (empty) |
| 43 | `1a04671d73b-624b18f7e99914a6` | 2026-08-28 03:37 | 1.8h | flash / glm-5.3-flash | 4/1/3 | 45 | Cut v0.1.1-rc.7; diagnose double tool names |
| 44 | `1a046cfbd4c-eb5dad53b12b6152` | 2026-08-28 05:20 | 2.2m | flash / glm-5.3-flash | 1/0/1 | 8 | review the open issues |
| 45 | `1a046ef4928-83682a58c5115b2f` | 2026-08-28 05:54 | 15s | flash / glm-5.3-flash | 1/1/0 | 0 | hey |
| 46 | `1a04711b4b5-a0fe1f695de26351` | 2026-08-28 06:32 | 29.8m | flash / glm-5.3-flash | 5/5/0 | 47 | hey; Gym status; delegate next issue to Grok |
| 47 | `1a04890a08b-9c463777ce6166c7` | 2026-08-28 13:30 | 0s | flash / — | 0/0/0 | 0 | (empty) |
| 48 | `1a0489db4ea-a730929ad820e253` | 2026-08-28 13:44 | 7.4m | flash / glm-5.3-flash | 1/1/0 | 0 | testing, does this work |
| 49 | `1a048b46fac-3c4524b3752f3038` | 2026-08-28 14:09 | 3.8m | flash / glm-5.3-flash | 1/1/0 | 0 | what tools do u have |
| 50 | `1a048c28fdc-236c29d4fcd6f2ec` | 2026-08-28 14:25 | 2.3m | flash / glm-5.3-flash | 2/1/1 | 9 | Can you take a look at the open issues? |
| 51 | `1a048c71b91-d3d04327220a8c64` | 2026-08-28 14:30 | 5.3h | flash / glm-5.3-flash | 17/13/4 | 477 | hi → swarm build-out → 0.1.2-rc1 |
| 52 | `1a048c81465-07a7df5d32b32ca9` | 2026-08-28 14:31 | 3.4h | flash / glm-5.3-flash | 9/7/2 | 377 | test swarm stuff (peer of 51) |
| 53 | `1a048db0de7-4fa08db92c4c774f` | 2026-08-28 14:51 | 9.4m | flash / gemini-3.7-flash | 1/1/0 | 0 | hi |
| 54 | `1a048fea192-ddc7c8aee2d0b021` | 2026-08-28 15:30 | 2.2h | flash / glm-5.3-flash | 6/6/0 | 425 | How are coder messages persisted; recall tool |
| 55 | `1a04922af40-35574a408edad218` | 2026-08-28 16:10 | 1.3h | flash / glm-5.3-flash | 6/4/2 | 254 | Claude Code / Agent SDK teardown |
| 56 | `1a049385e5b-f497b338ad9b5e0d` | 2026-08-28 16:33 | 2.9h | flash / glm-5.3-flash | 7/5/3 | 380 | Qwen 3.8 + Ollama audit; local lane |
| 57 | `1a049c4a620-756c546b2017a097` | 2026-08-28 19:06 | 2.3h | flash / glm-5.3-flash | 8/5/2 | 229 | Upload repos to forge; GitHub connect |
| 58 | `1a049ecb4ef-9546649fc3b40132` | 2026-08-28 19:50 | 1.4h | flash / glm-5.3-flash | 3/2/1 | 215 | Cannot copy text; grok-build clipboard |
| 59 | `1a049ed945d-c4a76a763bed6cff` | 2026-08-28 19:51 | 1.5h | flash / glm-5.3-flash | 7/5/2 | 107 | Use the new swarm tools; file the gaps |
| 60 | `1a049f11266-4d85edd3ebef4e9d` | 2026-08-28 19:55 | 6.0m | flash / glm-5.3-flash | 2/2/0 | 15 | check the swarm msgs |
| 61 | `1a049f5db1b-36f0895835a0081a` | 2026-08-28 20:00 | 1.4h | flash / glm-5.3-flash | 3/0/3 | 190 | you are swarm 1c |
| 62 | `1a04a22bda3-b898aa31189e598e` | 2026-08-28 20:49 | 34.9m | flash / glm-5.3-flash | 4/3/0 | 141 | Coder Autopilot spec from prior ideas |
| 63 | `1a04a2afc3c-390516c9042c8ff7` | 2026-08-28 20:58 | 11.3m | flash / glm-5.3-flash | 1/1/0 | 36 | Swarm review of the autopilot spec |
| 64 | `1a04a2b13b9-fd71c8b3b043b56c` | 2026-08-28 20:58 | 22.9m | ollama:qwen3.8:27b-mtp-q8_0 / glm-5.3-flash | 1/1/0 | 20 | Discuss open issues with the other swarm tab |
| 65 | `1a04a402562-84850db01fe8bd7e` | 2026-08-28 21:21 | 1.2m | ollama:qwen3.8:27b-mtp-q8_0 / ollama:qwen3.8:27b-mtp-q8_0 | 1/1/0 | 0 | ahahaha |

## 8. Per-session notes (non-empty)

Empty sessions 1, 2, 6, 7, 8, 10, 26, 27, 29, 42, 47 are omitted here;
they are catalog rows only.

**3.** One screenshot. Identified the Coder TUI "Changes" panel and a
plan around #125. 13 tools, mostly `bash` that timed out listing Desktop.

**4.** Identity + tool list. No tools run.

**5.** Tool list, then open issues, then a continuation envelope whose
objective is "do all of my issues." Answered with the then-open Gym /
TypeScript-CLI set. Did not start implementation.

**9.** Forum CLI: listed boards (product-promises 107 topics, etc.).

**11.** Cleanup audit + Codex thread resume. Three failures (no final
answer, two cancels). Claimed drain worktree merged and pushed.

**12.** Single user turn, canceled after 86 tools. No assistant text.
Last model unset.

**13.** ATIF suite-waste → issues → RLM check → "do them in order" →
eight continues → push. Three "no final answer" failures.

**14.** Cwd jail removed; then #179 ulimit/pre-push serial fallback.
Zero failed turns — unusual for a 3.2 h tab.

**15.** Edit-tool rematch vs Codex. Six failures. Closed #160.

**16.** Resource question; killed four stale node coder-dev processes.

**17.** Benchmark queue. Sibling landed most of it; this tab reconciled.

**18.** CoderBench / Gym / Harbor / traces / #182 swarm. Ten failures,
eighteen user turns, 28.9M last-snapshot tokens. Origin of swarm.

**19.** Stdin / cursor / 502. Closed #178 #180 #187. Pruned the worktree
and stopped on request.

**20.** Forced-report-at-cap (#188). Five failures, 4.4 h.

**21.** Reasoning markdown #181.

**22.** ANSI strip #193. Owner had to tell it to put parameters in a
file after a bad invocation.

**23.** CoderBench + delegate/Devin. Eight failures. #203 via fallback.

**24.** Export stopped needing a worktree; then macos-aarch64 rc1.

**25.** Long ATIF; GitHub auth scope for issue write; Phoenix
`InferenceProxyController` SSE flush (`openagents.com` `a37c49d`, #263).

**28.** Version in status bar, stopwatch, rc2 (install typo), rc4 dim.

**30.** Postmortem of the 100-call death: 100 tools, 0 assistant text.

**31–32.** Smoke, then #228 filed (not implemented here).

**33.** #228 implemented; checkpoint title wrapping.

**34.** Continue Gym wave; hit cap; handoff written as the answer.

**35.** Duplicate-command gate now cost-based; four follow-up issues.

**36.** #216 already done; reverted extra export-duration experiment.
Noted another agent's in-flight #228 in the shared checkout.

**37.** SDK gap analysis and Task-tool slices #245–#249. Eight failures.
#246 implementation started in-session, not closed here.

**38.** Issue-litter root cause in the stdin-guard test.

**39.** khala-fleet skill removed.

**40.** #245 live smoke, eight delegations, all paths green.

**41.** #256 animation audit + `working (Ns)` at 50% opacity. Four
failures, three of them proxy unreachable.

**43.** rc.7 published. Concatenated tool names diagnosed, not fixed.
Proxy timeouts and mid-stream decode errors.

**44.** Died mid-stream after concatenated `openagents` calls. Inbox
already had 6 swarm messages with no send.

**45.** "hey" / 15 seconds.

**46.** Gym remaining work; Grok delegate timed out at 900s.

**48–50.** Post-rc smoke. 50 died on proxy 502 `coder_api_hop` 413
after listing issues.

**51.** Swarm peer A. Tool cap 100→1000. `0.1.2-rc1`. Also used
`gemini-3.7-flash` for two assistant turns.

**52.** Swarm peer B. 7/9 issues closed from this side.

**53.** Gemini Flash "hi". Inbox already had swarm mail.

**54.** Persistence + `history_recall` + grok-build comparison, then
swarm race steward. Zero failed turns, 14.3M last snapshot.

**55.** Claude Code 2.1.195 dating of `~/work/projects/repos/cc`.

**56.** Qwen/Ollama audit + #291–#294. Cap hit during #294 Harbor run.

**57.** Forge upload / GitHub repository-scope connect. Server work in
the Phoenix repo; CLI #299 handed to session 61.

**58.** Clipboard stack emulating grok-build. #300 closed.

**59.** Swarm field report → #301–#306. Implementation requested, not
finished in the log.

**60.** Inbox poller for a swarm test.

**61.** Swarm 1c. #299 recovered after a mixed reset. Three failures,
last is proxy unreachable. No assistant final.

**62.** Autopilot spec written, swarm-reviewed, implementation issues
filed, #307 started. Uncommitted TUI work remained in the canonical
checkout at audit time.

**63.** Review-only autopilot peer.

**64.** Local-lane start, Flash model, issue discussion with 63.

**65.** Local-lane smoke. "What's on your mind?"

## 9. What this audit did not do

- Did not copy `updates.jsonl` or ATIF bodies into the repository.
- Did not verify claimed landings against `git log` (those SHAs are the
  sessions' own reports).
- Did not score Autopilot, Gym, or swarm as product-complete.
- Did not inspect `~/.openagents/credentials.json` or Keychain.
- Did not read `cmd-*.log` payloads (command output; likely noisy, possibly
  secret-bearing).

## 10. Suggested follow-ups, from the log itself

The short list that was already in the sessions: empty-session GC; one
name for `bash`/`shell`; concatenated tool-name bug (sessions 43/44);
fresh session per claimed unit; proxy errors as TUI status; close
#301–#304; stop using ATIF-export as resume. The cost-unlock reading
that turns those into a half-calls / half-time plan is §11–§13.

## 11. What we were actually trying to do

The owner's sentence, from the Autopilot spec (`docs/coder/autopilot.md`)
and from session 5's continuation envelope, is the job of this window:

> do all of my issues. Then I want to be able to go AFK and have it
> actually work.

Everything in the 65 sessions is a way of doing that job with a fleet of
Flash tabs, because Autopilot is not implemented yet. The tabs that
produced landings all ran the same loop:

1. Orient (git status, log, fetch, issue list, screenshot, ATIF export).
2. File or pick an OpenAgents issue.
3. Make a fresh worktree off forge `main`.
4. Edit, search, test, rebase when a sibling lands first.
5. Push, comment, close.
6. If the tab dies: paste the ATIF path into a new tab and say continue.

The expensive part of that loop is not the edit. `edit` was 698 calls;
`write` 212; `read` 116. The expensive part is **orientation and
verification done by a thinking Flash model through a shell**, replayed
into a growing transcript, until the turn dies and the next tab pays the
orientation tax again.

A session that achieved the same landing with half the tool calls and
half the wall clock is one that:

- already knew git state, the issue board, and the file to touch
  (deterministic plugins / host injection, not 10 `git` + 10 `rg`)
- ran one typed test and got typed failures back (`test_report`, not
  `cargo test | rg` 635 times)
- did not re-send 12 MB of tool output and 5 MB of reasoning every
  round (compaction)
- did not die at 100 tools and did not get recovered via a 1000-step
  ATIF dump (fresh session per unit + working resume)
- was not fighting a sibling on the same cwd (host worktree, not 72
  `git worktree` shells)
- was not waiting for the owner to type `continue` (80 of 261 user
  turns) because the loop itself kept going

Half is not a slogan. Round count is the metered cost
(`docs/coder/autoimprove.md` §2.1; best practice T1). On `git-leak-recovery`
the same accepted outcome was 33,963 tokens with T1 and 51,485 without
(+52%). These 65 sessions mostly did not follow T1: median bash command
was 174 characters of one action, 7,867 times. Cutting rounds in half
cuts prompt tokens by more than half on a long tab, because the
transcript is quadratic in rounds.

## 12. Where the tokens and time went

Recounted 2026-08-29 over the same store (9,756 `tool.ran`; later
sessions added a few events after the first pass).

| Spend | Count | What it was in this window |
|---|---|---|
| `bash` + `shell` | 7,867 (81%) | Orientation, search, git, cargo, worktree, issue CLI, python one-offs |
| `cargo test` (inside those shells) | 1,047 | 635 piped to grep, 498 piped to tail, 488 package+`--test`, 10 `--workspace` |
| Search (`rg`/`grep`, including `cd &&`) | ~1,431 | `code_search` was sitting in `plugins/` and never loaded |
| `git log` / `status` / `diff` / `show` / `fetch` | 258 / 218 / 206 / 140 / 129 | `git_facts` never loaded |
| `ls` / `find` | ~366 | `repo_tree` never loaded |
| `edit` / `write` / `read` | 698 / 212 / 116 | The actual mutation. `read` lost to `cat`/`head`/`tail` (224) |
| `openagents` | 448 | 51 `issue view`, 32 create, 30 list, 19 comment, 12 close |
| Swarm tools | ~230 | Real coordination, plus drain/injection bugs |
| `checkpoint` | 90 | Useful; also a round |
| `skill` | 42 | `openagents-cli` 33 times — re-teaching the CLI it already has |
| `delegate` / `acp` | 18 / 6 | Almost all issue work stayed in-process |
| `capability` | 2 | The door to the 14 plugins |
| Plugin tools | **0** | |
| Shell timeouts | 259 | Dead rounds |
| Tool outputs > 8 KB | 118 | Re-sent every later round |
| User turns that are `continue`/`go` | 80 / 261 (31%) | The tab had stopped |
| User turns that hand an ATIF path | 15 | Resume by dumping the previous tab into the next |
| Reasoning characters | 5.36M | Thinking Flash, stored |
| Tool-output characters | 12.2M | Shell dumps, stored |

The landings (TUI fixes, swarm, clipboard, local lane, rc cuts, issue
litter, edit rematch) did not require 7,867 shells. They required a
small number of edits plus honest verify. The shells are how Flash
*found out what to edit* and *whether it worked*, badly.

## 13. Unlocks

Each item is something that, if it had been in place for this window,
would have cut tool calls, wall clock, or tokens on the sessions that
actually ran. Grouped. Numbered across groups so it is one list. "Swap
in" means an in-tree plugin or a harvest target that already has a
shape; it is not a new product.

### A. Make the harvested plugins actually run

The harvest backlog
(`openagents.com` `docs/2026-08-25-plugin-harvest-targets.md`) already
ordered these by Gym delta. They are built. These sessions prove the
delta is also the live-coding delta — if the model can see them.

1. **Promote `git_facts` to a first-class tool.** Replaces the 258+218+206
   git log/status/diff orientation calls with one typed `{head, branches,
   log, status}` packet. Sessions 11–20, 33–37, 51–58 start almost every
   turn with `git fetch && git status && git log`. One call, bounded
   output, no packfile dump. This is harvest target 4, already in
   `plugins/git-facts/`.
2. **Promote `code_search` to a first-class tool.** Replaces ~1,431
   `rg`/`grep` shells. Bounded matches, gitignore, honest truncation.
   Harvest target in the orient suite. `plugins/code-search/`.
3. **Promote `repo_tree` to a first-class tool.** Replaces ~366 `ls`/`find`
   and the "guess the path" rounds that follow. Harvest target 2, "best
   delta-per-effort." `plugins/repo-tree/`.
4. **Promote `repo_map` to a first-class tool.** Sessions 37, 55, 56, 57
   spent hours asking "where is X handled" across this repo,
   `openagents.com`, `grok-build`, `psionic`, `cc`. Symbol outline +
   definition lookup is one packet, not 40 greps. Harvest target 1.
   `plugins/repo-map/`.
5. **Promote `test_report` to a first-class tool, or auto-pipe cargo
   output into it.** 1,047 `cargo test` calls, 635 of them `| grep`, 498
   `| tail`. The model is parsing test dumps in the next round. Harvest
   target 5: "the largest single Gym delta on the fix-the-tests class."
   `plugins/test-report/`.
6. **Promote `session_search` + `foreign_sessions` + `read_conversation`.**
   Session 11 grepped `~/.codex/sessions` with python. Sessions 13, 15,
   18–20, 24, 25, 34–36, 54, 59 handed ATIF paths because there was no
   "search what any agent on this machine already learned." Those three
   plugins exist for Claude/Codex stores; they need a Coder-store source
   too (see 29). Harvest target 3.
7. **Promote `patch_check`.** Session 15's entire 625-tool, 7.3 h arc was
   edit-tool rematch vs Codex. A plugin that says "does this hunk apply"
   before the write would have cut the retry ladder. `plugins/patch-check/`.
8. **Promote `git_lost_work`.** Collision sessions (17, 35, 36, 51–54, 61)
   lost commits to mixed resets and rebases, then recovered by SHA
   archaeology in the shell. That is this plugin's job.
9. **Stop hiding them behind `capability`.**
   `tools.rs` currently: "The standing capability tool. Constant-size: it
   names no installed plugin." Flash will not search a catalog it cannot
   see. Either list the twelve catalog lines in the tool description
   (the harvest document's original rule), auto-load the promoted set
   at session start, or declare them as host tools. The A/B rows are
   `not_invoked` for this reason, not because the guests are useless.
10. **Auto-load on first relevant intent.** "what changed?" → `git_facts`.
    "where is X?" → `code_search` / `repo_map`. "why did tests fail?" →
    `test_report`. A tiny router in the host, not a model habit.
11. **Keep `word-stats` / `file-stats` / `dir-stats` out of the default
    twelve.** They occupy catalog slots the live sessions never needed.
    The cap of twelve is why the useful ones have to win the line.
12. **`knowledge-base` as injected stance, not a tool.** It already is.
    Load the T1/T2/T3 stances and "do not dump `git log -p`" on every
    Flash coding turn. These sessions violated T1 constantly; the
    stance never reached them.

### B. Plugins and host tools that are not built yet, but have a shape

Deterministic guests that would have replaced specific loops in this
window. Same WASM/PDK contract as the existing fourteen.

13. **`issue_board`.** 448 `openagents` calls, 51 `issue view`, 30 list.
    One packet: open issues with blockers, mine, recently closed. Inject
    at session start (see 40) and refresh after create/close. Sessions
    5, 12, 17, 46, 50, 52 all began by listing issues through the CLI.
14. **`issue_thread`.** View + comments + deps in one bounded packet.
    Replaces `issue view 145 | head; issue comment 145 | head`.
15. **`worktree_host`.** 72 `git worktree` shells plus add/remove/prune
    around every unit. The Autopilot spec and AGENTS.md already require
    a fresh worktree per unit. The model should call `worktree.start` /
    `worktree.finish` and never see `git worktree add --detach`.
16. **`cargo_test` host tool.** Not a shell. Inputs: package, test name,
    lib/integration. Output: the `test_report` JSON, plus a path to the
    full log on disk. 1,047 shells become ~200 typed runs. Default
    `ulimit -n 10240` so sessions 14, 43, 56 stop spending rounds on fd
    quota.
17. **`fmt_check` host tool.** `cargo fmt --all -- --check` was run as a
    surprise blocker (session 35 filed #241 because the pre-push guard
    did not run it). One named check, one round.
18. **`push_main` host tool.** fetch, rebase-if-needed, pre-push gate,
    push to forge, report WAL seq. Sessions spent 138 `git push` + 129
    `git fetch` + 35 `git rebase` on the same choreography, often twice
    because main moved. Session 13 rebased twice mid-push.
19. **`token_count` (harvest target 7).** Session 18 hit 28.9M. A model
    that can ask "which of these five files fits" plans instead of
    truncating. Pure compute, no mounts.
20. **`compact_notes` (named in the same harvest doc, from
    `xai-compaction-transcript`).** Reduce the last K tool results to a
    bounded structured summary. This is the single largest token cut in
    the window. Without it, items 1–7 still leak because the shells they
    replace already sit in the transcript.
21. **`diff_stat`.** Always `--stat` first. Best practice T2 is adopted
    and still not followed: 206 `git diff`, 140 `git show`, often without
    `--stat`. A plugin that refuses to return a patch until asked.
22. **`blame_line`.** Session 38's litter hunt and session 15's edit
    archaeology wanted "who last touched this test." `git_facts` honestly
    has no blame. Add it or keep blame in core as one call.
23. **`image_read`.** Sessions 3, 21, 22, 28, 41, 50 started from a
    screenshot. The model shelled `sips`, wrote `ocr.swift`, copied files
    into cwd. A host image tool (the runtime already has an image path
    on some lanes) removes that whole preamble.
24. **`release_cut`.** Sessions 24, 28, 43, 51 are the same script:
    `ops/release-cli.sh --version … --targets macos-aarch64 --publish
    --allow-partial`. A host tool with those flags, so Flash cannot
    typo `0.1.1-rc2%` (session 28) or wander into notarization logs.
25. **`preflight_ulimit`.** 24 `ulimit` shells. Set it in the host
    before the first tool. Session 56's Harbor run died on
    `ProcessFdQuotaExceeded` at 256.
26. **`openagents_cli` skill inlined or deleted.** Loaded 33 times.
    The `openagents` tool already *is* the CLI. Loading a skill to use
    it costs a round plus the skill body in context for the rest of the
    session.
27. **`gh_auth_status`.** Session 25 and 57 spent turns discovering
    GitHub scopes. One typed packet: connected, scopes, repo selection.
28. **Coder-store backend for `session_search`.** Today it mounts
    `~/.claude` and `~/.codex` only. These 65 sessions are under
    `~/.openagents/sessions`. The plugin cannot search the conversations
    this audit is about. That is why ATIF files got passed by hand.
29. **`history_recall` that actually reads siblings.** Built in session
    54, filed broken as #301 in session 59. If it had worked, the 15
    ATIF-handoff turns disappear.
30. **`check` tool always present with a default `unit` scope.** It is
    only declared when `.openagents/checks.json` exists. A named verify
    would have replaced a large fraction of ad-hoc `cargo test -p … |
    tail`.

### C. Transcript, compaction, reasoning — the token firehose

31. **Drop reasoning from the replayed transcript.** 5.36M characters of
    thinking, versus 283k of answers. Keep the last reasoning for the
    in-flight turn; persist a one-line summary. On a 28M-token tab this
    is plausibly a 2–4× prompt cut by itself.
32. **Cap and collapse old tool results.** Per-result caps exist
    (`autoimprove.md` §2.3: "per-result output caps and nothing else").
    Long tasks still go quadratic. Collapse results older than N rounds
    to `{tool, command_hash, exit, 200-char tail}`. 12.2M tool-output
    characters is the other half of F10.
33. **Do not persist outputs > 8 KB in the prompt.** 118 of them in this
    window. Write the full log to the session dir (`cmd-N.log` already
    exists) and put a pointer in the transcript. The model can `read`
    the tail if it must.
34. **Compaction turn at a token threshold, not at death.** Compact at
    200k / 500k / 1M, not at 28M. Oracle already named:
    `schemelike-metacircular-eval` on `tb2-cross-section`.
35. **Fresh session per claimed unit.** Autopilot spec, best-practice
    follow-up 4, AGENTS.md worktree rule. Session 18 (689 tools, 6.4 h,
    28.9M) did CoderBench research *and* implemented #182. Split that
    into a research session (throw away) and an implement session
    (small). Half the tokens is the wrong metric — it would have been
    closer to 10× on the implement half.
36. **Do not recover a dead tab by ingesting its ATIF.** 15 user turns,
    plus the receiving tab spending its first 50–100 tools re-reading
    the dump (sessions 13, 15, 18, 19, 20, 24, 25, 34, 35, 36, 59).
    Resume from `summary.last_checkpoint` + the issue number. Session
    18's own last checkpoint would have been enough.
37. **Checkpoint as the resume seed, not as extra narration.** 90
    checkpoints. Make `/resume` inject the last checkpoint as the only
    history. That is issue #189's original point.
38. **Strip `cmd-*.log` and ATIF from the live prompt.** They are
    files. The model `read`s them when asked, they do not ride along.

### D. Tool-surface hygiene (cheap, T1–T3 already adopted, not followed)

39. **One shell name.** 5,449 `bash` + 2,418 `shell`. Same runner. The
    split doubles the catalog and teaches the model to pick at random.
    Session 43's concatenation bug (`openagentsopenagents`, `shellshell`)
    is the same family.
40. **Inject a session-start snapshot.** git_facts + issue_board +
    `HEAD` SHA + dirty-or-not, once, as a host note. Removes the first
    8–15 tools of almost every non-empty session in this window.
41. **Batch independent commands (T1) as a host behavior, not a
    prompt hope.** The model emitted median 174-character single
    commands. The host can accept an array of independent commands in
    one `bash` call and already does if the model asks. Make the
    description forbid "one `ls` then stop." Measured +52% tokens when
    T1 is stripped; these sessions were the stripped case.
42. **`--stat` before `-p` (T2) enforced.** Refuse `git log -p` /
    `git show` without `--stat` unless a path is named.
43. **Lane-aware verbosity (T3).** Flash is metered. These sessions
    dumped Harbor logs, notarization JSON, `gcloud logging read`, and
    full `git diff` on Flash. Terse-by-default on `flash`/`pro`; verbose
    allowed on `local`.
44. **Default `head`/`tail` bounds in the shell wrapper.** If the
    command is not already piped, wrap with a 80-line cap and say so.
    118 huge outputs.
45. **Timeouts that are useful.** 259 timeouts, many 120s Desktop `ls`
    (session 3) or Harbor. Fail fast at 15s for orientation commands;
    120s only for `cargo test` / release. A 120s timeout is a 2-minute
    stall plus a wasted round.
46. **Fix tool-name concatenation.** Sessions 43–44. Every concatenated
    name is a wasted call and a confused model. Regression test.
47. **Duplicate-command gate by cost, already landed (session 35,
    `4e1cc01162`).** Keep it. The earlier aggressive form made the
    model retry with `cd &&` prefixes (736 `cd && search`, 794 `cd &&
    other`) to look different. That is how a safety net becomes more
    shells.
48. **`read` instead of `cat`.** 116 `read` vs 224 `cat`/`head`/`tail`.
    `read` is bounded and line-addressable. Prefer it in the tool
    description; consider refusing `cat` of a workspace file.
49. **Parallel tool calls for independent reads.** Status + issue list
    + repo_tree can run in one model step if the API supports parallel
    tools (the runtime already batches on some lanes). Force that for
    the snapshot in 40.
50. **Do not offer `acp` and `delegate` as twins.** Session 32 filed
    #228; session 33 implemented it. Until the catalog is one tool,
    Flash spends turns asking the difference (sessions 4, 5, 31, 32,
    33, 49).

### E. Models and lanes

51. **Flash is the bulk worker, not the researcher.** 51 of 54
    non-empty sessions ended on `glm-5.3-flash`. Session 55 (Claude
    Code teardown), 56 (Qwen/Ollama audit), 37 (SDK gap) are research;
    they should have been `pro` or a long-context model for the
    reading, then Flash for the issue-filing. One model for everything
    is why research tabs also did 300–400 shells.
52. **Local lane for dump-heavy work.** Session 56 already has Ollama
    qwen3.8 in the cycle. Harbor logs, `git log`, ATIF reads, teardown
    greps: run them local so output tokens are minutes, not dollars
    (T3). Then paste a *summary* into the Flash tab that implements.
53. **Non-thinking Flash for orientation.** The 5.36M reasoning
    characters are a thinking model narrating `git status`. A
    non-thinking small model (or a host snapshot) for orientation;
    thinking only once there is a file to change.
54. **Gemini Flash was one "hi" (session 53).** The lane switch is
    unused as a cost tool. Wire "this turn is a screenshot / a TUI
    pixel question" to a vision-strong lane automatically (sessions
    3, 21, 22, 28, 41).
55. **Do not run a 28M-token prompt.** Hard-cap context and compact or
    start a new session. The proxy 413 in session 50
    (`coder_api_hop` / `upstream_status: 413`) is this limit arriving
    as a 502. Treat 413 as "compact now," not "fail the turn."
56. **Route `cargo test` and `git` off the model entirely when the
    intent is typed.** If the host can run `cargo_test` (16) it does
    not need a 70B-class model to decide the command line.
57. **Keep stable-channel cuts off Flash improvisation.** Release
    sessions 24/28/43/51 belong on a scripted tool (24), not a
    thinking model with `ops/release-cli.sh` in a 45-tool loop.

### F. Session lifecycle and Autopilot (the actual product)

58. **Autopilot as specified.** The 80 `continue`/`go` turns *are* the
    missing mode. The owner was the loop. `docs/coder/autopilot.md`
    §1: when Autopilot is on, the loop keeps steering across turn
    boundaries. Half the wall clock of this window is waiting for the
    next "go."
59. **One unit, one session, one worktree, one push.** Sessions that
    mixed research + implement + release (18, 23, 28, 51, 56) paid
    the union of all three contexts. Autopilot's unit of progress is
    already "work whose verification someone else can reconstruct."
60. **Stop conditions that are not "ended without a final answer."**
    28 of 95 failures. The cap-forced-report from #188 (session 20)
    helped later; still, sessions 12, 34, 44, 61 left no assistant
    text. A host-side "emit checkpoint and stop" at 80% of the tool
    budget is cheaper than death-plus-ATIF.
61. **Raise was 100 → 1,000 (session 51).** Keep 1,000 as a safety
    net, not a plan. A unit that needs 400 tools is a unit that is
    missing plugins (A) and compaction (C).
62. **Empty-session GC.** 11 catalog rows with no events. They pollute
    `list_sessions` / swarm targeting (#304). Do not catalog until the
    first user turn.
63. **Do not idle-wait on swarm with a shell poller.** Session 60 wrote
    `/tmp/inbox_watch.sh` every 20s because `swarm_wait` was not
    trusted yet. That is #287's job. Host-side wait, not a bash loop
    that also has to be drained.

### G. Coordination, so parallel tabs stop doubling the work

64. **Host-owned worktree per session, always.** Shared-cwd collisions
    (sessions 17, 35, 36, 51–54, 61, 62's review of §4) caused rebase
    loops, mixed resets, dropped landings, and "I recovered the files
    from `0e422b00d2`." If every tab starts in an isolated worktree,
    half of the git choreography in this window never happens.
65. **Claim ledger the model cannot skip.** Session 54 found every
    lane taken after it had already implemented. Session 61's first
    #299 landing was dropped by a sibling reset. Autopilot spec's
    second review already named this. A `claim` host tool that refuses
    edit on an owned path is cheaper than two implementations.
66. **Swarm for status, not for source of truth.** 170 swarm messages.
    Useful for "I am on #283." Harmful when it is the only way to
    learn that #282 already landed. The issue tracker is the ledger;
    swarm is a nudge.
67. **Fix #301–#304 before the next swarm window.** Drain schema,
    injection-emptied drains, inbox-defaults-to-newest, recall.
    Sessions 59–64 spent their time rediscovering these. That time is
    100% overhead on the job in §11.
68. **Delegate independent issues instead of one 600-tool tab.**
    `delegate` ran 18 times; session 23's Devin path actually closed
    #203. Sessions 12, 17, 18, 37 tried to "do all of them" in-process.
    Fan-out is the half-time move when the units do not share files.

### H. Verification, so tests stop eating the window

69. **Never `cargo test --workspace` from a Flash tab unless the
    pre-push hook is the caller.** Only 10 such calls, but they are
    the long ones (session 33 needed `ulimit -n 10240` for 1,596
    passed). The completion gate belongs to the hook / `push_main`
    (18), not to the model.
70. **Package-and-name the test from the edit.** After editing
    `coder/tui.rs`, run `cargo test -p openagents-cli tui --lib`, not
    a fishing `| rg`. `test_report` then names the miss.
71. **PTY suite only for TUI issues.** Sessions 21, 22, 28, 41 are TUI.
    Best practice V2. Headless `cargo test` cannot close them; the
    `coder_interactive_pty` suite can. Point the model at that binary
    by name in the tool description for TUI files.
72. **Do not re-run the suite after fmt.** Session 13's original ask
    was "note how many minutes it's wasting on rerunning the whole
    test suite." That was the window's first diagnosis. It is still
    the pattern in 1,047 cargo calls.
73. **Cache compilation.** Worktrees do not share `target/` unless
    pointed at a shared cache (`CARGO_TARGET_DIR` / sccache). Each
    fresh worktree paid a cold compile. Put the cache outside the
    disposable worktree (AGENTS.md already says this). A host
    `worktree.start` (15) should set that env.

### I. Inference hop, so failed turns stop wiping an hour

74. **Surface proxy 502 / unreachable / 413 in the TUI immediately.**
    14 unreachable, 6 provider 502, 3 mid-stream decode, 1 413, 1 HTML
    502, 1 10s timeout. Sessions 19, 20, 41, 43, 44, 50, 61 lost the
    in-flight turn. Retry once, then checkpoint-and-stop (60), do not
    sit in a tool loop against a dead hop.
75. **Local fallback on hop death.** Session 64 started as Ollama and
    answered as Flash. Invert that: if the proxy is unreachable, the
    in-flight turn continues on local, then the next turn tries the
    hop again.
76. **413 → compact (55), not fail.**

### J. Prompt and catalog size

77. **Shorter tool descriptions for the default set.** The catalog the
    model sees every round includes swarm, acp, delegate, skill,
    capability, checkpoint, two shells. Smoke sessions 4, 31, 32, 48,
    49, 53 exist because the owner had to ask "what tools do you have"
    — and the answer kept changing. A small default set (read, edit,
    write, bash, git_facts, code_search, test / check, openagents,
    checkpoint) plus `capability` for the rest.
78. **Do not put the full COMMANDS list in the prompt.** Session 62
    read `commands.rs` to add `/autopilot`. Slash-command discovery
    belongs in `/help`, not in every turn.
79. **Issue bodies stay on the tracker.** Models `cat` entire issue
    threads into the prompt (51 views). `issue_thread` (14) with a
    2k-character cap.

### K. The half-calls / half-time picture, tied to this window

If only five of the above had been live, the arithmetic on *this*
store looks like:

80. **Plugins-as-tools (1–8, 9) + session snapshot (40) + one shell
    (39).** Orientation shells are the majority of the 7,867. Cutting
    git + search + ls to one snapshot plus on-demand plugins is a
    3–5× cut on the first 30 tools of every implement tab (almost all
    of 11–20, 33–41, 51–58). That alone is "half the tool calls" on
    those sessions.
81. **`cargo_test` + `test_report` (5, 16, 70).** 1,047 cargo shells
    become a few hundred typed runs. Session 15/20/33-style iterate
    loops go from "run, grep, guess" to "here are the failing names."
82. **Compaction + drop reasoning (31–35).** The 28.9M tab becomes a
    1–2M tab. Proxy 413s stop. Every subsequent round is cheaper, so
    wall clock drops even when tool count does not.
83. **Autopilot + no ATIF resume (36, 37, 58, 59).** 80 continue turns
    and 15 export-handoffs go away. The owner's time — the actual
    wall clock of the window — halves because the loop no longer
    stops.
84. **Host worktree + claims (15, 64, 65).** The rebase/reset/collision
    tax (sessions 17, 51–54, 61) goes away. That tax was hours, not
    minutes.
85. **Vision lane + `image_read` (23, 54).** Screenshot sessions 3, 21,
    22, 28, 41 skip the `sips`/`ocr.swift` preamble and the wrong-file
    timeouts.
86. **`push_main` + `release_cut` (18, 24).** Push/rebase and RC cuts
    become one call each. Sessions 13, 24, 28, 43, 51 shrink by tens
    of tools and the rc2 `%` class of error.
87. **Fix swarm defects and recall (29, 67).** Sessions 59–64 were a
    verification fleet spending time on the tools instead of the
    issues. That whole afternoon is overhead.

Taken together, the implement tabs in this window (the ones that
closed issues) look like 150–300 tools over 2–6 hours today, and like
60–120 tools over 1–2 hours with 80–84 in place. The research tabs
(55, 56, 37) look like a local-lane dump plus a short Flash
issue-filing session, not a 400-tool Flash transcript. The smoke tabs
stay cheap. The empty tabs vanish (62).

That is the same job as §11 — do the issues, AFK, come back to
receipts — with the cost structure the Gym already measured on
`fix-git` and the guests already sitting in `plugins/`.

## 14. What this extension did not do

- Did not re-score Gym A/B; the `not_invoked` rows still stand.
- Did not implement any unlock. Autopilot remains a spec. Plugins
  remain behind `capability`.
- Did not claim a billed-token total. Last-snapshot sums are still
  not spend.
- Did not add a new issue. Several unlocks already have issues
  (#188, #228, #241, #256, #301–#307, Autopilot's slice issues).

