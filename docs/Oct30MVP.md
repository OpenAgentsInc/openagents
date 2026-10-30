# Oct 30 MVP: anyone can join as a Pylon

**Goal, for the owner's talk on 2026-10-30.** A visitor opens
openagents.com, or points their agent at a docs page there, runs **one
script**, and their computer joins the network as a **Pylon**. A Pylon is a
machine that announces itself on Nostr and answers decision jobs (Psionic
chapter 19). On stage, the audience sees Pylons appear and answer jobs. A
desktop app comes later; the script is today's path.

This is the full spec. **Codex implements it.** Push results to main as each
item lands, and record them in §4.

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
Elephant runs on the Google Cloud L4 (`psionic-l4-elephant`); it isn't public (see below).

### The end-to-end test

`python3 -I psionic/tests/pylon/pylon_e2e.py [--wss] OUT` builds the Coax
relay, the Pylon and the toy engine, starts them, and runs `buyer.ts`.

- **Results** at `fed0a442b2`, again at `753e949fc6`: 52/52 over ws:// with a
  relay restart, 53/53 over wss://, and 45/45 with Elephant on the L4.
- **Relay used:** only our own Coax relay on 127.0.0.1. **The Pylon has never
  been run against a public relay.**
- **Read-only probe of `wss://relay.openagents.com`** (2026-10-30):
  - the WebSocket upgrades, and the relay's first frame is
    `["AUTH","<64 hex>"]`, so the Pylon's AUTH-first path should work. The
    NIP-42 relay URL is exactly `wss://relay.openagents.com`.
  - NIP-11: `auth_required: false`, `restricted_writes: true`, NIPs 1, 9,
    11, 17, 29, 40, 42, 45, 50, 59, 65, 67, 70, 77, 94, 98.
  - The Rust relay (`0b401561a0^:crates/nostr-relay/src/store`) admits a kind
    when its `relay_allowed_kind` list is empty or contains that kind;
    `relay_allowed_pubkey` works the same way. Both are set through the
    NIP-86 management API, and the deployed values can't be seen from
    outside. Test A settles it: look for `OK false "blocked:"` or
    `"restricted:"`.
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
`STOP_FILE`, `CANCEL=0`, and `NODE_PATH` holding `@noble/curves` and
`nostr-effect`; `pylon_e2e.py` uses `coax/web/project/node_modules`. **It is
the test's scripted buyer** (jobs r-1 to r-8, cancellation, restart
handling), not a general demo client.

### A page-side reader already exists

`coax/web/project/pylons.ts` reads Pylons from the relay. It counts 26910
answers and 27010 statuses.

**Gotcha:** 25910, 26910 and 27010 are ephemeral (20000–29999), so relays
deliver them live and never store them. Counted from history, they come out
as zero. A live view has to subscribe and count from the moment it opens.
Kinds 30200 (beacons) and 10002 are stored.

### Not available for the demo

- **The Elephant L4** binds `127.0.0.1:8797` on a spot VM, and its tunnel is
  closed. It isn't public, so don't route joiners there. Real Elephant needs
  about 15 GB of device memory plus 14.5 GiB of pinned RAM, so it isn't for
  laptops.
- **The Coax relay (`coax/relay`) isn't ready for public exposure.** It is
  the tailnet R1. Server TLS, a flood test and the Rust differential are
  still in progress, and it has no rate limits. Use relay.openagents.com.
- **No Pylon desktop app exists** (`docs/desktop/coax-desktop.md` is a
  different app).

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

