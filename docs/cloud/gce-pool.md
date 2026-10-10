# The GCE pool: `openagents cloud up/down/status` and `chat work --on gce`

- Issue: [#10225](https://github.com/OpenAgentsInc/openagents/issues/10225)
  (audit issues 4, 5 and 6; phase 2 of the
  [cloud parallel execution audit](2026-10-02-cloud-parallel-execution-audit.md))
- Code: `crates/openagents-cli/src/cloud.rs` (the pool, the host agent),
  `crates/openagents-cli/src/chat_gce.rs` (`chat work --on gce`),
  `crates/openagents-cli/src/chat_placement.rs` (the placement `--on boat`
  and `--on gce` share)
- Image: [`oa-coder-host`](../deployment/coder-host-image.md), baked daily

```text
openagents cloud up --hosts 2              # grant the pool and start two hosts
openagents chat work --on gce --issues 10301,10302,10303,10304 --parallel 4
openagents cloud status                    # hosts, live runs, idle minutes, $/hour
openagents cloud down                      # delete every host, revoke the grant
```

The computer that runs these commands only orchestrates. Nothing is built on
it.

## One granted computer, many hosts

`cloud up` writes the pool record, `~/.openagents/cloud/pool.json`:

| Field | Meaning |
| --- | --- |
| `computer` | `gce`, the placement name every route record carries |
| `pool` | the pool's name (`p` and six hex digits), on every host's `openagents-pool` label |
| `grant`, `epoch` | `gce:<pool>` and its revocation epoch: each grant after a `cloud down` is the next epoch |
| `revoked_at` | set by `cloud down`; `chat work --on gce` then refuses |
| `machine`, `spot`, `max_hosts`, `idle_minutes`, `slots_per_host` | what `cloud up` was given (defaults `c3-standard-8`, spot, 8, 10, 2) |

That record is the grant. Typing `cloud up` is the operator granting the pool
to this computer, as typing `--on boat` is for Boat. `chat work --on gce`
runs only under a live grant and never on another computer, and the route
record of each run names the computer and the grant
(`chat_placement::granted`, the same function the Boat runs use):

```json
{"schema": "openagents.cloud.run.v1", "issue": 10249, "pool": "p8029b3",
 "host": "oa-pool-p8029b3-e1a1", "zone": "us-central1-a", "spot": true,
 "outcome": "landed", "wall_seconds": 73,
 "placement": {"computer": "gce", "workspace": {"project": "OpenAgentsInc/openagents"},
               "grant": {"id": "gce:p8029b3", "epoch": 3, "source": "operator"}},
 "run": {"task": "241df65a…", "projection": {"state": "completed", "check": "verified"},
         "cost_microusd": 1700, "wall_ms": 73000}}
```

Records append to `~/.openagents/cloud/runs.jsonl`. `--on cloud` is an alias
of `--on gce`.

The hosts themselves are found by label each time (`gcloud compute instances
list --filter=labels.openagents-pool=<pool>`): hosts delete themselves, so
GCE is the only list that stays true.

## Hosts

`cloud up --hosts N` starts hosts until the pool has N running, all at once:

- **Shape.** `c3-standard-8` (8 vCPU, 32 GB), 300 GB pd-balanced, from image
  family `oa-coder-host`, Shielded VM, no external address (Cloud NAT
  egress), service account `oa-coder-host@` (it reaches the sccache bucket
  and nothing else in the project). `--machine` picks another shape.
- **Capacity.** Spot in us-central1-a, b, c and f in turn, then on demand in
  the same zones, as the image bake does (spot `c3-standard-8` ran out in
  zone a twice on 2026-10-02). `--on-demand` skips spot. `OA_ZONES` and
  `OA_PROJECT` override the zones and the project.
- **Two runs per host.** Each run takes a slot lock
  (`~/.oa-pool/slot-0.lock`, `slot-1.lock`); a run that finds both taken
  exits 75 and its issue goes back to the queue. The issue flow leases its
  own Cargo target slot, so the first run gets the image's warm target and
  the second a cold one (sccache serves its dependencies).
- **Ready.** `cloud up` waits for the image's `OA_CODER_HOST_READY` on the
  serial console, then for the host agent, then builds `origin/main`'s
  `openagents` and `microcoder` on the warm target into `~/.oa-pool/bin`
  (one package per Cargo invocation, as the image runbook says). Later runs
  rebuild only when `origin/main` moved.
- **Self-delete.** The VM's startup script is the host agent
  (`cloud::HOST_AGENT`). A one-minute timer keeps `~/.oa-pool/busy` fresh
  while any run's process lives, and once it is older than
  `oa-pool-idle-minutes` (default 10) the host deletes its own VM through
  the compute API. `cloud up` grants `oa-coder-host@` the role
  `roles/compute.instanceAdmin.v1` **on that one instance**; it has no
  project role. If the delete is refused the host powers off instead (a
  stopped VM bills only its disk; `cloud down` deletes it). Every host also
  has a 12-hour maximum run duration as a backstop, and a spot host that is
  preempted is deleted, not stopped.
- **Images before 2026-10-03** lack `bubblewrap`, which Coder's run
  boundary and source-checkout guard need on Linux (the issue flow refuses
  without it). The host agent installs it at boot; `coder-host-setup.sh`
  installs it from the next bake on.

`chat work --on gce --parallel N` grows the pool to `ceil(N / 2)` hosts when
it has fewer, within the grant's `--max-hosts`. It never shrinks the pool:
only a host knows it is idle.

## Reach

`ssh` as user `coder` with a key of the orchestrating computer,
`~/.openagents/cloud/pool_ed25519` (made on the first `cloud up`). The public
half goes into each host's `ssh-keys` metadata, with project keys blocked
and OS Login off for those hosts. The connection runs through an IAP tunnel
(`gcloud compute start-iap-tunnel`; the account needs
`iap.tunnelResourceAccessor`, which the automation account holds). From
inside the VPC, `OA_POOL_SSH=internal` connects to the internal address
instead.

A run starts in a session of its own on the host (`setsid`), writing its
events to `~/.oa-pool/runs/<run>/out`; the orchestrator follows that file,
and a dropped connection is followed again from the next unread line (up to
five times). If GCE confirms that the host is gone or no longer running,
the issue resumes on another host in the same granted pool. When no slot is
available, the orchestrator starts a replacement within `--max-hosts`, trying
each spot zone before falling back to on demand. After three lost hosts, it
stops recovery and reports failure. A failed GCE listing or an SSH disconnect
alone never launches a duplicate run.

Each loss appends a `preempted` record to `runs.jsonl` before replacement starts,
with the host, task, timestamp, wall time, and cost. The final record includes
those records in `preemptions`. Wall time includes recovery; estimated cost
sums the run's slot share on each host. The replacement takes only the lost
task's claim and fetches its `coder/progress-<task8>` or
`coder/stranded-<task8>` branch (also accepting a full-task progress branch).
It restores the saved diff onto the latest default branch before the engine
starts, so the full recovered change passes the normal checks and landing.
When neither branch exists, it starts from scratch. A changed claim, fetch
error, or restoration conflict refuses recovery rather than discarding work.
Ctrl-C kills each running flow's session on its host and stops recovery.

## Credentials

The same reader as `--on boat` (`chat_boat::credentials`, engine logins
`api-keys`), read once on the orchestrating computer:

| Variable | Source, first found wins |
| --- | --- |
| `GH_TOKEN` | `OA_BOAT_GH_TOKEN`; Secret Manager `coder-pool-git-token`; `gh auth token` |
| `XAI_API_KEY` | `XAI_API_KEY`; Secret Manager `openagents-xai-api-key` |
| `OA_CODEX_AUTH` (preferred) | this computer's `~/.codex/auth.json` (`$CODEX_HOME`, or `OA_CODER_CODEX_AUTH`) | Codex on the owner's ChatGPT login: a copy with the **refresh token blanked**, so no run can rotate it (ChatGPT refresh tokens are single use: Codex's `refresh_token_reused`) and this computer stays signed in. The access token (10 days) must have 2 h left; Codex refreshes it on the Mac within 5 min of expiry. Coder then runs Codex `gpt-6.1-sol`, its first choice |
| `OA_CODEX_API_KEY` (only without a ChatGPT login) | `OA_CODER_OPENAI_API_KEY`; Secret Manager `coder-openai-api-key` |
| `OA_GIT_NAME`, `OA_GIT_EMAIL` | the same variables; `git config user.name/email` |

`GH_TOKEN` needs the scopes `repo` and `project`, as for `--on boat`
([boat-chat-work.md](boat-chat-work.md#credentials)): without `project` the
run lands and closes but says once that it could not read the board, and the
issue's Status stays where it was.

They travel inside the run's script on ssh's standard input, never on a
command line. The host saves the script with mode 600 and the script deletes
its own file before anything else runs. Nothing is in the image.

Coder gives Grok Build the `XAI_API_KEY` of the account's **login shell**,
not of the process that runs the flow (`coder::task::adapter::login`): with
the key only in the process environment Grok Build refuses with
"Authentication required" (seen 2026-10-02). So the run script writes the
key to `~/.oa-pool/engine.env` (mode 600), which `~/.profile` and
`~/.bashrc` source. That file goes with the host's disk when the host
deletes itself. With `OA_CODEX_AUTH` (the orchestrating computer's ChatGPT
login with its refresh token blanked) the script writes `~/.codex/auth.json`
(mode 600, replaced atomically by each run) and Coder runs Codex
`gpt-6.1-sol` on it, with no API key and no owner step. Without it, with
`OA_CODEX_API_KEY` the script pipes it to `codex login
--with-api-key` (an API-key login on the host's disk) and Coder runs Codex,
its first choice, as one lean `codex exec` session at `gpt-6.1-sol` medium
(#10275). Grok Build on the key runs `grok-4.7`, not the API's default
`grok-4.20-0309-non-reasoning`, which fakes edits (see `boat-chat-work.md`).
Claude Code is not signed in on pool hosts; subscription logins are pending
in `NEEDS_OWNER.md` ("Boat: choose how coding agents log in").

## Operate

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
openagents cloud status
gcloud compute instances list --project openagentsgemini --filter=labels.openagents-managed=coder-pool
gcloud compute instances get-serial-port-output HOST --zone ZONE --project openagentsgemini | grep OA_POOL
```

Serial markers: `OA_POOL_AGENT_READY` (the agent is installed),
`OA_POOL_IDLE_DELETE idle_seconds=N` (the host deletes itself),
`OA_POOL_IDLE_POWEROFF delete_http=N` (the delete was refused).

## Measured, 2026-10-02

Image `oa-coder-host-20261002`, spot `c3-standard-8` in us-central1-a, one
host, orchestrated from a VM in the same project over IAP (the Mac builds
nothing). Three `cloud up` runs:

| Step (seconds from the create call) | Run 1 | Run 2 | Run 3 |
| --- | --- | --- | --- |
| VM created | 14 | 11 | 28 |
| `OA_CODER_HOST_READY` | 65 | 66 | 58 |
| ssh and host agent ready | 66 | 67 | 82 |
| `openagents` and `microcoder` built: **ready for runs** | 223 | 134 | 149 |

The build is the warm-target delta: 157 s the first time (no sccache hits
for that day's workspace crates yet), 67 s later.

Issue [#10249](https://github.com/OpenAgentsInc/openagents/issues/10249), a
one-file documentation change, on the warm host:

| | |
| --- | --- |
| Run, claim to landed on `main` (commit `6b4dbe827d`) with checks | 73 s |
| Cost estimate (half a spot host for 73 s at about $0.17 an hour) | $0.0017 |
| Host lifetime: created 00:16:18 UTC, two runs, idle from 00:22:14, deleted itself (as `oa-coder-host@`, disk included) at 00:33:31 | 17 min, about $0.05 |

Earlier attempts on the same issue found the two gaps fixed above
(`bubblewrap`, the Grok key in the login shell) and one engine miss: Grok
Build's default model read the file, reported an edit it never made, and
the flow ended `unchanged` with nothing pushed.

## Cost

List-price estimate, us-central1 (`cloud::hourly_usd`; `OA_POOL_HOURLY_USD`
overrides it):

| | Per hour |
| --- | --- |
| Spot `c3-standard-8` and its 300 GB disk | about $0.18 |
| On-demand `c3-standard-8` and its disk | about $0.46 |
| One run's share (2 per host), spot | about $0.09 |
| An idle pool | $0: hosts delete themselves after 10 minutes |

Each issue comment says the run's wall time and its share of its host's
price.
