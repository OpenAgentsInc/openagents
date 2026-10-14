# Computers for chats audit: your own computer or a cloud one, picked when a chat starts

Date: October 14, 2026. Baseline: `main` at `a64c29d7de`, plus read-only
`gcloud` listings of project `openagentsgemini` taken the same day. Scope:
everything we have for attaching a computer to a chat (the person's own Mac or
PC, or a cloud computer they pay us for), the Environments page, the GPU plan,
the person's own Claude subscription, and Code Graph (#11622), measured
against one target flow:

> Type a request into the web app. A computer spins up at once (the person's
> own, or a paid cloud one). The chat is assigned to it. Claude Code, on the
> person's own Claude plan, runs Code Graph and does the work. The results
> show in the web app.

Method: read the code on `main` (paths and lines below are from that
revision), the deploy scripts under `deploy/`, and the live Google Cloud
inventory. Nothing was changed. "Live" below means the code path is on `main`
and the deploy config turns it on in production; it does not mean anyone
outside the owner's allowlist can use it.

## Plain-language summary

**We are closer than it looks, but the pieces are in four separate places and
three of them are for admins only.**

What works today:

- **Your own computer, from a new web chat.** If Coder is running on your Mac
  or PC with sync on, the web composer's "Where it runs" lists that computer
  as "Coder on {name}". Picking it starts a new chat whose message goes to
  Coder on that computer; the answers come back into the web chat. This is
  open to any signed-in person.
- **A cloud computer that spins up for one job, from a web chat.** "Solver in
  the cloud" takes a GitHub issue, starts a fresh Google Cloud computer just
  for it, does the work, shows progress in the same chat, ends with a pull
  request, and deletes the computer. It runs on our Gemini model, not Claude,
  only takes "solve issue #N", and only admins can use it.
- **Claude Code in a saved environment, from a web chat.** The Environments
  page lets you point at a repository; an agent sets it up, a clean build
  makes a computer image of it, and a fresh computer checks it. Once saved, a
  chat can pick "Claude Code in {repo} vN", which starts a fresh cloud
  computer from that image and runs Claude Code on your own Claude key; the
  answer joins the chat. Admins only.
- **All cloud computers are on our own Google Cloud**, through our own
  sandbox service. Nothing depends on an outside sandbox company.

What does not work yet:

- **"Instantly."** A fresh cloud computer takes about 1 to 4.5 minutes to
  start, and we keep no warm ones ready. The pools that could hold warm ones
  are scaled to zero.
- **One chat, one computer, kept.** Every cloud run gets a brand-new computer
  that is thrown away afterwards. The code for "a chat's own computer that
  remembers its files between turns" is written and tested but no chat uses
  it.
- **A list of "my computers" that includes cloud ones.** The web knows your
  own computers by name. Cloud computers are invisible as computers; they
  appear only as runs.
- **Paying for a cloud computer.** Prices are written down ($20 a month for
  100 hours, or per second from a prepaid balance) and the meter exists, but
  nobody can buy anything: checkout is not switched on and no cloud run is
  charged.
- **Your Claude plan on a cloud computer, for everyone.** Saving a
  `claude setup-token` in Settings works only for the owner's account,
  because Anthropic's terms bar us from storing plan tokens without their
  written approval. Everyone else must sign in to Claude inside the computer,
  which a throwaway computer forgets.
- **Code Graph.** Not built yet; no code has landed. When it lands it will
  be a tool for agents, and the web has no place to show its results.
- **GPUs.** There is a written plan for putting a GPU behind a chat's model,
  not for giving a chat's computer a GPU. None of it is built.

**The shortest honest path** is to stop treating these as four features and
make one: a chat gets a computer (yours, or a cloud one from a saved
environment), the computer keeps the chat's files between turns, Claude Code
runs there on your plan, and the web shows the computer and what ran on it.
Almost every part already exists; the missing parts are the joins, the warm
pool, billing, and opening it beyond admins.

## Status of each piece

Legend: **Live** (on `main` and turned on in production), **Live, admin
only** (turned on, but `agent_work::permitted` limits it to site admins and
named agent accounts), **Exists, unused** (code on `main`, not wired into a
product path or not deployed), **Planned** (a written plan only), **Missing**
(neither code nor plan), **Removed**.

