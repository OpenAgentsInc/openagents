# vertex-fleet — minimal "Claude-on-Vertex, PR-per-agent" fleet runner

A small, dependency-light runner that turns **one non-green product promise** into
**one Vertex-powered `claude -p` coding agent**, working in **one isolated git
worktree**, on **one branch**, that opens **one PR** for human review.

It is the smallest honest version of the "spin up cloud agents, one per promise"
idea: real agents doing real, reviewable work, with hard guardrails.

## Guardrails (enforced by the scripts)

- **PR-per-agent only.** Workers push `vertex-fleet/<promise>` branches and open
  PRs via `gh`. They **never** push to `main`.
- **No green flips.** The task brief forbids editing the product-promise registry
  or changing any promise state. Agents build the missing *piece*, they do not
  declare victory.
- **check:deploy is the merge gate.** Each worker runs `bun run check:deploy` and
  records pass/fail on the PR. A failing check is reported, not hidden.
- **Small by design.** Defaults to a few workers on `sonnet`/`haiku` to control
  cost — Vertex bills per token.
- **No secrets printed or committed.** Only env-var *names* and public endpoints
  appear anywhere.

## Components

| File | What it does |
|------|--------------|
| `assign.mjs` | Fetches the public product-promise registry (`https://openagents.com/api/public/product-promises`, browser UA), selects N non-green promises that still have **buildable, non-owner-gated** blockers, and emits one task brief each (the weekend-assault template, scoped to one promise). |
| `worker.sh` | Given one assignment: `git worktree add` from `origin/main`, `bun install`, run `claude --bare -p "<brief>" --permission-mode acceptEdits --allowedTools "Bash,Read,Edit,Write"` with the Vertex env, run `check:deploy`, commit to branch `vertex-fleet/<promise>`, push the branch, open a PR with `gh pr create`. Emits a one-line JSON result (incl. `total_cost_usd`). |
| `run.sh` | Orchestrator. Runs a few workers (sequential by default; `--parallel` opt-in), then prints PR URLs + per-worker `check:deploy` status + total cost. |

## Usage

```bash
# from the repo root of an openagents checkout
bash scripts/vertex-fleet/run.sh --count 3 --model claude-sonnet-4-6

# pick specific promises
bash scripts/vertex-fleet/run.sh --ids energy.flexible_load_proof.v1,autopilot.decision_queue.v1

# preview selection + briefs without spending tokens
bash scripts/vertex-fleet/run.sh --count 3 --dry-run

# build + check:deploy but don't open PRs
bash scripts/vertex-fleet/run.sh --count 2 --no-pr
```

Flags: `--count N`, `--state red|yellow|planned|any`, `--model <vertex model>`,
`--ids a,b,c`, `--parallel`, `--dry-run`, `--no-pr`.

Models (all serve 200-OK on `locations/global`): `claude-haiku-4-5` (cheapest,
breadth), `claude-sonnet-4-6` (default), `claude-opus-4-8` (hardest tasks; pass
`--model claude-opus-4-8`).

Results are also written to `/tmp/vf-results.jsonl`; per-worker logs to
`/tmp/vf-logs/<promise>.{agent.log,cost.json}`; assignments + briefs to
`/tmp/vf-assignments/`.

## The Vertex env (names only — no secret values)

Claude Code runs against Google Vertex AI when these are set:

```
CLAUDE_CODE_USE_VERTEX=1
ANTHROPIC_VERTEX_PROJECT_ID=openagentsgemini
CLOUD_ML_REGION=global
ANTHROPIC_MODEL=<vertex model id, e.g. claude-sonnet-4-6>
ANTHROPIC_SMALL_FAST_MODEL=claude-haiku-4-5
```

`worker.sh` exports these for you (overridable from the environment). All three
Anthropic models are enabled on `locations/global`; the raw `:rawPredict` global
endpoint can return intermittent 404-HTML, but the Claude CLI rides through with
built-in retries.

## Auth: local vs unattended GCE scale

- **Local (this Mac):** Claude's Vertex path uses **gcloud Application Default
  Credentials**. Run once, interactively:
  `gcloud auth application-default login` then
  `gcloud auth application-default set-quota-project openagentsgemini`.
  The ADC refresh token's RAPT expires periodically and needs an interactive
  re-login — this is the only "needs-owner" step locally.
- **Unattended GCE fleet:** use the **service-account metadata-server ADC** (no
  key files, no RAPT, no interactive reauth) — identical to how `oa-codex-control`
  authenticates today. Grant the runner SA `roles/aiplatform.user` +
  `roles/serviceusage.serviceUsageConsumer` on `openagentsgemini`. The same env
  block above works unchanged; the metadata server supplies tokens automatically.

## Cost note (pay-per-token)

Claude-on-Vertex is **metered per input/output token** (Google Cloud billing,
partner-model pricing). N agents = N× token spend — this is real money, not free
off a subscription seat. Each worker records `total_cost_usd` from the agent's
JSON result; `run.sh` prints the wave total. Prefer `haiku` for breadth, reserve
`sonnet`/`opus` for hard tasks, and keep waves small. A tiny task runs in the
~$0.002–$0.03 range depending on model.

## Concurrency reality

All instances authenticate as project `openagentsgemini` and **share that
project's per-model Vertex quota** (RPM/TPM) — N agents contend for one pool per
model. Live grants comfortably support a small (≈5–8 agent) start on
haiku/sonnet; raise quota deliberately via Cloud Quotas before going wide. See
`docs/2026-06-13-vertex-ai-anthropic-claude-runbook.md` and
`docs/launch/2026-06-20-cloud-agent-fleet-audit.md`.

## How this maps to the bigger fleet

This runner is the **PR-per-agent merge automation** that the cloud-agent fleet
audit calls the highest-leverage missing piece. To take it to unattended GCE
scale: run `run.sh` from a GCE instance (or have `oa-codex-control` launch
`worker.sh`) using the SA metadata ADC, enable + raise the control daemon's
durable queue (`OA_CODEX_QUEUE_ENABLED`, `OA_CODEX_QUEUE_MAX_CONCURRENCY`), and
add per-wave spend caps + receipt accounting
(`openagents.resource_usage_receipt.v1`).
