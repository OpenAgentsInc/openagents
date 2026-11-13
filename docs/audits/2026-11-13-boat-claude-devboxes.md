# Claude Code dev boxes on boat

Status: audit, 2026-11-13. Issue [#13474](https://github.com/OpenAgentsInc/openagents/issues/13474)
(project 22). This audit answers the dev box section of
[the build hosts audit](2026-11-13-build-hosts.md). It is a reading pass: it
created nothing, installed nothing and spent nothing.

## The answer

- **We can run Claude Code dev boxes on boat today.** The account works,
  the key is on this Mac, and every sandbox comes with `claude`, `gh`, Bun,
  Docker, Chrome and root. A box costs **$0.072 an hour** at the `large`
  size (8 vCPU, 16 GB, 111 GB of disk). That is about $0.86 for a 12-hour
  day, or about $53 for a month left running. The plan already includes
  about 277 `large`-hours this month.
- **What boat can't do:** no size has a GPU. Shots, goldens, frame times and
  acceptance runs stay on a GPU machine (the Mac, `coderos-4080`, or the L4
  host the build hosts audit proposes).
- **The account is on the $20 plan, not $100.** Its key is `box_20`. "Pro"
  is not a plan name in boat's docs or API. `xlarge` needs the $100 plan.
  The wallet is at -$1.38.
- **The fastest step is one box by hand, today, with no code** (stage 0
  below). Claude Code runs in `tmux` on it, and the box is saved as a
  template so the next one starts in seconds.
- **"Tied into our chat" is three to five agent-days of work.** That means
  a small Coax SDK in `coax/lib/boat/`, a `coder --on boat:ID` route, and a
  devbox conversation kind in the API so the phone can reach a box.
- **The owner has one step:** create a GitHub token for the boxes, scoped to
  this repository. It is in `NEEDS_OWNER.md`.

## What boat is (read 2026-11-13)

Sources: `https://docs.boat.dev/llms.txt` and the pages it links (machines,
pricing, limits, billing, snapshots, integrated agents, keys, API v1,
security, compliance), and a read-only `GET /limits` on our account. Boat
was called Box until recently, so old API names (`bx_` ids, `box_20`) still
show.

### Machines

| Size | vCPU | RAM | Disk | Per hour |
|---|---:|---:|---:|---:|
| `small` | 2 | 4 GB | 51 GB | $0.018 |
| `default` | 4 | 8 GB | 51 GB | $0.036 |
| `large` | 8 | 16 GB | 111 GB | $0.072 |
| `xlarge` (the $100 plan or higher) | 16 | 32 GB | 251 GB | $0.200 |

Every size is an x86_64 Ubuntu 24.04 VM with its own kernel, passwordless
`sudo`, systemd, Docker, `/dev/kvm`, a public address and outbound internet.
**No size has a GPU.** Graphics run on the CPU. Billing is per second while
a sandbox runs. The machines are in Germany, Finland and France. SOC 2 is
"in progress". Boat states a sandbox uptime of 99.95%.

The image includes `claude`, `codex`, `gh` 2.97, `git` 2.43, Bun 1.3.14,
Node 24, Rust 1.97, GCC 13, Clang 18, CMake, `rg`, `jq`, `ffmpeg` and Chrome
151. Boat updates the agents each time it prepares a machine.

### Stop, resume and snapshots

- **No pause.** A stop snapshots the disk. For 10 minutes boat keeps the
  machine, unbilled. A resume in that window usually gets the processes
  back too. After that, processes end, and enabled systemd services start
  again on resume.
- **What a snapshot keeps:** `/home/user`, folders you create at `/` (so
  `/nix` survives), and your changes to `/etc`, `/usr`, `/opt` and `/var`.
- **What it drops:** `/tmp`, `~/.cache`, the Docker build cache, the IP
  address, the hostname and the SSH host keys.
- **A stopped disk lasts at least 14 days.** A saved snapshot has no expiry.
  The plan includes 10 saved snapshots, and each one past that costs
  $0.056 a day.
- **A resume or fork is ready "in a few seconds, whatever the size of the
  disk".** File contents download in the background. A read waits only for
  the file it needs.
- **Auto-stop** defaults to 1 hour after a create or resume. It can be set
  up to 30 days on a paid plan, or turned off. A running boat prompt pushes
  the deadline. A process we start ourselves doesn't.

### Public URLs and access

`boat host ID PORT` gives `https://ID-PORT.on.boat.dev`. The URL is gated by
a token in it by default, so anyone holding the full URL can open it. SSH
works with a key you register (`POST /sandboxes/{id}/sshkey`). The address
changes on every resume, so an `~/.ssh/config` entry must be refreshed. A
desktop stream works in the browser.

### Plan, limits and spend: our account

All of these come from `GET /limits` on 2026-11-13 unless the table says
otherwise.

| Item | Value |
|---|---|
| Plan | $20 a month (`box_20`), billing active, plan renews (not set to cancel) |
| Included compute | 555.56 `default`-hours a period, all unused. That is $20 of time, or 277 `large`-hours. The period ends 2026-11-29 at 8:42 PM CST |
| Sandboxes at once | 100 |
| Starts (create, resume, fork) | 12 a minute, 60 an hour, 200 a day included. Extra starts cost $0.03 to $0.07 each, paid from the wallet |
| Sizes we can create | `small`, `default`, `large`. Not `xlarge` ("needs the $100/month plan or higher") |
| Wallet | -$1.38. Boat's docs say a negative balance is what you owe after a grace period |
| Running sandboxes | None. The last 50 are all stopped, the newest from 2026-10-10 |
| Saved snapshots | 16, against 10 included (see Risks) |
| Agent credentials in boat | None connected. The default environment `base` is "safe for third parties", so it passes nothing of ours to a sandbox |
| Command timeout (docs) | 30 s by default, 600 s at most. A detached command has no limit |
| API rate (docs) | 10,000 requests a minute per IP |
| Dollar cap (docs) | None on a personal account. With auto top-up off, spend stops at the plan and the wallet. When both are used up, boat gives 24 hours of grace, then stops the sandboxes and keeps their disks |

Overage past the plan is billed at the size's rate, from the wallet. Wallet
top-ups run from $15 to $100,000.

## Can a Claude Code session live there?

Yes. Each point below says what does it.

| Need | How it works on boat | Checked? |
|---|---|---|
| `claude` installed | Pre-installed and kept current by boat | Docs |
| Logged in, no browser | `CLAUDE_CODE_OAUTH_TOKEN` from `claude setup-token`, passed to that one sandbox (`noEnv: true` plus `env`). Our /chat runner already takes this variable. A token file exists at `~/work/.secrets/claude-code-oauth-token` (dated 2026-10-09). The other way is boat's dashboard sign-in, which keeps the token on boat's side and refreshes it | Docs; the token's validity wasn't checked |
| `gh` and pushes | `GH_TOKEN` set to a fine-grained token for `OpenAgentsInc/openagents` only. Boat's own "Connect GitHub" passes the owner's whole GitHub access, which is broader than a box needs | Docs; no such token exists yet |
| Survives a stop | `~/.claude` lives in `/home/user`, so `claude --resume` continues after a resume. The `tmux` process itself ends after the 10-minute window | Docs |
| How long it runs | A detached command has no time limit. The box runs until its auto-stop, which we set (up to 30 days) and extend. Turn auto-stop off only with our own cap in place (stage 2) | Docs |
| Repo and toolchain | Clone over HTTPS with the token. Nix and Bun install the way `coax/remote/host-startup.sh` does it on a build host (about 4 minutes there). The Coax compiler bootstraps in 15 to 21 minutes the first time, because the box can't read the build hosts' compiler bucket without GCP credentials. After that, save the box as a saved snapshot, and every later box starts from it in seconds | Estimated from `docs/metaverse/remote-builds.md`; not run on boat |
| First setup | About 30 minutes once: clone, Nix, Bun, compiler, first `coax test`. Then seconds per box | Estimate |
| Headless Chrome | Chrome 151 runs headless. With no GPU, WebGPU can only be software (SwiftShader). A shot can show that a page draws, but it doesn't count for goldens, frame times or acceptance, which the repo measures on a real GPU. Our `shot` flags (`--use-angle=metal`) are Mac flags, so a Linux box needs its own | Not checked |

### What may go on a box

| Allowed | Never |
|---|---|
| The Claude OAuth token, for that box only | Anything else from `~/work/.secrets/` |
| A fine-grained GitHub token for this one repository | The gcloud service account, Secret Manager access, deploy rights |
| The public repository and its build caches | Production database URLs, Breez, Spark, LND or treasury keys, Nostr secret keys |
| A boat key? No: the box doesn't need one | Our boat API key, or any `account.admin` key |

The repository is public, so boat holding the code is a small risk. The
tokens are the real risk. A stolen GitHub token can push to `main`, because
agents push `main` directly. So keep the token to one repository, rotate it,
and revoke it when a box is deleted.

## The Coax SDK

### HTTP API, not a wrapper around the CLI

- **The rule.** Tools are Coax ([tools-in-coax.md](../../coax/tools-in-coax.md)).
  A wrapper that parses the CLI's `--json` output is logic, not a thin
  wrapper.
- **Where it runs.** The hosted API runs on Cloud Run and can't depend on a
  third-party binary being installed.
- **Streaming fits.** The API documents an OpenAPI 3.1 contract (`v1`). Its
  streams are newline-delimited JSON (`stream: true` on a command, and the
  attach stream of a detached one). That is exactly what the gateway's curl
  transport already reads, line by line, without blocking
  (`coax/lib/gateway/transport.coax`, `gw_poll`).
- **The key stays hidden.** The transport takes the key through curl's
  `--variable`, never through an argument or a log.
- **Precedent.** The old Rust SDK was HTTP too.

The CLI stays for people: `boat ssh`, a quick `boat limits`.

### What the old Rust SDK teaches

Crate `crates/boat`, deleted by the Nuke. Read it with
`git show 853417f290:crates/boat/src/...`. It was added in `10bedbc3ee`
(#10216), and its plan is `docs/cloud/2026-10-02-boat-sdk-plan.md` at that
commit. It was about 10,200 lines. Most of that was code generated for 69
operations from a pinned `boat-v1.yaml`. The hand-written part was about
1,750 lines: `client.rs` 543, `helpers.rs` 453, `follow.rs` 429 and
`exec.rs` 329.

Its consumers were the old Coder cloud backend, the Environments operator,
the Computers driver (`BoatDriver`) and `openagents chat work --on boat`
(`chat_boat.rs`). The same API shape had our own server, `oa-boat`
(`cb80cf6874`, #11256): it ran on Cloud Run and gave each sandbox a GCE VM.
None of this survives on `main`. Neither the `oa-boat` service nor its VMs
exist on GCP today (checked with `gcloud`, read-only). So `BOAT_API_BASE` in
`~/work/.secrets/boat.env` points at nothing.

What to carry over:

1. **Retry classes.** Retry only GETs, and create or fork calls that carry an
   `Idempotency-Key`. Honour `Retry-After`. Never resend a command, prompt or
   stop. Treat `502 boat_direct_failed` as "may be running".
2. **Following a detached command.** Keep a byte cursor the caller can store,
   so output arrives exactly once across a reconnect. Kill the whole process
   tree. Today's API adds an attach stream with reconnect offsets every 10
   minutes, which replaces most of the old polling.
3. **Key hygiene.** The key never prints. Refuse boat.dev unless
   `BOAT_HOSTED=1`, which is the guard that kept hosted boat opt-in. Refuse
   redirects and plain HTTP except on loopback. The error type carries the
   status, code, request id and `Retry-After`, never the server's message.
4. **The fixtures.** 69 spec fixtures and 96 redacted captures of real
   responses. Port the ones for the calls we use as replay fixtures.
5. **What went wrong on 2026-10-02 and 10-03**, from `docs/cloud/boat-chat-work.md`:
   - Detached commands were refused (`400 sandbox_direct_failed`) for
     minutes after `ready`, on 7 of 12 starts.
   - A 33 GB template took 1 to 14 minutes to restore. Large binaries read
     before then failed.
   - Folders came back owned by root.

   Boat's docs now say a resume is ready in seconds and that reads wait per
   file. Stage 0 checks this, and the template stays small: Nix, Bun, the
   compiler and a clone, not a 45 GB build tree.

Leave out the generator. The account, keys, webhooks and organizations
calls stay out too. We need about 25 operations, written by hand.

### Layout

| File | What |
|---|---|
| `coax/lib/boat/wire.coax` | The one codec: request bodies and response readers for the calls below, plus error codes. Each shape gets a row in `proto/registry.json` (vendor, `boat.v1`), so `wire-check` covers it |
| `coax/lib/boat/client.coax` | `BoatClient { base, key_env, key_value, hosted }` on the gateway's curl transport. Sync calls use the gateway's request path. Streams use `gw_start` and `gw_poll` |
| `coax/lib/boat/sandboxes.coax` | Create (size, `noEnv`, `env`, `ttlSeconds`, from a saved snapshot), get, list, update (auto-stop), stop, resume, fork, delete, usage |
| `coax/lib/boat/commands.coax` | Run synchronously; stream (NDJSON frames `started`, `stdout`, `stderr`, `exit`, `error`); detached with attach, reconnect and a stored offset; status; kill |
| `coax/lib/boat/files.coax`, `snapshots.coax`, `ssh.coax` | Write and read files; save a saved snapshot and start from one; register an SSH key and write the `~/.ssh/config` entry after each resume |
| `coax/lib/boat/agent.coax` | Boat's built-in agents: prompt, conversations, events with a cursor, steer, interrupt |
| `coax/lib/boat/policy.coax` | `#authority`, zero surviving mutants: which sizes, how many boxes, auto-stop bounds, the daily dollar cap and the retry class of each call |
| `coax/lib/boat/fixtures/`, `checks.coax` | Replay fixtures through the transport's `GW_REPLAY`, and `#test`s of every call and error against them. A live check runs only by hand, with a scoped key |

**Errors come back as a typed result, not text:**

- `config`, `transport`, `decode`, `too_large`, `deadline`
- `http`, which carries the status, code, request id and retry-after
- named cases for the codes we act on: `rate_limited` (with
  `nextStartAt`), `payment_required` (402), `starting` (409
  `sandbox_starting`), `snapshot_expired` (409 `snapshot_data_expired`),
  `provider_not_configured` (409) and `may_be_running` (502)

**The calls to write first:**

1. `GET /limits`
2. Create, get, stop, resume, delete
3. Commands: synchronous, streamed, and detached with attach
4. Write a file
5. Save a saved snapshot and start from one
6. `sshkey`
7. Usage

The built-in agent calls come after these.

**Size.** About 2,000 lines of Coax with its checks: three to five
agent-days for the SDK and the policy. The Coder route and the chat
integration come on top (stages 3 and 4).

## Tied into our chat

### What exists today

- **The /chat page served locally** (`apps/openagents-web/chat-web.coax`
  with `coder mcp --http`) lists Coder's sessions and this folder's Claude
  Code conversations through Coder's `/local/*` side. It runs Claude Code,
  Codex and Microcoder there, and continues a closed conversation with
  `coder fork ID TEXT --same-conversation`. Settings, This machine, shows
  which agents are ready.
- **Coder can run work on another machine.** `--on ssh:DEST` works today
  (`coder/tui/remote.coax`). A session it starts leaves a
  `sessions/ID.remote` record, and `status`, `log`, `stop` and `attach` go
  back to that machine.
- **The hosted /chat** (`coax/web/api/chat.c`, the runner in
  `coax/web/agent/`) runs our Claude engine on the Coax Claude Agent SDK. It
  keeps a conversation as a managed session (`chat.conversations`,
  `chat.turns`, `chat.events`). The page polls `/api/chat/state` every 2 s
  while a turn runs. This is what the phone reaches.
- **`openagents chat work --on gce|archil|boat`** was Rust and is gone. The
  CLI plan lists Boat among the stacks that "don't return". This audit
  reverses that for dev boxes only, at the owner's word.

### The fastest useful version

1. **Today: stage 0 by hand.** One `large` box and Claude Code in `tmux`.
   The owner or an agent reaches it with `boat ssh` from the Mac. This isn't
   in the chat yet, but it takes agent sessions and builds off the Mac at
   once.
2. **This week: `coder --on boat:ID`.** The SDK resumes the box when it is
   stopped and refreshes the SSH entry. From there it is the existing `ssh:`
   route. The locally served /chat then starts, lists and continues Claude
   Code sessions on the box from the agent menu. That has to be checked:
   this audit didn't confirm that `/local/sessions` lists sessions that run
   remotely.

### The proper version (works from the phone)

A devbox conversation kind in the hosted API:

- **Send.** The API (holding a scoped boat key from Secret Manager) resumes
  the box if needed. It then starts our Claude engine on the box as a
  detached command (`coder run --json --engine claude`, or the /chat engine
  itself) and follows its NDJSON attach stream into `chat.events`. The page
  and the phone read it as they do now.
- **Message while it works.** Queue the message for the next turn, as today.
  A later step can pass it mid-turn the way boat's `steer` does.
- **Stop and resume** are buttons on the conversation. Stop ends the turn,
  then stops the box after its idle time. The conversation list shows the
  box's state and the day's cost.

**A shortcut, with a catch.** Boat's built-in agent (`POST /prompt` with
`provider: claude`, events by cursor, `steer`, `interrupt`, conversations
that survive a stop) would get the phone there faster. It skips running our
own engine. But it runs boat's harness instead of ours, and the events
arrive only by polling, about once a second. It also ties the chat to a
part of the API our own Boat-shaped server never had. Use it only to test
the chat side early. The proper version runs our engine, so the same path
works on `coderos-4080` or our own GCP.

## Compared with the build hosts audit's options

"Real use" means 12 hours a day.

| Option | Cost a day of real use | Survives | GPU | Setup | Risk |
|---|---|---|---|---|---|
| boat `large` (8 vCPU, 16 GB) | $0.86, mostly inside the $20 plan's included time | Yes: no preemption is documented, a stop keeps the disk, sessions resume | No | About 30 min once, then seconds from a template | A third party holds the tokens; boat had 502s on 2026-10-10; 8 vCPU is a quarter of a build host |
| boat `default` (4 vCPU, 8 GB) | $0.43 | Same | No | Same | Too small for compiler builds next to Claude |
| `coderos-4080` (ours) | $0 extra | Yes | RTX 4080 | Hours: clone, `tmux`, login | One machine, already shared (watcher, decision models), two heavy jobs at once at most |
| GPU spot host (`g2-standard-8`, L4) | About $3.60 | No: spot | L4 | About a day, plus re-measuring goldens | Preemption; it is a browser host, not a dev box |
| Spot dev box (`c3d-highcpu-30`) | $3.30, plus about $0.67 for the disk | No: spot can be taken mid-session | No | About a day | Preemption; GCP's $1,500-a-month rule |
| On-demand dev box (same type) | $13.44 | Yes | No | About a day | Cost |

Per vCPU-hour, boat `large` ($0.0090) and a spot build host ($0.0092) cost
the same. Boat adds survival, saved templates and fast resume. It lacks
size and a GPU. The sensible split:

- boat boxes hold Claude sessions and their builds and tests;
- `coderos-4080` stays the GPU box and the fallback;
- the build pool keeps the pipeline's checks.

## Risks and rules

- **The "no hosted Boat" rule and this exception.** The rule (2026-10;
  protocol spec §24.1) says sandboxes and Environments run on our own GCP
  behind the Boat API shape, and that hosted boat.dev is opt-in only. On
  2026-11-13 the owner opted in for Claude dev boxes: our own agents'
  development work on our public repository. Everything that serves users
  (sandboxes, Environments, Computers, any product-side machine) stays on
  our GCP. The SDK keeps the `BOAT_HOSTED=1` guard, so a hosted call is
  always a stated choice.
- **Custody.** Boat holds the public repository, the Claude token and the
  GitHub token on each box. Keep both tokens to one repository and one
  account, give each box its own copy (`noEnv: true`), and revoke them on
  delete.
- **Spend limits, all of them:**
  - Boat's side: the $20 plan's 555.56 included `default`-hours. Then the
    wallet, with no dollar cap on a personal account. Then 24 hours of
    grace, then boat stops the sandboxes.
  - Our side, stage 2: `large` only, at most 2 boxes at once, auto-stop 12
    hours by default (the owner can extend it), and a daily cap of $5. Two
    `large` boxes for 24 hours cost $3.46.
  - Each cap appears in the conversation and in `tool.sh boat ls`. No limit
    is hidden. The owner can change each one.
- **Saved snapshots over the included count.** There are 16 against 10
  included, at $0.056 a day for each extra. With the wallet at zero, boat
  deletes the extras after 7 days. Most are from the old stack:
  - `oa-coder-main-20261006` and `-20261007`, about 44 GB each, from before
    the Nuke
  - `oa-coder-runtime-*`
  - six `oaenv-build-*`
  - seven `gym-*`

  They are useless to the new tree. Deleting them is the owner's call, by
  name.
- **When boat is down.** It returned 502s on 2026-10-10, which is why
  `oa-boat` was built. Work on a box is pushed often, so an outage loses only
  unpushed work. Sessions move to `coderos-4080` with the same Coder route
  (`--on ssh:coderos-4080`). Stopped disks come back when boat does.
- **Exit plan.** The SDK's base URL is a setting. Our own Boat-shaped
  service on GCP (the deleted `oa-boat` is the model: commands, files,
  snapshots and usage on GCE VMs) can come back behind the same calls. This
  is the reason the proper chat path runs our engine through commands, not
  boat's built-in agent.
- **Pre-push hooks on a box.** The box has no GCP credentials, so
  `coax/remote/remote.sh` can't reach the build pool from there. A box
  builds and tests on its own 8 cores. Stage 0 checks what the hooks do
  there.

## Plan

| Stage | What | Size |
|---|---|---|
| 0 | One box by hand: `large`, `noEnv` with the Claude and GitHub tokens, Nix, Bun, the clone, the compiler, saved as `oa-devbox-YYYYMMDD`. Claude in `tmux`. Record the setup and build times, whether detached commands work right after `ready`, and whether software WebGPU draws a page | Today, once the GitHub token exists |
| 1 | The SDK core, `coax/lib/boat/`, checked against replay fixtures | 2 to 3 days |
| 2 | `policy.coax` (`#authority`) and `coax/web/tool.sh boat` with `up`, `ls`, `ssh`, `stop`, `resume` and `cost` | 1 day |
| 3 | `coder --on boat:ID`, with the locally served /chat listing and driving box sessions | 1 day |
| 4 | The devbox conversation kind in the hosted API: send, follow the stream, stop, resume. Reachable from the phone | 2 to 3 days |
| 5 | A daily template refresh, the cost line in the conversation, and the fallback to `coderos-4080` | 1 day |
| 6 | Our own Boat-shaped service on GCP, only if we need it | Later |

The first step that can happen today is stage 0. It needs the GitHub token
from the owner. Until then a box can still build and run Claude, but it
can't push.

## What this audit couldn't check

- Whether `~/work/.secrets/claude-code-oauth-token` is still valid. It
  wasn't used.
- Anything that needs a running sandbox: real setup and build times on 8
  vCPU, the detached-command and restore problems of early October,
  software WebGPU in headless Chrome, and the pre-push hooks on a box.
- Whether boat preempts or moves running sandboxes. The docs describe none.
- Whether Coder's `/local/sessions` lists `--on ssh:` sessions to the chat
  page.
- Boat's billing invoices. The wallet figure is from `GET /limits`.
- The second page of the sandbox list. The first 50 sandboxes are all
  stopped, and older ones go back before 2026-10-02.
