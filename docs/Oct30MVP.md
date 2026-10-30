# Oct 30 MVP: anyone can join as a Pylon

**Goal, for the owner's talk on 2026-10-30.** A visitor opens
openagents.com, or points their agent at a docs page there, runs **one
script**, and their computer joins the network as a **Pylon**. A Pylon is a
machine that announces itself on Nostr and answers decision jobs (Psionic
chapter 19). On stage, the audience sees Pylons appear and answer jobs. A
desktop app comes later; the script is today's path.

This is the full spec. It was collected from the sessions on coderos-4080 on
2026-10-30: `openagents-d7` (Nostr, signer), `openagents-20` (#4828 signer
audit) and `openagents-1a` (Psionic, chapter 19 owner). `openagents-3d`
couldn't be reached. Claim a work item by writing your name in its row, and
push updates to this file.

---

## 1. What exists and works today (on `main`)

### The Pylon program

- **Source:** `psionic/lib/pylon/run.coax`. The host is
  `psionic/platform/pylon-host.c`.
- **Build:** `psionic/platform/pylon-build.sh OUT plain`, which produces
  `OUT/pylon`. It emits C from Coax (`coax c ... --run-steps 250000000`) and
  links sockets, TLS and entropy. TLS is OpenSSL through Nix on Linux
  (`coax/platform/tls-env.sh`) and Network.framework on macOS (`9b5dae736a`,
  `15ef4e1748`).
- **What it does:**
  - connects to a relay over `ws://` or `wss://`;
  - answers NIP-42 AUTH;
  - publishes a NIP-65 relay list (kind 10002) and a beacon (kind 30200,
    addressable, `d` = `--slug`);
  - subscribes to decision jobs (kind 25910) addressed to its key;
  - answers each job with a result (26910) or a status (27010).

  Job and answer contents are NIP-44 encrypted. Every event is signed in
  Coax (`coax/lib/nostr/event_sign.coax`). On a dropped connection it
  reconnects with backoff, re-authenticates, resubscribes and republishes its
  beacon.
- **Default relay:** `wss://relay.openagents.com`, an owner decision of
  2026-10-30 (`run.coax:76`, `PY_DEFAULT_LIST`).
- **Self-check (chapter 159):** before publishing anything it runs its engine
  against a shipped reference set and records the identity it serves. Both
  `--references` and `--identity-state` are required:
  `psionic/fixtures/pylon/psionic-toy.selfcheck` or `elephant-1.selfcheck`.
- **Flags:** `--relay`, `--engine http://HOST:PORT`, `--model`, `--family`,
  `--slug`, `--label`, `--test-key-label`, `--key-file` (refused today),
  `--decide-ms`, `--ttl`, `--refresh`, `--memory-gb`, `--declare-identity`,
  `--references`, `--identity-state`, `--stop-file`, `--doctor`, `--list`.
- **Ready line:** the log prints `pylon: online, waiting for jobs` once it
  has authenticated, published and subscribed.

### The engine

`psionic-serve`, built by
`psionic/platform/serve-build.sh OUT release plain`. Use the toy engine,
`psionic-serve --toy --port PORT`, for laptops. Real models need a GPU:
Elephant runs on the Google Cloud L4 (`psionic-l4-elephant`), which
`openagents-1a` reaches through an IAP tunnel at `127.0.0.1:18797`.

### The end-to-end test

`python3 -I psionic/tests/pylon/pylon_e2e.py [--wss] OUT` builds the Coax
relay, the Pylon and the toy engine, starts them, and runs `buyer.ts`.

- **Results** at `fed0a442b2`, again at `753e949fc6`: 52/52 over ws:// with a
  relay restart, 53/53 over wss://, and 45/45 with Elephant on the L4.
- **Relay used:** only our own Coax relay on 127.0.0.1. **The Pylon has never
  been run against a public relay.**
- **The exact Pylon command it uses,** a template for the join script:

```sh
pylon --relay "$RELAY" --engine "http://127.0.0.1:$EPORT" --model psionic-toy \
  --slug "$SLUG" --label "$LABEL" --test-key-label "$SECRET_LABEL" \
  --stop-file "$STOP" --decide-ms 30000 \
  --references psionic/fixtures/pylon/psionic-toy.selfcheck \
  --identity-state "$STATE_FILE"
```

### The buyer side

`psionic/tests/pylon/buyer.ts` (bun):
- reads the beacon with `REQ {kinds:[30200], "#d":[SLUG]}`;
- encrypts jobs with `nostr-effect` NIP-44 and signs them with `@noble/curves`;
- checks the answers.

