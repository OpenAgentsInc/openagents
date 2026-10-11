# Boat (formerly Ascii Box): our history, their API today, and a Rust SDK plan

- Date: 2026-10-02
- Status: in progress. B1 (`crates/boat`), B2 (streaming, following and killing commands), B3 (fixtures and the gated live test), B5 (`crates/boat-template`) and B6 ([`openagents chat work --on boat`](boat-chat-work.md), [#10220](https://github.com/OpenAgentsInc/openagents/issues/10220)) are on `main`.
- **2026-10-10 update ([#11256](https://github.com/OpenAgentsInc/openagents/issues/11256)):**
  after hosted Boat returned 502 and took Environments down, the SDK's
  default base is our own Boat-compatible service on GCE,
  [`oa-boat`](oa-boat.md). Hosted boat.dev is opt-in only (`BOAT_HOSTED=1`).
- Parent:
  [Cloud parallel execution audit](2026-10-02-cloud-parallel-execution-audit.md).
  This plan adds Boat as a second placement backend next to the GCE pool that
  the audit recommends.
- Method:
  - `git` history of this repository and of the private `OpenAgentsInc/coder`
    repository.
  - Boat's public docs (`https://docs.boat.dev/llms.txt` and the pages it
    lists), and its two OpenAPI files (`boat-v1.yaml` and the legacy
    `box-v1.yaml`).
  - Read-only calls to the live API with the owner's key, which is held only
    in the gitignored file `~/work/.secrets/boat.env`. Nothing billable was
    created. No key, response header, or account identity is reproduced here.

Link conventions:
- `OA@8f84d0` is
  `https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/<path>`.
- `CODER@c8821c` is
  `https://github.com/OpenAgentsInc/coder/blob/c8821c72eb/<path>`. That
  repository is private.
- Commit links are `https://github.com/OpenAgentsInc/<repo>/commit/<sha>`.

## 1. The name

**Box, from Ascii (ascii.dev), is now Boat (boat.dev).** The company is still
called ASCII, and its docs site is still built by the same team. Here is what
changed:

| Was | Is now |
| --- | --- |
| `https://ascii.dev` | `https://boat.dev` (301 redirect) |
| `https://docs.ascii.dev/box/api/v1` | `https://docs.boat.dev/api/v1` (301, then 308) |
| API base `https://ascii.dev/api/box/v1` | API base `https://boat.dev/api/v1` |
| "Box", `/boxes`, `BoxBearerAuth`, `X-Box-Org` | "sandbox", `/sandboxes`, `BoatBearerAuth`, `X-Boat-Org` |
| `BOX_API_KEY` | `BOAT_API_KEY`, with keys that begin `boat_` |
| npm `@asciidev/box-sdk` (last version 0.0.37, 2026-09-16) | npm `@boatdev/sdk` 1.5.0 (MIT) and PyPI `boat-sdk` 1.5.0 |
| CLI `box` | CLI `boat` (`curl -fsSL https://boat.dev/install \| sh`) |

What did not change:
- Sandbox IDs still start with `bx_`.
- The webhook and delete-confirmation headers are still named `X-Ascii-*`.
- The legacy base URL still answers. On 2026-10-02, authenticated
  `GET /limits` returned 200 on both `https://ascii.dev/api/box/v1` and
  `https://boat.dev/api/box/v1`.
- The docs still publish the legacy spec as `/openapi/box-v1.yaml`. Its title
  is now "Boat Public API v1".

From here on, this document says **Boat**.

## 2. What we built against Box

### 2.1 July 2026: a compatibility facade in this repository

- **The teardown.**
  `OA@8f84d0 docs/teardowns/2026-07-19-ascii-box-optibox-openagents-gcp-analysis.md`
  ([`5ddd0036f3`](https://github.com/OpenAgentsInc/openagents/commit/5ddd0036f3))
  read the public API, pinned at SHA-256 `9ae1e0b7…`, and the MIT TypeScript
  SDK `@asciidev/box-sdk@0.0.24`.
  - It described Box as "a hosted Linux-computer control plane": provision or
    resume a persistent machine, queue a Codex or Claude Code prompt, read
    events, and stop it into a filesystem snapshot. At the time each machine
    was a Hetzner CX33 (4 vCPU, 8 GB).
  - It recommended **not** replacing our Google substrate. It recommended
    serving a subset of the Box v1 API from our own base URL, so the
    unmodified SDK could act as a conformance client.
- **The facade.** Managed-sandbox epic
  [#9023](https://github.com/OpenAgentsInc/openagents/issues/9023) built a
  default-off Box v1 facade (SBX-03). It answered the pinned SDK over our own
  GCE sandboxes, and its staging live matrix passed. It is described in the
  parent audit, §1.1.
- **Removal.** The facade and everything under it were deleted in the
  2026-09-18 reset,
  [`dabc08102f`](https://github.com/OpenAgentsInc/openagents/commit/dabc08102fddd72118d710d644a69c5c4eab95a2).
  Only a pointer to the teardown survives on `main`, in
  `docs/protocol/2026-09-26-teardown-coverage.md`.

### 2.2 September 2026: a Rust SDK, Gym on the vendor, then our own Box server

All of this was in the private `coder` repository, and all of it is still
there (`CODER@c8821c`). It was never deleted. Feature work was paused by the
owner on 2026-09-06.

| Date | What | Where |
| --- | --- | --- |
| 09-05 | **A Rust SDK for all 59 Box operations.** Generated from the pinned OpenAPI, with `reqwest` and rustls. It has three-state `Nullable<T>` values, debug output that redacts credentials, no automatic retries, idempotent create and fork, polling helpers, an event cursor, and webhook signature checks. The default deadline is 660 s, which is the 600 s command limit plus margin | `crates/coder-box` ([`a45fe45cc1`](https://github.com/OpenAgentsInc/coder/commit/a45fe45cc1)). Its generator is `crates/coder-box/schema/generate.py` |
| 09-05 | **Gym on the vendor.** Gym attempts ran on Box, with local Docker as the fallback. Each Gym task became a named snapshot, and every attempt forked from it. Each attempt recorded what its box cost | [`c80ae0b008`](https://github.com/OpenAgentsInc/coder/commit/c80ae0b008), [`bb7773a513`](https://github.com/OpenAgentsInc/coder/commit/bb7773a513), [`6dc3ee4013`](https://github.com/OpenAgentsInc/coder/commit/6dc3ee4013) |
| 09-05 | **Contract fixtures.** A fixture for each of the 59 operations, plus redacted vendor captures. The capture recorded an account limit of 150 starts a day, which a repository-search experiment reached | `crates/coder-box/fixtures/`, `docs/box/parity-tracker.md` ([`9d7fb9bbb4`](https://github.com/OpenAgentsInc/coder/commit/9d7fb9bbb4), [`f1c667e8ed`](https://github.com/OpenAgentsInc/coder/commit/f1c667e8ed)) |
| 09-05 to 09-07 | **Our own Box-compatible server.** `coder-serve` served the Box API behind `BOX_API_BASE`, with these parts: `coder-box-control` (lifecycle, TTL, placement fencing), `coder-box-host` (a QEMU/KVM VM per box on GCE), and `coder-box-agent` (the agent in the guest). It covered environments, secrets, snapshots (capture, restore, resume, fork, named), webhooks, events, prompts, interrupt, desktop, hosted ports and SSH keys | [`d415e82720`](https://github.com/OpenAgentsInc/coder/commit/d415e82720), [`66fbd5604e`](https://github.com/OpenAgentsInc/coder/commit/66fbd5604e), [`274598afc3`](https://github.com/OpenAgentsInc/coder/commit/274598afc3), [`024931b1c3`](https://github.com/OpenAgentsInc/coder/commit/024931b1c3), [`07fc185d33`](https://github.com/OpenAgentsInc/coder/commit/07fc185d33) |
| 09-06 | **Gym moved to owned compute.** CoderCloud delegate lane | [`80a4adecb3`](https://github.com/OpenAgentsInc/coder/commit/80a4adecb3), [`95ac2b3f4c`](https://github.com/OpenAgentsInc/coder/commit/95ac2b3f4c) |
| 09-06 | **Phase 1 measurements** on our hosts: ready in 116.6 s cold and about 27.5 s warm. Gym tasks openssl and regex-log passed; password-recovery failed with `504 guest_timeout` | `docs/box/phase-1-proof.md` |
| 09-06 | **The pause.** "Feature expansion in this design is on hold" | `docs/box/2026-09-05-box-compatible-infrastructure-audit.md` |

**What worked.** The SDK, the fixtures, and Gym on the vendor's API (forking a
named snapshot per attempt). Our own server passed the SDK's contract test.

**Where it stopped.**
- Our own hosts were slower than the vendor's: about 27.5 s warm, against
  "a few seconds" for a Boat resume or fork.
- Production hosts still lacked their credentials, waiting on coder #398.
- Snapshots were proven on the Docker backend only.
- The scaler and pool work it depended on was switched off.
- The `coder-box-pool` host still runs in GCE today (see parent audit §2.2).

**What remains in the live account** (read-only, 2026-10-02):
- 14 archived sandboxes from 2026-08-23 to 2026-09-05.
- 10 named snapshots called `gym-*`, from 0.03 to 0.31 GB each, all from
  2026-09-05.
- One environment, `base`.
- The $20 plan, with 555.6 hours of `default` time remaining. Standard limits:
  100 sandboxes at once, and 12 starts a minute, 60 an hour and 200 a day.
- **5 API keys. 4 of them are unrestricted and never expire** (marked as
  grandfathered).

## 3. Boat today: the API and the product

Sources: `https://docs.boat.dev` (Quickstart, Pricing & Limits, Machine
Capabilities, Environments, Snapshots & Copies, Long-Running Tasks, Integrated
agents, API Keys, Webhooks, Billing, FAQ, Boat SDKs, Public API v1) and
`https://docs.boat.dev/openapi/boat-v1.yaml`. This document was written
against the spec with SHA-256 `9f5d55d6f722ada70109d32f804424c7514d896f7a876333890e058091a4656c`
(69 operations).

### 3.1 Machines, price, limits

| Size | $/hour | vCPU | RAM | Disk for your files |
| --- | --- | --- | --- | --- |
| `small` | 0.018 | 2 | 4 GB | 12 GB |
| `default` | 0.036 | 4 | 8 GB | 50 GB |
| `large` | 0.072 | 8 | 16 GB | 125 GB |
| `xlarge` | 0.200 | 16 | 32 GB | 251 GB (needs the $100 plan or higher, and allocation on request) |

- **Billing.** Per second, only while a sandbox runs. A stopped sandbox is
  free and keeps its snapshot. Egress up to 2 TB a month per sandbox is
  included.
- **Plans** set concurrency and start limits. Plan time comes back as machine
  time.

  | Plan | Sandboxes at once | Starts per minute / hour / day |
  | --- | --- | --- |
  | $20 | 100 | 12 / 60 / 200 |
  | $100 | 300 | 30 / 210 / 840 |
  | $500 | 1,000 | 65 / 420 / 1,680 |
  | $2,000 | 2,000 | 90 / 600 / 2,400 |

  Create, fork and resume each count as one start.
- **The machine itself.**
  - Ubuntu 24.04, Linux 6.8, x86_64. Docker with BuildKit.
  - `/dev/kvm` is available inside, so Firecracker or QEMU can run in a
    sandbox.
  - No GPU.
  - Hosts are AMD Ryzen 9 9950X with shared vCPUs. Boat's own benchmark of a
    Node.js build-and-test loop ran 1.5 to 2.9 times faster than Daytona, E2B,
    Freestyle and Modal at 4 vCPU.
- **Regions.** EU only: Germany, Finland and France. Round trips from the US
  are about 100 to 200 ms.
- **Trust.** SOC 2 is "in progress". Sandboxes are full VMs with sudo.

### 3.2 Lifecycle and persistence

- **States.** `provisioned`, `cloning`, `ready`, `idle` and `running` count
  as active; `stopped` and `archived` do not.
- **Create.** `POST /sandboxes` takes `type`, `ttlSeconds`, `env`,
  `environment`, `noEnv`, `setupScript`, `from` (a named snapshot), `failFast`
  and `org`. `failFast` returns within about 1.5 s when no capacity is free,
  instead of queuing.
- **Auto-stop.** The default lifetime is 1 hour, counted from create or
  resume, never from last activity. Boat has no idle timer, so the caller has
  to stop sandboxes itself.
- **Snapshots** are incremental and content-addressed. One is taken every
  minute while a sandbox is ready or idle, and one more on stop.
  - Captured: `/home/user` (including `.git` and, by default, `target/`
    directories), Docker named volumes, and your changes under `/etc`, `/usr`,
    `/opt` and `/root`.
  - Not captured: processes, memory, and the Docker build cache.
  - `.boxignore` excludes paths, using gitignore syntax.
- **Getting a sandbox back:**
  - **Resume** brings the same sandbox back.
  - **Fork** makes a copy of it as it is now.
  - **Template** (`POST /named-snapshots`, then create with `from`) deploys a
    frozen named snapshot.
  - **Download** pulls a snapshot's chunks through signed URLs.

  Resume, fork and template deploy all take "a few seconds, whatever the
  sandbox holds". Files stream in during the first moments, and a read waits
  until its file has arrived. Size can change on resume or fork.

### 3.3 Running work

- **Commands.** `POST /sandboxes/{id}/commands` runs synchronously by default,
  up to 600 s.
  - `detached: true` returns a process ID. Poll
    `GET /sandboxes/{id}/commands/{processId}`.
  - `stream: true` returns NDJSON frames as output arrives: `started`, then
    `stdout` and `stderr`, then `exit` or `error`. **Streaming is new since our
    SDK.**
  - Commands are never retried automatically. A `502 boat_direct_failed`
    means the command may already be running.
- **Integrated agents.** `POST /prompt` queues work for a harness: `codex`,
  `claude`, `pi`, OpenCode or Prime Agent. One sandbox can run many
  conversations in parallel. Supporting calls:
  - `GET /events`, which can be filtered by conversation
  - `GET /conversations`
  - `GET /prompts/{id}` for status
  - `POST /steer`, which is new
  - `POST /interrupt`
- **Files.** `GET` and `PUT /files`, `GET /artifacts`, and snapshot file
  reads that never contact the machine (`/snapshots/{id}/tree`,
  `/snapshots/{id}/files`).
- **Access.** Desktop stream, hosted ports on public HTTPS, SSH keys, and
  sharing with an organization.
- **Usage.** `GET /sandboxes/{id}/usage` returns one sandbox's machine time
  and list-price cost. `GET /limits` returns balances in seconds of `default`
  time.

### 3.4 Credentials

- **API keys.** Bearer `boat_…`. New keys carry a scope (actions, sandboxes,
  environments) and expire within at most 365 days. Scoped keys are refused
  on unknown routes.
- **Keys inside a sandbox.** The key written into a sandbox can manage only
  that sandbox.
- **Creating keys.** `POST /api-keys/scoped` needs a browser session or an
  admin key. `rotate` and `revoke` are new in the API.
- **Environments** are versioned templates. Each one sets the repositories,
  variables, secret files, and four switches: pass GitHub, pass secrets, pass
  Boat credentials, pass agent credentials.
  - `noEnv: true` withholds every account credential, and then only the `env`
    values you send reach the sandbox.
  - Fork, or resume with `noEnv`, scrubs owner credentials from the disk
    before the sandbox becomes reachable.
- **Agent logins.**
  - Claude Pro/Max and ChatGPT Plus/Pro/Team subscriptions connect once on
    Boat's dashboard. Boat then refreshes the tokens on its own servers before
    every start and prompt, and writes `~/.codex/auth.json` and
    `~/.claude/.credentials.json` into each sandbox.
  - API keys (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `CLAUDE_CODE_OAUTH_TOKEN`)
    work as well.

  This solves the refresh-token problem the parent audit raises, but **Boat
  then holds custody of the owner's subscription tokens**.

  > Note (October 8, 2026): Anthropic's terms forbid platforms from collecting,
  > storing, or intermediating Claude.ai credentials for their users. This path
  > may serve only the owner's own work on the owner's own plan, never customer
  > Claude plans. See [Bring your own Claude](claude-code-byo.md).
- **Webhooks.** At-least-once, signed (`X-Ascii-Signature`). Events cover
  ready, restored, error and archived.

### 3.5 API changes since our SDK's pin

Our SDK was pinned on 2026-09-05 at 59 operations. The current API has 69.

- **Renamed:** `boxes` became `sandboxes`, `deleteBox` became
  `deleteSandbox`, and `listBoxSnapshots` and `getLatestBoxSnapshot` became
  `listSandboxSnapshots` and `getLatestSandboxSnapshot`.
- **Paths:** `/boxes/{boxId}` became `/sandboxes/{sandboxId}`.
- **New operations:** `conversations`, `steer`, `usage`, `share`,
  `deleteSandboxSnapshots`, `listOrganizations`, `setActiveOrganization`,
  `createScopedApiKey`, `rotateApiKey`, `revokeApiKey`.
- **New fields:** NDJSON streaming on `command`, `failFast`, `from` and
  `setupScript` on create, `fast` on prompt, and `X-Boat-Org` / `org` on
  most calls.

## 4. Boat against what the audit needs

| Need (parent audit §3) | Boat | GCE spot pool (audit §4) |
| --- | --- | --- |
| Fast start for N runs | A fork or template deploy takes a few seconds, at "roughly constant cost regardless of how much the template holds". Starts are limited to 12 a minute and 200 a day on the $20 plan (840 a day on the $100 plan) | A host boot takes about 90 s (a September target, never measured), or seconds when a host is warm. GCE quotas only |
| Warm image: repo, toolchains, compiled `main` | A named snapshot of a `large` sandbox in which `origin/main` has been built. `target/` under `/home/user` is captured, and 37 to 43 GB fits in 125 GB. Rebuild daily | A baked GCE image (audit issue 2) |
| Isolation per run | A full VM per sandbox | A VM per host, with a worktree per run |
| Engine logins | Boat's subscription refresh (Boat holds the tokens), or `noEnv` plus per-sandbox API keys | Each host logs in itself, or API keys |
| Artifacts and results | `git push` from inside the sandbox (a GitHub token from the environment), `GET /artifacts`, snapshot file reads | `git push` plus a bucket |
| Scale to zero | Built in: a stopped sandbox is free. We must call stop, because the TTL counts from start | Hosts drain themselves (to build) |
| **Cost per run-hour** | **$0.072** for `large` (8 vCPU, 16 GB, no preemption), or $0.036 for `default` (4 vCPU, 8 GB; a September run measured `rustc` peaking at 3.7 GB on one crate) | About $0.07 per spot slot (4 vCPU, 12 GB, can be preempted), plus idle hosts if a floor is kept |
| Location and custody | EU only. Our code and logins sit with a vendor that has no SOC 2 yet | Our project, us-central1, our IAM |
| Heavy, sustained CPU | Shared vCPUs. Boat says sustained 100% CPU loads fit dedicated compute better | Dedicated vCPUs (spot) |

**When to use which.**
- **Boat first** for bursty fan-out of the owner's own Coder runs: it starts
  in seconds, costs nothing idle, and needs no fleet to run. At about $0.072 a
  run-hour, 16 runs for an hour cost about $1.15, the same as GCE spot, with
  no preemption and no hosts to look after.
- **The GCE pool** in four cases:
  - work that must stay in our project or the US
  - sustained heavy builds
  - more than about 200 starts a day without moving to a bigger plan
  - Boat being unavailable

  The router treats both as granted computers, so switching between them is
  a placement choice, never a silent substitution.
- **Measure before deciding** (issue B8). Boat's numbers above come from its
  own docs, not from our runs.

## 5. Rust SDK plan

### 5.1 Name and home

- **Crate.** `crates/boat` (package `boat`, `publish = false`), in this
  monorepo, where the Coder crates now live. It is a port of
  `CODER@c8821c crates/coder-box`, not a rewrite.
- **Contract.** The pinned spec goes in `crates/boat/schema/boat-v1.yaml`,
  with its SHA-256 recorded. `operations.json` lists all 69 operations.

### 5.2 Design

Carry over from `coder-box`, which was proven in September:
- Generated models and operations from the pinned spec, using the existing
  `schema/generate.py`, adapted for the renames.
- `Nullable<T>` for three-state fields, and an `extra` map that keeps unknown
  fields.
- One async `reqwest` client with rustls. No redirects. An HTTPS-only base
  URL, with plain HTTP allowed only on loopback for tests.
- A 660 s default deadline and a 32 MiB JSON response cap.
- Typed `Error::Api` carrying the status, the error envelope, the
  `requestId`, and `Retry-After`.
- Debug and Display output that never prints the key, URLs with secrets,
  desktop/VNC URLs, or payloads.
- Polling helpers for ready, prompt, detached command and deletion, with
  shared cancellation.
- An event cursor and webhook signature verification.

Add:
- **Auth.** `BOAT_API_KEY` from the environment, or a `SecretSource` that
  reads Secret Manager secret `boat-api-key`, held in the automation service
  account's project. `BOAT_API_BASE` overrides the default
  `https://boat.dev/api/v1`. The key never appears in logs, errors, traces or
  fixtures.
- **Organizations.** Optional `org` / `X-Boat-Org` on the client.
- **Streaming exec.** `exec_stream()` returns a `Stream` of `CommandFrame`
  (`Started`, `Stdout`, `Stderr`, `Exit`, `Error`) from NDJSON, with a bounded
  line buffer. `exec_detached()` starts a command and `wait_command()` polls
  it.
- **Retries.** Retry only on 429 and 5xx, with jitter, and only for
  idempotent reads and for create or fork carrying the caller's
  `Idempotency-Key` and the same body. Commands, prompts and stops are never
  retried. A `502 boat_direct_failed` comes back as "may be running".
- **New operations.** Typed `usage`, `conversations`, `steer`, `share`,
  scoped-key create, rotate and revoke.
- **Legacy base.** A test proves `https://boat.dev/api/box/v1` still answers
  while it exists. The SDK targets only the new base.

### 5.3 Testing

- **Contract tests**, offline. Port the 59 spec fixtures and the observed
  captures, renaming Box to Boat. Add fixtures for the new operations from
  the spec's examples. A check fails when the spec digest no longer matches.
- **Recorded fixtures**, read-only, refreshed with a capture script:
  `/me`, `/limits`, `/orgs`, `/sandboxes`, `/environments`,
  `/named-snapshots`, `/api-keys`. Account identities, emails, keys and
  request IDs are redacted.
- **Gated live test.** It is `#[ignore]` and runs only when both
  `BOAT_API_KEY` and `OA_BOAT_LIVE=I_ACCEPT_BOAT_COST` are set. Locally the key
  comes from `~/work/.secrets/boat.env`; in the cloud it comes from Secret
  Manager. It does the following:
  1. creates one `small` sandbox with `ttlSeconds=600`, an idempotency key and
     `noEnv=true`
  2. waits until it is ready
  3. runs `exec_stream("uname -a")`
  4. writes and reads a file
  5. stops it, deletes it, and polls the deletion
  6. confirms through `/sandboxes/{id}/usage` that the cost was below 1 cent

  Every step tears down even on failure. The test uses a scoped, expiring key
  (actions limited to sandbox create, read, exec, file, stop and delete), not
  one of the unrestricted keys.
- **No calls in the default suite.** CI and local `cargo test` never reach
  the network.

### 5.4 Placement: a Boat computer

```text
openagents chat work --issues --parallel N --on boat
        │  granted computer "boat" = the owner's Boat account (key in Secret Manager)
        ▼
for each run:  POST /sandboxes {from: "oa-coder-main-<date>", type: "large",
                                noEnv: true, env: {git + engine creds}, ttlSeconds}
               exec_stream("openagents chat work --issue <n> …")   (detached + poll for long runs)
               run pushes to main itself (fetch, rebase, retry) · logs → artifacts / GCS
               POST /stop  → stopped sandbox is free; delete after the result is recorded
```

- **The template.** A daily job forks yesterday's template, or creates a
  sandbox. It runs the repository's `.agents/setup`: clone or fetch, the
  toolchains, the `openagents` binary, and a `cargo build` of `origin/main`
  into a target slot. It then saves the named snapshot `oa-coder-main-<date>`
  and deletes old ones. This is the same contract as the GCE image (parent
  audit issues 2 and 10), so both backends share one setup script.
- **Starting a run.** The router's placement names `boat` explicitly. The
  computer's free capacity is the lower of the plan's concurrent limit and the
  remaining start budget from `GET /limits`. When either runs out, the run
  waits; it never moves to another computer without the caller choosing.
- **Engine logins.** By default `noEnv: true`, and the caller passes
  `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` from Secret Manager. Using Boat's
  subscription refresh for the owner's ChatGPT or Claude plan is a separate
  decision for the owner, because Boat would hold those tokens (issue B7).
- **Cleanup.** The host-side runner (or the CLI) stops each sandbox the
  moment its run ends, because Boat's TTL is not an idle timer. A run's TTL is
  set to its limit plus a margin as a backstop.
- **Results.** Each run records the sandbox ID, the template name and the
  cost from `/usage` in the issue comment.

## 6. Issues to open

- **B1. Boat: Rust SDK `crates/boat`, ported from `coder-box` and generated
  from pinned `boat-v1.yaml`.** All 69 operations, the renames, `X-Boat-Org`,
  redaction, no key in output.
- **B2. Boat: streaming and detached exec in the SDK.** NDJSON
  `exec_stream`, `exec_detached` and `wait_command`. No command retries;
  `boat_direct_failed` is "may be running".
- **B3. Boat: offline contract fixtures and a gated live test.** Port the 59
  fixtures, add the 10 new operations, a read-only capture script, and a
  live test gated by `OA_BOAT_LIVE` with a key that expires.
- **B4. Boat: key hygiene.** Owner revokes the 4 unrestricted, never-expiring
  keys. Mint a scoped, expiring key for the SDK and the live test. Store it as
  Secret Manager `boat-api-key` and keep `~/work/.secrets/boat.env` for local
  use only.
- **B5. Boat: daily `oa-coder-main-<date>` template.** A `large` sandbox runs
  `.agents/setup`, builds `origin/main`, saves a named snapshot and prunes old
  ones. Record its size and the time to build it.
- **B6. Coder: `--on boat` placement backend.** One template deploy per run,
  the issue flow inside the sandbox, an explicit stop and delete, and the cost
  from `/usage` in the issue comment. Capacity comes from `/limits`.
- **B7. Boat: decide how engines log in.** Either API keys with `noEnv`, or
  Boat's managed ChatGPT/Claude subscription refresh. Write down the custody
  trade-off and the owner's choice.
- **B8. Measure Boat `large` against a GCE spot slot against the Mac on the
  same issue.** Time to running, time for the delta build, total wall time,
  cost, and failures. Record it in `docs/cloud/`.
- **B9. Coder repository: retire the Box names.** `BOX_API_KEY` and
  `BOX_API_BASE` become `BOAT_*` and the default base becomes
  `https://boat.dev/api/v1`. Decide whether the owned Box-compatible server
  stays paused or is retired, along with its GCE host `coder-box-pool`
  (parent audit issue 1).

## 7. B5: the template, measured

First template `oa-coder-main-20261002`, built 2026-10-02 by the Cloud Run job
`oa-boat-template` from `40bfa2b842` (runbook:
[boat-template.md](../deployment/boat-template.md)).

| What | Measured |
| --- | --- |
| Shared setup (`coder-host-setup.sh --warm`) on a `large` sandbox | 1,031 s (17.2 min): packages 7 s, repo 2 s, rust 6 s, engines 6 s, `cargo fetch` 31 s; `openagents-cli` build 161 s + tests 80 s; `microcoder` 106 s + 44 s; `coder` 36 s + 193 s (some test targets did not compile on that main, `partial=true`); release `openagents` 357 s |
| Sizes | warm slot 93.2 GB, `~/.cargo` 1.5 GB, rustup 3.3 GB, `/home/user` 98.4 GB (sccache excluded by `.boxignore`) |
| Named snapshot | 108.2 GB (`sizeBytes`), saved in 10.2 s from the stopped sandbox |
| Fork to ready (`POST /sandboxes {from}` until `ready`) | 76.4 s |
| First command in the fork | fails until root-owned directories are repaired (runbook); the repair took 19.7 s |
| `cargo build -p openagents-cli` in the fork, nothing changed | **701 s**, recompiling 4 workspace crates (`coder`, `coder-labor`, `microcoder`, `openagents-cli`) |
| The same after fetching `main` (53 commits newer) | failed: `spark-primitives` needs protoc's well-known types (`libprotobuf-dev`, now in the setup script on `main`) |

What this shows:
- **The template is not yet warm in a fork.** Boat restores files lazily, and
  cargo reported the workspace under `/var/lib/ascii-lazy/retired/home/openagents`
  rather than `/home/user/openagents`, so workspace crates' fingerprints no
  longer matched and rebuilt. Reading about 93 GB of dependency artifacts
  through the lazy restore made even those 4 crates take 701 s. Boat's own
  docs warn that build artifacts "slow every restore down". Follow-up:
  [#10251](https://github.com/OpenAgentsInc/openagents/issues/10251).
- **Ready is 76 s, not "a few seconds"**, for a 108 GB template.
- The daily build itself works and costs about $0.05 a day (sandbox plus job).

### 7.1 Making a fork warm (#10251, #10274)

Measured 2026-10-03 on Boat `large` forks (8 vCPU, 16 GB), by hand over SSH
and with `boat-template probe`. "Restore" is the time until Boat's lazy
`ascii-lazyfs` mount on `/home/user` retires and the home is plain disk.

What was wrong with the first template:

- **The restore never finished.** Boat serves `$HOME` through a FUSE mount
  while a background extract fills the disk. On the 108 GB template a build
  waited on two 2 GB debug executables in the slot (`debug/codebase-kb`,
  `deps/openagents-*`); the restore's watchdog logged "no hydration
  progress" for over 20 minutes and cargo sat at 0 % CPU. That, not the
  path, was the 701 s.
- **The retired path.** When the mount retires it is moved to
  `/var/lib/ascii-lazy/retired/home`; a process whose working directory was
  inside keeps that path. Builds must start in a new process after the
  restore.
- **Mtimes.** The restore keeps some mtimes to the nanosecond and cuts
  others to the second. Cargo then saw a dependency newer than its
  dependent (`StaleDependency`, `max_mtime ... nanos: 0`) for every pair
  built within one second, and a fork's first build recompiled 53 crates.
- **Ownership.** The repair walked through the lazy mount took 3 to 100 s
  and missed directories the restore created later, so a fallback build
  failed with `Permission denied` in the slot (#10274).

The template on `main` now (`oa-coder-main-20261003` was the first pruned
one): the slot is pruned like the GCE image's (24.3 GB, template 33.0 GB
against 108.2 GB), mtimes are cut to whole seconds before the save, and
`openagents`/`microcoder` stay for `chat work --on boat`. Every fork runs
`scripts/cloud/boat-fork-ready.sh`: repair `~/.ascii`, wait for the
restore, repair the rest on plain disk (about 0.5 s).

| Approach (template) | Ready | Restore | First build in the clone | Worktree of `origin/main` |
| --- | --- | --- | --- | --- |
| Build at once on the lazy mount (33 GB) | 35.5 s | (not waited) | failed: `Permission denied` removing a build script, then `can't find crate` for rlibs the slot holds | failed |
| Wait for the restore, reading the slot ahead (33 GB) | 16.1 s | 118.9 s | 59.2 s, 43 crates | 71.3 s, 70 crates |
| Wait for the restore (33 GB), three forks | 13 to 16 s | 72.5, 160.6, 830.1 s | 56.8 to 58.7 s, 53 crates | 65.5, 66.1 s |
| Wait, then whole-second mtimes (33 GB), four forks | 7 to 62 s | 316.6, 437.1, 438.5, 439.0 s | **14.8**, 41.6, 44.9, 45.1 s, **1 crate** | 71.3, 121.0, 125.6, 141.1 s |
| `boat-template probe` of the rebuilt template `oa-coder-main-20261003b` (36.5 GB: the 33 GB layout plus the two kept binaries) | 60.2 s | 446.3 s | 19.7 s, 1 crate | 63.6 s, 70 crates |
| No target, sccache in GCS, 100 % hits (7.3 GB) | 8.7, 10.4 s | 32.4, 53.5 s | (cold target) 125.6, 230.0, 233.9 s | the same |

What the numbers say:

- **Builds in a fork are warm once the restore is done and mtimes are
  whole seconds**: the first build in the clone compiles only
  `openagents-cli` (14.8 s on a quiet machine, about 45 s on busy ones);
  the worktree build compiles only the 70 workspace crates, as on the GCE
  image (94 s there).
- **The restore is the cost, and it varies tenfold**: 33 GB took 72 s on the
  quietest fork and 830 s on the slowest (75 to 460 MB/s), slowest when
  several forks restored at once. The best fork-to-first-build was 16 + 73 +
  15 s, about 105 s; a typical one is 3 to 9 minutes (the probe of the
  rebuilt template: 528 s, $0.012). The restore log shows why: a machine
  that already holds a chunk uses it ("identical file already on this
  machine, not fetched"); the rest comes from object storage at 75 to
  100 MB/s. That is below what
  Boat lets us control: the mount gives no ordering (reading the slot ahead
  made it slower, 118.9 s), and a build before it finishes fails.
- **A real run works from it.** `openagents chat work --on boat --issues
  10277` (a throwaway docs issue) from `oa-coder-main-20261003b`: the run
  waited for the restore, used the template's binaries, and landed and
  closed the issue in 602 s for $0.0120 (#10274).
- **sccache from GCS instead of a snapshotted target** restores 7.3 GB in
  32 to 54 s, but a cold build with every compile a cache hit still took
  126 to 234 s (754 objects fetched from us-central1), and only hits when
  the target directory's path is the same as when the cache was written (0
  of 754 with a different path). It was not adopted; the bucket-scoped
  service account made for the test was deleted.