| Piece | Status | Evidence |
| --- | --- | --- |
| **Your own computers** | | |
| Computer name and sign-in (`coderos-4080` style) | Live | `crates/openagents-login/src/lib.rs:147-174` (`OPENAGENTS_COMPUTER_NAME`, else `scutil --get ComputerName`, else `hostname`); sign-in by device code or an `oa_sk_` key (`:60`), or `OPENAGENTS_TOKEN` (`:53`) |
| Check-in every 10 s; online if seen in the last 60 s | Live | `crates/coder-sync/src/lib.rs:86,731,1073`; server `crates/openagents-web/src/coder_sync.rs:677,684,1617` |
| Computer records | Live | One JSON object per account, `.coder.json`, on disk or GCS (`crates/openagents-web/src/chat_store.rs:277-287,317,341,1084,1104`); at most 16 computers; no SQL table |
| `GET /v1/computers`, `DELETE /v1/computers/{name}` (#11601), sync choice, activity | Live | `crates/openagents-web/src/phone_api.rs:103-111`, `remove_computer_named` `:710`, `remove_computer` `:746`; CLI `openagents computer remove NAME` (`crates/openagents-cli/src/computer.rs:297`); added in `fae5dbe1b1` |
| Coder to web chat sync (titles, messages, replies typed on the web, Stop/Message commands) | Live, opt-in with `/sync on` | `crates/coder-sync/src/lib.rs:1-40,1256,1484,1594`; `crates/coder-new/src/account_sync.rs:23` |
| Web chat "Coder on {computer}" for a new chat | Live, any signed-in account | `crates/openagents-web/src/composer_row.rs:13-27,250-253,368`; `pages/chat.rs:621` `start_on_coder` → `coder_sync::start_from_web` (`coder_sync.rs:1101`); offline computers listed disabled (`composer_row.rs:880-881`) |
| Moving an existing chat to a computer; "Run in Coder on {computer}" under answers | Removed | `2a71bde8ff` (#11555) deleted `pages/chat_to_coder.rs` and the router's automatic hand-off; a later message for Coder in an existing chat is refused (`composer_row.rs:196` `CODER_NEW_CHAT`) |
| "Continue on a Cloud computer" when your computer is offline | Live, admin only | `crates/openagents-web/src/pages/chat_continued.rs:1-32` (#11050); runs Claude Code in the project's saved environment on a fresh computer |
| API requests that run on your own computer's Codex or Claude subscription (`openagents/code`, `pay: "mine"`) | Live | `crates/openagents-web/src/own_runs.rs:1-30,96-97` (#11080); production passes `--own-runs-token` (`deploy/production/web.sh:73`); Coder side `crates/coder-new/src/own_runs.rs` |
| Mac-only jobs sent to a linked Mac (`/v1/mac-jobs`) | Live | `crates/openagents-web/src/mac_jobs.rs:59,108-119`; `docs/cloud/linked-mac.md` (#11223) |
| Remote control through the account (phone and `/settings/agents` → queued command → picked up at next report) | Live | `POST /v1/agents/actions`; `crates/openagents-web/src/agents_page.rs:1-15` |
| Remote control through paired hosts (NIP-HOST/NIP-REACH; iroh, direct TCP/WebSocket, relay fallback) | Live in the phone and desktop apps | `crates/coder-host`, connector `client/connector.rs:1-13`; desktop runs `coder host serve --keychain --iroh --control` (`crates/openagents-desktop/src/lib.rs:16-22`) |
| One identity for "a computer" across both systems | Missing | The web knows a computer by its checked-in name; the apps know a host by its key; nothing links the two |
| `coder-control` (scoped Nostr task control) | Exists, unused | No crate depends on it |
| Pylon as a local provider | Live on one machine, not connected to chats | `crates/pylon` takes NIP-CJ jobs and runs them on a local Psionic server; one instance on `coderos-4080` (workspace `NEEDS_OWNER.md:64-73`); the gateway can mount Pylons as `pylon:<id>` model upstreams (`crates/inference/src/upstream/pylon.rs`), web chat never routes to one. `docs/psionic-and-pylon.md:214-227` is stale (says the repo has no Pylon source) |
| **Cloud computers** | | |
| `oa-boat`: our own Boat-shaped sandbox service (one GCE VM per sandbox, stop/resume/fork, snapshots, usage in dollars) | Live | Cloud Run `oa-boat` (min 1, max 1 instance, image `oa-boat:7f2ede07…`, verified by `gcloud run services describe`); `crates/oa-boat/src/api.rs:338-389`, `service.rs:1-18`, `gce.rs:242,364-368`; project `openagentsgemini`, zones us-central1-a/b/c/f, at most 50 sandboxes, TTL 1 h, idle stop 30 min (`main.rs:67-91`) |
| Sandbox sizes | Live | small e2-standard-2, default e2-standard-4, large n2d-standard-8 (falls back to e2-standard-8), xlarge n2d-standard-16 (`crates/oa-boat/src/sizes.rs:37-85`); no GPU size |
| Boot time | Live, slow | "A GCE insert in a busy zone can take 1 to 4.5 minutes" (`docs/cloud/oa-boat.md:75-76`) |
| `oa-boat` stubs: `/environments` list, prompt, steer, events, desktop, share | Exists, returns empty or 501 | `crates/oa-boat/src/api.rs:320-322,372-379` |
| `boat` client defaults to our own `oa-boat`; boat.dev only with `BOAT_HOSTED=1` | Live | `crates/boat/src/lib.rs:28-34`; production `BOAT_API_BASE` on Cloud Run `coder` points at `oa-boat` (verified) |
| Daily `oa-coder-host` image and `oa-coder-main-YYYYMMDD` template | Exists; schedule not verified | `crates/boat-template/src/main.rs:1-17`; `docs/deployment/coder-host-image.md` |
| Environments page (`/environments`): repo → setup agent → clean build to an image → fresh-machine check → save a version → run Claude Code | Live, admin only | Routes `crates/openagents-web/src/environments/mod.rs:88-117`; statuses `view.rs:30-41`; enabled by `--environments` when `BOAT_API_KEY` is set (`deploy/production/web.sh:54-65`); gate `crates/openagents-web/src/agent_work.rs:1-47`; setup model `ENVIRONMENTS_MODEL=google/gemini-3.8-flash` (verified on Cloud Run) |
| Deleting an environment | Missing | No delete route in `environments/mod.rs`; no delete on `Studio` (`crates/coder-environment-operator/src/studio.rs:537-1041`) |
| Recipes, builds, verification | Live inside the web server process | Recipe `crates/coder-environment/src/lib.rs:212-249`; setup `coder-environment-setup/src/lib.rs:1-30`; build captures an immutable GCE image `coder-environment-build/src/lib.rs:1-30`; verify `coder-environment-verify/src/lib.rs:1-30`; a direct GCE provider exists but production uses `"provider":"boat"` (`coder-environment-operator/src/gce.rs:1-68`) |
| Web chat "Claude Code in {repo} vN" | Live, admin only | `composer_row.rs:16-19`; `pages/chat.rs:653-713` `start_claude` → `chat_work::begin` (`pages/chat_work.rs:723-766`) → `studio.run_claude_pushing` (`studio.rs:859`) → a Cloud job pinned to the saved image (`studio/claude.rs:1-18`); step progress and in-chat questions not wired (`chat_work.rs:37-42`) |
| A chat's own computer kept between turns (checkpoint after each turn, restore on the next) | Exists, unused by chats | `crates/coder-working-computer/src/lib.rs:1-26` (CMP-01); used only for the Environments setup machines (`coder-environment-operator/src/lib.rs:227-233`); chat runs always start a fresh computer |
| Solver in the cloud: issue → fresh GCE box → pull request → box deleted | Live, admin only | Spec `docs/cloud/solver-on-gcp.md`; #11559-#11562, #11572, #11600 on `main`; dispatcher VM `oa-solver-dispatch-1` (e2-small, RUNNING, label `issue=11559`, verified) running `oa-solver-box --serve --slots 2`; claims via `POST /v1/work-hosts/{name}/claim` (`crates/solver-box/src/dispatch.rs:149-156`); boxes through `oa-boat`, TTL 6 h, deleted on every exit (`crates/solver-box/src/lib.rs:47,589-600,634-640,662-672,685-725`); gate `work_runs.rs:1352-1363`; model Gemini Argon on Vertex, no Claude (`solver-box` has no Claude path) |
| Chat worker `oa-coder-worker-1` (answers ordinary web chats) | Live | e2-standard-4, RUNNING (verified); reached over NIP-CJ on `wss://relay.openagents.com` (`crates/openagents-web/src/ask.rs:1-60`); Gemini 3.8 Flash on Vertex with fallbacks (`docs/deployment/chat-worker.md:31-60`); no toolchain, no sandbox |
| GCE pool for the CLI (`openagents cloud up/down`, `chat work --on gce`) | Exists, operator tool | `crates/coder-cloud/src/pool.rs:26,850-884`; `crates/openagents-cli/src/chat_gce.rs`; `docs/cloud/gce-pool.md`; spot `c3-standard-8`, 12 h max run |
| `coder-pool`, `coder-pool-spot`, `coder-box-pool` instance groups | Exist, scaled to 0 | `gcloud compute instance-groups managed list`: all three size 0, target 0, us-central1-b (verified); nothing in this repo resizes them; they were driven by the retired `coder-serve` |
| `coder-serve` | Removed (#11614) | `deploy/production/render.py:122-126`; `crates/openagents-web/src/lib.rs:783`; `docs/deployment/coder-serve-retirement.md` |
| `oa-node`, `oa-workroomd`, `oa-codex-control` | Removed | Deleted in `dabc08102f` ("Nuke", 2026-09-18); `docs/cloud/README.md:173-175`; leftover VMs `oa-codex-control-1` and `oa-managed-sandbox-control-*` are TERMINATED (verified) |
| `oa-dev-env-1` … `-36` (c3-standard-22) | Stopped VMs | All TERMINATED in us-central1-b (verified); last started 2026-10-11; not referenced by any chat path |
| `coder-cloud-web` | Exists, unused | Nothing depends on it |
| **Paying for computers** | | |
| Pro plan: $20 a month, 100 machine-hours, 2 at once, $0.18 an extra hour | Planned, meter exists | `crates/retail-cloud/fixtures/environment-plan.json` (`"status": "proposed"`); meter `crates/retail-cloud/src/environment.rs:183-238`; Settings page `crates/openagents-web/src/plan.rs:1-30` |
| Subscribe button (Stripe Checkout) | Exists, off | Needs `--plan-checkout`; production does not pass it (`deploy/production/web.sh:72`); webhook writer `crates/gateway/src/subscriptions.rs:1-28,1049-1072` |
| Plan meter storage | Live but not durable | `--plan-meter /tmp/plan-meter.sqlite` inside the container (`deploy/production/web.sh:72`) |
| Prepaid compute balance, 40 msat a second (144 sats an hour) | Exists behind the retail gate | `docs/cloud/compute-balance.md`, `docs/cloud/retail-prices.md:28-30`; `crates/pay-ledger/src/compute.rs`; `crates/route-contract/src/price_book.rs:282`; fake payments only |
| Charging a cloud run to the person | Missing | Solver shows the box's dollars from `oa-boat` usage (`work_runs.rs:386-388`, `oa-boat/src/service.rs:1998-2010`) but nothing debits a balance |
| **Your Claude plan** | | |
| Policy: sign-in inside the computer by default; stored plan tokens only with Anthropic's written approval | Live policy | `docs/cloud/claude-code-byo.md:34-79`; `docs/byok/claude-plan.md:289-321` |
| Saving a `claude setup-token` in Settings, Claude | Live, owner's account only | Allowlist `OPENAGENTS_WEB_CLAUDE_TOKEN_ACCOUNTS`, default the owner only (`crates/openagents-web/src/cloud/byo.rs:91-120`); production sets no list (only staging does, `deploy/staging/render.py:55-60,105`) |
| Credential forms accepted | Live | Five: plan token (`sk-ant-oat…`), Anthropic API key, Bedrock, Vertex, Foundry (`crates/openagents-web/src/cloud/byo.rs:43-49`); one credential per account, workspace and membership epoch under subject `byo:computers` (`:40,178-186`); saving one replaces the others (`:285-287`); expires after 90 days (`:42`); checked with one `GET /v1/models` before saving (`:222-252`); the gateway refuses plan tokens for model calls (`crates/gateway/src/inference_byok.rs`) |
| Storage of saved Claude credentials | Live | AES-256-GCM per entry through `oa_seal`, scope and digest bound in, rotatable keyring (`cloud/custody.rs:16-24,172,283`); keyring from Secret Manager `openagents-web-production-byo-keys` (`deploy/production/render.py:93`); the server refuses to start without it (`crates/openagents-web/src/main.rs:152-198`); files per account in `$WEB_STATE/byo` on the account-store NFS disk, mode 700 (`deploy/production/web.sh:9-19,72`); revoke erases the entry (`byo.rs:292`) |
| Plan token on a cloud computer from a web chat ("Claude Code in {repo}", "Continue on a Cloud computer") | Live, admin only | `byo::run_key` decrypts it for one run (`byo.rs:514-548`; callers `pages/chat_work.rs:736,1015,1103`, `chat_agents.rs:951-952,1315`) → `studio::claude::Key` in memory (`coder-environment-operator/src/studio/claude.rs:94-125`) → **written as a plaintext file** `/tmp/oa-coder-{job}.env` on the VM (`crates/coder-cloud/src/boat_backend.rs:284-290`), sourced and deleted by the launch script (`coder-cloud/src/runtime.rs:272`), then held as `CLAUDE_CODE_OAUTH_TOKEN` in the process environment (`crates/coder-new/src/claude_print.rs:204,294`); never in the run record or image; full long-lived token, not scoped; removing it takes effect at the next run, not mid-run |
| Plan token on work hosts (`/v1/work`, briefed and bare engines) | Live code | Released once per claim in the HTTPS claim response (`work_runs.rs:894-914,2091`); `scripts/work/worker.py:131-133,160` passes it only into the child's environment, not to disk |
| Per-turn, in-memory, device-bound release to a resident cloud computer (`release_turn`, BYO-05) | Exists, unused | `byo.rs:390-471` has no callers since `cb11bee283` (the Cloud web reset); `docs/cloud/claude-code-byo.md:231` overstates it |
| Claude on your own computer | Live, local login only | Coder uses that machine's own `claude` login, API key, or local token (`crates/coder-delegate/src/delegate.rs:1726-1760`); the server never sends a stored token to a person's computer, and the chat-computer library refuses `CLAUDE_CODE_OAUTH_TOKEN` (`crates/coder-working-computer/src/lib.rs:68,433`) |
| Claude in Solver boxes or the chat worker | Not used | Solver uses Gemini Argon on Vertex plus a GitHub token; the chat worker uses Gemini 3.8 Flash; an ordinary web chat turn never runs Claude Code |
| **GPU** | | |
| "Connecting a GPU to a chat" (a GPU behind the chat's model: our H100, Modal, a Pylon, or your own GPU) | Planned | `docs/inference/gpu-chat.md` (status "design", 2026-10-12); none of `upstream/gpu.rs`, `openagents gpu`, `oa-gpu-host`, `door_down` exists |
| GPU for a chat's computer (to run code on a GPU) | Missing | No GPU size in `oa-boat` (`sizes.rs`); the GCE pool uses `c3-standard-8` only |
| H100 and A100 VMs, TPU v5p-8 | Live for decisions, not for chats | `clef-decisions-h100` (a3-highgpu-1g) and `clef-decisions-a100` (a2-highgpu-1g) RUNNING; TPU `cd-v5p8-ue5a-s` v5p-8 READY in us-east5-a (verified); opt-in `tpu` decision order (`crates/gateway/src/decisions.rs`, #11459) |
| **Code Graph** | | |
| Code Graph plugin (#11622) | Planned, in progress elsewhere | Issue open and claimed; no `crates/*graph*` and no commit on `main` |
| Plugin runtime (Wasmtime, no WASI, receipts) on the machine where Coder runs | Live | `crates/plugin/src/engine.rs:18`, `lib.rs:1-9`; called from `crates/coder/src/runtime.rs:60` and `crates/coder-delegate/src/guests.rs:1-37` |
| Plugin results on the web | Missing | The web shows only catalog cards for plugins a reply names (`crates/openagents-web/src/suggestions.rs:196-218`) and a purchases list (`purchases.rs:1-15`); neither the web server nor the gateway links the plugin runtime |
| **Phone and desktop** | | |
| Phone Computers tab (enroll, presence, tasks, terminals over NIP-HOST) and account computers | Live | `crates/coder-computers/src/lib.rs:1-13`; `crates/openagents-mobile/src/account_link.rs:1-51` |
| Desktop: this Mac as a host, phone pairing by QR | Live | `crates/openagents-desktop/src/lib.rs:1-22`; it does not list or pick other computers |
| Cloud computers on phone or desktop | Missing | Cloud runs appear only as items on `GET /v1/agents` (`crates/openagents-web/src/work_runs.rs:1368`, `environments/board.rs:1-17`) |

## How each path works today

### Your own computer

Coder signs in once (device code, or an `oa_sk_` key on a headless box like
`coderos-4080`), names itself from `OPENAGENTS_COMPUTER_NAME` or the system
name, and checks in every 10 seconds. The website lists computers seen in the
last minute in the composer. A new web chat aimed at one is queued like a
reply; Coder takes it at its next check-in, runs it with whatever engine and
logins that computer has (including the person's own `claude`), and syncs the
transcript back. Replies typed on the web for that chat go to the same
computer. Since #11555 an existing chat is never moved, so "pick a computer"
means "start a new chat there".

This is the one path that already satisfies "assign the chat to a computer,
see it on the web" for everyone, and it already runs Claude on the person's
own plan without us touching the credential.

### A cloud computer for one job

Two paths share the same machinery (`oa-boat` on our Google Cloud):

1. **Claude Code in a saved environment.** The environment's image is a GCE
   image made by a clean build and checked on a fresh machine. A chat run
   boots a fresh VM from exactly that image, applies the Claude credential for
   that run only, runs Claude Code, pushes through a one-repository GitHub
   credential (`environments/push.rs:1-21`), and puts the answer in the chat.
2. **Solver in the cloud.** A dispatcher VM claims the run, asks `oa-boat` for
   a large box, runs our solver container with Gemini Argon, streams progress
   cards into the chat, opens a pull request, and deletes the box whatever
   happens.

Both throw the computer away. Neither shows the computer itself (files,
terminal, what is installed) on the web; both show only the run.

### Paying

Every price we would need is written: a Pro plan with included hours, a
per-second rate against a prepaid balance, and per-run dollar figures from
`oa-boat`. The parts that would take money are switched off until the owner
confirms prices and the funded qualification passes. The plan meter in
production lives in `/tmp` inside the container, so it would not survive a
restart if turned on as is.

### The person's Claude plan

Anthropic's terms let a person sign in to the unmodified `claude` inside any
computer, including one we host, and let us keep the person's own API key or
cloud-provider credential. They do not let us store plan tokens without
written approval. So:

- On the person's own computer, it already works: their login stays there.
- On a cloud computer, the person can sign in inside it, but a fresh
  throwaway computer forgets that login. Keeping it needs the chat's own
  computer with checkpoints (`coder-working-computer`), which the terms
  explicitly allow ("sign in once per computer", `claude-code-byo.md:61-64`).
- A saved `setup-token` is the only way to skip that, and it stays limited to
  the owner's account until Anthropic approves in writing. The owner action
  that unlocks it is not in `NEEDS_OWNER.md` today, although `claude-plan.md`
  says it is.

How a saved credential reaches a cloud computer today: the web server
decrypts it for one run and hands it to the run in memory; the run writes it
to a short-lived plaintext file in `/tmp` on the fresh VM, the launch script
reads and deletes that file, and Claude Code gets it as an environment
variable. It is never in the run record or the image. Three things fall short
of "scoped, not on disk in plain text, revocable": the full long-lived token
is sent (nothing narrower exists from Anthropic for plan tokens), it touches
the VM's disk briefly and stays readable in the process environment, and
removing it in Settings only affects the next run. The better path is already
written: `release_turn` (`byo.rs:390-471`) releases per turn, in memory, to
one bound computer, and has had no caller since the Cloud web reset. The
`/v1/work` hosts already pass it only in memory.

On the person's own computer nothing needs delivering: Coder uses the login
already there, and the server never sends a stored token to a person's
machine. API keys and cloud-provider credentials are a separate class we may
keep per account and are not limited by the allowlist.

### Code Graph on a fresh computer

#11622 designs Code Graph as a deterministic graph of the code (functions,
types, modules; calls, imports, impls), built by CI after merges and fetched
by readers, with typed queries for Coder and the plugin SDK. Plugins run as
WebAssembly inside Coder on whatever machine Coder runs on. On a fresh cloud
computer that means: the image (the environment build, or the daily
`oa-coder-host` image) must include Coder with the Code Graph plugin; the run
fetches the published graph for the commit, or builds one locally for
uncommitted work; Claude Code needs Code Graph exposed as a tool it can call
(an MCP server or Coder's tool bridge), since Claude Code does not load our
Wasm plugins itself. To see results on the web, the run must send them back
as a typed result (the same way Solver sends progress cards) and the chat
page must draw them. Neither the tool bridge for Claude Code nor a web view
exists.

## Gaps, in the order they block the target flow

1. **Admins only.** Environments, Claude Code runs, Solver, and "Continue on a
   Cloud computer" all sit behind `agent_work::permitted`
   (`agent_work.rs:1-47`). Until that gate opens for paying accounts, nobody
   else can spin up a cloud computer from the web.
2. **No paid computer.** Checkout is off, the meter is in `/tmp`, and no cloud
   run debits anything. Opening gap 1 without this means we pay for every
   stranger's computer.
3. **Not instant.** A fresh VM takes 1 to 4.5 minutes; there is no warm pool
   (the three instance groups are at 0 and nothing in this repo drives them);
   environment setup is a multi-minute agent run on top.
4. **No computer per chat.** Every run is a fresh, thrown-away VM. The chat's
   own computer that keeps files and the Claude login between turns
   (`coder-working-computer`) is built but not wired to chats.
5. **Claude plan on cloud computers.** Without a saved plan token (owner only)
   and without a kept computer (gap 4), a person on a Claude plan has to sign
   in again on every fresh computer. API-key users are fine. Where a saved
   token is used, it is written briefly to `/tmp` on the VM in plain text,
   sits in the process environment, and revocation waits for the next run;
   the in-memory per-turn release (`release_turn`) is built but unwired.
   Solver and the ordinary chat worker never run Claude at all, so "Claude
   on my token" today means only the admin-only environment path.
6. **"Computers" is two systems.** The web's named computers and the apps'
   keyed hosts never meet, and cloud computers are not computers anywhere in
   the UI. There is no single "my computers" list holding your Mac, your PC,
   and your cloud computers.
7. **Code Graph is not built**, and a Claude Code run cannot call our Wasm
   plugins without a bridge.
8. **The web shows runs, not computers or tool results.** No view of a
   computer's files or terminal from the web chat, no rendering of plugin
   results, and step progress and in-chat questions for Claude runs are not
   wired (`chat_work.rs:37-42`).
9. **Environments cannot be deleted**, and `oa-boat`'s own environment list,
   prompt, events, desktop, and share endpoints are stubs.
10. **GPU computers do not exist.** No GPU size in `oa-boat`; the GPU plan is
    about model serving, not a chat's computer.

## Recommended build order

Smallest first; each step reuses what is named and adds no parallel system.
Sizes are rough: S is a day or less, M a few days, L a week or more.

1. **One "Computers" list on the web (S).** Extend `GET /v1/computers` and the
   composer row to include the person's cloud computers (from `oa-boat`
   sandboxes and work runs they own) next to their own machines. Same
   records, one list. No new service.
2. **A chat's own cloud computer (M).** Wire `coder-working-computer` (its
   Boat provider already exists) into `chat_work::begin`: the first Claude
   run in a chat creates the computer from the saved environment version,
   later turns restore its checkpoint instead of booting fresh. This also
   fixes the Claude-plan login problem for everyone: sign in once inside that
   computer, as the terms allow. In the same change, deliver any saved
   credential through `release_turn` (in memory, per turn, bound to that
   computer) instead of the `/tmp` env file, so removing it in Settings stops
   the next turn on a live computer too.
3. **Warm computers (M).** Keep N stopped-but-ready sandboxes per popular
   environment image in `oa-boat` (stop/resume already exists) and hand one
   out on request; resume is much faster than a cold insert. Retire the three
   idle instance groups rather than reviving `coder-serve`.
4. **Paid computers (M).** Move the plan meter off `/tmp` onto the account
   store, have each run (Solver, Claude, chat computers) report its `oa-boat`
   seconds to the meter, then pass `--plan-checkout` once the owner confirms
   the price.
5. **Open the gate to paying accounts (S).** Change `agent_work::permitted` to
   "admin, named agent account, or an account with a Pro plan or balance",
   with the plan's limit on computers at once (2).
6. **Code Graph as a tool on every computer (M, after #11622 lands).** Bake
   Coder and the graph into the `oa-coder-host` image and environment builds;
   expose the graph queries to Claude Code through an MCP server Coder runs;
   post the answers as typed run events.
7. **Show it on the web (M).** Draw the computer (status, files changed,
   terminal tail) and typed tool results (Code Graph impact lists first) in
   the chat, reusing Solver's progress-card path; wire Claude step progress
   and in-chat questions (`chat_work.rs:37-42`).
8. **Environment delete and the `oa-boat` stubs (S).**
9. **One identity for computers (L).** Link a NIP-HOST key to the account's
   computer name at enrollment so the phone, desktop, and web show the same
   list.
10. **GPU computers (L, only if wanted).** Add a GPU size to `oa-boat` (for
    example `g2-standard-8` with one L4) behind the same plan meter, after
    the decision below.

Steps 1 to 5 deliver the target flow for Claude API-key users and for plan
users who sign in once per computer. Steps 6 and 7 add Code Graph and the
web view.

## Open decisions for the owner

- **Pricing.** Confirm or change the proposed Pro plan ($20 a month, 100
  machine-hours on a 2 vCPU / 8 GB computer, 2 at once, $0.18 an extra hour)
  and the per-second balance rate (144 sats an hour). Decide whether Solver
  and Claude runs count against the same hours.
- **Claude plan tokens.** Ask Anthropic for written approval to store
  `claude setup-token` values for all users, or accept "sign in once per
  computer" as the plan-user path. Add the request to `NEEDS_OWNER.md` (it is
  referenced but missing).
- **Regions.** Everything is in `us-central1` today. Decide whether to offer
  Europe or the US east coast for latency or data residency.
- **Idle shutdown.** Today a sandbox stops after 30 idle minutes and dies
  after 1 hour (Solver boxes 6 hours). Choose the defaults for a chat's own
  computer: how long it stays up after the last turn, how long a stopped
  computer's disk is kept, and whether storage beyond the plan's 20 GB is
  charged.
- **Warm pool size.** How many ready computers we pay for in advance, and
  for which images.
- **GPU.** Whether a chat's computer should ever have a GPU, and if so which
  (L4 is the cheap option; H100 is about $6.40 an hour spot per the GPU
  plan), or whether GPUs stay behind the model picker only, as
  `docs/inference/gpu-chat.md` proposes.
- **Who may use cloud computers before payment opens.** Admins only (today),
  invite list, or a free trial allowance.

## Sources

- Plans: `docs/cloud/managed-computers.md`, `docs/cloud/coder-cloud.md`,
  `docs/cloud/oa-boat.md`, `docs/cloud/solver-on-gcp.md`,
  `docs/cloud/gce-pool.md`, `docs/cloud/claude-code-byo.md`,
  `docs/byok/claude-plan.md`, `docs/cloud/retail-prices.md`,
  `docs/cloud/compute-balance.md`, `docs/inference/gpu-chat.md`,
  `docs/web/cloud-reset.md`.
- Earlier audit: `docs/cloud/2026-10-02-cloud-parallel-execution-audit.md`.
- Live inventory (read-only, 2026-10-14): `gcloud compute instances list`,
  `gcloud compute instance-groups managed list`, `gcloud run services
  describe oa-boat` and `coder`, `gcloud compute tpus tpu-vm list`.
