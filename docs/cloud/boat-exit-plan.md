# The boat exit plan

Stage 6 of [#13474](https://github.com/OpenAgentsInc/openagents/issues/13474).
Dev boxes run on hosted boat, at the owner's opt-in (2026-11-13). This page
says how to leave it, when to, and how to know that the replacement works.
Nothing here is built on GCP yet: that needs the owner's yes and money,
and the trigger below hasn't happened.

## The switch

Everything that talks to boat goes through one SDK, `coax/lib/boat/`, and
the SDK takes its base URL from `BOAT_API_BASE`:

| Who | Setting |
|---|---|
| `coax/web/tool.sh boat` and Coder's `--on boat:ID` | `BOAT_API_BASE` (and `BOAT_HOSTED=1` only for boat.dev) |
| The /chat runner's devbox turns (`coax/web/agent/devbox.coax`) | `OA_BOAT_API_BASE` on the runner, passed to the engine as `BOAT_API_BASE` |

Point it at a Boat-shaped service and nothing else changes. The proper chat
path runs our own engine (Coder on our Claude engine) through boat's
commands, not boat's built-in agent, so the same path works on any host that
answers these calls.

## The contract our service must answer

Only the calls the SDK makes, in the shapes `coax/lib/boat/wire.coax` reads
(boat's v1 API, `docs.boat.dev/openapi/boat-v1.yaml`, read 2026-11-13):

| Call | What we need from it |
|---|---|
| `GET /limits` | `canStart`, `activeSandboxes`, `maxActiveSandboxes`, `starts.{minute,hour,day}` |
| `POST /sandboxes` | `from` (a template), `type`, `environment` or `noEnv`, `name`, `ttlSeconds`; honour `Idempotency-Key` |
| `GET /sandboxes`, `GET /sandboxes/{id}` | `id`, `name`, `state` (`ready`/`idle`/`running` up, `archiving`/`archived` down), `type`, `ip`, `sshEndpoint`, `ttlSeconds`, `archiveAfter` |
| `POST /sandboxes/{id}/stop`, `/resume`, `PATCH /sandboxes/{id}`, `DELETE /sandboxes/{id}` | Stop keeps the disk. Resume may change `type`. Delete wants `X-Ascii-Confirm-Delete` |
| `POST /sandboxes/{id}/commands` | Synchronous (`timeoutSeconds` up to 600), `stream: true` NDJSON, `detached: true` returning `processId` |
| `GET /sandboxes/{id}/commands/{pid}/stream` | NDJSON `attached`, `stdout`/`stderr` with `nextOffset`, `heartbeat`, `exit`, `reconnect`, from `stdoutOffset`/`stderrOffset`, every byte once |
| `GET`/`DELETE /sandboxes/{id}/commands/{pid}` | Status, and kill the whole process tree |
| `GET`/`PUT /sandboxes/{id}/files` | UTF-8 or base64 content, 5 MiB at most |
| `POST`/`GET`/`DELETE /saved-snapshots` | Templates by name, `status` `saving` then `ready` |
| `POST /sandboxes/{id}/sshkey` | `sshUser`, `machineIp` or `sshEndpoint`, and the `hostKey` we pin |
| `GET /sandboxes/{id}/usage` | `seconds` and `dollars`, for the daily cap |
| `GET /environments`, `PUT /environments/{id}/vars/{key}` | How a box gets its two tokens without a template holding them |
| Errors | The envelope `{ok:false, status, code, requestId}`, `Retry-After` on 429, and the codes `BtErrKind` names |

The deleted `oa-boat` service (`cb80cf6874`, #11256: Cloud Run in front of
one GCE VM per sandbox) is the model. Read it with `git show`.

## How to know it works

```sh
BOAT_API_BASE=https://boat.example.internal/api/v1 coax/web/tool.sh boat conform                 # every read, decoded by our codec
BOAT_API_BASE=... coax/web/tool.sh boat conform --box bx_XXXXXXXX                                # one box's get and usage
BOAT_API_BASE=... coax/web/tool.sh boat conform --lifecycle                                      # create, commands, attach, files, stop, delete
```

`conform` prints one line per call and fails on any answer our codec can't
read. `--lifecycle` makes one `small` box under the dev box policy and deletes
it, so it bills: run it only against our own service, or with the owner's yes
on boat. Against the loopback fake used for stages 2 to 5, every read and
the whole lifecycle passed on 2026-11-13.

## When to build it

Build it when one of these holds. Until then the money and the upkeep go
elsewhere.

- Boat is down for more than a working day, or more than twice in a month.
  For shorter outages, Coder's fallback to `coderos-4080` covers new sessions
  (stage 5, `bp_boat_down`).
- Boat changes its terms, its price above the plan, or where it keeps our
  disks in a way the owner won't accept.
- We need what boat can't give: a GPU box for goldens and frame times, or
  boxes in our own project for anything that serves users. Product
  sandboxes stay on our GCP in any case (protocol spec §24.1).

Cost to build: the audit's estimate for `oa-boat` was 3 to 5 agent-days, plus
GCE VMs at about the spot dev box's price ($3.30 a 12-hour day for
`c3d-highcpu-30`).
