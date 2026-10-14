# Solver in the cloud: a GitHub issue to a pull request on a Google Cloud box

2026-10-13. Umbrella [#11559](https://github.com/OpenAgentsInc/openagents/issues/11559);
the container and box runner [#11560](https://github.com/OpenAgentsInc/openagents/issues/11560),
the web [#11561](https://github.com/OpenAgentsInc/openagents/issues/11561), the Coder door,
dispatcher and entry points [#11562](https://github.com/OpenAgentsInc/openagents/issues/11562).

A person asks a web chat (or the Coder SDK, or the CLI) to solve a GitHub
issue. [Solver](../coder/solver.md) takes it to a pull request on a Google
Cloud box of its own, inside our solver container, with every hand on the
Coder loop and every model call on **Gemini 4 Argon** through Vertex AI. The
progress shows in the same chat, the pull request link ends the chat's turn,
and the box is deleted. Nothing runs on the person's computer.

## The flow

1. **Ask.** In a chat (new or existing) with a project that has a
   repository, the person picks **Solver in the cloud** under "Where it
   runs" and names an issue (a link, `OWNER/NAME#N`, or `#N` in the
   project's repository). Ids are read only after that choice; there is no
   keyword matching, and nothing under an answer offers it (no buttons
   under answers, no second chat). The Coder SDK
   (`coder_sdk::cloud_solver`), the CLI (`coder solver N --cloud`) and the
   `/work` page's "Who works on it" send the same request.
2. **Queue.** The website records a work run (`POST /v1/work`
   `{repo, issue, land: "pr", engine: "solver"}`, `crates/openagents-web/src/work_runs.rs`)
   tied to the chat. The chat shows one task row linked to `/work/{id}`.
   Under the person's session it also asks the account service for a
   credential-broker ticket for the repository (`oa_auth::repos::broker`,
   good for the run's 6-hour wait and an hour more); the ticket, not a
   GitHub token, waits in the run's record and is never shown.
3. **Box.** The box dispatcher (`oa-solver-box`) takes solver runs only
   (`POST /v1/work-hosts/{name}/claim` `{engines: ["solver"]}`) and asks the
   Box SDK (`crates/boat`, against our own [oa-boat](oa-boat.md) on GCE; never
   hosted boat.dev) for a box: a `large` on-demand (then `xlarge`, then
   `default` when Google has no capacity for the size) sandbox from the
   `oa-coder-host` image, its env holding no credential.
4. **Repository.** The dispatcher writes the run's credentials to the box
   through the files API (mode 600, outside snapshots, deleted by the run's
   first step): the requester's GitHub token when the account has connected
   GitHub (the claim's `run.github_token`: the website redeems the run's
   ticket once, when the dispatcher takes the run, for an installation
   token scoped to that one repository, answers it to that host only with
   `Cache-Control: no-store`, and drops the ticket from the record),
   otherwise the pool's git token, as cloud Coder does today
   ([boat-chat-work.md](boat-chat-work.md#credentials)); a short-lived
   Artifact Registry pull token; a Vertex AI access token, rewritten every
   20 minutes while the run lives. The box pulls the solver container and
   makes a blobless clone of the repository.
5. **Solve.** The container runs `coder solver N --engine coder --plain
   --json` with every hand (Intaker, Briefer, Coder, Tester) on
   `google/gemini-4-argon` through the Coder loop's Vertex door: Vertex AI's
   OpenAI-compatible endpoint in project `openagentsgemini`, location
   `global` (the only location that serves Argon), bearer from the token
   file. Issue Sense questions still go to the Decisions API (Clef first).
   No step or time budget is added.
6. **Stream.** The dispatcher follows the solver's NDJSON and reports it
   (`POST /v1/work-hosts/{name}/runs/{id}`) as progress lines: `box`,
   `stage`, `briefing`, `thinking`, `tool`, `checks`, `pr`, `cost`,
   `notice`. The chat shows the latest twelve under the task row in plain
   words as they arrive (`work_runs::solver_words`: "The computer is up",
   "Thinking: …", "Editing crates/…", "Checks: 2 of 2 checks passed"; the
   hands' own instructions and the computer's ids are left out). The
   Agents panel lists the run with its time and cost so far and a Stop
   that cancels it: the dispatcher's next report gets `{cancel: true}`,
   kills the run and deletes the box.
7. **Result.** The run's result carries the pull request URL, the checks,
   the files, the time and the cost; the chat's turn ends with one message:
   "Solver in the cloud finished OWNER/NAME#N in 2 min 49 s for $0.05
   ($0.04 model, $0.01 computer): checks 2/2 passed, pull request URL".
8. **Release.** The dispatcher deletes the box when the run ends, however it
   ends. A failed run's box is deleted too; its `change.patch` and
   `summary.json` are kept in the run's result first.

## What exists, what is new

| Piece | Exists | New here |
| --- | --- | --- |
| Box SDK and backend | `crates/boat`, `crates/oa-boat` (GCE sandboxes, files, commands) | nothing |
| Solver | `coder solver` in `crates/coder-new` (`--engine coder`, `--plain --json`, `summary.json`) | nothing in the loop |
| Argon | Vertex global; the gateway row and the Gemini dialect with thought signatures (#11546, #11548) | a **Vertex door** for the Coder loop (`CODER_VERTEX_TOKEN_FILE`, `CODER_VERTEX_PROJECT`, `CODER_VERTEX_LOCATION`) so a box needs no OpenAgents sign-in |
| Work runs | `/v1/work`, work-host claim and report, chat task rows, `/work/{id}`, the agent boards | engine `solver` ("Solver in the cloud"), claim by engine, GitHub token in the claim, solver progress kinds in the chat |
| Container | none | `deploy/solver-box/` (Dockerfile, Cloud Build, `solver-box-run`): coder, the briefing scripts, git, gh, python, the pinned Rust toolchain |
| Dispatcher | `chat_boat.rs` (the `--on boat` orchestrator) as the model | `oa-solver-box`: claim, box, credentials, follow, report, delete |
| Entry points | `POST /v1/work`, the composer's "Where it runs" | "Solver in the cloud" in the picker; `coder solver N --cloud`; `coder_sdk::cloud_solver` |

## Failure modes

| What fails | What the person sees | What happens |
| --- | --- | --- |
| No dispatcher running (`oa-solver-dispatch-1` down) | the row stays "Waiting for a computer" | the run is cancelled after 6 hours, as every work run |
| oa-boat has no capacity or refuses | "No computer was free" | the run fails; no box to delete |
| Box never reachable | "The computer did not start" | the box is deleted, the run fails |
| Container pull fails | "The solver could not start" | box deleted, run fails |
| No GitHub access to the repository | "Connect GitHub with access to OWNER/NAME" | box deleted, run fails |
| Vertex refuses (quota, 429, 5xx) | the hand's error, in plain words | the hand fails; Solver ends `failed`, no PR |
| Checks fail after the hand-backs | "Checks failed" with the failing checks | no PR; the diff is in the result |
| Dispatcher dies mid-run | the row stops updating | the run fails after 10 quiet minutes; the box stops at oa-boat's idle stop and TTL |
| The person presses Stop | "Stopped" | the dispatcher kills the run and deletes the box |

## Cost

- **Argon** on Vertex: $2 input, $0.10 cached input, $10 output per million
  tokens until 2026-11-30; $4, $0.20 and $20 from 2026-12-01. A small issue
  is roughly 0.3 to 1.5 million input tokens across the four hands, most of
  it cached reads after the first turn: about $0.10 to $1.
- **Box**: a `large` on-demand box (n2d-standard-8) is $0.338 an hour plus
  disk; a 10-minute run is about $0.06.
- The run's result names the model cost the solver counted and the box's
  billed dollars from oa-boat's usage; unknown stays unknown, never zero.

## The dispatcher host

Production's dispatcher runs on **`oa-solver-dispatch-1`** (GCE, project
`openagentsgemini`, zone `us-central1-a`, `e2-small`, Debian 13), as the
systemd service **`oa-solver-dispatch`**: `oa-solver-box --serve
--metadata-token --site https://openagents.com --name oa-solver-dispatch-1
--slots 2`. It takes up to two Solver runs at a time from openagents.com
and makes their boxes on oa-boat. The service restarts 10 s after any
exit, forever, and is enabled at boot.

- **Its account** is `oa-solver-dispatch@openagentsgemini.iam.gserviceaccount.com`:
  it reads the three secrets below, pulls the binary from the
  `openagentsgemini-coder-artifacts` bucket, and its metadata-server tokens
  are the boxes' Vertex AI token (`roles/aiplatform.user`) and Artifact
  Registry pull token (`roles/artifactregistry.reader` on `openagents`).
- **Secrets** (Secret Manager, project `openagentsgemini`), read at each
  start by `oa-solver-dispatch-start` into the process's environment only,
  never a file: `oa-boat-api-key` (`BOAT_API_KEY`, the same key the website
  uses for oa-boat), `openagents-work-host-token` (`OA_WORK_HOST_TOKEN`, the
  site's `OPENAGENTS_WORK_HOST_TOKEN`), and `coder-pool-git-token`
  (`OA_SOLVER_GH_TOKEN`, the pool's GitHub token, used only for a run whose
  requester has no GitHub token in the claim). A new secret version is
  picked up at the next restart.
- **Files** ([`deploy/solver-box/dispatcher/`](../../deploy/solver-box/dispatcher)):
  `cloudbuild.yaml` builds `oa-solver-box` on Debian trixie from a commit
  and puts it, the unit and the start script at
  `gs://openagentsgemini-coder-artifacts/oa-solver-box/COMMIT/`;
  `startup.sh` is the instance's startup script: on every boot it installs
  the commit named by the instance metadata `oa-solver-box-commit` into
  `/opt/oa-solver` and (re)starts the service.

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
H=(oa-solver-dispatch-1 --zone us-central1-a --project openagentsgemini)   # zsh or bash
# Logs (journald; also on the serial console)
gcloud compute ssh "${H[@]}" --tunnel-through-iap --command 'sudo journalctl -u oa-solver-dispatch -n 100 --no-pager'
gcloud compute instances get-serial-port-output "${H[@]}" | grep oa-solver-dispatch
# Restart
gcloud compute ssh "${H[@]}" --tunnel-through-iap --command 'sudo systemctl restart oa-solver-dispatch'
# Move to another commit: build it, point the host at it, rerun the startup script
gcloud builds submit --no-source --config deploy/solver-box/dispatcher/cloudbuild.yaml \
  --substitutions _COMMIT=$(git rev-parse origin/main) --project openagentsgemini --region us-central1 \
  --service-account projects/openagentsgemini/serviceAccounts/oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com \
  --gcs-log-dir gs://openagentsgemini-oa-mvp-cloud-build-logs/oa-solver-box
gcloud compute instances add-metadata "${H[@]}" --metadata oa-solver-box-commit=$(git rev-parse origin/main)
gcloud compute ssh "${H[@]}" --tunnel-through-iap --command 'sudo google_metadata_script_runner startup'
# Stop taking runs (runs already taken finish or are stopped by the site)
gcloud compute ssh "${H[@]}" --tunnel-through-iap --command 'sudo systemctl stop oa-solver-dispatch'
```

**The requester's own GitHub token (#11572):** when a Solver run is asked
for, the site asks the account service for a credential-broker ticket for
the run's repository. Without the GitHub App (production today) the ticket
is on the person's own GitHub OAuth grant: it is issued only when that
token holds `repo` (they connected **private repositories**) and GitHub
says they can push to the repository. At claim the ticket is redeemed
once, and the token goes only to the claiming host as `run.github_token`
(never logged or kept on the run), so the pull request is opened as the
person. If their access can't do it, the run is refused in the chat in
plain words with one link, **Connect private repositories**
(`/auth/github/repos?access=private`); `POST /v1/work` answers the same
error with `fix_path`. An account that installed the GitHub App gets the
App's installation token for that one repository instead. The pool's
token is used only when the site can't ask (no account service, or the
repository isn't one of the person's projects).

Each run logs one line when it is taken, naming which GitHub token it
uses ("with the requester's GitHub token" or "with the pool's GitHub
token", never the token), then its progress and its pull request or why
it ended. The solver image is the binary's `solver_box::IMAGE`; moving the
image means a new commit and the steps above.

## Operate

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
gcloud compute instances list --filter labels.openagents-managed=oa-boat --project openagentsgemini
gcloud artifacts docker images list us-central1-docker.pkg.dev/openagentsgemini/openagents/oa-solver-box
```

The dispatcher, the image and the Vertex door are documented with their
code: `deploy/solver-box/README.md` and `crates/solver-box`.
