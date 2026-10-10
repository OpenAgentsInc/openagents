# S1 inventory, 2026-10-10 batch

The evidence packet for the batch frozen in
[2026-10-10-selection.md](2026-10-10-selection.md) (commit `ec8f1d0e14`,
16:54:09 UTC). Every attempt on every selected issue is listed, failures
included. Raw run records stay on the environment (`~/s1/` on
`oa-dev-env-1`: the `chat work --json` streams, the Coder task store and its
ATIF traces); this page carries what they say.

## Corrections to the frozen file

- Its header says "frozen 16:58 UTC" and the gap issues "filed 16:55 UTC".
  Both are wrong estimates. The issues were filed at about 16:53 and the
  file was committed at 16:54:09 UTC, 37 s before the first run started
  (16:54:46). The selection itself was not changed.

## Order changes

- **21:35 UTC, owner priority.** The owner wants to watch the runs on the
  phone and the web without running commands. #11228 (fleet rows for cloud
  runs) moves to the front, right after the gap issues already in flight
  (#11242, #11243). Its follow-up
  [#11250](https://github.com/OpenAgentsInc/openagents/issues/11250) (stage,
  cost so far, landing queue state, links) was filed at 21:37 UTC and runs
  next. It is a later addition, not part of the frozen five, and is counted
  separately. #11233, #11226 and #11234 follow.

## How the runs ran

- **Where.** `oa-dev-env-1` (GCE `c3-standard-22` spot, 300 GB, us-central1-b,
  account `oa-mvp-automation@`). It was stopped (idle stop) and was started
  for this batch at 16:52 UTC. The Mac ran only `gcloud compute ssh` over IAP
  and git for the docs on this page.
- **Session.** `eval "$(scripts/cloud/dev-env-session.sh)"`. GitHub as
  `AtlantisPleb`, Claude Code on the owner's subscription token, Jev,
  OpenRouter, and gcloud as the VM's account.
- **Engine.** Claude Code `2.1.291`, model `claude-opus-5-5`. Coder chose it
  because Codex is not signed in on the environment.
- **Binaries.** `openagents` and `microcoder` were built from `origin/main`
  (`ec8f1d0e14`) on the environment: 4 m 23 s, debug, sccache on GCS.
- **Command.** `openagents chat work --local --json --issues N --parallel 2 --land main|queue`,
  wrapped by `~/s1/run.sh`. Each issue flow gets its own worktree
  (`~/.openagents/worktrees/openagents-<task12>`) and a leased build slot.
- **Costs.** These are Claude Code's own reported `cost_usd` for each turn,
  read from the task's ATIF traces, plus the Jev decision call's price
  estimate. The agent ran on the owner's Claude subscription, so these are
  list-price equivalents, not per-token bills. Environment compute is not
  split per issue: it is listed once below. A cost the trace does not carry
  is `unknown`, never 0.

## Per issue

Status as of 21:40 UTC. Times are UTC. Costs are Claude Code's reported
list-price `cost_usd` for the agent, plus the Jev decision call.

### #11243 Trace replay for issue-flow landings (gap): **landed, replay verified**

| Attempt | Task | Start | Outcome | Agent cost | Decision cost | Tokens in / out | Checks |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | — | 16:54:46 | `not_started`: `microcoder` was missing beside the `~/.openagents/bin/openagents` used for the run | $0 (no engine started) | none | — | — |
| 2 | `b78d7b74c5c6` | 17:00:41 | **landed `27b206a24d`** 17:03:46, issue closed, board Done | $0.758 | $0.0003 | 858,270 / 13,264 | Gate: no Rust package changed, so no tests ran. The agent ran `test_traces.py` (21 pass) itself |

- **Replay.** `traces.py capture --landed 27b206a24d --issue 11243 --check "python3 -m unittest scripts/bench/traces/test_traces.py"`,
  then `replay --on local` on the environment: **verified** (`exact_replay`, 18.7 s).
  Receipt `sha256:82936fd0…e1b`, diff `sha256:50b529a2…765`, tree `b087da70`.
  The trace's own cost is `null` ("not recorded by the issue flow"); the
  cost above comes from the task's ATIF trace.
- **Caveat.** The trace marks its check result `recorded_by: issue_flow_gate`,
  but the gate ran no Python tests for this change. The replay was the first
  run of those tests outside the agent.
- **Deploy.** None needed: a script and docs.

### #11242 Issue flow `--land queue` (gap): **not landed, attempt 3 running**

| Attempt | Task | Start | Outcome | Agent cost | Decision cost | Tokens in / out |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | — | 16:54:46 | `not_started` (`microcoder` missing, as above) | $0 | none | — |
| 2 | `ad9ded0b205b` | 17:00:41 | `failed` 19:41. The change was done and green at 17:48. `main` then moved +11, +5 and +5 commits during three re-checks of about 25-35 min each. The third re-check was red (`coder` conformance test `a_reworded_question_set_is_drift_the_inventory_reports`). Kept on `coder/stranded-ad9ded0b` | $1.871 | $0.0003 | 2,168,109 / 20,817 |
| 3 | `92e74345c592` | 19:41 | running. It started from `origin/main`, not the stranded branch, because the flow resumes stranded work only after a lost host. Gate red (the same conformance test), fix turn 2 in checks | pending | pending | pending |

### #11228 Fleet rows for cloud runs: **not landed, attempt 2 running**

| Attempt | Task | Start | Outcome | Agent cost | Decision cost | Tokens in / out |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `149fb4071529` | 18:19:39 (`--land main`: #11242 had not landed) | `failed` 19:45. Green at about 19:00, then `main` moved +8 and the re-check was red: `main`'s own stale `tree.json` (`the_bundled_tree_is_generated_from_this_help`) and `openagents-web` auth tests against the change's new cloud-session fields. Kept on `coder/stranded-149fb407` | $4.520 | $0.0003 | 6,642,558 / 41,061 |
| 2 | `ed3e775a2832` | 19:46 (resumed from `coder/stranded-149fb407` with `OPENAGENTS_CODER_PLACEMENT=gce OPENAGENTS_CODER_RECOVER_TASK=<task>`) | running. It regenerated `tree.json`. Gate red: `coder` tests timed out at 1,200 s (cold build) and the disk filled (`No space left on device`, see below). Fix turn 2 in checks | pending | pending | pending |

### #11233, #11226, #11234: **not started**

## Environment cost

<!-- filled at the end -->

## Failures and why

1. **Binary layout.** The flow looks for `microcoder` beside `openagents`.
   The environment's `~/.openagents/bin` held only `openagents`. Fixed for
   the run by building both from `origin/main` into `~/s1/bin`.
2. **Landing livelock.** `main` took 38 commits in 3 h (one every 4.7 min),
   mostly direct pushes in the crates these issues touch. A full re-check
   takes 25-35 min. Filed [#11248](https://github.com/OpenAgentsInc/openagents/issues/11248).
3. **`main` red.** `tree.json` was stale after a direct push, so every
   change touching `openagents-cli` fails its re-check. Noted on #11248.
4. **Semantic conflicts after a rebase** strand the change, with no repair
   turn (only textual conflicts get one). The next run starts from scratch
   unless it is told to recover the stranded branch.
5. **CPU oversubscription.** Two flows at `-j22` on 22 vCPUs reached a load
   of 183. From 19:00 `run.sh` sets `CARGO_BUILD_JOBS=11` and
   `RUST_TEST_THREADS=8` (verified in the gate's processes).
6. **Disk full at 300 GB.** Three build slots grow well past their 25 GB cap
   (59 GB seen) during `--all-features` test builds, and the free-space
   floor is checked only at admission. Grown online to 500 GB at 20:55 UTC.
7. **Image gaps.** `zsh` and `fish` were missing, so the CLI completions tests
   fail on environments. Fixed in `cded309dd7`, landed through the queue
   from the Mac.

## Gaps found and what was done

| Gap | Size | What was done |
| --- | --- | --- |
| Issue flow cannot land through the queue | small | #11242, run through the loop (in progress) |
| Issue-flow landings cannot be replay-verified | small | #11243, landed `27b206a24d` through the loop |
| `zsh`/`fish` missing on the image | small | `cded309dd7`, submitted from the Mac with `openagents land submit`, landed by `oa-land-worker` on the environment |
| Landing livelock on a busy `main`; `main` red from direct pushes | large | Filed [#11248](https://github.com/OpenAgentsInc/openagents/issues/11248) on project 22 |
| Disk 300 GB too small for 2 flows | infra | Disk grown to 500 GB (`gcloud compute disks resize` plus `growpart`/`resize2fs`) |
| Two flows thrash 22 vCPUs | small | Per-flow `CARGO_BUILD_JOBS`/`RUST_TEST_THREADS` in the run wrapper; general fix proposed in #11248 |
| Owner cannot watch runs without commands | owner priority | #11228 moved to the front; follow-up [#11250](https://github.com/OpenAgentsInc/openagents/issues/11250) filed |
| Gate runs no checks for Python-only changes | small | Not fixed yet |
| A red-after-rebase run does not resume its stranded branch | small | Worked around with the recovery variables; not fixed yet |