| # | Item | Details | Who |
| --- | --- | --- | --- |
| **A** | **Prove the public relay** (do first; everything depends on it) | Build `pylon` and `psionic-serve` and run them against `wss://relay.openagents.com` for 2 minutes with the toy engine and a random `--test-key-label`. Check these, and record the results below: <br>• the log reaches `pylon: online, waiting for jobs`; <br>• the relay sends NIP-42 AUTH and accepts the AUTH event; <br>• the relay accepts the 10002 and 30200 events, and `REQ {kinds:[30200]}` from a second client returns the beacon; <br>• a 25910 job sent to the Pylon's pubkey gets a 26910 back through the relay. <br>The deployed relay is the old Rust `nostr-relay`. Its NIP-11 advertises NIPs 1, 9, 11, 17, 29, 40, 42, 45, 50, 59, 65 and others (`coax/nostr.md` §17). Nobody has checked whether its policy admits kinds 25910, 26910, 27010 and 30200 from arbitrary pubkeys, or how it rate-limits. **If it refuses them:** the owner has to allow the kinds (and pubkeys) through the relay's NIP-86 management API. The Coax relay isn't a safe fallback (see §1). | Codex |
| **B** | **One-command join script** | Put it at `scripts/join-pylon.sh` (or similar) on main and serve it from openagents.com. It should: <br>• check prerequisites: git; a C/C++ compiler (gcc/g++ on Linux, Xcode CLT on macOS); Nix on Linux for OpenSSL; bun only for the buyer; <br>• clone or update the repo into `~/.openagents/pylon`; <br>• build: `coax/build.sh` (about 3 min on a fast machine, about 8 min on a loaded one; nobody has a laptop number yet), then `pylon-build.sh OUT plain` and `serve-build.sh OUT release plain`. **Don't pass `--test-trust`**: it trusts only `CXTLS_TEST_CA`. The plain build uses the system trust store. Build at or after `753e949fc6`, the signer's message-binding fix; <br>• generate `SECRET_LABEL="$(head -c 32 /dev/urandom \| xxd -p -c 64)"`, store it in a `0600` file in the joiner's state directory, and never print it. **Caveat:** today the Pylon only takes the label as `--test-key-label` on the command line, which other users on the machine can see in `ps`. A small change is needed: `--test-key-label-file PATH`, reading the label from the file. Or accept the risk on single-user machines for the demo; <br>• pick `SLUG=pylon-$(hostname -s)-<4 random hex>` and `LABEL="<hostname> (demo)"`; <br>• start `psionic-serve --toy --port <free port>`, wait for the port, then start `pylon` with the command template in §1, pointed at the relay chosen in A; <br>• print the Pylon's pubkey (npub) and slug, and say how to stop it (Ctrl-C or the stop file). <br>**Known build problems to fix or work around:** <br>• On coderos-4080 (gcc 15), `serve-build.sh` fails: `-Werror=misleading-indentation` at `psionic/platform/serve-supervisor.c:18`. The script adds `-Wno-misleading-indentation` only for `cuda-host.c`. Add it to that compile line too, or fix the indentation. `COAX_CC` can't carry flags. <br>• **TLS trust on non-NixOS Linux:** Nix OpenSSL's default verify paths may not find the CA bundle. Set `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt` (Debian and Ubuntu) or `/etc/ssl/cert.pem`, or the wss:// handshake fails as untrusted. On macOS the Pylon uses Apple TLS (`docs/verification/data/apple-tls-2026-10-30`). <br>• `cc` on PATH may not be a C compiler (on coderos-4080 it is the Claude CLI). The build scripts honour `COAX_CC`/`CC`; set `COAX_CC=gcc`. <br>• The Wasm toolchain isn't needed for a Pylon. <br>Test on a clean Linux machine and a clean macOS machine. | Codex |
| **C** | **Prebuilt binaries** (optional, speeds up joining) | Build `pylon` and `psionic-serve` for linux-x86_64 and macos-arm64. Host them on openagents.com with SHA-256 sums printed by the script and on the page. Drop this if time is short; building from source is fine. | Codex |
| **D** | **`openagents.com/pylon` page** | The page should show: <br>• what a Pylon is, in one paragraph; <br>• the join command (`curl -fsSL https://openagents.com/pylon/join.sh \| bash`, or `git clone` plus the script); <br>• a **live list of Pylons**: a `REQ {kinds:[30200]}` subscription on the relay showing each beacon's label, model identity, free slots, last-seen time and npub; <br>• the limits (§3). <br>Implement it as a Coaxial page (`apps/openagents-web/pylon/`). The project pages already subscribe to relays in Coax (`project-live.coax`, `socket.coax`), the verifier library is in `nip01-verify.coax`, and `coax/web/project/pylons.ts` already reads Pylons. Count answers live (see §1). Deploy with `coax/web/release.sh`, from the Mac. If a Coaxial page can't be ready in time, a plain-text `index.md` plus a static listing is the floor. | Codex |
| **E** | **Agent-readable docs** | `apps/openagents-web/pylon/index.md`: the same content written as numbered steps an agent can follow: prerequisites, the command, how to tell it's online, how to stop it, troubleshooting. Link it from the page and from `/llms.txt` or the docs index if there is one. | Codex |
| **F** | **Stage demo buyer** | A small script, `psionic/tests/pylon/demo-buyer.ts` or similar, built on `buyer.ts`'s code. It lists the live beacons on the relay, sends one decision job (25910, NIP-44 to the Pylon) to each, or to a chosen one, and prints each answer (26910) with the answering identity and latency. The jobs have to be valid `openagents.systemone.v1` requests for the toy model (see `buyer.ts` and NIP-DEC). Optionally, a button on the page that does the same in the browser. | Codex |
| **G** | **Rehearsal** | Two machines (e.g. the Mac and coderos-4080) join through the script against the public relay. The page lists both, and the demo buyer gets answers from both. Write the result in §4. | Codex |

**Fastest order:** start A now, alongside B's build-script skeleton. Then D
and E. Then F. G about an hour before the talk.

---

## 3. Limits and what to say on stage

- **Keys are test-grade.** Today a Pylon's key is
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

- 2026-10-30: an earlier attempt to build `pylon` and `psionic-serve` on
  coderos-4080 for test A was stopped before it ran. `serve-build.sh` had
  failed on `-Werror` (see B), and the Pylon build didn't finish. A is not
  done.

