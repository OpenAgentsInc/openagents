# Khala Terminal-Bench Through OpenAgents Smoke (#6272)

Status: live bounded smoke, 2026-06-25. This is an internal dogfood run, not a
decision-grade Terminal-Bench claim.

## What Happened

- Production D1 had one unapplied migration,
  `0234_pylon_openauth_links.sql`. That migration adds
  `agent_credentials.openauth_user_id`; without it, fresh agent registration and
  bearer-token inference auth both returned `500 internal_server_error`.
- Applied the pending migration to remote D1 `openagents-autopilot` with
  Wrangler OAuth. Afterward:
  - `POST /api/agents/register` returned `201`.
  - `GET /api/agents/me` with the fresh registered token returned `200`.
  - A tagged `POST /api/v1/chat/completions` request to `openagents/khala`
    returned `200`.
- The direct tagged probe reported `usage.total_tokens = 367`, and
  `GET /api/public/khala-tokens-served` moved by exactly `367`.

## Harbor Run

Run refs:

- `runRef`: `run.gym.terminal_bench.khala.6272.20260625T185215Z`
- `jobRef`: `job.gym.harbor_terminal_bench.khala.6272.20260625T185215Z`
- profile: `khala-public-heuristic`
- model requested through LiteLLM: `openai/openagents/khala`
- public model served: `openagents/khala`
- runner: Harbor
- agent: `terminus-2`
- dataset: Terminal-Bench 2.0

Shape:

```sh
OPENAI_API_KEY=<redacted> \
OPENAI_BASE_URL=https://openagents.com/api/v1 \
harbor run \
  --dataset terminal-bench/terminal-bench-2 \
  --agent terminus-2 \
  --model openai/openagents/khala \
  --n-tasks 1 \
  --n-concurrent 1 \
  --jobs-dir .tmp/terminalbench-6272-khala/jobs \
  --job-name khala-one-task-20260625T185215Z \
  --yes \
  --allow-agent-host openagents.com \
  --allow-environment-host openagents.com \
  --agent-kwarg api_base=https://openagents.com/api/v1 \
  --agent-kwarg max_turns=12 \
  --agent-kwarg 'llm_call_kwargs={"extra_headers":{"x-openagents-demand-kind":"internal","x-openagents-demand-source":"openagents-gym","x-openagents-client":"harbor-terminal-bench-6272"}}'
```

> Demand-origin self-tag (#6298): this run already sent
> `x-openagents-demand-kind: internal`, which classifies its token-ledger events
> and (with default-on capture) its captured traces as internal dogfood, kept
> out of the external real-user corpus. The CANONICAL internal source slug for
> the Terminal-Bench path is now `harbor_terminal_bench` — future Harbor runs
> should send `"x-openagents-demand-source":"harbor_terminal_bench"`. See the
> demand-origin self-tag convention in
> `docs/inference/2026-06-25-khala-heartbeat-runbook.md`.

Results:

- Harbor completed `1/1` bounded task with `0` exceptions.
- Task selected by Harbor: `make-mips-interpreter`.
- Reward: `0.0`; this was a path smoke, not a solve-rate claim.
- Runtime: `2m 24s`.
- Harbor stats: `97,417` prompt tokens, `3,601` completion tokens,
  `101,018` total tokens.
- Public Khala counter after the direct probe was `20,037,540`; after the Harbor
  run it was `20,313,289`. That larger delta includes this run plus concurrent
  live traffic, while the Harbor result's own token accounting is `101,018`.

## Gym Projection

The progress pusher posted the run twice:

- a running snapshot while the Harbor task was active;
- a completed snapshot after Harbor finished.

Public `/api/public/gym/run-progress` returned this completed row:

- `phase`: `completed`
- `decisionGrade`: `false`
- `publication`: `web_authorized`
- counts: completed `1`, passed `0`, failed `1`, running `0`, error `0`
- tokens: prompt `97,417`, completion `3,601`, total `101,018`

The same public projection also showed a separate `run.gym.terminal_bench.khala-live`
row still running at the time of this note. That run is separate from this
bounded #6272 smoke.

## Honest Boundary

This proves the path:

Harbor Terminal-Bench 2.0 -> Terminus 2 -> `openagents/khala` -> public token
counter -> Gym run-progress ingest -> public `/gym` projection.

It does not prove a decision-grade Terminal-Bench score, and it does not prove
GLM-via-Khala. GLM-via-Khala still depends on the GLM endpoint reachability,
durable-host, Worker secret arming, and verification smoke tracked separately.