Its environment: `RELAY`, `CONNECT` (defaults to `RELAY`), `SLUG`, `MODEL`,
`STOP_FILE`, `CANCEL`, and `NODE_PATH` pointing at `@noble/curves`. **It is
the test's scripted buyer** (jobs r-1 to r-8, cancellation, restart
handling), not a general demo client.

### Docs

- Chapter 19: `psionic/book/ch019-a-pylon-answers-decisions.md`.
- The wire formats: `nips/openagents/NIP-PYLON.md` and `NIP-DEC.md`. NIP-DEC
  states that decision events are ephemeral (20000–29999), so relays store
  none. Buyers resend missed jobs, and the Pylon resends its kept answer for
  a repeated job id.

### Website

- openagents.com is built with Coaxial: pages are Coax drawn with WebGPU
  (`coax/web.md`, `apps/openagents-web/`).
- Routes come from directories: `apps/openagents-web/X/index.html` is `/X`,
  and `index.md` next to it is the plain-text version for agents.
- Deploys go through `coax/web/release.sh`: staging, smoke, a golden walk,
  then production. It currently runs from the Mac; Linux builds work, but
  headless WebGPU screenshots don't work on the RTX 4080 host.

---

## 2. Work items

| # | Item | Details | Owner |
| --- | --- | --- | --- |
| **A** | **Prove the public relay** (do first; everything depends on it) | Build `pylon` and `psionic-serve` and run them against `wss://relay.openagents.com` for 2 minutes with the toy engine and a random `--test-key-label`. Check these, and record the results below: <br>• the log reaches `pylon: online, waiting for jobs`; <br>• the relay sends NIP-42 AUTH and accepts the AUTH event; <br>• the relay accepts the 10002 and 30200 events, and `REQ {kinds:[30200]}` from a second client returns the beacon; <br>• a 25910 job sent to the Pylon's pubkey gets a 26910 back through the relay. <br>The deployed relay is the old Rust `nostr-relay`. Its NIP-11 advertises NIPs 1, 9, 11, 17, 29, 40, 42, 45, 50, 59, 65 and others (`coax/nostr.md` §17). Nobody has checked whether its policy admits kinds 25910, 26910, 27010 and 30200 from arbitrary pubkeys, or how it rate-limits. **If it refuses them:** the fallback is to expose the Coax relay (`coax/relay/`) publicly for the day. That has NIP-42 and the R1 routing (`docs/verification/2026-10-29-coax-relay-r1.md`), but not the Rust relay's limits. It needs a public host and TLS, and **owner approval**, because it is a production-facing service. | |
| **B** | **One-command join script** | Put it at `scripts/join-pylon.sh` (or similar) on main and serve it from openagents.com. It should: <br>• check prerequisites: git; a C/C++ compiler (gcc/g++ on Linux, Xcode CLT on macOS); Nix on Linux for OpenSSL; bun only for the buyer; <br>• clone or update the repo into `~/.openagents/pylon`; <br>• build: `coax/build.sh` (about 3 min on a fast machine, about 8 min on a loaded one), then `pylon-build.sh OUT plain` and `serve-build.sh OUT release plain`; <br>• generate `SECRET_LABEL="$(head -c 32 /dev/urandom \| xxd -p -c 64)"`, store it in a `0600` file, and never print it; <br>• pick `SLUG=pylon-$(hostname -s)-<4 random hex>` and `LABEL="<hostname> (demo)"`; <br>• start `psionic-serve --toy --port <free port>`, wait for the port, then start `pylon` with the command template in §1, pointed at the relay chosen in A; <br>• print the Pylon's pubkey (npub) and slug, and say how to stop it (Ctrl-C or the stop file). <br>**Known build problems to fix or work around:** <br>• On coderos-4080 (gcc 15), `serve-build.sh` fails: `-Werror=misleading-indentation` at `psionic/platform/serve-supervisor.c:18`. Fix the indentation, or don't use `-Werror` for joiners. <br>• `cc` on PATH may not be a C compiler (on coderos-4080 it is the Claude CLI). The build scripts honour `COAX_CC`/`CC`; set `COAX_CC=gcc`. <br>• The Wasm toolchain isn't needed for a Pylon. <br>Test on a clean Linux machine and a clean macOS machine. | (`openagents-1a` was asked) |
| **C** | **Prebuilt binaries** (optional, speeds up joining) | Build `pylon` and `psionic-serve` for linux-x86_64 and macos-arm64. Host them on openagents.com with SHA-256 sums printed by the script and on the page. Drop this if time is short; building from source is fine. | |
| **D** | **`openagents.com/pylon` page** | The page should show: <br>• what a Pylon is, in one paragraph; <br>• the join command (`curl -fsSL https://openagents.com/pylon/join.sh \| bash`, or `git clone` plus the script); <br>• a **live list of Pylons**: a `REQ {kinds:[30200]}` subscription on the relay showing each beacon's label, model identity, free slots, last-seen time and npub; <br>• the limits (§3). <br>Implement it as a Coaxial page (`apps/openagents-web/pylon/`). The project pages already subscribe to relays in Coax (`project-live.coax`, `socket.coax`), and the verifier library is in `nip01-verify.coax`. Deploy with `coax/web/release.sh`, from the Mac. If a Coaxial page can't be ready in time, a plain-text `index.md` plus a static listing is the floor. | |
| **E** | **Agent-readable docs** | `apps/openagents-web/pylon/index.md`: the same content written as numbered steps an agent can follow: prerequisites, the command, how to tell it's online, how to stop it, troubleshooting. Link it from the page and from `/llms.txt` or the docs index if there is one. | |
| **F** | **Stage demo buyer** | A small script, `psionic/tests/pylon/demo-buyer.ts` or similar, built on `buyer.ts`'s code. It lists the live beacons on the relay, sends one decision job (25910, NIP-44 to the Pylon) to each, or to a chosen one, and prints each answer (26910) with the answering identity and latency. The jobs have to be valid `openagents.systemone.v1` requests for the toy model (see `buyer.ts` and NIP-DEC). Optionally, a button on the page that does the same in the browser. | |
| **G** | **Rehearsal** | Two machines (e.g. the Mac and coderos-4080) join through the script against the public relay. The page lists both, and the demo buyer gets answers from both. Write the result in §4. | |

