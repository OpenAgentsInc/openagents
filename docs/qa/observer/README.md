# QA Observer execution loop (QA-2, #8907)

The Observer RUNS checks against live production surfaces — it does not
narrate what an agent read once. Each run executes the durable check
registry, evaluates typed expectations, writes a dated JSON results artifact,
and prints a bounded human summary. On sustained drift it emits (and,
only behind an explicit flag, executes) the GitHub issue command that files
the finding with probe evidence.

Program context: epic #8904 and the transcript program
(`docs/transcripts/252.md` / `252-notes.md` — Observer "reveals hidden
stuff". `docs/transcripts/2026-06-25-qa-agent-khala-dogfood-and-qa-on-every-push.md`).
Proof-design semantics live in `docs/assurance/`. This loop is the executing
monitoring layer for live product surfaces.

## Pieces

| Piece              | Path                                                              |
| ------------------ | ----------------------------------------------------------------- |
| Check registry     | `scripts/qa-observer-registry.ts` (`OBSERVER_CHECK_REGISTRY`)     |
| Executor           | `scripts/qa-observer.ts` (`pnpm run qa:observer`)                 |
| Tests              | `scripts/qa-observer.test.ts` (`pnpm vp test --root . scripts/qa-observer.test.ts`) |
| Results artifacts  | `docs/qa/observer/results/qa-observer-run-<runAt>.json`           |

## The registry

Each check is `{ id, surface, probe, expectation, cadence, severityOnDrift }`.
Probes are HTTP GETs against the production API (default base URL
`https://openagents.com`) or repo-local commands. Expectations are small typed
rules (`number_gt`, `timestamp_within_ms`, `array_non_empty`, `string_equals`,
`field_type`, `every_item_has_keys`) evaluated against the parsed probe body.

Seed checks:

| id                                  | Surface                                            | Severity on drift |
| ----------------------------------- | -------------------------------------------------- | ----------------- |
| `public.khala_tokens_served`        | `/api/public/khala-tokens-served`                  | high              |
| `public.khala_tokens_served_history`| `/api/public/khala-tokens-served/history`          | high              |
| `public.khala_model_mix`            | `/api/public/khala-tokens-served/model-mix`        | medium            |
| `public.khala_channel_mix`          | `/api/public/khala-tokens-served/channel-mix`      | medium            |
| `public.pylon_stats`                | `/api/public/pylon-stats`                          | medium            |
| `forum.launch_status`               | `/api/forum/launch-status`                         | medium            |
| `khala_sync.capture_health`         | `/api/internal/khala-sync/capture-health`          | critical          |

The khala-sync liveness check (changelog `last_version` vs capture checkpoint
`pushed_through_version`, #8556) has **no public probeable surface** — the
route is admin-bearer-gated. The check is therefore honestly `unrunnable`
unless `OPENAGENTS_ADMIN_API_TOKEN` is present in the executor environment.
The token is read from the environment only, sent as a header, and never
printed or written to artifacts.

## Honest states

- `pass` — probe ran, every rule held.
- `drift` — probe ran and a rule failed, OR the surface itself failed
  (HTTP non-2xx, network failure, non-JSON body). Failed rules and bounded
  evidence are recorded.
- `unrunnable(reason)` — the probe's precondition is missing (e.g. required
  bearer env absent, command not spawnable). Recorded with the reason.
  **Never silently passes** and never counts toward `pass`.

Exit code is 1 when any check with `severityOnDrift` `high` or `critical`
drifted in the run. 0 Otherwise. `unrunnable` alone never fails the run — it
is visible in the summary and artifact instead.

## Running it

From the repo root:

```sh
pnpm run qa:observer            # run checks whose cadence is due
pnpm run qa:observer -- --all   # force every check
pnpm run qa:observer -- --base-url https://openagents-monolith-staging-ezxz4mgdsq-uc.a.run.app
```

Due-ness is judged from prior artifacts in the results directory: a check is
due when no prior result exists or the newest one is older than its cadence.

## GitHub issue integration (spam-proof by default)

Sustained drift means the same check drifted in **>= 2 consecutive runs**
(judged from prior artifacts). For each sustained drift the executor prints
the exact `gh issue create` command it WOULD run (title
`QA Observer drift: <check id>`, label `qa-observer`, body carrying the
bounded probe evidence). It executes GitHub commands **only** behind the
explicit `--file-issues` flag. With the flag set it first looks for an open
`qa-observer` issue with the same title and comments on it instead of
creating a duplicate. A scheduled run without the flag can never spam issues.

```sh
pnpm run qa:observer -- --all --file-issues   # operator-invoked filing
```

## Scheduling on our cloud (Cloud Scheduler -> Cloud Run job)

The loop belongs on OUR Google Cloud (project `openagentsgemini`,
`us-central1`) as a Cloud Run **job** triggered by Cloud Scheduler — the same
project/SA pattern the monolith deploy script uses for its per-minute cron
(`apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh`
`--with-scheduler`). Non-interactive gcloud uses the workspace automation SA
config (`CLOUDSDK_CONFIG=<workspace>/.secrets/gcloud-sa-config`, SA
`oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com`).

Owner-authorized operator commands (do not run as part of landing this doc —
creating cloud resources is a deliberate operator step):

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
PROJECT=openagentsgemini REGION=us-central1

# 1. A container image that has the repo + pnpm install. Reuse the monolith
#    build pattern (gcloud run deploy --source builds via Cloud Build); for a
#    job the equivalent is `gcloud run jobs deploy --source .` from the repo
#    root with the command below.
gcloud run jobs deploy qa-observer \
  --source . \
  --command node \
  --args "--import,tsx,scripts/qa-observer.ts,--all" \
  --set-secrets "OPENAGENTS_ADMIN_API_TOKEN=openagents-monolith-admin-token-prod:latest" \
  --max-retries 0 --task-timeout 10m \
  --project "$PROJECT" --region "$REGION"

# 2. Scheduler trigger every 15 minutes (matches the shortest check cadence).
gcloud scheduler jobs create http qa-observer-cron \
  --project "$PROJECT" --location "$REGION" \
  --schedule "*/15 * * * *" \
  --uri "https://run.googleapis.com/v2/projects/$PROJECT/locations/$REGION/jobs/qa-observer:run" \
  --http-method POST \
  --oauth-service-account-email "oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com"
```

Notes for the scheduled shape:

- The job's filesystem is ephemeral, so scheduled runs cannot accumulate
  artifact history in-container. The bounded summary and per-check states go
  to stdout (Cloud Logging). Durable in-repo artifacts are produced by
  operator/agent runs that commit them. Wiring artifact upload to GCS (and
  reading prior runs from there so consecutive-drift works across scheduled
  runs) is the natural next increment and should reuse the monolith's GCS
  seam.
- Do NOT pass `--file-issues` to the scheduled job until the GCS-backed
  prior-run state exists. Without durable history every scheduled run sees
  streak 1 and the flag would be inert anyway, but the invariant is: filing
  stays operator-invoked or durable-state-backed, never blind.
- The admin token secret name above is the CFG-9 secret map's
  `openagents-monolith-admin-token-prod`. Granting it to the job makes the
  khala-sync liveness check runnable in the scheduled shape.

## First real run

The first committed evidence artifact is in `results/` (see the newest
`qa-observer-run-*.json`), produced by `pnpm run qa:observer -- --all`
against production.
