# Work on an issue: the briefed agent by default

2026-10-11, [#11258](https://github.com/OpenAgentsInc/openagents/issues/11258).

Whenever someone asks for an issue to be worked, from openagents.com, the
API, or the CLI, the engine is the **briefed agent** of
[#11211](../inference/briefed-agent-ab.md) (−53.9% cost per accepted change,
2.3× faster than bare Claude Code at the same success rate), not bare Claude
Code. Bare Claude Code is the fallback, and every run says which engine did
the work and why.

## The engine: `scripts/work/work_issue.py`

"Work on issue N in repo R":

1. **Briefing.** The issue from `gh` at `origin/main`; the context finder
   (`scripts/filefind`, #11210) and the brief generator
   (`scripts/bench/briefed-ab/briefing.py`) list the files, excerpts, a
   change plan, similar past changes, the checks and the repo rules.
2. **Briefed agent.** `crates/briefed-agent` with `verify` (#11229): Read,
   Edit, Write, Grep and Glob held to a fresh worktree, a briefing-built
   system prompt, and `verify`, which compiles, runs the change's tests and
   formats (`scripts/work/local-exec`: on this machine, builds taking turns
   under one lock per target dir, sccache on GCS).
3. **Judged by the diff.** The driver replays the checks itself on what the
   agent actually changed: `cargo fmt` and `cargo test` for every package
   the diff touches. A run passes only with a diff and every check green.
4. **Escalation to bare Claude Code** (`claude -p "Complete this issue…"`, a
   fresh worktree, judged the same way), recorded in the result as
   `escalated`:
   - low briefing confidence (the finder found nothing, or filefind's best
     file scores under `OA_WORK_MIN_CONFIDENCE`, 0.05);
   - a missing capability (no Rust package for `verify`, or the briefed
     agent isn't installed);
   - the briefed agent made no change, failed `verify` repeatedly
     (`OA_WORK_MAX_VERIFY_FAILS`, 4) with no pass, or its change failed the
     replayed checks.
5. **Landing.** On a pass it commits (with the issue's title) and then, as
   asked: `queue` hands the branch to the [landing queue](land-queue.md)
   (`openagents land submit --issue N`), `pr` pushes `work/issue-N-…` and
   opens a pull request, `none` keeps the commit.

Each run prints one JSON progress line per step and ends with
`{"type": "result", engine, escalated, ok, cost_usd, secs, checks, diff,
commit, landed}`. Cost is what Claude Code reports at list price; when it
reports none, it is `null` and shown as "unknown", never zero.

## Whose Claude

A run always uses the requesting person's own Claude sign-in:

- **Web and API:** the credential saved in Settings > Claude
  (`crate::cloud::byo`). The work host gets it when it takes the run, once
  (`POST /v1/work-hosts/NAME/claim`), and puts it only in that run's process
  environment, in a home of the run's own with no `~/.claude`, after
  removing the host's own Claude variables. Without a saved sign-in the run
  stops and says "Save your own Claude sign-in in Settings > Claude". The
  server's key is never used. One account's runs take turns (a Claude plan
  runs one automated turn at a time).
- **CLI on this computer:** this computer's own Claude Code login.

## Entry points

| Where | What |
| --- | --- |
| `/work` | "Work on an issue": repository, issue number or link, how it lands, and the runs with engine, time, cost, checks, and the pull request or landing entry. `/work/{id}` shows one run live. |
| Chat, project chat | A reply the typed router reads as work on code (the `work.dispatch` route) shows "Work on this issue" for each issue link, `OWNER/NAME#N`, or `#N` (in the project's repository) its message names. Ids are read only after the router chose the route. |
| Claude Code runs | A Claude Code run (the composer's "Where it runs", `/chat/{id}/claude`) whose request names issues hands them to the briefed agent instead, one task row each in the chat, linked to `/work/{id}`; the result joins the chat. |
| Agent fleet | Several agents on a request that names issues: one briefed run per issue (source `fleet`). |
| GitHub tools | `/chat/{id}/github` has "Work on an issue"; after a change on an issue, "Work on this issue". |
| Phone and web boards | Runs are items on the `OpenAgents Cloud` board of `GET /v1/agents` (status, engine, cost, last line); Stop reaches them. |
| API | `POST /v1/work {repo, issue, land: pr\|queue\|none, engine?: briefed\|bare}` → `201 {id, url, events}`; `GET /v1/work`, `GET /v1/work/{id}?after=N&wait=S`, `GET /v1/work/{id}/events` (server-sent `progress`, `state`, `result`), `POST /v1/work/{id}/cancel`. Auth: `Authorization: Bearer sess_…` (`coder login`), agent-work accounts only. |
| CLI | `openagents chat work --issues N` runs the briefed agent here (scripts built into the binary); `--on boat\|gce` sends the issues to `/v1/work`; `--engine bare` keeps the Coder issue flow. |

## Work hosts

The website keeps the runs (`crates/openagents-web/src/work_runs.rs`, chat
store keys `work-runs/…`); work hosts take them. A host is a cloud
environment from the `oa-coder-host` image (as
[dogfood-dev-on-prod.md](dogfood-dev-on-prod.md) starts one, with
`--service-account oa-mvp-automation@… --scopes cloud-platform`), set up
with `scripts/work/host-setup.sh [SITES]`: numpy for the finder, the
session (GitHub and the embeddings key), the briefed agent and the CLI in
`~/.openagents/work/bin`, the finder's index, and `oa-work-worker.service`
(`scripts/work/worker.py`, two runs at once). Hosts prove themselves with
`OPENAGENTS_WORK_HOST_TOKEN` (Secret Manager `openagents-work-host-token`,
readable by the web's runtime accounts and `oa-mvp-automation`). A run whose
host stops reporting for 10 minutes fails; one no host takes in 6 hours is
cancelled.

Limits today: work runs are for agent-work accounts (the site admin, and
staging's test account); landing pushes with the host's GitHub token, so
`queue` and `pr` reach repositories that token can push to (#11226 adds the
person's own GitHub connection); runs on one host share its machine (one
VM per run is the next step for people other than the owner).

## Proof (2026-10-11)

Three real open issues, each worked twice on the cloud environment
`oa-dev-env-3` (spot `c3-standard-22`, image `oa-coder-host`, account
`oa-mvp-automation@`), with the owner's own Claude sign-in and
`claude-opus-5-5`, from the same `origin/main`. Both engines are judged the
same way: the checks replayed on the actual diff (`cargo fmt` and
`cargo test -p briefed-agent`).

| Issue | Briefed agent (default) | Bare Claude Code (`--engine bare --land none`) | Result |
| --- | --- | --- | --- |
| [#11260](https://github.com/OpenAgentsInc/openagents/issues/11260) verify log skips cached calls | 61 s, $0.23, checks 2/2 (22 tests), filefind briefing | 56 s, $0.48, checks 2/2 | [PR #11266](https://github.com/OpenAgentsInc/openagents/pull/11266) |
| [#11261](https://github.com/OpenAgentsInc/openagents/issues/11261) symlink escapes the worktree | 58 s, $0.20, checks 2/2 (23 tests) | 53 s, $0.40, checks 2/2 | [PR #11267](https://github.com/OpenAgentsInc/openagents/pull/11267) |
| [#11262](https://github.com/OpenAgentsInc/openagents/issues/11262) new file counted as a miss | 48 s, $0.21, checks 2/2 (23 tests) | 48 s, $0.39, checks 2/2 | [PR #11268](https://github.com/OpenAgentsInc/openagents/pull/11268) |
| Total | 167 s, $0.64 | 157 s, $1.27 | |

On these three small issues the briefed agent cost 49% less at about the
same wall time (the A/B's 2.3x speedup came from larger issues, where bare
Claude Code explores longer). No run escalated.

Web path: on staging, `POST /v1/work` as the agent-work test account was
taken by the work host within 3 s; the account has no saved Claude sign-in,
so the run stopped with "Save your own Claude sign-in in Settings > Claude"
and the server's credentials were never used. Production serves the same
build (revision `coder-web-a74a2376f9-20261011031455`); the production runs
from the website wait on the owner's sign-in (NEEDS_OWNER.md).

What the proof found and fixed:

- A first bare run had the host's GitHub token: Claude Code pushed its own
  change to `main` (1d16475293) and closed #11257 itself, skipping the
  run's checks. The engines now run with no GitHub credential, `gh` and
  `openagents` refuse, and `git push` has no destination; the driver lands.
- The briefing copied `filefind.py` without `cards.py`, so the finder fell
  back to `lite`; it now copies both.
