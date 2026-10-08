# Alice on Codex: handoff

This note records where Alice, the workshop agent, stands on coding with
Codex as of 2026-10-07. It covers what landed, the state of issue #10893,
how to run her on Codex, the known issues, and the steps left for the
owner. [Workshop agent](workshop-agent.md#coding-on-codex) is the full
reference.

## What landed

| Commit | Change |
| --- | --- |
| `1ee8e40f5b` | Her record gains an `engine` field, which `openagents agent engine NAME coder\|codex` sets. On Codex, Coder stays the driver on its own model and delegates the file edits to the Codex agent (`acp_subagent`, on the owner's ChatGPT login). The new `openagents coder chat --codex-writes` flag lets that Codex edit the working directory under its own `workspace-write` sandbox, with no network. Codex's tokens become a spend record of their own (harness `codex-cli`), and a Codex usage limit goes in the capacity book and is said once. |
| `57b141e64b` | A studio turn passes `--codex-writes` only when its prompt asks for Codex. `scripts/desktop/dev-host.sh` installs `openagents` beside `microcoder`, so the studio runs the host's own build. |
| `203a8cac4c` | The workspace snapshot bound rises from 4 GiB to 16 GiB. This repository's tracked files passed 4 GiB, so the old bound refused every studio task on it at admission. |
| `655a57b3c2` | Read-only questions never go to Codex and never ask the owner: terminal mode sends no Codex directive, and a gated chat's Codex is always read-only. She answers where the Merge station is, and merges her own waiting change when the owner asks. Each request starts her pane on a fresh screen with one `now:` status line, and the goal bar clears once a goal is done, merged and rejected changes included. |

## Issue #10893

Issue #10893 asks to drop the command hint from the agent's `PROPOSED` line.
Alice did the work on Codex as task `b2fa59bd6e0b` and merged it at the
Merge station when asked. Commit `d3f3ad546c` landed on `main` and closed
#10893 on October 7, 2026.

- **The change.** She changed one file, `crates/openagents-cli/src/agent.rs`
  (19 lines added, 8 removed). It adds a `proposed_line` function that both
  `show` and `ask --wait` use, and a unit test that pins the format. It meets
  the acceptance criteria. `cargo test -p openagents-cli --bin openagents --
  agent` passes on current main with the change applied (8 tests), and the
  file is formatted.
- **Time.** The request took 1,910 seconds. Coder's turn, including the
  Codex delegations, took 367 seconds; the studio's checks spent the rest
  waiting for a build lease.
- **Cost.** Codex ran three delegations: two under `workspace-write` and one
  read-only. Together they used 1,794,101 input tokens (1,642,368 of them
  cached) and 8,799 output tokens on the ChatGPT login, which reports no
  dollar price. Coder, on its own model through OpenRouter, used 34,641
  tokens.
- **Her first merge.** When asked, she ran her own merge, which refused
  with: "The checkout at /Users/christopherdavid/work/openagents-phone is
  not on a branch." The host workspace checkout was detached, so the merge had no
  branch to land on. The checkout was then put on `alice-landing`, and the
  change was merged and pushed to `main`.
- **Transcripts.** Run `openagents agent log alice` for her journal. The Coder
  session, with the Codex child chats, is
  `~/.openagents/coder-new/sessions/task-b2fa59bd6e0b5db9117eff1d45500391e30dd42629e4fbb0e295895b2fb1a91e.atif.json`.

Earlier runs failed for reasons the commits above fixed:

1. An older `openagents` rejected `--codex-writes`.
2. The workspace checkout predated the file.
3. Admission refused the 4.28 GiB checkout.

## Run Alice on Codex

1. Set her engine:

    ```sh
    openagents agent engine alice codex
    ```

2. Hand her coding work in task mode, so it runs in her own worktree:

    ```sh
    openagents agent ask alice "Work issue #N: SUMMARY. Acceptance: ..." --mode task --wait
    ```

3. Review her change, then merge it yourself or ask her to merge it:

    ```sh
    openagents studio review TASK --diff
    openagents agent ask alice "Can you merge your change instead of me?" --wait
    ```

Questions in terminal mode stay on Coder's own model. To return her coding
to Coder's model, run `openagents agent engine alice coder`.

## Known issues

- **Spend records.** Her owner attestation now expires on October 7, 2027.
  Task-mode runs still don't pass through her spend meter.
- **Dev host install and follow job.** The follow job installs `main` only
  while the host is idle, and a build can wait a long time for a lease. After
  the crash at 14:50 the host's launchd plist was missing and a stale
  `~/.openagents/dev-host/follow.lock` stopped every follow pass. Restoring
  the plist from its latest backup in `~/.openagents/dev-host/` and removing
  the lock fixed both.
- **Workspace checkout.** Alice now works in `~/code/openagents`, on `main`.
  Studio worktrees start from the workspace's `HEAD`, so it must stay near
  `origin/main`. The older `~/work/openagents-phone` checkout is on
  `alice-landing`; it is no longer her workspace.
- **Snapshot bound.** Admission hashes the whole workspace, up to 16 GiB. The
  repository's tracked files are 4.28 GiB. Build output inside the workspace
  counts toward the bound, so keep target directories outside it.
- **Flaky test.** The test `canceling_native_codex_cleans_up_its_children` in
  `coder-new` can fail when it runs in parallel with the crew tool-free test,
  which installs a process-wide approval gate. It passes when run on its own.

## Owner steps

No steps remain for #10893. Her checkout is on a branch, her key is
attested, and her charter permits a merge when you ask. The
[runbook](alice-runbook.md) covers subsequent work and attestation renewal.
