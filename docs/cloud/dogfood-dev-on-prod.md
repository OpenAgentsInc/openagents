# Developing OpenAgents on our own production environment

2026-10-10. Goal: an agent takes an OpenAgents issue from the V1 board
(project 22) to code, builds it with the shared cache, tests it, lands it
on `main`, moves the board, and deploys, with no MacBook involved. Today
most of that runs on the owner's MacBook: dozens of agents, Rust builds,
worktrees, pushes and deploys. The Mac hit 0 GB free on 2026-10-10 and
holds every credential.

This page is the gap list and the record of what was fixed. The proof is
issue [#11224](https://github.com/OpenAgentsInc/openagents/issues/11224),
taken end to end on a cloud environment (see [Proof](#proof)).

## What already exists

| Piece | Where | State |
| --- | --- | --- |
| Daily GCE image with the repo, toolchains, a warm Cargo target, sccache on GCS, `claude`, `codex`, `gh` | [`oa-coder-host`](../deployment/coder-host-image.md) | Works; the bake had failed 4 days (gap 1, fixed) |
| Pool of self-deleting hosts that run `chat work --on gce` and land on `main` | [gce-pool.md](gce-pool.md) | Works, orchestrated from a computer that reads credentials |
| Shared build cache | sccache, `gs://openagentsgemini-autopilot-rust-sccache/oa-coder-host/` | Works on GCE hosts |
| Boat daily template and Boat runs | [boat-template.md](../deployment/boat-template.md) | Works, but `large` is 8 vCPU / 16 GB / 125 GB |
| Web Environments and Claude Code runs | [agent-work.md](../deployment/agent-work.md) | Production, owner only, public repos, no push credential |
| Web agent fleet (#11164), PR review and merge (#11169), deploy approvals (#11170) | `crates/agent-fleet`, `openagents pr`, deploy plugin | Closed; they run on whatever computer hosts the session |
| Board updates | `scripts/dev/issue-board.sh` (REST only) | Works anywhere `gh` has `project` scope |

## Gap list

Severity: **blocker** stops the loop; **major** works with a manual step or a
Mac; **minor** costs time or money.

| # | Gap | Severity | Fix | State |
| --- | --- | --- | --- | --- |
| 1 | **Image bake broken.** `oa-coder-host-image-daily` failed 2026-10-07..10: the 100 GB builder disk has 7.7 GB left when the `microcoder` warm build needs 20 GB. Every new host and environment boots a 4-day-old image. | blocker | Bake on 200 GB; pool hosts 300 GB (#11224) | Fixed: `6652765b03` + `5cb6c4ff92`, image `oa-coder-host-20261010` |
| 2 | **Environment size.** The workspace needs well over 100 GB of target. Boat `large` has 125 GB and 16 GB RAM: it fits only the pruned 24 GB slot, not an integrator's full build. | blocker for integration on Boat | Integrator and builders on GCE `c3-standard-22`, 300 GB pd-balanced, from `oa-coder-host` (`oa-dev-env-1`). Keep Boat for single-crate agent runs | Done (`oa-dev-env-1`) |
| 3 | **Shared cache.** Without it every environment rebuilds cold. | major | sccache on GCS is already wired into the image through `oa-rustc-wrapper` and the VM's own account. Measured below | Works |
| 4 | **Credentials live on the Mac.** Claude token (#11204) was only in `~/work/.secrets/claude-code-oauth-token`; the Jev key only in `typesafe.env`; Codex only in the Mac's `~/.codex`. | blocker | Secret Manager: `dev-claude-code-oauth-token`, `dev-typesafe-api-key` added; `coder-pool-git-token` (repo + project scopes) and `openagents-openrouter-api-key` already there. [`scripts/cloud/dev-env-session.sh`](../../scripts/cloud/dev-env-session.sh) reads them through the VM's metadata account into the shell and a mode-600 file on the VM's disk (`~/.openagents/dev-env.env`; `/dev/shm` is wiped by logind when the last ssh session ends), never printing them | Fixed |
| 5 | **gcloud on an environment.** The Mac uses a key file (`gcp-mvp-automation.json`). | major | Attach `oa-mvp-automation@` to the environment VM: gcloud acts as it through the metadata server, no key file | Fixed (`oa-dev-env-1`) |
| 6 | **Production deploy needs chris@.** `scripts/deploy/web.sh promote/shift` retries with chris@ because `oa-mvp-automation` lacks `iam.serviceAccounts.actAs` on production's runtime account `157437760789-compute@` (checked with `testIamPermissions`: none). Staging runs as the automation account. | blocker for production deploys from an environment | Owner grants `roles/iam.serviceAccountUser` on that one service account to `oa-mvp-automation` (NEEDS_OWNER.md). Production stays behind the #11170 approval | Owner step |
| 7 | **Deploy scripts assume the Mac's paths.** `web.sh` defaults `CLOUDSDK_CONFIG` to `~/work/.secrets/gcloud-sa-config`. On a VM that directory is absent, gcloud gets an empty config and falls back to the metadata account, so it works; the chat worker and Cloud Run deploys are `gcloud builds submit --no-source` / `run deploy` and need no Docker. | minor | Nothing required; documented | Works |
| 8 | **Git push to `main` from an environment.** | blocker | `GH_TOKEN` from `coder-pool-git-token`, `gh auth setup-git`, identity from the token's account | Proven (`6652765b03`) |
| 9 | **Board updates from an environment.** | major | `issue-board.sh` with the same token (`project` scope) | Proven |
| 10 | **Web Environments cannot push.** Setup machines get no GitHub credential, and only public repos. | major | Use the owner's GitHub connection for the owner's own environments; a per-run, repo-scoped token. Filed #11226 | Filed |
| 11 | **Concurrency.** One agent per Boat sandbox; GCE pool hosts run 2 slots each; the env runs any number of agents but they share one warm target (the #11121 run saw `can't find crate` from concurrent builds in one target). | major | One leased Cargo slot per agent (`crates/coder/src/task/targets.rs`, already used by the issue flow); hosts sized for 2-3 slots; sccache shares compiled crates between slots and hosts | Works for the issue flow; worktree agents must use the lease |
| 12 | **Integrator / serial landing.** On the Mac one integrator agent rebuilds, tests and lands in order. The issue flow's `--land main` rebases, checks and pushes per issue with a retry, which is serial landing per issue, but nothing holds a single integrator queue across hosts. | major | Run the integrator on `oa-dev-env-1` (the big box) and give it the landing queue; filed #11227 | Fixed: `openagents land` ([land-queue.md](land-queue.md)), integrator as `oa-land-worker` on the env |
| 13 | **Visibility.** Runs on a GCE host show in the issue comments, `runs.jsonl` on the orchestrating computer, and the serial console; not in the web fleet list or on the phone. | major | Report environment runs as fleet rows (`AgentRow`) to the web/phone. Filed #11228 | Filed |
| 14 | **The orchestrator is a computer.** `openagents cloud up` / `chat work --on gce` read credentials on the computer that runs them (Codex login from `~/.codex`). | major | Run the orchestrator on `oa-dev-env-1` with `dev-env-session.sh`; Claude Code on the owner's subscription token is the engine there (Codex's ChatGPT refresh token cannot be shared) | Works on the env |
| 15 | **Disk and cost limits.** Pool hosts delete themselves after 10 idle minutes and live at most 12 h; `oa-dev-env-1` is a spot VM that stops on preemption but has no idle stop. | minor | Stop the env when idle; about $0.33/h spot for `c3-standard-22` + $0.04/h for 300 GB. Proven 2026-10-10: `oa-dev-env-1` powered itself off after 1,805 idle seconds (`OA_DEV_ENV_IDLE_STOP`), and a start brought the integrator back | Fixed: `dev-env-agent.sh` powers it off after 30 idle minutes (#11227) |
| 16 | **CoderOS-4080 as a build host.** 28 cores, 125 GB RAM, but 58 GB free of 937 GB on 2026-10-10, close to the 50 GB floor. | minor | Keep it as an extra builder only after its disk is cleaned (never `~/openagents/target/release`, never Pylon); prefer GCE | Not used |
| 17 | **Stale hosts.** `coder-pool-w2v4` and `coder-box-pool-8b1t` (`c3-standard-8`, 200 GB) have run since 2026-10-02. | minor | Check whether Coder Cloud uses them; else delete | Noted |
| 18 | **What only works on the Mac.** Xcode, iOS builds, TestFlight uploads, macOS UI tests, desktop captures. | major | Environments hand Mac-only steps to a Mac linked to the account, through the Coder link, computer ops and the own-runs channel (#11080); signing and App Store keys stay on the Mac, store uploads need approval. [#11223](https://github.com/OpenAgentsInc/openagents/issues/11223), separate agent | In progress elsewhere |

## The loop on an environment

```text
openagents.com / phone           GCE project openagentsgemini
  issue on project 22  ───────▶  oa-dev-env-1 (c3-standard-22, 300 GB, image oa-coder-host)
                                   eval "$(scripts/cloud/dev-env-session.sh)"   # Secret Manager → shell
                                   openagents chat work --local --issues N --land main
                                     ├─ claims the issue, board → In Progress
                                     ├─ Claude Code (owner's subscription token) writes the change
                                     ├─ cargo build/test on a leased slot, sccache on GCS
                                     ├─ rebase, push to main
                                     └─ comment, close, board → Done
                                   scripts/deploy/web.sh stage   (staging, as oa-mvp-automation)
                                   production: approval (#11170), then promote/shift (needs gap 6)
```

Start an environment like `oa-dev-env-1`:

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config   # or Cloud Shell
gcloud compute instances create oa-dev-env-1 --project openagentsgemini --zone us-central1-b \
  --machine-type c3-standard-22 --provisioning-model SPOT --instance-termination-action STOP \
  --image-family oa-coder-host --image-project openagentsgemini \
  --boot-disk-type pd-balanced --boot-disk-size 300GB --no-address \
  --service-account oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com --scopes cloud-platform \
  --shielded-secure-boot --shielded-vtpm --shielded-integrity-monitoring \
  --labels openagents-managed=dev-env \
  --metadata "block-project-ssh-keys=TRUE,enable-oslogin=FALSE,ssh-keys=coder:$(cat KEY.pub)"
```

Reach it over IAP as `coder`. The environment holds deploy rights (it acts as
`oa-mvp-automation`), so it is the integrator's box; worker hosts keep the
narrow `oa-coder-host@` account that reaches only the cache.

## Proof

Issue [#11224](https://github.com/OpenAgentsInc/openagents/issues/11224)
(the bake's disk, gap 1) was filed on project 22 and taken end to end on
`oa-dev-env-1` (spot `c3-standard-22`, 300 GB, image
`oa-coder-host-20261006`, account `oa-mvp-automation@`), 2026-10-10. Nothing
ran on the Mac but the `ssh` over IAP that started the commands.

1. `eval "$(scripts/cloud/dev-env-session.sh)"`: GitHub (`AtlantisPleb`),
   Claude Code (owner's subscription token), Jev and OpenRouter keys from
   Secret Manager; gcloud as `oa-mvp-automation@` from the metadata server.
2. Built `origin/main`'s `openagents` and `microcoder` on the image's
   4-day-old warm slot: **333 s**, sccache on GCS 104 hits / 180 misses.
3. `openagents chat work --local --json --issues 11224 --parallel 1 --land main`:
   Coder claimed the issue, Claude Code (`claude-opus-5-5`) changed
   `scripts/cloud/build-coder-host-image.sh` (100 → 200 GB),
   `crates/coder-cloud/src/pool.rs` (200 → 300 GB, plus a test that ties the
   two), and two docs; `cargo test -p coder-cloud` passed (63 tests) under the
   build lease; Coder rebased, pushed **`6652765b03` to `main`**, commented the
   evidence, closed the issue, and the board moved it to **Done**. **257 s**
   from start to landed; sccache 232 hits / 267 misses for the run.
4. Deploy: the bake is that change's deploy. From the environment,
   `gcloud scheduler jobs run oa-coder-host-image-daily` started it as
   `oa-mvp-automation@` with no key file. It still ran out of disk (1.8 GB
   free before `coder-new`): the four warm packages' unpruned test outputs no
   longer fit 200 GB either. A second change from the environment
   (`5cb6c4ff92`) prunes each package's test executables right after its
   test build; the next bake, again started from the environment, **passed**:
   `oa-coder-host-20261010` (200 GB), free space 152 → 126 GB across the
   four packages, final warm slot 39 GB, bake 2,146 s, boot smoke 127 s
   (`fetch=ok sccache=ok`). The family now resolves to today's image.
5. This page and `dev-env-session.sh` were committed and pushed to `main`
   from the same environment.

What the run found and fixed on the way:

- Coder's engines get the login shell's environment **less every
  `*_TOKEN` variable**, so `CLAUDE_CODE_OAUTH_TOKEN` alone left Coder saying
  no agent was signed in. `dev-env-session.sh` now also writes Claude Code's
  own login file (`~/.claude/.credentials.json`, mode 600) from the token,
  which Coder's probe and the `claude` CLI both accept.
- `/dev/shm` is wiped by logind's RemoveIPC when the last ssh session
  ends, which took the session file with it; it now lives in
  `~/.openagents/dev-env.env` (`5cb6c4ff92`).
- The image's `/usr/local/bin/openagents` is the bake day's release build;
  the agent found it lacks `openagents lease` and used the slot's fresh
  debug binary. A fresh image (gap 1) fixes it daily.
