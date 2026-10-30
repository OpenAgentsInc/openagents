# Oct 30 MVP: anyone can join as a Pylon

Goal for today's talk (2026-10-30, about 4 hours from writing): a visitor
goes to openagents.com, or points their agent at the docs, runs one script,
and their computer joins the network as a Pylon. A Pylon is a machine that
answers decision jobs over Nostr (Psionic chapter 19). A desktop app is
later; a script is enough for today.

This page is the working plan. Claim a row by putting your session name in
it, and push updates as you go.

## What already works (on main)

- **The Pylon program**, `psionic/lib/pylon/run.coax`, built by
  `psionic/platform/pylon-build.sh OUT plain`. It:
  - connects to a relay over `ws://` or `wss://` (`relay_conn.coax`);
  - answers the relay's NIP-42 AUTH;
  - publishes a NIP-65 relay list and a kind 30200 beacon;
  - subscribes to kind 25910 jobs addressed to its key;
  - answers each with a 26910 result or a 27010 status, decided by a local
    engine. Job contents are NIP-44 encrypted.

  Every event is signed in Coax (`event_sign.coax`). On a dropped
  connection it reconnects, re-authenticates, resubscribes and republishes
  its beacon.
- **End-to-end test:** `python3 -I psionic/tests/pylon/pylon_e2e.py [--wss] OUT`.
  It passes 52/52 over ws:// and 53/53 over wss:// against our own Coax
  relay on 127.0.0.1, with the toy engine. It also passes with Elephant on
  the Google Cloud L4.
- **The engine:** `psionic-serve --toy`, built by `psionic/platform/serve-build.sh`.
  It runs on any laptop. Real models (Elephant) need a GPU host.
- **The buyer side:** `psionic/tests/pylon/buyer.ts` (bun) sends encrypted
  jobs and checks the answers with independent code (`@noble/curves`,
  `nostr-effect`).
- **TLS on macOS and Linux:** OpenSSL through Nix on Linux, Network.framework
  on Apple (`9b5dae736a`, `15ef4e1748`).
- **Chapter 19 text:** `psionic/book/ch019-a-pylon-answers-decisions.md`.

## Gaps for a public, joinable demo

| # | Gap | Status / notes | Owner |
| --- | --- | --- | --- |
| 1 | **Public relay.** Pylons must reach a relay anyone can use. Candidate: `wss://relay.openagents.com`, the deployed Rust relay. Unknown whether it accepts kinds 30200, 25910, 26910, 27010 and 10002, and whether it requires NIP-42. The Coax relay (`coax/relay/`) runs only on the tailnet (R1). | **Test first:** run the Pylon with the toy engine against it for 60 s. | openagents-d7 (testing now) |
| 2 | **Keys.** Real key files (`--key-file`) are refused until the #4828 secret-source audit is marked done (`docs/issue4828.md`; nearly done). Today the key is `sha256(--test-key-label)`. | For the demo, each joiner passes a long random label from `/dev/urandom`, which works as a fresh random key. Never a shared label. Real keys come after #4828 and owner approval. | — |
| 3 | **Join script.** One command: check prerequisites, fetch the repo, build `coax` (`coax/build.sh`, about 3 min), build `pylon` and `psionic-serve`, generate a random label, start `psionic-serve --toy`, start `pylon --relay wss://relay.openagents.com ...`. | Linux needs a C/C++ compiler and OpenSSL (Nix, `coax/platform/tls-env.sh`). macOS needs Xcode command-line tools. On this Linux host `cc` is not a C compiler: use gcc (`COAX_CC=gcc`). Prebuilt binaries would make joining faster but need hosting. | proposed: openagents-1a |
| 4 | **Website page.** `openagents.com/pylon`: what a Pylon is, the one-line join command, and a live list of current beacons (kind 30200 from the relay). Pages are Coaxial (Coax plus WebGPU); see `coax/web.md` and `apps/openagents-web/`. | A static instructions page plus a live beacon list. The project pages already read relays in Coax. | proposed: openagents-d7 |
| 5 | **Docs for agents.** A Markdown page (e.g. `apps/openagents-web/pylon/index.md`, served next to the page) that an agent can read and follow. | Same content as 4, written as steps. | with 4 |
| 6 | **Showing work flow.** Something has to send jobs so the audience sees Pylons answer: `buyer.ts` pointed at the public relay and a joined Pylon, or a button on the page. `/v1/systemone` fanning out to Pylons is chapter 20 (not built). | Simplest: run `buyer.ts` on stage against one or more joined Pylons and show the 26910 answers. | open |
| 7 | **Abuse and limits.** A public relay with anyone's beacons. A Pylon runs a toy engine and answers jobs addressed to it. Nothing is paid. | State on the page that this is a demo: test-grade keys, no money, a toy model. | — |

## Order of work (fastest path)

1. Test the Pylon against `wss://relay.openagents.com` (gap 1). If the relay
   refuses the kinds, the fallback is to expose the Coax relay publicly for
   the day. That needs owner approval and a host.
2. The join script (gap 3), tested on a clean Linux and a clean macOS
   machine.
3. The `openagents.com/pylon` page and its Markdown (gaps 4 and 5), deployed.
4. A demo rehearsal: two machines join, `buyer.ts` sends jobs, and the page
   shows both beacons.

## Results log

(append as tested)
