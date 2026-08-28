# Local Coder session audit

Date: 2026-08-28.
Status: evidence. Read-only over the owner's local session store.
Companion: this directory's other dated notes describe product intent; this
file describes what the live Coder TUI actually recorded.

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

These are already filed or implied by the sessions; this list is a
reading order, not new authority.

1. Empty-session GC or "don't catalog until the first user turn."
2. One name for `bash`/`shell`.
3. Concatenated tool-name bug (session 43/44), with a regression test.
4. Keep the 1,000-call cap, but start a fresh session per claimed unit
   (Autopilot spec § already leans this way).
5. Inference-proxy 413 / 502 / unreachability as a first-class TUI
   status, not a silent `turn.failed`.
6. Close the swarm defects the later tabs reproduced: #301–#304.
7. Do not treat ATIF-export handoff as the long-term resume path;
   `history_recall` was built in session 54 and then reported broken
   across sessions in #301.
