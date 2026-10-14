# Solver box: the solver container and its dispatcher

Issue [#11560](https://github.com/OpenAgentsInc/openagents/issues/11560),
spec [docs/cloud/solver-on-gcp.md](../../docs/cloud/solver-on-gcp.md).

A GitHub issue goes to a pull request on a Google Cloud box from our own
Box service ([oa-boat](../../docs/cloud/oa-boat.md)), inside this container,
with every solver hand on `google/gemini-4-argon` through Vertex AI.

## The image

`us-central1-docker.pkg.dev/openagentsgemini/openagents/oa-solver-box:<commit>`
(the repository's tags are immutable, so there is no moving `:latest`; the first
build, `193aedc`, kept a `:latest` tag that can be neither moved nor removed,
and its gh is too old for the solver: never use it), built by [`cloudbuild.yaml`](cloudbuild.yaml) from a pushed
commit:

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
gcloud builds submit --no-source --config deploy/solver-box/cloudbuild.yaml \
  --substitutions _COMMIT=$(git rev-parse origin/main) \
  --project openagentsgemini --region us-central1 \
  --service-account projects/openagentsgemini/serviceAccounts/oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com \
  --gcs-log-dir gs://openagentsgemini-oa-mvp-cloud-build-logs/oa-solver-box
```

| Inside | Where |
| --- | --- |
| Release `coder-new` (also linked as `coder`) | `/usr/local/bin` |
| The briefing scripts (`scripts/filefind`, `scripts/coder`, `scripts/bench/briefed-ab`, `scripts/bench/traces`) | `/opt/oa-tools`, named by `CODER_ISSUE_RUN_TOOLS` |
| Rust 1.97.1 (rust-toolchain.toml) with rustfmt and clippy, build tools, protobuf | the `rust:1.97.1-slim-trixie` base |
| git, python3 with numpy, ripgrep, bubblewrap, jq | Debian packages |
| gh (current; Debian's 2.46 lacks fields the solver reads) | GitHub's apt repository |
| The entrypoint [`solver-box-run`](solver-box-run) | `/usr/local/bin/solver-box-run` |

## The entrypoint contract

The dispatcher mounts `/run/oa-solver` (a tmpfs on the box) into the
container. `solver-box-run` sources `/run/oa-solver/env` and deletes it:

| Variable | Meaning |
| --- | --- |
| `OA_REPO`, `OA_ISSUE` | `OWNER/NAME` and the issue number |
| `GH_TOKEN` | the GitHub token the clone, the issue reads and the pull request use |
| `OA_GIT_NAME`, `OA_GIT_EMAIL` | the commit author |
| `OA_SOLVER_MODEL` | every hand's model, default `google/gemini-4-argon` |
| `CODER_VERTEX_TOKEN_FILE` | `/run/oa-solver/vertex-token`, a Vertex AI access token the dispatcher rewrites every 20 minutes |
| `CODER_VERTEX_PROJECT`, `CODER_VERTEX_LOCATION` | `openagentsgemini`, `global` |
| `OPENAGENTS_TOKEN` | optional and unused while the Vertex door is set: the Coder loop's Vertex door ([#11562](https://github.com/OpenAgentsInc/openagents/issues/11562)) reads the token file above for every request, sends each hand's model (Argon by default) to Vertex AI's OpenAI-compatible endpoint, and counts its cost from Vertex's tokens |

It runs `gh auth setup-git`, a blobless clone into `/work/repo`, then
`coder-new solver $OA_ISSUE --repo /work/repo --github $OA_REPO --engine
coder --coder-model M --briefer-model M --tester-model M --state
/work/state --plain --json`. Stdout is the solver's NDJSON, the script's own
`{"type":"solver_box_step"}` lines, and a last
`{"type":"solver_box_summary", ok, pr, outcome, cost_usd, checks, files,
added, removed, commit, error, summary, patch}` line (the patch is cut at
200 KB). Exit 0 when a pull request opened, 1 when the run ended without
one, 2 when it never reached the solver.

## The box runner

`oa-solver-box` ([`crates/solver-box`](../../crates/solver-box)) runs one
issue: it asks our Box service for a `large` box (then `xlarge`, then
`default`, when Google has no capacity for the size; on demand, TTL 6 h, idle
stop 30 min, no env), makes `/run/oa-solver` and `/run/oa-solver-host` on
the box's tmpfs (mode 700), writes the env file, the Vertex AI token and an
Artifact Registry token there through the files API (mode 600), and starts
[`box-run.sh`](../../crates/solver-box/src/box-run.sh) detached: it puts
Docker on the box when the image lacks it, signs Docker in with the registry
token and deletes it, pulls this image and runs it with `--privileged`
(bubblewrap, the hands' run boundary, needs namespaces). The runner follows
the output as progress lines (`box`, `stage`, `briefing`, `thinking`,
`tool`, `checks`, `pr`, `cost`, `notice`), rewrites the Vertex AI token every
20 minutes, reads the box's usage and deletes the box however the run ends.
The result is in the work-run result shape (`ok`, `engine: "solver"`,
`cost_usd` = model plus box, unknown stays null, `secs`, `checks`,
`diff.{files,added,removed}`, `landed.url`, `error`) plus `box`, `patch` and
`reply`.

```sh
set -a; . ~/work/.secrets/boat.env; set +a     # BOAT_API_KEY, BOAT_API_BASE
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
cargo run -p solver-box -- --local-issue AtlantisPleb/coder-one-sandbox#1 --result /tmp/solver-result.json
```

The image is `crates/solver-box`'s `IMAGE` (a commit tag), or
`OA_SOLVER_IMAGE`, or `--image`. The GitHub token is `OA_SOLVER_GH_TOKEN`, else `gh auth token`. Google
access tokens come from `gcloud auth print-access-token`, or the metadata
server with `--metadata-token` (when the runner itself runs on Google
Cloud).

## The dispatcher

`oa-solver-box --serve [--site URL] [--name NAME] [--slots N] [--once]`
([`dispatch.rs`](../../crates/solver-box/src/dispatch.rs), #11562) takes
solver work runs from the site (`POST /v1/work-hosts/{name}/claim
{engines: ["solver"]}`), runs each with `solver_box::run_until`, and reports
its progress lines (`{secs, phase, text, data}`, `phase` being the kind) every
5 seconds, at least every 30, and then the result (`POST
/v1/work-hosts/{name}/runs/{id}`). The site's `{cancel: true}` stops the run
and deletes its box. The host token is `OA_WORK_HOST_TOKEN`, else the
`openagents-work-host-token` secret. The GitHub token is the claim's
`run.github_token` (or `run.env.GH_TOKEN`), else the pool's.

People ask through the site (`POST /v1/work {repo, issue, land: "pr",
engine: "solver"}`), the Coder SDK (`coder_sdk::cloud_solver::submit` and
`follow`), or the CLI: `coder solver N --cloud [--json] [--detach]`.
