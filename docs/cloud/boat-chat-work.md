# `openagents chat work --on boat`: Coder issue runs on Boat sandboxes

- Issue: [#10220](https://github.com/OpenAgentsInc/openagents/issues/10220) (Boat B6)
- Plan: [Boat SDK plan §5.4](2026-10-02-boat-sdk-plan.md)
- Code: `crates/openagents-cli/src/chat_boat.rs` (the orchestrator),
  `crates/openagents-cli/src/chat_work.rs` (`--on`), `crates/boat` (the SDK)

```text
openagents chat work --on boat --issues 10301,10302 --parallel 2
```

The computer that runs this command only orchestrates. Nothing is built on
it. Each issue runs on a Boat sandbox of its own.

## What one issue does

1. **Dispatcher.** A start is taken from a queue that keeps every rolling
   minute under Boat's start limit (12 a minute on the $20 plan). Before each
   start, `GET /limits` is read; when the day's starts (200 on the $20 plan)
   are gone, the issue is reported `not_started` instead of waiting until
   tomorrow. `--parallel` (1 to 16) is how many sandboxes run at once.
2. **Start.** A `large` sandbox (8 vCPU, 16 GB, $0.072 an hour) is created
   `from` the newest ready `oa-coder-main-<date>` named snapshot (the daily
   template, B5 #10219). `--template NAME` or `OA_BOAT_TEMPLATE` picks
   another. When no template exists, the command builds one **seed** sandbox
   with the template's own host setup (`scripts/cloud/coder-host-setup.sh
   --warm`), stops it, forks it once per issue, and deletes it at the end.
   The seed takes tens of minutes; a template start takes seconds.
3. **Reachable.** A sandbox from a template reports `ready` before Boat
   runs commands on it, and its *detached* commands were refused with `400
   sandbox_direct_failed` for minutes at a time (on 7 of 12 starts on
   2026-10-02) while plain commands ran. So nothing here uses detached
   commands: the command waits until a plain `true` runs (up to ten
   minutes; a sandbox that never gets there is deleted and replaced once,
   its cost added to the run's), starts the run with a plain command in a
   session of its own (`setsid nohup`, output to `/tmp/oa-run.out` and
   `.err`, exit code to `/tmp/oa-run.exit`), and reads its output every two
   seconds from the byte offset it reached, base64-encoded so offsets stay
   exact.
4. **Credentials.** Written to `/tmp/oa-run.env` through the files API; the
   sandbox's command sources and deletes that file before anything else
   runs. See below.
5. **Binaries.** The run reads the template's binaries through once first
   (they stream in from the template, and a read of the 2 GB debug
   `openagents` failed mid-stream twice on 2026-10-02; a failed read is
   retried six times, then the run builds its own), then uses the
   `openagents` and `microcoder` the template built (`<slot>/debug/`, the template's `origin/main`), run in
   place (a copy reads gigabytes the sandbox may still be streaming in), with
   `OPENAGENTS_CODER_CONTROLLER` pointing at that `microcoder`. `OA_BOAT_BUILD=1` builds `origin/main`'s instead, on
   the warm target (not `--locked`). Building in a fresh template sandbox
   failed on 2026-10-02 while its files were still streaming in (`can't
   find crate` for rlibs the template holds, `Permission denied` in the
   slot), so the default does not build. When the template left
   `~/.openagents` owned by root (#10219), the run re-executes itself as
   root in the same `HOME`.
6. **The issue flow** runs there: `openagents chat work --local --json
   --issues N --parallel 1` — the same flow as on a Mac: claim comment,
   worktree of `origin/main`, engine turn, checks, the multi-machine landing
   of #10226 (fetch, rebase, plain push, jittered retry), evidence comment,
   close.
7. **Streaming.** The flow's NDJSON events stream back through that
   reader and print here exactly as a local `chat work` prints
   them, marked with the issue. Under `--json` every line carries `issue`;
   the extra events are `boat_sandbox`, `boat_seed`, and `route_record`, and
   the final `issue` event adds `sandbox`, `wall_seconds`,
   `machine_seconds`, and `cost_usd`.
8. **Cost.** When the flow ends, `GET /sandboxes/{id}/usage` gives the
   sandbox's billed seconds (Boat counts `default`-size seconds, so a
   `large` sandbox bills two a second) and list-price dollars. They go into a comment on
   the issue and into a route record appended to
   `~/.openagents/boat/runs.jsonl`: placement computer `boat`, grant source
   `operator` (the person typed `--on boat`), and a
   `route_contract::record::RunOutcome` with `cost_microusd` and `wall_ms`.
9. **Teardown.** No run leaves its sandbox running (stopped is free; Boat's
   TTL counts from start, not idleness, so it is only a 12-hour backstop).
   The sandbox is deleted when the issue landed, was skipped, or was
   closed; otherwise it is stopped and kept for inspection, and
   `openagents boat delete ID` removes it. Ctrl-C kills every running flow
   (its process group) on its sandbox and stops the sandboxes.

**If the orchestrating command dies** (its machine stops, the shell is
killed), each run keeps going on its sandbox, because it runs in a session
of its own: it still lands, comments, and closes. Nothing then stops or
deletes that sandbox before its 12-hour lifetime ends, and no cost comment
is posted; list the account's sandboxes and `openagents boat delete ID` the
ones a dead command started. (Seen 2026-10-03: the orchestrator's own
sandbox reached its lifetime mid-run.)

## Credentials

Every credential is read once on the orchestrating computer, and for each
run is written to the sandbox's `/tmp/oa-run.env`, which the run's command
reads and deletes first. That keeps credentials:

- out of every template and snapshot: `/tmp` is not captured by Boat
  snapshots, the sandboxes start with `noEnv: true` (no account credential
  from Boat), and a run's sandbox is never saved as a named snapshot;
- out of command lines (Boat records commands) and out of this command's
  output and logs;
- gone with the sandbox, which is deleted after landing.

| Variable | Source, first found wins | Used for |
| --- | --- | --- |
| `GH_TOKEN` | `OA_BOAT_GH_TOKEN`; Secret Manager `coder-pool-git-token` (the September pool's git token); `gh auth token` on this computer | the flow's `gh` (claim, comments, close) and `git push` through `gh auth setup-git` |
| `XAI_API_KEY` | `XAI_API_KEY`; Secret Manager `openagents-xai-api-key` | Grok Build, the engine the flow uses under `api-keys`. Coder gives Grok Build the key of the login shell, which it starts with an empty environment, so the run writes it to `/tmp/oa-engine.env` (mode 600, outside snapshots), the profile sources that file, and the run deletes it when it ends |
| `OA_GIT_NAME`, `OA_GIT_EMAIL` | the same variables; this computer's `git config user.name/email` | commit identity |

`coder-pool-git-token` is an owner OAuth token with push to the repository.
A GitHub App installation token (Secret Manager `coder-github-app-key`, one
hour, one repository) is the narrower follow-up; the reader above is the one
place to change.

### Engine logins: `--engine-logins api-keys|boat`

The default is `api-keys` (`OA_BOAT_ENGINE_LOGINS` sets the default).

- **`api-keys`.** The issue flow's engines are Codex, Claude Code, and Grok
  Build. Coder's Codex route needs a ChatGPT login, not an OpenAI API key,
  and its Claude route needs a Claude Code sign-in; only Grok Build takes an
  API key (`XAI_API_KEY`). So with `api-keys` the flow runs on Grok Build.
- **`boat`.** The sandboxes start with `noEnv: false`, so Boat writes the
  ChatGPT and Claude subscriptions the owner connected on Boat's dashboard
  (`~/.codex/auth.json`, `~/.claude/.credentials.json`) into each sandbox,
  refreshing them on its servers before every start. Coder then prefers
  Codex, then Claude Code, as on the Mac. Boat holds those tokens; that is
  the owner's decision (B7, NEEDS_OWNER "Boat: choose how coding agents log
  in"). Switching later is only this flag: nothing else changes.

## Router placement

`--on boat` is the operator naming the computer: the route record's
placement is `computer: "boat"` with an operator grant
(`boat:<sandbox id>`). The run never moves to another computer on its own;
when Boat has no starts left the issue says so and stays open.

## Measured

See the #10220 closing comment for the first real run (issues, wall time,
machine time, and cost per run).