**Fastest order:** start A now, alongside B's build-script skeleton. Then D
and E. Then F. G about an hour before the talk.

---

## 3. Limits and what to say on stage

- **Keys are test-grade** (from `openagents-20`). Today a Pylon's key is
  `sha256(--test-key-label)`. Real key files (`--key-file`) are refused in
  code (`run.coax:167-168`). Lifting that needs two things:
  1. new code that reads a key file safely: an owned buffer, wiped after
     use, never logged;
  2. **an explicit owner sign-off in `NEEDS_OWNER.md`.** The #4828 audit
     being done is not production approval.

  For the demo, each joiner uses a fresh random label, generated by the
  script and kept in a `0600` file. Say on the page:
  - these are throwaway keys;
  - anyone who learns the label can act as that Pylon;
  - don't reuse a label, don't derive it from anything personal or from a
    real nsec, and don't show it in screenshots or logs;
  - don't point the demo at a real identity.
- **The signer audit (#4828).** The whole `nostr_sign_step` entry passes the
  large source-level secret audit (`7876e3db27`, `26d2e0d9c1`,
  `docs/verification/2026-10-30-signer-source-audit.md`). The compiled
  release signer shows no key-dependent branch or address outside its
  declared decisions (`coax/tests/nostr/schnorr_sign_secret_probe.sh`).
  Limits:
  - the audit covers the signer's source only;
  - host copies and memory wiping are outside it;
  - checked builds rely on the source proof;
  - there's no hardware timing, iOS or production approval.

  The final mutation re-gate is in progress; status in `docs/issue4828.md`.
- **Toy model.** The demo engine is `psionic-serve --toy`, not a real model.
  Elephant needs a GPU (the L4).
- **No money.** Decision work is free. Paying Pylons in Bitcoin is a later
  owner decision (Psionic PLAN, Part IV).
- **Privacy.** A Pylon that serves a request reads it. NIP-44 hides jobs only
  from the relay and other observers.
- **No routing yet.** `/v1/systemone` fanning out to Pylons is chapter 20,
  not built. On stage, jobs come from the demo buyer (F).
- **Ephemeral events.** Jobs and answers aren't stored by relays (NIP-DEC).
  A Pylon that's offline when a job is sent misses it.

---

## 4. Results log

(append as tested, with commit and time)

- 2026-10-30, `openagents-d7`: an attempt to build `pylon` and
  `psionic-serve` on coderos-4080 for test A was stopped when the owner
  reassigned the work. `serve-build.sh` had failed on `-Werror` (see B). The
  Pylon build didn't finish. A is not done.
