# Issue velocity, 2026-10-18 to 2026-10-19: what slowed it, what to change

Recorded 2026-10-19 at about 22:00 UTC, over the 24 hours from
2026-10-18 21:00 UTC. This audit follows the
[2026-10-17 close-timeline audit](2026-10-17-nostr-identity-close-timeline.md)
and [Foreman's lessons](../foreman/lessons-learned.md) (L1–L13), and uses the
[compile-time audit](2026-10-19-rust-compile-times.md) for build numbers. It
asks why issues closed slower than the agents could write them. It ranks the
fixes by the time they would have saved.

Sources: GitHub issue timestamps, `git log origin/main`, `openagents --json
land status --all`, Foreman's chat (`9fa93200`), the Pylon 1.0 chat
(`03a26a6d`), and the terminal coordinator's own session log. Times are UTC.

## Summary

- **Throughput was high.** `main` took 380 commits (205 by Codex, 159 by
  Claude-driven sessions as AtlantisPleb). 91 issues closed, with a median
  of 2.3 h from filing to close (p90 19.9 h). 69 issues were filed, and 17
  of them are still open.
- **Most of the lost time wasn't coding.** It went to five things:
  1. **Infrastructure that turned itself off.** The landing integrator
     powered off at 19:23 with work about to arrive. An entry sat for
     15 minutes until a person noticed. Pool hosts deleted themselves under
     a coordinator's ssh session, mid-test.
  2. **A coordinator that failed silently.** After the Claude account
     switch, Foreman resumed a Codex thread as a Claude session (#12205).
     From 21:25, every Foreman turn ended empty within 4 seconds. The host
     sent it 12 more check-ins over 35 minutes, and nothing raised an
     alarm.
  3. **Work lost or bounced for mechanical reasons.** Both queue bounces in
     the window were stale generated files or a missing dependency. `main`
     took 9 "Regenerate …" commits. A failed pool run deletes its worktree,
     so Codex's diff is gone and the repair starts over.
  4. **Engines without a fallback where they ran.** On the GCE pool, Codex
     wrote each Pylon change, one test failed, and no Claude login was
     there to repair it (#12203). Six of the eight Pylon runs failed this
     way.
  5. **Placement on the Mac.** Builders for the Pylon and crate-split chats
     ran on the owner's Mac, at a load average of 39 to 77. The desktop app
     was unusable, and every build on the Mac slowed down (#12201).
- **The landing queue is almost unused.** 3 of 380 commits landed through
  it. 14 of its 19 entries were withdrawn, and their issues landed by
  direct push instead. Landed entries took a median of 28 minutes. The
  queue costs time and guarantees nothing when everyone bypasses it.

## What happened

### The Pylon 1.0 batch (#12187–#12197)

Eleven issues were filed at 19:31–19:37. By 22:00, four had closed: #12191
at 21:07, #12195 at 21:25, #12192 at 21:38, and #12197 at 21:45. Seven are
still open. In the Pylon chat's words: "Codex writes the change, one test
fails, and the pool has no Claude login." In more detail:

- #12187, #12188, #12189, #12194, #12195 and #12197 each reported a failed
  check on the pool. #12195 and #12197 later landed after a Claude repair
  on the Mac.
- "The Codex diffs are gone (each run's worktree is deleted when it ends),
  so repairs start from the saved builder patches."
- "The host dropped my connection mid-test. Pool hosts delete themselves
  after 10 idle minutes, and my ssh work doesn't count as busy." The chat
  worked around it by touching `~/.oa-pool/busy` every minute.
  `595f995341` fixed it for new hosts.
- #12192 was queued at 21:16 from `oa-pool-pb22a59-6696`. The integrator
  `oa-dev-env-1` had been off since 19:23 ("not running, seen 2h ago").
  The entry landed at 21:38, 6 minutes after the VM was started by hand.

### Foreman

| Time | Event |
| --- | --- |
| 20:25 | Foreman reports a Mac load of 39 (77 over 15 minutes), partly from its own three Codex chats. |
| 20:48 | "Host still unreachable; falling back to the pool." The agent host for #12184 didn't answer. |
| 21:02 | "#12183's cloud run failed and #12202's never started, so I restarted both." |
| 21:17 | The owner's Claude weekly limit ran out. Every Claude chat was told to stop. |
| 21:21 | The host switched Foreman to Codex ("Claude Code isn't signed in on this computer"). |
| 21:25 | Back on Claude Code, the host resumed the Codex thread ID as a Claude session: "No conversation found with session ID 01a1560a-…". |
| 21:25–22:00 | Twelve check-ins and two messages from other chats got empty turns. |
| 21:46 | The fix (#12205, `b2fe268778`) landed. `1b756d8b6a` then fixed the engine's non-test build, which #12205 had broken because only `cargo test` ran. |

For about 35 minutes the coordinator of every open issue was gone. The
check-in loop counted its empty turns as answers.

### The landing queue

| State | Entries (24 h) |
| --- | ---: |
| Withdrawn | 14 |
| Landed | 3 |
| Bounced | 2 |

- **Landed:** median 28 minutes, maximum 49.
- **Bounced:** #12136 bounced because the bundled CLI tree was stale
  (`tree::tests::the_bundled_tree_is_generated_from_this_help`). #12163
  bounced because `futures_util` was missing from `openagents-cli`, plus
  one engine test. Both are mechanical. The integrator could regenerate
  the first and name the fix for the second in seconds, not after 57 and
  71 minutes.
- **Withdrawn:** every issue checked (#12084, #12128, #12136, #12149,
  #12159, #12163 and others) closed through a direct push.
- **Submitters:** 9 entries came from the Mac, 6 from CoderOS, and 4 from
  pool hosts.

### Cloud capacity

- **GCE pool:** worked. `chat work --on gce` placed four issues on Codex
  and created a spot host in 20 s when the three hosts were full.
- **Archil:** all four sandboxes created at about 21:45 failed with
  "initialization failed" before they started. Earlier, Archil's firewall
  blocked `api.github.com`, rustup and ChatGPT sign-in. Agents there can
  push but can't comment, close issues or run Codex.
- **Agent host** (`oa-agent-host-1`): unreachable at 20:48.

## Fixes, ranked

Estimated savings are for a day like this one.

| # | Fix | Saves | State |
| --- | --- | --- | --- |
| 1 | **Coordinator liveness.** The host treats two empty or errored Foreman turns in a row as down. It starts a fresh session with the saved handoff and tells the owner. | 35 min of no coordination; more on a night run | Proposal |
| 2 | **The integrator never sleeps on work.** Count queued entries as busy. Let pool hosts start the integrator, or make `submit` fail loudly when it can't. Alert when the heartbeat is older than 5 minutes. | 15–20 min per stall | #12206; idle stop off for the 1.0 push |
| 3 | **Regenerate in the integrator.** Before checks, run every regenerate command whose test would fail (CLI tree, SDK reference, API contract, route map, Android bindings), commit the result into the entry, then check. | Both bounces; 9 hand commits | Proposal |
| 4 | **Fail fast in the queue.** `cargo fmt --check` and `cargo check --tests` on the touched packages before full tests: 36–76 s instead of up to 38 min. | 71% of check time went to entries that bounced | #12198 |
| 5 | **Keep a failed run's diff.** Pool and Archil runs upload the worktree's diff and the failing check's output to GCS before they delete it. The repair starts from it. | One full re-run per failed issue | Proposal |
| 6 | **Repair on the same host.** A pool run whose check fails hands the diff and the failure to a second engine there (Claude through #12203, or Codex again with the failure in its prompt) before it reports failure. | 6 of 8 Pylon runs | #12203 |
| 7 | **Builds leave the Mac by default.** A delegated chat's build places itself on an idle cloud computer (#12201). The Mac refuses agent builds above a load average of 16, except for the owner's own. | The unusable desktop; slower Mac builds for everyone | #12201 |
| 8 | **Check what ships, not only tests.** The briefed agent's check runs `cargo check` (no `--tests`) beside `cargo test -p` for each touched package. | `1b756d8b6a`-style follow-ups | Proposal |
| 9 | **One landing path.** After fixes 2–4, `chat work` and Foreman's dispatch submit to the queue and never push directly. Until then, stop asking chats to use the queue and then withdraw. | Duplicate work: an entry and a push | Decision for the owner |
| 10 | **More than one Claude login in the capacity book.** Rotate to the next login on a usage limit instead of stopping every Claude chat. Foreman keeps its own session per runtime (`b2fe268778`). | The 21:17 wind-down and restart | Proposal |
| 11 | **Archil falls back.** The fan-out runner sends an issue to the GCE pool when its sandbox fails to start or the work needs a blocked host. Archil stays secondary until its start failures are explained. | Four issues an hour late | Proposal |
| 12 | **Pool hosts stay up under ssh.** | Lost test runs | Done, `595f995341` |
| 13 | **Measure every day.** A script prints this audit's numbers (closes, median filing-to-close, queue states, failed runs by cause, coordinator downtime) into Foreman's daily check-in (L13). | Finds the next stall in hours, not after a day | Proposal |

### Suggested order

1. **Today:** fixes 1, 2 and 6 cost the most and are small.
   - Fix 1 is a counter in the check-in loop
     (`apps/openagents/crates/engine/src/foreman.rs`).
   - Fix 2 is a change to `scripts/cloud/dev-env-agent.sh` and the
     `submit` wake path.
   - Fix 6 is the open #12203.
2. **Next:** fixes 3, 4 and 5 make the queue fast enough to be the default.
   After that, fix 9 makes it the only path.
3. **Then:** fixes 7, 8, 10, 11 and 13.

## What went well

- **Builds got faster.** #12178 cut rebuild times by 50–82% (see the
  compile-time audit).
- **The pool scaled itself.** It added a host when the pool was full and
  ran Codex on one ChatGPT sign-in with no re-login.
- **Infrastructure stalls got fixed the same day.** Once someone saw them,
  the dead integrator, the pool's ssh deletes and the Foreman session bug
  each had a fix or an issue within an hour.
