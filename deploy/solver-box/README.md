# Solver box: the solver container and its dispatcher

Issue [#11560](https://github.com/OpenAgentsInc/openagents/issues/11560),
spec [docs/cloud/solver-on-gcp.md](../../docs/cloud/solver-on-gcp.md).

A GitHub issue goes to a pull request on a Google Cloud box from our own
Box service ([oa-boat](../../docs/cloud/oa-boat.md)), inside this container,
with every solver hand on `google/gemini-4-argon` through Vertex AI.

## The image

`us-central1-docker.pkg.dev/openagentsgemini/openagents/oa-solver-box:<commit>`
(and `:latest`), built by [`cloudbuild.yaml`](cloudbuild.yaml) from a pushed
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
| git, gh, python3 with numpy, ripgrep, bubblewrap, jq | Debian packages |
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
| `OPENAGENTS_TOKEN` | optional: an OpenAgents sign-in key, so the Coder loop reaches Argon through the OpenAgents gateway until its Vertex door ([#11562](https://github.com/OpenAgentsInc/openagents/issues/11562)) reads the token file above |

It runs `gh auth setup-git`, a blobless clone into `/work/repo`, then
`coder-new solver $OA_ISSUE --repo /work/repo --github $OA_REPO --engine
coder --coder-model M --briefer-model M --tester-model M --state
/work/state --plain --json`. Stdout is the solver's NDJSON, the script's own
`{"type":"solver_box_step"}` lines, and a last
`{"type":"solver_box_summary", ok, pr, outcome, cost_usd, checks, files,
added, removed, commit, error, summary, patch}` line (the patch is cut at
200 KB). Exit 0 when a pull request opened, 1 when the run ended without
one, 2 when it never reached the solver.

## The dispatcher

`oa-solver-box` ([`crates/solver-box`](../../crates/solver-box)) claims
solver runs from the website, makes a `large` box per run, installs Docker
on it, writes the credentials through the files API, runs this container
with `--privileged` (bubblewrap, the hands' run boundary, needs namespaces),
follows its output, reports progress lines and the result, and deletes the
box however the run ends.

```sh
set -a; . ~/work/.secrets/boat.env; set +a     # BOAT_API_KEY, BOAT_API_BASE
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
# One issue, no website: progress lines on stdout.
cargo run -p solver-box -- --local-issue OWNER/NAME#N
# Against the website: claim solver runs until stopped (or --once).
OPENAGENTS_WORK_HOST_TOKEN=... cargo run -p solver-box -- --name solver-1 --origin https://openagents.com
```

The GitHub token for `--local-issue` is `OA_SOLVER_GH_TOKEN`, else
`gh auth token`. The Artifact Registry pull token and the Vertex token come
from the dispatcher's own Google identity (`gcloud auth print-access-token`,
or the metadata server on GCE).
