# The GCE pool: `openagents cloud up/down/status` and `chat work --on gce`

- Issue: [#10225](https://github.com/OpenAgentsInc/openagents/issues/10225)
  (audit issues 4, 5 and 6; phase 2 of the
  [cloud parallel execution audit](2026-10-02-cloud-parallel-execution-audit.md))
- Code: `crates/openagents-cli/src/cloud.rs` (the pool, the host agent),
  `crates/openagents-cli/src/chat_gce.rs` (`chat work --on gce`),
  `crates/openagents-cli/src/chat_placement.rs` (the placement `--on boat`
  and `--on gce` share)
- Image: [`oa-coder-host`](../deployment/coder-host-image.md), baked daily
- Budget, accounts, the briefed engine on the pool:
  [#12058](https://github.com/OpenAgentsInc/openagents/issues/12058)

```text
export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config   # a service account, never a person
openagents cloud up --hosts 2              # grant the pool and start two hosts
openagents chat work --on gce --issues 10301,10302,10303,10304 --parallel 4 --land queue
openagents cloud status                    # hosts, live runs, idle minutes, $/hour, spend vs. budget
openagents cloud budget --daily 300        # the compute budget (the default)
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

## The engine on a pool host

`chat work --on gce` runs every engine on the pool. The default is the
briefed agent on Codex (`--engine briefed:codex`): the pool's hosts carry
the orchestrating computer's ChatGPT login copy and the pool's Claude
token, so a run that hands off to Claude Code can sign in. Each
host runs `openagents chat work --local --engine E`, which makes the
briefing, runs the agent with `verify` (it compiles, tests and formats on
the host), replays the checks on the real diff, and lands through the
landing queue with `--land queue` ([work-on-issues.md](work-on-issues.md)).
`--engine bare` runs the Coder issue flow there instead. Hosts build
`briefed-agent` beside `openagents`, and the host agent installs numpy for
the context finder. (`--on boat` still sends the briefed agent to
openagents.com's work hosts through `POST /v1/work`.)

## The compute budget

`~/.openagents/cloud/budget.json`, set with `openagents cloud budget`
(`crates/coder-cloud/src/budget.rs`):

| Limit | Default | Enforced at |
| --- | --- | --- |
| Spend over the last 24 hours (`--daily`) | $300 | every host start (`cloud up`, growth for `--parallel`, a replacement host): refused when what's spent plus an hour of the new hosts would pass it. Every run start: refused once it's spent. |
| Spend per run (`--per-run`) | $10 | each run: its session on the host is killed once its slot's share of the host's price reaches it |
| Hosts at once (`--max-hosts`) | 8 | `cloud up --max-hosts` can't exceed it; no start passes it |
| Idle minutes (`--idle-minutes`) | 10 | the most `cloud up --idle-minutes` may set; each host deletes itself after that long with no run |

Spend is estimated at list price from `~/.openagents/cloud/spend.json`, one
entry per pool host. A host is billed from GCE's creation time until it
was first seen gone (or until now), and never past its 12-hour backstop.
Every `cloud` command and every run's host check refreshes the entries. A host
that deletes itself between two listings is billed until the second one,
so the estimate errs high, never low. `cloud status` and `cloud budget`
show "about $X of $Y spent over the last 24 hours".

**Agents.** A process that acts for an agent (`--agent ID`, or
`OPENAGENTS_AGENT_ID`) starts hosts only under that agent definition ID's
own grant, `openagents cloud budget --agent ID --daily USD`
(`--agent ID --revoke` removes it). Without a grant, its `cloud up` is
refused with the command for the owner to run. Grants are keyed by agent
ID, never by name, so any custom agent can be granted what Foreman is.

## Accounts

Run the pool as a service account: the automation account's configuration
(`CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config`). A person's login,
such as `chris@`, expires daily and can't reauthenticate without the
person. `cloud up` records the configuration and the account in the pool
record. Every later gcloud call of the pool, ssh's IAP tunnel included,
uses that configuration when the environment names none, so an agent's
tool call without `CLOUDSDK_CONFIG` still runs as the service account.
`cloud up` warns when gcloud is signed in as a person, and `cloud status`
names the account. Each gcloud call has a 10-minute limit. The audit of
every call and the roles behind it is in
[automation-account.md](../deployment/automation-account.md#the-gce-pool-and-foremans-fan-out).

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
| `CLAUDE_CODE_OAUTH_TOKEN` (every engine, for the hand-off to Claude Code) | `CLAUDE_CODE_OAUTH_TOKEN`; Secret Manager `coder-pool-claude-token` (owner-approved pool token, #12139) |
| `OA_CODEX_AUTH` (preferred) | this computer's `~/.codex/auth.json` (`$CODEX_HOME`, or `OA_CODER_CODEX_AUTH`) | Codex on the owner's ChatGPT login: a copy with the **refresh token blanked**, so no run can rotate it (ChatGPT refresh tokens are single use: Codex's `refresh_token_reused`) and this computer stays signed in. The access token (10 days) must have 2 h left; Codex refreshes it on the Mac within 5 min of expiry. Coder then runs Codex `gpt-6.1-sol`, its first choice |
| `OA_CODEX_API_KEY` (only without a ChatGPT login) | `OA_CODER_OPENAI_API_KEY`; Secret Manager `coder-openai-api-key` |
| `OA_GIT_NAME`, `OA_GIT_EMAIL` | the same variables; `git config user.name/email` |
| `OPENAGENTS_APP_TOKEN`, `OPENAGENTS_ORIGIN` | this computer's openagents.com sign-in (`OPENAGENTS_APP_TOKEN`, else `coder login`), so each run shows on the account's agent list (`/settings/agents` and the phone) with Stop and Message (#11228) |

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
Claude Code signs in with `CLAUDE_CODE_OAUTH_TOKEN` from the run's own
process environment (#12139): every engine carries it, so a briefed Codex run
that hands off to Claude Code after repeated verify failures no longer stops
at "no Claude sign-in in this environment". The token is never written to the
host's disk (no `~/.claude` login, no env file); the runtime test
`turn_credentials_travel_in_memory_and_no_token_file_is_left` holds that.

## Operate

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
openagents cloud status
openagents cloud budget
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
