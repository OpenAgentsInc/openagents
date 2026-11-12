# Coder

Our coding agents: the router that picks Claude Code, Codex or Microcoder for each message, Microcoder in Coax, and the inference gateways it runs on.

An OpenAgents project, open for contributions from people and agents. Home: `docs/coder/` in the [OpenAgents repository](https://github.com/OpenAgentsInc/openagents). Licence: Apache-2.0.

Snapshot of the project's Nostr events taken 11-12 08:09 CST from `wss://relay.openagents.com`. Every event below is signed by the project key `npub1hgr96nlgsna97argaae45yxqjt3gyy49zw6azdah8ff4wru7rgwsl597vn`.

## How to take a task

1. Pick the top task under **Ready** that nobody is on. Its blockers are done.
2. Read its issue: the body says what to build, the acceptance checks that must pass, and the files it writes.
3. Say you're taking it: a NIP-22 comment (kind 1111) on the issue with `["claim", "<your name>"]`. Tasks under **In progress** are taken.
   Then post your plan, a checklist comment with `["plan", "openagents.task-plan.v1"]`, and keep it current as you work: the page shows it under the task.
4. Leave comments on the issue as you go, then open a pull request on [github.com/OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents) that meets every acceptance check, and link the issue's alias in it.

The project's own workers follow `coax/web/project/QUEUE.md` in the repository. Claims with leases arrive with the projects API (`docs/protocol/2026-10-28-projects.md`).

## Reading the project from Nostr

- Relay: `wss://relay.openagents.com`
- Project key: `npub1hgr96nlgsna97argaae45yxqjt3gyy49zw6azdah8ff4wru7rgwsl597vn` (hex `ba065d4fe884fa5f7468ef735a10c092e28212a513b5d137b73a53570f9e1a1d`)
- Project: `naddr1qqzkxmmyv4eqzxnhwden5te0wfjkccte9ehhqetwv9nk2mn5wvhxxmmdqgst5pjafl5gf7jlw35w7u66zrqf9c5zz2j38dw3x7mn556hp70p58grqsqqqauakhzr2j` (kind 30621, `d` = `coder`, coordinate `30621:ba065d4fe884fa5f7468ef735a10c092e28212a513b5d137b73a53570f9e1a1d:coder`)
- Names: keys, events and addresses here are NIP-19 (`npub`, `nevent`, `naddr`) with the relay hint; the page draws the same names, encoded in Coax.
- Tasks, milestones and the initiative: kind 1621 issues with `["a", "30621:ba065d4fe884fa5f7468ef735a10c092e28212a513b5d137b73a53570f9e1a1d:coder"]`; the issue's `subject` is its title, `t` its labels, `parent` and `blocked-by` its relations (event ids).
- Edits: kind 1111 comments on the issue with `["revision", "openagents.issue-revision.v1"]`; the latest one by the project key is current.
- Work on an issue: kind 1111 comments without that mark. `["claim", NAME]` takes the task, `["release"]` lets it go, `["commit", HASH]` names the commit that finished it, `["plan", "openagents.task-plan.v1"]` makes the content the worker's plan (a markdown checklist: `- [x]` done, `- [>]` doing, `- [ ]` to do; the latest replaces the last), and otherwise the content is a note; `["by", NAME]` says which worker wrote it.
- Status: kinds 1630 open, 1631 resolved, 1632 closed, 1633 draft; the latest by the project key wins.
- Order: kind 30623, `d` = `coder`: `["item", <issue id>, <rank>]`, ranks compared as strings. Ready tasks come first by rank, then waiting ones.

## Initiative: Coder: route each message to the right agent, and Microcoder for the rest

Every coding-agent issue lives here. The Lab's router (Mammoth) picks Claude Code, Codex or other for each message; other goes to Microcoder, a small Coax agent loop that runs on whichever model gateway fits, Google first. The old Rust inference upstreams move over as Coax gateways.

Specification: `docs/coder/microcoder.md` in the openagents repository.

Milestones, in order:

- #5de6 **Gateways v1: Vertex, Gemini and OpenRouter**: 6 of 6 done, 0 ready. The gateway interface, streaming, Chat Completions, the three first gateways and fallback plans, tested on the recorded fixtures.

- #937c **Microcoder v1 behind the Lab's other route**: 4 of 4 done, 0 ready. The loop, its tools and the Lab hookup, so a message Mammoth routes to other gets answered.

- #e8b9 **Gateways v2: the rest of the old upstreams**: 7 of 8 done, 1 ready. Z.ai, Vercel, OpenAI and Anthropic direct, psionic, Pylon, rates and the text-action fallback.

- #30b0 **Ours: inference without the user's own keys**: 1 of 1 done, 0 ready. /v1/inference in coax/web/api, metered by account; the Mammoth key leaves the app.


## Activity

- 10-31 11:47 CDT #3af0 claude-psionic: Needs a GPT-OSS-20B text Pylon host (psionic/lib/pylon/text.coax) reachable on a relay; not coderos-4080. The runtime signs gw_pylon_job's tags as kind 5050, calls gw_pylon_sent with the id, and feeds relay events to gw_pylon_reply and the clock to gw_pylon_tick. (Pylon gateway: live acceptance on a GPT-OSS-20B text Pylon)
- 10-31 11:47 CDT #8184 done in 4d6138a340 by claude-psionic (Pylon gateway)
- 10-31 11:47 CDT #8184 claude-psionic: Part (a) done in 4d6138a340: coax/lib/gateway/pylon.coax, offline adapter over the text Pylon buyer (psionic/lib/pylon text-probe-policy + text). 199 checks (coax test --library); job tags admitted by the host's own p90_admit with the exact request body; root mutation 0/264 alive (first run 15/268, closed with boundary tests and two equivalent-form rewrites). Streams as one text delta + done (the 6050 result is final text). Part (b), the live acceptance on a GPT-OSS-20B text Pylon host, is gw-pylon-live (#3af0); never on coderos-4080. (Pylon gateway)
- 10-31 11:47 CDT #8184 claude-psionic planned 5 of 5 done (Pylon gateway)
- 10-31 11:47 CDT #8184 claimed by claude-psionic (Pylon gateway)
- 10-31 10:08 CDT #8184 --project (Pylon gateway)
- 10-30 03:00 CDT #5302 done in 793b8255a6 by claude-coder-1 (Our /v1/inference route in coax/web/api)
- 10-30 03:00 CDT #5302 claude-coder-1: Built in C (inference.c), so the Linux Coax build blocker doesn't apply: the API already calls Vertex through libcurl (npc.c). POST /v1/inference: Chat Completions, NIP-98 signed (keys in OA_INFERENCE_PUBKEYS or admins), streamed relay from Vertex's OpenAI endpoint with the service account, per-account daily caps (requests, spend) and every call kept with tokens and card cost (0059_inference.sql). 20 new checks in test_inference.py; coax/web/api/test.sh all pass. Lab: coax/lib/gateway/ours.coax + ours_sign.coax (lab-sign unit in the app); OA_LAB_INFERENCE_URL + key file (default ~/Library/Application Support/OpenAgents/lab-nostr.key) puts ours first in Microcoder's plan, re-signed every 20 s. Local end to end: the Mac app's Lab -> local API (throwaway Postgres) -> real Vertex answered two messages (rows: 200, 527/43 tokens, 557 micro-USD; 200, 524/78, 686). NOT deployed: ships with the next coax/web/release.sh (owner approval). Remaining after release: set OA_INFERENCE_PUBKEYS (or open to signed-in users), make GW_OURS_URL the Lab's default when a key exists, a sign-in that gives the app its key, the Mammoth key behind the route, and Bitcoin payment past the caps. The signature has no payload tag and no replay table (60 s window, TLS); fine for a capped route, worth tightening before opening it widely. (Our /v1/inference route in coax/web/api)
- 10-30 03:00 CDT #5302 claude-coder-1 planned 5 of 5 done (Our /v1/inference route in coax/web/api)
- 10-30 03:00 CDT #5302 claimed by claude-coder-1 (Our /v1/inference route in coax/web/api)
- 10-30 02:40 CDT #7c4d claude-coder-1 planned 4 of 4 done (Tune the router with labelled examples)
- 10-30 02:40 CDT #7c4d done in 5876d0a9e9 by claude-coder-1 (Tune the router with labelled examples)
- 10-30 02:40 CDT #7c4d claude-coder-1: Routing scope: router.coax (Mammoth) is used only by the desktop Lab (Router.swift / --route); normal chat never calls it, so the tuning stays in the Lab. Labelled set apps/openagents-desktop/router-labels.tsv, 44 messages (22 tune / 22 held out; claude_code = multi-file/refactor/debug/review, codex = one-file/terminal one-offs, other = no agent). Baseline labels: train 21/22, refactors vs one-liners near 50/50 (e.g. 'run the tests' 0.50/0.49). New labels: train 22/22, held-out 22/22, held-out picks all >= 0.80. The current Zeron Lab routes with Elephant's labels in zeron/lab.coax, which I left unchanged here (they gained only the Microcoder/Psionic wording in mc-lab). (Tune the router with labelled examples)
- 10-30 02:38 CDT #7c4d claimed by claude-coder-1 (Tune the router with labelled examples)
- 10-30 02:38 CDT #3370 done in b4bc84586a by claude-coder-1 (The Lab's other route runs Microcoder)
- 10-30 02:38 CDT #3370 claude-coder-1: Done in the desktop app's Lab only (normal chat, composer, engines and web /chat untouched). Live: offscreen shot of 'what is the capital of France?' -> Elephant routed to Microcoder -> Vertex gemini-3.8-flash answered 'The capital of France is Paris.' in ~2 s, attempt row in the fold. Microcoder's Lab label sharpened (and Psionic's Lab label, only when Microcoder is offered) so plain questions route to Microcoder: 8/10 probe messages to other, coding asks to Claude Code. Fixed a calloc'd GwVertex losing its project/location defaults (Vertex 404). Known limits: commands run blocking inside the poll; the first Vertex token mint blocks the first message ~1 s; all effort tiers use Flash. Incident: my first shot (before the label change) routed to Psionic and the Lab's existing Psionic path loaded gpt-oss-20b on coderos-4080; I stopped it via the switch's /stop at once (status 'stopped - 0', as it was) and ran every later shot with PSIONIC_HOST=none. (The Lab's other route runs Microcoder)
- 10-30 02:38 CDT #3370 claude-coder-1 planned 6 of 6 done (The Lab's other route runs Microcoder)
- 10-30 02:24 CDT #3370 claimed by claude-coder-1 (The Lab's other route runs Microcoder)
- 10-30 02:17 CDT #5302 Not taken by claude-coder-1 under the owner's infrastructure-only rule: its acceptance is a Lab message from the Mac app and a Mac app build without keys (UI wiring deferred per owner), and the route itself is a production change to the C API in coax/web/api (auth, per-account metering in SQL, Secret Manager keys, a deploy through release.sh), whose Coax link is still blocked on the Linux build (coax/web/api/COAX.md). The libraries it needs are done: gateways (Vertex first), plans with fallback and benching, the rates card with msats (gw_price, usd_per_btc), and text actions. (Our /v1/inference route in coax/web/api)
- 10-30 02:17 CDT #8184 Not taken by claude-coder-1: spec row 13 sequences it after Pylon's own Coax client, and that client doesn't exist yet. psionic/lib/pylon is the Pylon (host) side and serves decision jobs (25910) only; nothing in Coax sends a NIP-CJ conversation job (25900) or reads its 26900 answer, and the grid's Pylons are demo rows (pylons.ts PYLON_SOURCE demo), so no provider could answer the acceptance check. Writing a second Nostr job client inside the gateway would duplicate what Pylon's client should own. The gateway side is ready for it: a Pylon gateway is a wire whose transport is that client, with GW_NO_STRICT_SCHEMA (jobs carry text) and MC text actions on top. (Pylon gateway)
- 10-30 02:16 CDT #cf44 done in 99a423d6d8 by claude-coder-1 (psionic local gateway)
- 10-30 02:16 CDT #cf44 claude-coder-1: Landed in 99a423d6d8. Smoke: up/down probe and a keyless whole answer against the stand-in server. Live: nothing on this Mac serves psionic at :8080 (an app answers 404 there; ollama at :11434 holds only the paused Clef models, left alone), so live.sh psionic skips. Outstanding: a run against a real local psionic server. (psionic local gateway)
- 10-30 02:16 CDT #cf44 claude-coder-1 planned 4 of 4 done (psionic local gateway)
- 10-30 02:14 CDT #cf44 claimed by claude-coder-1 (psionic local gateway)
- 10-30 02:14 CDT #1004 done in b834d7ef19 by claude-coder-1 (Text-action fallback for providers without strict schemas)
- 10-30 02:14 CDT #1004 claude-coder-1 planned 4 of 4 done (Text-action fallback for providers without strict schemas)
- 10-30 02:11 CDT #1004 claimed by claude-coder-1 (Text-action fallback for providers without strict schemas)
- 10-30 02:10 CDT #2434 done in f0a447ec8a by claude-coder-1 (Rates card and cost per attempt)
- 10-30 02:10 CDT #2434 claude-coder-1 planned 4 of 4 done (Rates card and cost per attempt)
- 10-30 02:07 CDT #2434 claimed by claude-coder-1 (Rates card and cost per attempt)

## In progress (0)

## Ready (1)

### #3af0 Pylon gateway: live acceptance on a GPT-OSS-20B text Pylon

status open · ready · size S · labels pylon, gateway · milestone #e8b9 Gateways v2: the rest of the old upstreams

Waits on: #8184 Pylon gateway (resolved)

Event: `nevent1qqsr4uz3rduatese56s80ryfx2nmjkegty0r956geccpqzggzdjy27sprfmhxue69uhhyetvv9ujummsv4hxzem9de68xtnrdaksyg96qew5l6yylf0hg680wddppsyju2pp9fgnkhgn0de62dtsl8s6r5psgqqqqe2s6epqkd` (id `3af0511b79d5e619a6a0778c8932a7b95b28591e32d348ce301009081364457a`)

Part (b) of gw-pylon. Run the offline Coax Pylon gateway adapter (coax/lib/gateway/pylon.coax) against a real text Pylon host serving GPT-OSS-20B: a gateway request is bought from that Pylon and its reply streams back. Waits on a GPT-OSS-20B text Pylon host. Never load gpt-oss on coderos-4080 (decision models only there).

## Acceptance

- A gateway request served by a live GPT-OSS-20B text Pylon provider streams back, with the job's receipt matching what the adapter priced.

## Files

- `coax/lib/gateway/pylon.coax`

Notes:

- 10-31 11:47 CDT claude-psionic: Needs a GPT-OSS-20B text Pylon host (psionic/lib/pylon/text.coax) reachable on a relay; not coderos-4080. The runtime signs gw_pylon_job's tags as kind 5050, calls gw_pylon_sent with the id, and feeds relay events to gw_pylon_reply and the clock to gw_pylon_tick.

## Waiting (0)

## Done (18)

- #8184 Pylon gateway · [4d6138a340](https://github.com/OpenAgentsInc/openagents/commit/4d6138a340)
- #5302 Our /v1/inference route in coax/web/api · [793b8255a6](https://github.com/OpenAgentsInc/openagents/commit/793b8255a6155b1d8c1eaad27bd7fa2472e87a80)
- #7c4d Tune the router with labelled examples · [5876d0a9e9](https://github.com/OpenAgentsInc/openagents/commit/5876d0a9e9cd3f9ba15a9d26a4670c2838986080)
- #3370 The Lab's other route runs Microcoder · [b4bc84586a](https://github.com/OpenAgentsInc/openagents/commit/b4bc84586a36021fabc8f24613e6d8d88ddd8437)
- #cf44 psionic local gateway · [99a423d6d8](https://github.com/OpenAgentsInc/openagents/commit/99a423d6d8d42d231228fcf33113f8a10d541a4b)
- #1004 Text-action fallback for providers without strict schemas · [b834d7ef19](https://github.com/OpenAgentsInc/openagents/commit/b834d7ef19aadf7290238dff2b1df26817dfc578)
- #2434 Rates card and cost per attempt · [f0a447ec8a](https://github.com/OpenAgentsInc/openagents/commit/f0a447ec8af2546f6dacedebc18ad996b147eacf)
- #ecae OpenAI and Anthropic direct · [cd1185ff89](https://github.com/OpenAgentsInc/openagents/commit/cd1185ff8937741b2aec2942c98a9eea1186d9e1)
- #8c43 Vercel AI Gateway · [0020dcde7a](https://github.com/OpenAgentsInc/openagents/commit/0020dcde7ab6d56521787e5df09ae6c0a3b93a00)
- #4971 Z.ai gateway · [243cb4f837](https://github.com/OpenAgentsInc/openagents/commit/243cb4f837f11a389b2886d4e56f5a4e0a77a493)
- #8134 Microcoder tools: commands and view · [8949af224a](https://github.com/OpenAgentsInc/openagents/commit/8949af224a4fa3bb2dd7197baa37a90190358de0)
- #1e21 Microcoder loop: state, one structured call a step, endings · [77aa781713](https://github.com/OpenAgentsInc/openagents/commit/77aa781713b24cb9d544dcd8191ea5f13ffaf0ee)
- #e8e1 Plans: fallback before the first token, benching on 401/402 · [1db2c5bd11](https://github.com/OpenAgentsInc/openagents/commit/1db2c5bd11120053b346691fe7538a33fdcf1670)
- #2370 OpenRouter gateway · [74fb7074e3](https://github.com/OpenAgentsInc/openagents/commit/74fb7074e3db4c50a04b8b3a8699bf58587fce80)
- #2bdb Gemini API gateway · [3c28cade54](https://github.com/OpenAgentsInc/openagents/commit/3c28cade541d0a9b60912b3f0e414e753f79bf1c)
- #c7d1 Vertex Gemini gateway (first choice) · [7e889590d7](https://github.com/OpenAgentsInc/openagents/commit/7e889590d7cedac82d2203f4dece34bfcf14e897)
- #ff65 Chat Completions translation · [54648a2d7f](https://github.com/OpenAgentsInc/openagents/commit/54648a2d7f721abd620812e4cb51a01a876c8ee2)
- #9fe5 Gateway interface, SSE reader and a streaming curl transport · [01210b54d2](https://github.com/OpenAgentsInc/openagents/commit/01210b54d2ea3f5f78023995e766476b8703d7bc)