- 2026-10-30, B (initial implementation): `scripts/join-pylon.sh` now checks
  prerequisites, uses a shallow sparse source checkout, reuses only a
  matching checked compiler, bounds each build, launches the loopback toy
  engine and Pylon, waits for online readiness, and stops both on Ctrl-C or
  a stop file. The throwaway label is read through `--test-key-label-file`,
  not exposed as an argv value; only its public key/npub is printed. Coax
  demo-label checks passed 1/1 and reference mutation 18/18; native file
  metadata/bounds/refusal/clearing tests passed under ASan/UBSan. The actual
  Pylon compiled/linked and refused a missing label file with exit 2. Offline
  join tests passed for a source path containing spaces, readiness, private
  label creation, public output and stop/cleanup. Linux/macOS public join
  qualification is pending. `python3` is also required for the existing
  engine build tooling; the Wasm toolchain and Bun are not join dependencies.

- **A passed, 2026-10-30 14:55–14:58 UTC (09:55–09:58 CDT).** Native
  Mac Pylon built from `a944bfbaaacfea649c473a32243943d3f5475956`, using
  the qualified Coax compiler and system TLS trust, served
  `oct30-mac-8d6f69` on `wss://relay.openagents.com` for 164.8 seconds.
  Its two-case reference self-check passed; NIP-42 authentication and relay
  list/beacon publication succeeded. A second fresh-key buyer retrieved and
  verified its beacon, authenticated, sent an encrypted 25910 job and
  verified/decrypted the matching 26910 answer (233 ms round trip, 45 ms
  provider execution). No relay policy changes were needed. Clean shutdown
  published an offline beacon: 0 refused, 0 drops, 1 answered. This supersedes
  the earlier stopped attempt above. No real identity was used.
  Evidence: [`data/oct30-pylon-public-a`](verification/data/oct30-pylon-public-a/)
  (public log, observed interval, source/compiler/binary hashes); buyer evidence
  is recorded separately with F. These exact inputs passed; unrelated main
  commits do not call for rerunning A.

- **F — stage buyer, 2026-10-30 14:58 UTC, `8c2f3defe5`.**
  `psionic/tests/pylon/demo-buyer.ts` now lists verified fresh v1 beacons,
  sends one NIP-44 toy job per selected/all available toy Pylon, subscribes
  before publishing, and prints verified answering identity plus latency.
  Fresh random buyer keys remain in memory; no production identity or money.
  AUTH/job refusal, stale targets and bounded missing-answer timeouts are
  explicit. Model mismatch is never silently substituted. Setup and the
  supported demo profile: `psionic/tests/pylon/DEMO.md`.
  `bun run --cwd psionic/tests/pylon test:demo`: **11/11**, including a full
  loopback AUTH/discovery/encrypted two-provider exchange, wrong/invalid
  events, relay refusal, signed provider error, and timeout (1.89 seconds).
  **Public interoperability:** before 14:56:14 UTC, the independently
  signed buyer authenticated to `wss://relay.openagents.com`, read
  `oct30-mac-8d6f69`, received the relay's 25910 acceptance and decrypted a
  verified 26910 answer from npub
  `npub1k246jd3yex8sg5z7klavsd483mf40y6hl4txgeh08st3huq4z3cqturlvk`.
  Identity `psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573`;
  choice `no`, probabilities yes `0.437947251` / no `0.562052749`, **233 ms**.
  Exact command/result, runtime versions and original buyer source hash are
  in `docs/verification/data/oct30-demo-buyer/public-a.json` (source snapshot
  alongside it). This proves the buyer/Pylon public exchange; A's Pylon
  build/10002/two-minute evidence and G's two-machine rehearsal are recorded
  separately by the demo owner.

- **B — Linux join passed, 2026-10-30 15:03 UTC.** The actual join script
  at `efe5c86819` built Pylon and the toy engine with GCC/Nix, passed both
  reference cases, authenticated with NIP-42 to the public relay using normal
  system TLS trust, and published `oct30-coderos-demo` online. The join took
  about **137 seconds** with an existing isolated checkout and matching checked
  compiler; that timing excludes source download and compiler bootstrap.
  A fresh 64-character random throwaway label stayed in a **0600** file and
  only the file path entered argv; npub/public key were printed. The Linux
  label-file host tests also passed with ASan/UBSan. Public logs and exact
  source/compiler/binary identities are in
  [`data/oct30-join-linux`](verification/data/oct30-join-linux/). The node was
  left running for G. A separate fresh-clone Mac qualification is owned by
  the demo owner. Native input hashing now uses the same compile-time work
  cap as the Pylon build. A focused failed-start regression first reproduced
  linked-stop-file truncation, then passed with the cleanup guard; the guard
  is in `488220542e`. This does not change the tested native source graph.
