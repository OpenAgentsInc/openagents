# Audit: Antigravity in Coder and the Golemancy lab

Written 2026-11-11. This records what was built and found when Google's
Antigravity CLI (`agy`) was added to Coder and to the Golemancers'
Workshop, and what six real tasks showed about where it fits. Issues:
[#13192](https://github.com/OpenAgentsInc/openagents/issues/13192) (closed) and
[#13193](https://github.com/OpenAgentsInc/openagents/issues/13193) (open
until the Codex runs are done).

## Antigravity in Coder (#13192)

- **Install.** The CLI is closed source; its repository holds a README, a
  changelog and examples. The official installer downloads one binary,
  checks its SHA-512 and copies it to `~/.local/bin/agy`; its last step,
  `agy install`, edits shell files and was skipped. Version 1.5.6 runs on
  NixOS as is.
- **Sign-in.** Google Cloud Application Default Credentials on the
  `openagentsgemini` service account, so runs use our credits. Coder reads
  the settings from `~/.openagents/coder/antigravity.env` (mode 600), which
  names the key's path, not the key.
- **How Coder drives it.** `agy acp` speaks the Agent Client Protocol, the
  protocol Coder's Cursor engine already used. Cursor and Antigravity now
  share one client and engine; what differs is a `CuAgent` value
  (`coax/lib/agent/cursor/transport.coax`). Antigravity's specifics are in
  `coax/lib/agent/engine/antigravity.coax`.
- **What works.** `--engine antigravity` (or `agy`), `/engine antigravity`
  and Shift-Tab; `run`, `fork`, `replay` and the terminal; `--ask` approvals;
  models and effort; the session log; resume; per-turn usage; missing-CLI and
  signed-out notices.
- **Routing.** Antigravity is a candidate only when `CODER_ROUTE_ENGINES`
  names it and no other engine is eligible. With no allowlist nothing
  changed.
- **Checked.** A stand-in `agy` (`coder/tui/agy_standin.coax`) covers the
  cases. Claude, Codex, Cursor, Microcoder and echo gave identical output in
  25 before/after runs. Real turns on our credits: allowed, it listed a
  folder and edited a file; denied, it stopped and left the file unchanged; a
  fork resumed the same Antigravity session.
- **Limits.** Pressing `a` allows once only, because `agy` has no "allow
  always". Cost is logged as unknown: `agy` reports tokens, not price.
  `agy`'s own settings let it read inside the workspace and write under the
  temp folder without asking, so Coder cannot refuse those. Only service
  account sign-in was tried. Aethyrforge's engine picker does not list
  Antigravity yet.

## Antigravity in the Golemancy lab (#13193)

- **Coding agent.** `EVERGLADE_CODER_ENGINE=antigravity`, or
  `EVERGLADE_TIER_FAMILY=gemini`, which also maps the rubric's tiers to
  Gemini models: Flash low for Haiku, Flash high for Sonnet, Argon for
  Opus, Carbon for Fable (Argon and Carbon at low thinking, see Speed). Claude stays the default and the picker stays
  Haiku.
- **Parity with the Claude coder.** The same rules, worktree, nudges, Stop,
  budget and per-turn ledger lines, in Next issue and in Autopilot. The
  coder ends with `REPORT: done|blocked SUMMARY` in place of Claude's
  in-process `report` tool. Tool limits are enforced through `agy`'s
  permission requests: rehearsal allows only reads and searches; web, paths
  outside the worktree, subagents, the browser, image generation and swarms
  are always refused. Turns are priced from the rates card, since `agy`
  reports no cost.
- **Golems board.** `coax/lib/agent/observe/antigravity.coax` reads `agy`'s
  conversations (`~/.gemini/antigravity-cli/brain/ID/.system_generated/logs/transcript.jsonl`),
  so the board and Verse OS show them working, waiting or idle.
- **Rehearsal.** The Workshop runs only on the Mac, so a headless driver
  (`apps/everglade-desktop/next-drive.coax`) calls `next.coax` as the Mac
  host does. Two read-only laps on `gemini-3.8-flash-low` picked real
  issues (#13218, #13198) and each reported done after one nudge.
  Antigravity cost about $0.06 and the picker $0.12. An earlier bug in the
  driver ran about 20 laps before it was stopped, about $1.26.
- **Not verified.** The Mac app was not built. No real (non-rehearsal)
  Antigravity coding run happened in the Workshop.

## Experiments: what Antigravity is good for

Method: six tasks rebuilt from this repository's history, each started from
the parent of a commit that did the work, given the commit's intent, and
judged by that commit's own checks. One run per task and configuration, in
fresh worktrees, with a 15-minute limit; nothing was pushed. The full
write-up, with the tasks and per-run records, is
[docs/research/2026-11-11-antigravity-task-fit.md](../research/2026-11-11-antigravity-task-fit.md).

| Configuration | Passed | Cost | Per pass | Median wall time |
| --- | --- | --- | --- | --- |
| `gemini-4-carbon-medium` | 4/6 | $7.38 | $1.85 | 713 s |
| `gemini-4-argon-medium` | 5/6 | $5.02 | $1.00 | 191 s |
| `gemini-3.8-flash-medium` | 5/6 | $4.34 | $0.87 | 375 s |
| Claude Sonnet 5.5 | 6/6 | $0.93 | $0.16 | 27 s |
| Claude Opus 5.5 | 6/6 | $2.11 | $0.35 | 38 s |

- **Where Antigravity does well.** All three Gemini models passed the
  rename, the code explanation and the writing-rules edit. Argon and Flash
  also passed the bug fix and the shared glyph-atlas change.
- **Where it falls short.** On the wasm-size feature, Argon and Flash got
  the exit codes right but printed the failure message instead of a warning.
  Carbon hit the 15-minute limit on the bug fix and the atlas change, with
  single replies taking 4 to 5 minutes, which looks like capacity for a model
  released the day before. Claude finished every task in under a minute;
  Argon took 2 to 14 times as long, and Flash used 30 to 64 steps on the
  larger tasks.
- **Recommendation.** Keep Claude as the rubric's default. Offer
  `gemini-4-argon-medium` as an opt-in for the Haiku and Sonnet tiers, and
  as Autopilot's overflow engine when Claude's plan limits run out, with a
  time limit and a check of the change against the issue. No Gemini on the
  Opus or Fable tiers yet; leave Carbon out until its latency settles.
- **Confidence.** Low for pass rates: one run per cell and one task per
  kind, so a one-task difference is noise. The speed and cost gaps were
  large on every task.
- **Spend.** $16.74 of Google credits for the 18 Gemini runs, plus under
  $0.30 for setup, within the $40 cap. Gemini runs are priced at Vertex's
  on-demand rates from the Cloud Billing catalog. The Claude dollar figures
  are list-price equivalents; those runs used the owner's subscription.
- **Not run.** Codex: its login is out of quota until 2026-11-13, which is
  what keeps #13193 open. A compiler task: the commits that fit have no
  published compiler build, and building one takes longer than the run
  limit. A multi-file refactor: cut for time.

## Speed

Antigravity's slowness was on the model's side, not Coder's. #13227 found
long medium-effort thinking, minute-long waits for a first token and dropped
prompt caches in the `global` location, so coderos-4080 now uses `us`, and a
model call agy gave up on now fails the turn. #13193's follow-up made low
thinking the default for Antigravity in Coder and the Workshop, and added a
stall watchdog: after 120 s of silence with no tool running, Coder cancels the
turn and asks it to continue, at most twice. Rerun on the six slow cases
(T2 and T5), all passed, in half the time or less except T5 on Carbon (900 s,
unfinished, to 667 s). Low effort was most of the gain (T2 on Argon: 100 s at
low, 253 s at medium). The watchdog fired once. This is six runs, one each.
Details: the research doc's "Why Carbon was slow" and "Speed fixes and
rerun".

## Bugs found and fixed

- **Coder ran Opus whatever model it asked for** (`3372907d82`). Since
  `7aa8d77f91` (#13183) the shared Claude engine compared the model Claude
  Code started with against its own catalog choice once initialize was
  answered. Claude Code's model list rewrote that choice to Opus, so the
  engine sent `set_model claude-opus-5-5` ahead of every owner's first
  message. This affected Coder, the turn runner, the Workshop's picker and
  coder, `tiers.coax` and the server engines, and is why the Workshop's
  Haiku picker billed its first turn as Opus. Owners' models are no longer
  touched; a check holds it and fails without the fix. A real
  `coder run --engine claude --model claude-sonnet-5-5` answered on
  `claude-sonnet-5-5`.
- **Gemini 3.8 Flash on Vertex was priced at half** (`351b057450`). The
  rates card (`coax/lib/gateway/card.coax`) held $0.75 / $0.075 cached /
  $3.75 per million, Vertex's batch and flex rates. On-demand is $1.50 /
  $0.15 / $7.50 (SKUs 2A35-AC68-14D6, 8658-6939-8BEC, 90FD-DC5F-DC55).
  Gemini 4 Argon now has its own row ($4.00 / $0.20 / $20.00, SKUs
  3BAE-62B7-C9B9, 3B3E-7322-E883, 1E84-F91D-8B95) instead of the Carbon
  fallback.
- Found while building the engine and fixed in `cebb49aaa5`: `coder replay`
  reported DIFFERENT for any fork that resumed its parent's Cursor or
  Antigravity session; `coder ls` ran an 11-letter engine name into the
  state column; `coax/lib/agent/cursor/tests.coax` loaded a moved file.

## Still open

- The Codex column of the experiments, after 2026-11-13, then #13193 closes.
- A Mac build and an Antigravity rehearsal in the Workshop window.
- Aethyrforge's engine picker listing Antigravity.
- The owner's own `gcloud` login has expired; everything here used the
  project's service account. `gcloud auth login` restores it.
- `coder/tui/smoke.py` fails two checks (psionic `run --json`, Microcoder's
  gateway replay), which fail the same way on the code before this work.
