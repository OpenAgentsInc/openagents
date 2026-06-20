# gemini-fleet — "Gemini-on-GCP via opencode, PR-per-agent" fleet runner

A small, dependency-light runner that turns **one non-green product promise** into
**one [opencode](https://opencode.ai) coding agent driven by a first-party Google
Gemini model on Vertex AI**, working in **one isolated git worktree**, on **one
branch**, that opens **one PR** for human review.

It is the Gemini twin of `scripts/vertex-fleet/`. The engine is different
(opencode + Gemini instead of Claude Code + Claude-on-Vertex) but the **PR shape
is identical** — branch prefix `gemini-fleet/<promise>` — so the merge gate
`/tmp/fleet-merge.sh` treats these PRs the same way.

## Why Gemini-on-Vertex (cost)

Gemini is a **first-party Google model**. Run through **Vertex AI**, its usage
bills as a **Google Cloud SKU on project `openagentsgemini`**, which is covered by
the **GFS cloud credit ($85k)** — *not* charged to a card. That is the whole
point of this variant: keep the fleet running on credit-covered Google compute.

(Contrast: third-party/partner models on Vertex, and pay-per-token subscription
seats, still cost real money. Gemini-on-Vertex does not, while the credit lasts.)

## Engine: opencode, headless

opencode (`/Users/christopherdavid/.opencode/bin/opencode`, v1.17.8) is a
provider-agnostic Bun-based coding agent. It runs **non-interactively** via:

```bash
opencode run --model google-vertex/gemini-2.5-pro \
  --format json \
  --dangerously-skip-permissions \
  "<the task brief>"
```

- `opencode run [message..]` is the headless one-shot path (no TUI).
- `--model google-vertex/<gemini>` selects the Gemini model on the built-in
  `google-vertex` provider.
- `--format json` emits the raw JSON event stream (one event per line) → we save
  it as the run trace and parse `properties.info.cost` for cost.
- `--dangerously-skip-permissions` auto-approves edit/bash so the agent can do
  real work unattended. (opencode defaults to *allow* anyway; this is explicit
  and survives any restrictive global `opencode.json`.)
- opencode operates in its **current working directory**, so `worker.sh` `cd`s
  into the per-promise worktree before invoking it.

## The Gemini model

Default: **`gemini-2.5-pro`** (GA, strong coding model). opencode also lists
`gemini-2.5-flash` (cheaper/faster), and preview `gemini-3.x` ids. Override with
`--model`, e.g. `--model gemini-2.5-flash` or a fully-qualified
`--model google-vertex/gemini-3.1-pro-preview`. A bare id (`gemini-2.5-pro`) is
auto-prefixed to `google-vertex/gemini-2.5-pro` by `worker.sh`.

Proven live: `opencode run -m google-vertex/gemini-2.5-pro "...PONG..."` returned
`PONG` (exit 0) against project `openagentsgemini` / `us-central1`.

## Auth: ADC (credit-covered), local vs unattended GCE

opencode's `google-vertex` provider **auto-loads** when a project env var is set
and signs every request via `google-auth-library` with the `cloud-platform`
scope — i.e. **Application Default Credentials (ADC)**. No API key is read or
printed by these scripts.

Env (names only — no secret values; `worker.sh` exports these, overridable):

```
GOOGLE_VERTEX_PROJECT=openagentsgemini     # (or GOOGLE_CLOUD_PROJECT)
GOOGLE_CLOUD_PROJECT=openagentsgemini
GOOGLE_VERTEX_LOCATION=us-central1         # (or VERTEX_LOCATION; "global" also valid)
VERTEX_LOCATION=us-central1
```

- **Local (this Mac):** uses the owner's `gcloud auth application-default`
  credentials (already authed for `openagentsgemini`). RAPT can expire and need
  an interactive `gcloud auth application-default login` — the only "needs-owner"
  step locally.
- **Unattended GCE fleet:** the same code path uses the **service-account
  metadata-server ADC** (no key files, no reauth) — identical to how
  `oa-codex-control` authenticates. Grant the runner SA `roles/aiplatform.user`
  (+ `roles/serviceusage.serviceUsageConsumer`) on `openagentsgemini`; the env
  block above works unchanged.

> Fallback (documented, not used here): Google **AI Studio** API key
> (`GEMINI_API_KEY` on opencode's `google` provider, `google/gemini-2.5-pro`) has
> a free tier. The Vertex/ADC path is preferred because it bills the GFS credit
> and matches our existing GCP auth — so this runner ships on Vertex.

## Guardrails (enforced by the scripts)

- **PR-per-agent only.** Workers push `gemini-fleet/<promise>` branches and open
  PRs via `gh`. They **never** push to `main`.
- **No green flips.** The task brief forbids editing the product-promise registry
  or changing any promise state. Agents build the missing *piece*.
- **check:deploy is the merge gate.** Each worker runs `bun run check:deploy` and
  records pass/fail on the PR.
- **Open-PR dedup.** `assign.mjs` skips promises that already have an open
  `gemini-fleet/*` PR (so `git worktree add -b` can't collide).
- **No secrets printed or committed.** Only env-var *names* and public endpoints
  appear anywhere.

## Components

| File | What it does |
|------|--------------|
| `assign.mjs` | Fetches the public product-promise registry (`https://openagents.com/api/public/product-promises`, browser UA), selects N non-green promises with **buildable, non-owner-gated** blockers, emits one task brief each, and dedups against open `gemini-fleet/*` PRs. Same selector as vertex-fleet. |
| `worker.sh` | Given one assignment: `git worktree add` from `origin/main`, `bun install`, run `opencode run --model google-vertex/<gemini> --format json --dangerously-skip-permissions "<brief>"` with the Vertex ADC env, run `check:deploy`, commit to branch `gemini-fleet/<promise>`, push, open a PR with `gh pr create`. Emits a one-line JSON result; writes a full JSON-event trace. |
| `run.sh` | Orchestrator. Runs a few workers (sequential by default; `--parallel` opt-in), then prints PR URLs + per-worker `check:deploy` status + total cost. |

## Usage

```bash
# from the repo root of an openagents checkout
bash scripts/gemini-fleet/run.sh --count 3 --model gemini-2.5-pro

# pick specific promises
bash scripts/gemini-fleet/run.sh --ids energy.flexible_load_proof.v1,autopilot.decision_queue.v1

# preview selection + briefs without spending tokens
bash scripts/gemini-fleet/run.sh --count 3 --dry-run

# build + check:deploy but don't open PRs
bash scripts/gemini-fleet/run.sh --count 2 --no-pr
```

Flags: `--count N`, `--state red|yellow|planned|any`, `--model <gemini model>`,
`--ids a,b,c`, `--parallel`, `--dry-run`, `--no-pr`.

Results are written to `/tmp/gf-results.jsonl`; per-worker logs to
`/tmp/gf-logs/<promise>.agent.log`; assignments + briefs to `/tmp/gf-assignments/`;
full JSON-event traces to `~/work/gemini-fleet-traces/<date>/` (indexed in
`~/work/gemini-fleet-traces/index.jsonl`).

## Concurrency reality

All instances authenticate as project `openagentsgemini` and **share that
project's per-model Vertex quota** (RPM/TPM). Keep waves small and raise Gemini
quota deliberately via Cloud Quotas before going wide. See
`docs/2026-06-13-vertex-ai-anthropic-claude-runbook.md` and
`docs/launch/2026-06-20-cloud-agent-fleet-audit.md`.

## How this maps to the bigger fleet

This is the Gemini-on-credit twin of the PR-per-agent automation. To take it to
unattended GCE scale: run `run.sh` from a GCE instance (or have
`oa-codex-control` launch `worker.sh`) using the SA metadata ADC.
