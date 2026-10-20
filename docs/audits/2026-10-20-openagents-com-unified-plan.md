# One openagents.com: themes, prior synthesis, and a unified plan

Recorded 2026-10-20, at `d3b447cad5`. Owner request: read the transcript
archive, note every theme, find where we have already synthesized it, and
plan how to bring the ideas together into one Nostr-first website and web
app at openagents.com, with social profiles and social agent use.

Sources: all 290 episode files in [`docs/transcripts/`](../transcripts/README.md)
(skimmed in full, vision episodes read closely), the audits listed under
[Prior synthesis](#prior-synthesis), `nips/openagents/`, and the routes in
`crates/openagents-web/src/lib.rs`. Transcripts are machine-made. Quotes
below are as transcribed; check the audio before you publish one.

- [Summary](#summary)
- [Themes](#themes)
- [Prior synthesis](#prior-synthesis)
- [The unified product](#the-unified-product)
- [Information architecture](#information-architecture)
- [Phased plan](#phased-plan)
- [Risks and open questions](#risks-and-open-questions)

## Summary

- The archive asks one question for three years: how can people **own,
  inspect, improve, and earn from** agent software? Every product reset
  answers it with the same five parts: open protocols (Nostr), open money
  (Bitcoin), reusable paid components (plugins and skills), inspectable work
  (graphs, traces, receipts), and a network of people and agents.
- Most of the parts now exist as crates. What's missing is one place where
  a person sees them together. Today the website is a chat app over HTTP;
  the network lives in NIPs, the CLI, and the phone.
- **The plan:** the Nostr key is the account and the public profile. Agents
  get keys and profiles of their own, owned by a person through NIP-OA.
  Profiles show signed evidence (XP, receipts, labels, earnings) instead of
  self-reported numbers. The GPUI web app is the shell; server HTML stays the
  first paint, the fallback, and every public page.
- **First three moves:** person profiles and NIP-05 names (U1, #12254), agent
  keys and public profiles (U2, #12255), and one front door at `/` (U7,
  #12260) on top of GPUI web chat phases 0d and 2 (#12219, #12222).

## Themes

Episode numbers point at `docs/transcripts/NNN.md`. "Today" names what is in
the tree now.

| Theme | What the archive says | Episodes | Today |
| --- | --- | --- | --- |
| Open agents and ownership | AI should not belong to closed labs; "open models, open data, open compute, open money, open protocols." | 001, 047, 086, 101, 141, 150, 166, 282, 288 | Whole repo is open source; [`README.md`](../../README.md) |
| Inspectability | "Open must mean inspectable": every step, input, model, and cost visible. Later: proof, not activity. | 033–035, 038, 162, 237, 249–252, 259 | `crates/atif` traces, `crates/receipts`, `/agent/{id}` graph |
| AgentGraph | A node graph that shows and edits an agent; edited graphs sold with revenue share. Returns as knowledge plus execution graphs on Nostr. | 038–040, 059–060, 073–075, 085, 125, 250 | `crates/agent-graph`, `apps/openagents/web/agent-graph`, `/agent/{id}`; [AgentGraph audit](2026-10-19-agentgraph-unit.md) |
| Agents as social actors | Profiles, follows, reviews, and shareable agent pages (094–102); agents as NPCs with guilds (116); the agent network and Reed's law (200, 237); forum where agents meet (230–235); Sarah as a public agent (260–272). | 005, 094, 098, 100, 116, 139, 177, 200, 209–212, 230, 235, 237, 255, 269 | Verse agents with keys (Alice, `docs/verse/agent-identity-and-engrams.md`); `/v1/published-agents`; no social profiles |
| Nostr identity and NIPs | Log in with Nostr (082); plugins as Nostr events (066); one seed for wallet and identity (139, 207); NIP-90 markets (142, 203, 214, 266–267); "signed JSON over WebSockets" (289). | 066, 082, 088, 125, 139, 141, 177, 203, 207, 214, 237, 263, 266, 267, 288, 289 | NIP-06 identity and `POST /v1/sessions/nostr` (#12006); `/login` QR and NIP-07; `crates/nostr`, `crates/nostr-relay` at `relay.openagents.com` |
| Bitcoin, Lightning, Spark | Agents hold their own balances (042, 062); pay contributors daily or by the minute (092–099); "velocity" (153); Spark (143, 169); pay plugin authors per call (289, 290). | 001, 037, 042, 062–064, 086, 092–099, 143, 153, 169, 173, 212, 223, 230, 289, 290 | `crates/spark-wallet` (phone, CLI), `crates/x402`, `crates/wallet`; no browser wallet ([Spark audit](2026-10-17-spark-wallet.md)) |
| Compute markets and Pylons | GPUtopia swarm (001–019); Pylon node (144); "fracking Apple Silicon" (201); compute market launch (213–214); decentralized training (216–224, 236–238). | 001, 086, 138, 144, 145, 174, 178, 201, 203, 213, 214, 221–224, 237, 284 | `crates/pylon`, NIP-PYLON, `/pylons`, desktop Settings > Earn; [`docs/compute/pylon-1.0.md`](../compute/pylon-1.0.md) |
| Plugins, skills, revenue share | WASM plugins (048–075); the Agent Store pays builders in Bitcoin (092); MCP store (165); "npm for agent intelligence" (104, 206, 238, 286); first solver packages a plugin and is paid (269, 289, 290). | 037, 048–058, 066, 085, 092, 102, 104, 164, 165, 195, 199, 206, 237, 269, 286, 289, 290 | `crates/plugin-*`, `crates/plugin-registry` (NIP-REG, NIP-EXT), `tenancy::skills`, payout worker (#10190) |
| Coding agents | Faerie and AutoDev (020–032, 103–123); repo maps (156–163); Autopilot (199–212); Probe, Khala Code, Desktop (218–258); Coder (275–290). | 020, 031, 036, 103, 123, 156, 199, 218, 237, 246, 275, 285–290 | `crates/coder`, `crates/microcoder`, `apps/openagents`; Coder 1.0.0 |
| Training, Gym, and evals | Tests as critic (029); SWE-bench (120); DSPy optimization (206); Psionic and Tassadar (216–238); Gym and Terminal-Bench (287, 288); plugin evals (289). | 029, 036, 120, 121, 186, 206, 216, 217, 222, 236–238, 287–290 | `crates/gym`, `crates/gym-leaderboard`, `crates/psionic`; `/gym` removed from the site on 2026-09-29 |
| Decision models | Typed judgments between turns (206, as DSPy signatures); System One, Jev, and typed decisions (285–290). | 206, 285, 286, 287, 289, 290 | `crates/jev`, `crates/kev`, `crates/laya`, `crates/gateway` (`POST /v1/systemone`) |
| Spatial UI and Tony Stark | "I want my Tony Stark Jarvis" (156, 158); StarCraft HUD for agent fleets (170, 186, 199); MediaPipe hand tracking (179, 185); hands in the compositor (285). | 111, 117, 145, 150, 156, 158, 170, 176, 179, 185, 186, 208, 214, 285 | `apps/openagents/web/hands`, `/lander9`; `crates/coder-compositor` (`src/hands.rs` is a stub) |
| Verse, the Grid, games | "The UI patterns we need are found in games" (116); Ruins of Atlantis (189); the Verse over the run board (240); "a Snow Crash world" with XP and auction houses (284). | 036, 116, 148, 172, 189, 237, 240, 283, 284, 290 | `crates/verse*`, `crates/everglade-web`, NIP-MV, NIP-XP; `/grid`, `/everglade`, `/lander3`, `/lander5` |
| Phone and voice | Onyx (139–153); Tricoder (191–194); Sarah (260); "one conversation everywhere" (281, 289, 290). | 100, 139–153, 187, 191–195, 260, 276, 281, 289, 290 | OpenAgents iOS and Android in `apps/openagents/apps/` |
| The website as front door | Replace ChatGPT (002, 067, 089); agent store (092); "front door of the agent internet" (209–212); "single home for all this" (274); ChatGPT-style homepage (225). | 002, 004, 038, 067, 086, 089, 092, 100, 126–138, 164, 209–212, 225, 229, 231, 269, 274, 289 | `crates/openagents-web` (maud and HTMX), `/gpui`, `/lander4`–`/lander9` |
| Data, knowledge, and disclosure | Pay for data and traces (147, 215, 245); crowd knowledge graph (154, 155); who may see a trace (046, 245, 269). | 046, 147, 154, 155, 215, 245, 269, 285 | NIP-KB (`microcoder` knowledge), NIP-VAULT (`oa-vault`) |
| Network effects | Reed's law with no Dunbar limit (200, 237); "the moat is a network" (269); "the value is in the network" (220). | 165, 168, 195, 200, 220, 230, 237, 255, 269, 286 | Spread across crates; no surface shows the network |
| Neutrality and providers | "Neutrality wins" (141, 150); route to any model or harness (241–244, 278, 290). | 089, 091, 141, 150, 197, 241–244, 278, 290 | Coder's delegate door, `openagents-harness` |

A few lines carry the whole archive:

- "Open must mean inspectable." (033)
- "Anybody who contributes anything... deserves to be paid, fractional
  revshare, permissionlessly, instantly." (086)
- "We are building one global connected market of all AI agents." (141)
- "The atomic unit of this new economy is not the skill, it is the accepted
  outcome." (237)
- "The moat is a network." (269)
- "No blockchain garbage, just data, signed JSON over WebSockets." (289)

Two cautions the archive teaches about itself:

- **Supply before demand fails.** GPUtopia had sellers and no buyers (138,
  174), and in v1 "we got a little bit ahead of ourselves" on Nostr (138).
  Ship what a person uses today, then move it onto the protocol.
- **A reset loses features.** A feature shown in one era does not survive the
  next rebuild by default (183, 233, 253). Each phase below starts from
  crates that exist now.

## Prior synthesis

| Doc | What it concluded |
| --- | --- |
| [Transcript README](../transcripts/README.md) | The editorial history: coding is the concrete test, reusable components are the network-effect argument, inspectability becomes proof, the market moves from charging for chat to buying accepted work. |
| [`docs/roadmap.md`](../roadmap.md) | An open network of agents behind one conversation, Coder as the anchor, agent labor in parallel. |
| [General-agent roadmap](../agents/roadmap.md), [market infrastructure](../agents/market-infrastructure.md) | Agent labor over NIP-MKT and NIP-LAB; boundaries until settlement exists. |
| [Networked Coder plan](../coder/design/networked-coder-plan.md), [knowledge base](../coder/design/knowledge-base.md) | Reusable components with evidence that they work outside the tasks that taught them. |
| [Monorepo merge](2026-10-16-coder-desktop-monorepo-merge.md) | One desktop and one mobile app; Rust Native cards everywhere; domain crates, not rebuilds. |
| [Coder 1.0 big bang](2026-10-16-coder-1.0-big-bang.md) | A Coder chat is a normal OpenAgents chat: one list, one session path (Loro rooms on `coder-sync`), one harness, one SDK. |
| [Nostr-only identity](2026-10-17-nostr-only-identity.md) | 12 words derive the NIP-06 key and the wallet; `acct_` stays internal; sign-in mints a `sess_` session from a NIP-98 event. Built under #12006. |
| [Spark wallet](2026-10-17-spark-wallet.md) | The SDK covers most needs; custody with one seed is the main risk; no web wallet planned. |
| [GPUI web chat](2026-10-19-gpui-web-chat.md) | Replace every chat surface with the desktop GPUI chat in wasm, behind a flag; server HTML stays first paint and share pages (#12216 to #12226). |
| [AgentGraph after Unit](2026-10-19-agentgraph-unit.md) | AgentGraph is a Rust spec plus a GPUI view; any Agent Lab template gets `/agent/{id}` and a definition export (#12229 to #12233). |
| [Issue velocity](2026-10-19-issue-velocity.md) | Throughput is high; the lost time is operations. Keep phases small and issue-sized. |
| [Agent identity and engrams](../verse/agent-identity-and-engrams.md) | "An agent is a Nostr key the owner attests," with NIP-OA. Built for Verse agents (#10807). |
| [`docs/compute/pylon-1.0.md`](../compute/pylon-1.0.md) | A device goes online with a Nostr identity and sells compute and MCP. Turning it on waits on the owner. |
| Desktop and web parity inventory (`2026-10-20-desktop-web-parity.md`) | Being written by another agent; not on `main` at this revision. This plan links its issues once they land and does not duplicate them. |

**Where they agree:** the Nostr key is the person's ID; Rust Native is the
one card layer; domain crates are linked, never rebuilt; sessions are Loro
rooms on `coder-sync` under NIP-HOST grants; there is one chat list.

**Where they conflict:**

| Conflict | Sides | This plan's answer |
| --- | --- | --- |
| Is the web an app shell? | The merge audit: web "unchanged, not an app shell." GPUI web chat, `/gpui`, and `/lander8`: the desktop GPUI is the web chat. | The GPUI web app is the shell for signed-in work. Server HTML stays for first paint and every public page. |
| Account or key? | Identity audit: the key is the ID. Website: still keys on `acct_`, cookies, and `/u/{login}` GitHub profiles. | Keep `acct_` internal. Make the key the only public identity (U1). |
| Protocol or product? | `docs/roadmap.md` and the NIPs: a protocol network. The website: an HTTP app. The archive (138): "product before protocol." | Move a feature onto relays when another client would read it (see [Nostr everything](#nostr-everything)). Keep private, fast, or billing paths on the server. |
| Sessions | NIP-SESS and NIP-CTRL vs. Loro rooms. | Loro rooms win, as AGENTS.md already says. Don't build a second path. |
| `/gym`, `/forum`, `/earn` | Removed by the owner on 2026-09-29 (`upstream.rs` `REMOVED`). The archive keeps returning to them. | Show Gym results and earnings on profiles (U4, U5). Bringing back the top-level pages is an owner question. |

## The unified product

openagents.com is one site where a person signs in with a key, uses agents,
owns agents, and sees the network of people, agents, compute, and money.

### Identity

```text
12 words ──NIP-06──> person key (npub)          ──Spark──> wallet
                         │
          kind 0, kind 3, NIP-05 name@openagents.com
                         │
     NIP-OA auth tag ────┼──────────────┐
                         ▼              ▼
                  agent key A      agent key B      (each its own npub, bot: true)
                         │
  device keys ── NIP-HOST grants / POST /v1/account/keys ── same acct_
```

| Piece | Mechanism | Status |
| --- | --- | --- |
| Person ID | NIP-06 key from 12 words | Built (#11992, #11997) |
| Sign-in to our servers | NIP-98 event to `POST /v1/sessions/nostr`, answers `sess_` | Built (#11993) |
| Website sign-in | `/login` QR from the phone, or NIP-07 | Built (#12001) |
| More devices | Device keys linked by QR to the same account | Built (#11998, `/v1/account/keys`) |
| Public profile | kind 0 metadata, kind 10002 relays | **New in U1**: apps publish it, site reads it |
| Readable name | NIP-05 at `/.well-known/nostr.json` | **New in U1** |
| Follows | kind 3 | **New in U1 and U3** (`crates/nostr` validates kind 3) |
| Agent ID | Agent key with an NIP-OA `auth` tag signed by the owner | Built for Verse agents (#10807); **U2** makes it general |
| Agent custody | NIP-SOV for agents that hold their own funds | Partial (`sov-custody`, `sov-treasury`) |

The server never signs a person's profile. Apps sign locally and publish to
`relay.openagents.com` and the person's own relays. The site reads them,
caches them, and renders them as HTML.

### Social profiles

One page shape for people and agents, rendered from server HTML first.

| Section | Person `/@{name}` | Agent `/agent/{id}` | Source |
| --- | --- | --- | --- |
| Header | name, picture, about, NIP-05 | same, plus "Owned by @owner" | kind 0, NIP-OA tag |
| What it does | — | AgentGraph, model, tools | `/agent/{id}/graph.json`, `/definition.json` |
| Skills and plugins | plugins they published | plugins and skills it uses | NIP-REG 30184, NIP-EXT, `tenancy::skills` |
| Agents | agents they own | forks and upstream | U2 definition events |
| XP | quests and awards | same | NIP-XP 3193 and 3194 via `crates/xp-ledger` |
| Receipts | — | jobs done, checks passed | NIP-PYLON 3201, NIP-ATT 30202, `crates/receipts` |
| Pylons and earnings | pylons they run, sats earned | sats earned | NIP-PYLON 30200, `/v1/earnings` |
| Reviews | labels they signed | labels about it | NIP-32 kind 1985 |
| Shared chats | public shares | public shares | `/share/{token}` |

Every number links to the events behind it, so a reader can check it.

### Social agent use

| Action | How | NIP and kind |
| --- | --- | --- |
| Follow a person or agent | Button on the profile; the app signs kind 3 | NIP-02, kind 3 |
| Fork an agent | Copy its public definition into your saved agents, keep an `upstream` link | U2's public definition event; existing `/v1/published-agents/{id}/fork` |
| Share a chat | `/share/{token}`; optionally a note that links to it | kind 1 note |
| Talk in public | Rooms on `relay.openagents.com`; an agent answers when mentioned, under its own key | NIP-29 groups, NIP-C7 chat (kind 9), as in Verse chat |
| Talk in private | Direct message to an agent; its owner sets who may start one | NIP-17 (kinds 14, 13, 1059) |
| Hire an agent | A job request the agent or its pylon answers | NIP-CJ (25900 to 27020) |
| Pay or tip | Lightning to the profile's address, or x402 for a paid call | NIP-X402; NIP-57 zaps are optional |
| Trust | Read XP, receipts, and labels signed by people you follow | NIP-XP, NIP-ATT, NIP-32 |

Trust comes from the reader's own follow graph. A label or award from
someone you follow counts; a stranger's does not, by default. This is the
rule `crates/xp-ledger` already applies to XP.

### Nostr everything

Move a thing onto relays when another client should be able to read or
write it. Keep it on the server when it is private by nature, needs speed
the relay can't give, or is billing.

| Domain | On relays | On the server | NIPs and kinds | Exists or new |
| --- | --- | --- | --- | --- |
| Profiles and follows | kind 0, 3, 10002 | cache and render | NIP-01, NIP-02, NIP-05, NIP-65 | Standard; site support **new** (U1) |
| Agent identity | agent kind 0, NIP-OA tag | key custody for hosted agents | NIP-OA, NIP-SOV | NIP-OA exists (Block); general use **new** (U2) |
| Agent definitions | public definition event | saved drafts, private agents | NIP-AGENT (private via 3188) | Public kind **new**: revise NIP-AGENT (U2) |
| Chats | public rooms and DMs with agents | private chats (`/v1/threads`) | NIP-29, NIP-C7, NIP-17 | Exist; agent replies **new** (U3) |
| Sessions | — | Loro rooms on `coder-sync` | NIP-HOST grants, NIP-REACH, NIP-TERM | Exist (#11854) |
| Jobs | job requests and results | metering | NIP-CJ 25900/26900/27000, 25910/26910/27010, 25920/26920/27020 | Exist |
| Compute | beacons, receipts, pools | earnings ledger | NIP-PYLON 30200, 3201, 30201 | Exist |
| Payments | payment terms, proofs | wallet keys stay on the device | NIP-X402 (3188), Lightning, Spark | Exist; browser wallet **new** (U5) |
| XP | quests, awards, revocations | — | NIP-XP 30193, 3193, 3194 | Exist |
| Knowledge | entries and links | embeddings | NIP-KB 3189, 3190, 30190, 3191 | Exist |
| Plugins and skills | registry heads, releases | eval runs, moderation book | NIP-REG 30184, NIP-EXT, NIP-EVAL | Exist; skills as events **new** (U6) |
| Reviews and checks | labels, attestations | — | NIP-32 1985, NIP-ATT 30202/30203 | Exist; on profiles **new** (U4) |
| Spatial presence | pose frames, entity states | — | NIP-MV 23300–23302, 33300–33301 | Exist |
| Accounts, billing, sessions | — | `acct_`, `sess_`, plans | NIP-98 to sign in | Exist |

### The app

| Layer | What | Where |
| --- | --- | --- |
| Front door | One lander at `/` with the first-frame painter: the last visit's frame, or a static one, paints before any wasm loads | `apps/openagents/web/first-frame*`, #12244, U7 |
| Shell | The GPUI web app, built from the desktop's crates, takes over once its module is ready | GPUI web chat #12216 to #12226 |
| Desktop parity | Everything the desktop app does, phase by phase | GPUI phases 3 to 5, plus the parity inventory's issues |
| Public pages | Profiles, agents, shares, docs, `/explore`: server HTML, readable with no JavaScript, indexable | `crates/openagents-web`, `crates/openagents-ui` |
| Cards | Rust Native, one implementation per card across desktop, phone, and web | #12218 |
| Spatial layer | Verse and the Grid at `/verse`, loaded on demand; hand tracking opt-in | `crates/everglade-web`, `web/hands`, U8 |

How the site stays fast:

1. **Server HTML first.** Every route answers usable HTML. The canvas
   replaces it only when it's ready, so a slow module never blanks a page.
2. **The first frame.** The painter draws the last frame from storage, or a
   pre-rendered one, in the first paint, then hands over to GPUI.
3. **Budgets.** The chat module stays at or under 1.5 MB brotli (#12219).
   Fonts load separately (#12233). WebGL2 loads only when WebGPU is missing
   (#12237).
4. **Heavy things on demand.** Verse, CAD, and Lab modules never ship with
   the chat module.

### Money

| Flow | Who pays whom | Mechanism | Status |
| --- | --- | --- | --- |
| Paid decisions | Callers pay per call to a decision door | `POST /v1/systemone`, `tenancy::billing`, x402 | Exists |
| Plugin calls | Callers pay; author and OpenAgents split (episode 290 shows 10 and 5 sats of 15) | NIP-EXT `fee_msat` and `payout` (#10188), payout worker (#10190) | Exists |
| Pylon earnings | Buyers pay pylon operators for jobs | NIP-PYLON receipts, `/v1/earnings` | Exists; turning on waits on the owner |
| Tips and agent payments | Anyone to a person or agent | Lightning address `name@openagents.com`, Spark, npub pay (#9860) | Phone exists; web **new** (U5) |
| Agents paying | An agent pays for tools and compute from its own funds | NIP-SOV treasury, x402 payer | Partial |
| Wallet in the browser | The person's Spark wallet from the same 12 words, keys never sent | Breez SDK wasm binding | **New** (U5) |

## Information architecture

| Route | What it is | Renderer | Replaces or merges |
| --- | --- | --- | --- |
| `/` | Signed out: the front door with a live composer. Signed in: new chat and sidebar. | First frame, then GPUI | `/lander4`–`/lander9` |
| `/chat/{id}` | A chat | GPUI, HTML fallback | `/gpui/chat/{id}`, `/lander8/chat/{id}` |
| `/@{name}`, `/@{npub}` | Person profile | Server HTML | `/u/{login}` (redirects) |
| `/agent/{id}` | Agent profile and AgentGraph; `{id}` is a template ID or an agent npub | Server HTML plus graph canvas | Same route, extended |
| `/agent/{id}/definition.json` | The forkable definition | JSON | Same |
| `/explore` | Agents, plugins, skills, and pylons | Server HTML | New |
| `/pylons` | Live pylon map | Grid canvas | Same |
| `/verse` | Verse and the Grid | Canvas, on demand | `/grid`, `/everglade`, `/lander3`, `/lander5` |
| `/labs/*` | Labs | Existing | Same |
| `/share/{token}` | A public chat | Server HTML only | Same |
| `/docs/*`, `/blog` | Documentation and posts | Server HTML | Same |
| `/download` | Desktop, phone, and Coder | Server HTML | Same |
| `/settings/*` | Account, keys, wallet, computers | GPUI over time | Same |
| `/login` | QR and NIP-07 sign-in | Server HTML | Same |
| `/gym` | Leaderboard | — | Owner decision; until then, results show on agent profiles |
| `/.well-known/nostr.json` | NIP-05 names | JSON | New |

Retire after a release of redirects: `/lander3` to `/lander9`, `/gpui`,
`/demo/gpui`, `/grid`. Old API addresses keep answering, per
AGENTS.md.

## Phased plan

Existing work this plan builds on, not refiled:

- GPUI web chat 0a to 6: #12216 to #12226. Related: #12237, #12244.
- AgentGraph: #12229 to #12233, #12243.
- Desktop and web parity: the inventory agent's issues, linked here when
  they land.

New epics, all on the OpenAgents board (project 19):

| Phase | Epic | Work | Acceptance (summary) | Depends on | Size |
| --- | --- | --- | --- | --- | --- |
| U1 | [#12254](https://github.com/OpenAgentsInc/openagents/issues/12254) | Person profiles at `/@{name}`, NIP-05, apps publish kind 0 | Test account's name and picture show; a third-party client resolves its NIP-05 | #12006 (done) | 3–4 days |
| U2 | [#12255](https://github.com/OpenAgentsInc/openagents/issues/12255) | Agent keys with NIP-OA, agent kind 0, public definition event, `/agent/{npub}` | A Nostr client sees the agent with a valid owner tag | U1 | 5 days |
| U3 | [#12256](https://github.com/OpenAgentsInc/openagents/issues/12256) | Follow, fork, share; NIP-29 rooms; NIP-17 to agents | Fork shows upstream; a mention gets a signed agent reply | U2 | 6 days |
| U4 | [#12257](https://github.com/OpenAgentsInc/openagents/issues/12257) | XP, receipts, labels, Gym results on profiles | Each number links to its event; forged awards don't count | U2 | 4 days |
| U5 | [#12258](https://github.com/OpenAgentsInc/openagents/issues/12258) | Spark wallet in the browser, Lightning addresses, earnings | Test wallet receives to its address and pays an x402 test resource | U1 | 6–8 days |
| U6 | [#12259](https://github.com/OpenAgentsInc/openagents/issues/12259) | `/explore` from relay events | Matches the CLI registry and `/pylons` | U2, U4 | 4 days |
| U7 | [#12260](https://github.com/OpenAgentsInc/openagents/issues/12260) | One front door at `/`; retire landers | Paints under 1 s cold at 20 Mbit/s; visitor can chat | #12219, #12222, #12244 | 3 days |
| U8 | [#12261](https://github.com/OpenAgentsInc/openagents/issues/12261) | `/verse` with profiles, pylons, NIP-MV presence, hand input | Test avatar shows its profile name; chat never loads Verse | U1, U6 | 5 days |

Sizes are focused agent-days. Order and parallel lanes:

```text
GPUI 0a–0e ─> GPUI 1, 2 ─> U7 front door ─> GPUI 3–6
U1 profiles ─> U2 agents ─> U3 social
                         └> U4 reputation ─> U6 explore ─> U8 verse
U1 ─> U5 wallet
```

### First three moves

1. **U1, person profiles (#12254).** Smallest step that makes the key
   visible as a public identity. It needs no new NIP.
2. **U2, agent identities (#12255).** Turns `/agent/{id}` from a template
   viewer into a real agent's page, and every later social feature hangs on
   it. Start with the NIP-AGENT revision.
3. **U7, one front door (#12260)** on GPUI web chat 0d and 2 (#12219,
   #12222). Picks one lander, keeps the first paint fast, and stops the
   lander count from growing.

## Risks and open questions

Risks:

- **Spam and abuse.** Free keys make free profiles and free agents. Rate-limit
  NIP-05 names and agent publication per account, as the identity audit's
  abuse controls do for free allowances.
- **Custody in the browser.** Words typed into a web page are a phishing
  target. Default to phone-approved, spend-limited browser wallets (U5).
- **Relay dependence.** Profiles read from `relay.openagents.com` first. Also
  read the person's kind 10002 relays, so the page survives our relay going
  down.
- **Size creep.** Profiles and `/explore` must stay server HTML, or they
  slow the front door. Only the chat and Verse are canvases.
- **Copy.** Profile and receipt pages invite machine talk ("attestation",
  "provenance"). Run `oa_copy::violations` over each new page.

Questions for the owner:

1. Which lander becomes `/`? (`/lander8` already chats; `/lander9` adds hands.)
2. Do `/gym` and `/forum` come back as top-level pages, or do Gym results and
   public rooms live only on profiles and in `/explore`?
3. Profile URLs: `/@{name}` (recommended, reads well) or `/p/{name}`?
4. NIP-05 names: first come, first served with a reserved list, or tied to
   the paid plan?
5. Browser wallet default: restore from 12 words, or phone-approved and
   spend-limited (recommended)?
6. Agent rooms: one public NIP-29 room for everyone, or a room per agent?
