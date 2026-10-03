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
5. **Restore, ownership, binaries.** Boat restores a template sandbox's
   home lazily, through a FUSE mount that is retired when the restore
   finishes. Building or reading the template's large binaries before then
   failed or stalled on 2026-10-02/03 (`can't find crate`, `Permission
   denied` in the slot, a 2 GB read aborted mid-stream), and a repair of the
   root-owned directories a template sandbox comes back with, walked
   through the mount, missed directories restored later (#10274). So
   `ready` repairs only Boat's own `~/.ascii` (a plain command), and the
   run first runs `scripts/cloud/boat-fork-ready.sh` (uploaded to
   `/tmp/oa-boat-fork-ready.sh`): it waits until the restore is done (1 to
   14 minutes for the 33 GB template, #10251) and then gives every
   root-owned path under `HOME` back to the user, on plain disk, in about a
   second. The run then reads the template's binaries through (retried six
   times) and uses the `openagents` and `microcoder` the template built
   (`<slot>/debug/`, the template's `origin/main`), run in place, with
   `OPENAGENTS_CODER_CONTROLLER` pointing at that `microcoder`.
   `OA_BOAT_BUILD=1`, or binaries that cannot be read, build `origin/main`'s
   instead on the warm target (not `--locked`), one package per invocation
   as the template warmed them.
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
is posted; `openagents boat list` shows the account's sandboxes (a `*`
marks the ones billing) and `openagents boat delete ID` removes the ones a
dead command started. (Seen 2026-10-03: the orchestrator's own
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
| `OA_CODEX_AUTH` (preferred) | this computer's `~/.codex/auth.json` (`$CODEX_HOME`, or `OA_CODER_CODEX_AUTH`) | Codex on the owner's ChatGPT login: a copy with the **refresh token blanked**, so no run can rotate it (ChatGPT refresh tokens are single use: Codex's `refresh_token_reused`) and this computer stays signed in. The access token (10 days) must have 2 h left; Codex refreshes it on the Mac within 5 min of expiry. Coder then runs Codex `gpt-6.1-sol`, its first choice |
| `OA_CODEX_API_KEY` (only without a ChatGPT login) | `OA_CODER_OPENAI_API_KEY`; Secret Manager `coder-openai-api-key` | Codex, under `api-keys`: the run pipes it to `codex login --with-api-key` and unsets it, so Coder prefers Codex (`gpt-6.1-sol`, medium) as one lean `codex exec` session (#10275). Absent, the run is on Grok Build |
| `OA_GIT_NAME`, `OA_GIT_EMAIL` | the same variables; this computer's `git config user.name/email` | commit identity |

`coder-pool-git-token` is an owner OAuth token with push to the repository.
It needs the scopes `repo` (claim comments, assignees, close, `git push`) and
`project` (read and move the issue's Status on the OpenAgents board, project
19; `read:project` alone reads it but cannot move it). Without `project` the
run still lands, comments and closes; it says once that the board could not
be read and leaves the Status as it was.
A GitHub App installation token (Secret Manager `coder-github-app-key`, one
hour, one repository) is the narrower follow-up; the reader above is the one
place to change.

### Engine logins: `--engine-logins api-keys|boat`

The default is `api-keys` (`OA_BOAT_ENGINE_LOGINS` sets the default).

- **`api-keys`.** The issue flow's engines are Codex, Claude Code, and Grok
  Build. Coder's Codex step loop needs a ChatGPT login, but its lean
  `codex exec` session takes an API-key login, and Coder runs Codex that way
  whenever Codex's login is an API key (#10275). So with `api-keys` the flow
  runs on Codex when an OpenAI key is given (`coder-openai-api-key`), else
  on Grok Build (`XAI_API_KEY`). Claude Code needs a sign-in.
- **The Grok model.** On `XAI_API_KEY` Grok Build's own default is
  `grok-4.20-0309-non-reasoning`, which reported edits it never made
  (#10221). A Grok route that keeps the default therefore runs `grok-4.7`
  on the API login (`acp_client::grok::API_KEY_MODEL`), and a turn on a
  model known to fake tool results (`FAKES_TOOL_RESULTS`) is refused with
  that reason instead of ending `unchanged`. `coder.providers grok:MODEL`
  passes `--model MODEL`; a login that does not offer it (`grok models`)
  connects its own default, and the turn is refused as a model mismatch.
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
