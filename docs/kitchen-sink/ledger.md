# Kitchen Sink promise ledger

Draft, 2026-10-09, for [#11125](https://github.com/OpenAgentsInc/openagents/issues/11125).
This is the full list behind the [Kitchen Sink spec](README.md). The
promises registry ([#11122](https://github.com/OpenAgentsInc/openagents/issues/11122))
is meant to be seeded from it.

Each row is one promise from the [video archive](../transcripts/README.md),
written as what OpenAgents does for a person. Rows marked "X only" come from the
account's public posts ([second pass](twitter.md)); [§T](#t-references-from-the-x-archive)
links each row to the posts that made it. Duplicates across eras are
merged, and the episode list shows where each one was made.

## Status words

| Status | Meaning | Registry word (#11122) |
| --- | --- | --- |
| **Live** | In production today. The evidence column names the proof. | shipped |
| **Launching** | Built on `main` and on staging. Ships with the Monday 2026-10-12 web + terminal release if the smoke test passes ([#11102](https://github.com/OpenAgentsInc/openagents/issues/11102)). | in launch |
| **Partial** | Some of it is built, but not in front of users yet, or only part of the promise holds. | next |
| **Missing** | Nothing a person can use yet. | later |
| **Dropped** | Left out on purpose. The reason is given. | (not listed, or listed as withdrawn) |

Surfaces: **W** website, **T** terminal (`coder`), **M** mobile, **D** desktop,
**V** the Verse, **N** network (open protocol or API, any client).

The evidence column is current as of `4e14c405e7` (2026-10-09). It was not
re-tested for this draft. A row moves to **Live** only once the release smoke
test passes on production.

## A. Account and identity

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | You have one OpenAgents account, signed in with GitHub, that works on every surface. | W T M D | Launching | GitHub sign-in on staging; `docs/auth/README.md`; production account service still being stood up | 116, 274, 277, 289 | #11094, #11039 |
| A2 | You sign the terminal or desktop in by approving a short code on the website. | T D | Launching | `coder login` uses device codes (`crates/openagents-login`); `/device` | 277 | #11045 |
| A3 | You can try a chat before you make an account, and those chats move to your account when you sign in. | W | Launching | Guest chats are claimed at sign-in (release notes) | 008, 180, 228 | #11039 |
| A4 | You see and manage every computer signed in to your account. | W | Launching | Settings > Computers | 281 | #11045 |
| A5 | Your username and public profile match your GitHub name. | W | Partial | `/u/{login}` exists; no public-profile promise yet | 274 | — |
| A6 | One recovery phrase gives you both your identity on the open network and your Bitcoin wallet. | M D | Partial | Phone has identity keys and the Spark wallet; web sign-in by key is not offered | 082, 143, 144, 207, 208 | — |
| A7 | Your agents get their own identity and keys. | N V | Partial | Designed for the Verse's Alice (`docs/verse/agent-identity-and-engrams.md`); not in the product | 199, 209, 235, 275 | — |
| A8 | You can export all your data and leave whenever you want. | all | Partial | `coder /export`, Delete all chats on the web; no whole-account export | 129, 227 | — |

## B. Chat with one general agent

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| B1 | You talk in one conversation, and the network brings in the right agent, model, or specialist for what you ask, so you don't pick between bots. | all | Partial | Web chat is live; routing to abilities through Jev is partly built | 086, 087, 100, 211, 269, 289 | #11106 |
| B2 | You can pick from many top models and switch partway through a chat. | W T D | Partial | Composer pickers (#11097 done); model list depends on the gateway | 067, 084, 089, 118, 150, 256 | #11097 |
| B3 | If one model provider fails or refuses, the agent keeps working on another. | all | Partial | Gateway `openagents/auto` router; fallback isn't shown to users | 118, 265 | #11079 |
| B4 | Answers stream in, and a slow start shows that it's still working instead of failing. | W T | Launching | #11112, #11087 done | 080, 081 | #11087, #11112 |
| B5 | You can pin, rename, archive, search, and delete chats. | W | Launching | #11036, #11038 | 089 | #11036, #11038 |
| B6 | Deleting means deleted, including "Delete all chats." | W T | Launching | Settings > Delete all chats; deletes sync both ways | 227 | #11038, #11046 |
| B7 | You group chats into projects tied to the GitHub repos you choose. | W | Launching | `/projects`, GitHub App repo picker | 137, 228 | #11034, #11056 |
| B8 | You can paste a link or ask a question that needs the web, and the agent reads the page. | W T | Partial | Gateway has hosted web search; not promised in launch copy | 067, 069, 073, 076, 044 | — |
| B9 | You can send images and documents, and answers cite where they came from. | W M | Missing | No file attach in web chat; image input exists in the gateway only | 008, 011, 019, 083, 090 | — |
| B10 | The agent remembers lasting facts about you, and you can see, edit, and delete them. | all | Missing | `crates/memory-stream`, `crates/knowledge` exist; no user memory | 005, 020, 113, 270, 274 | — |
| B11 | Once you allow it, the agent learns how you like to work from your past chats. | all | Missing | — | 286 | — |
| B12 | You can talk to the agent and hear it answer. | M D W | Missing | Voice left with Sarah and Onyx | 111, 151, 152, 251, 270 | — |
| B13 | Answers can include live pieces such as slides, tables, and buttons, not only text. | W D | Partial | Desktop has slides; typed components are planned | 289 | #11113, #11114 |
| B14 | It is free to start, and a $20 Pro plan unlocks stronger models and more capacity. | W T | Partial | Stripe Pro is built (`29032775fb`); kept out of launch copy | 119, 164, 242, 277, 280 | — |
| B15 | You can share a team workspace and its chats with teammates. | W | Missing | — | 136, 137 | — |
| B16 | You never see ads in your chats, and our websites use no tracking cookies. | all | Live | No ads; the website counts its own use with no cookies, third-party scripts, or IP addresses (privacy policy section 5; `analytics::tests::a_page_view_counts_its_template_referrer_and_device_and_sets_no_cookie`, `no_row_ever_holds_an_address_an_id_or_text_from_the_request`) | X only | #11153 |

## C. Coding with Coder

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | You install the terminal app with one command and type `coder`. | T | Launching | `curl … /cli/install.sh`; Coder 1.0.0-rc.5; stable publishes 10-12 | 218, 275, 277 | #11091 |
| C2 | You see every command and step it runs, including its helper agents and its reasoning. | T D W | Launching | Coder transcript and trace views | 033, 157, 249, 254, 277, 279 | — |
| C3 | It does what you ask without stopping for permission; you stay in charge. | T | Launching | Coder default behavior | 272, 275, 277, 280 | — |
| C4 | You can queue and edit your next messages while it works. | T D | Launching | Commits landed; issue still open | 254, 255 | #11121 |
| C5 | It shows how far along a task is. | T | Launching | Jev's % done | 284 | #11115 |
| C6 | Hand it a GitHub issue and get back a tested pull request. | T W | Partial | Works in Coder sessions, but there's no one-click issue-to-PR flow on the web | 020, 025, 030, 123, 161, 228, 287 | — |
| C7 | It hands parts of a job to Codex, Claude, Devin, or other agents in one conversation and combines the results. | T D | Partial | ACP delegation replaced the sample plugins (#11096); Devin runbook | 195, 198, 264, 267, 278 | #11096 |
| C8 | Give it a list of work and walk away; it keeps going, even overnight. | T D | Partial | Coder sessions and the scheduler crate; no "walk away" launch flow | 031, 167, 195, 199, 228, 250, 281 | — |
| C9 | It looks around cheaply before calling an expensive model, so the same work costs less. | T | Partial | Jev in Coder; Coder One prototype folded in; measured on 8 Terminal-Bench tasks only | 285, 286, 287 | — |
| C10 | It picks the cheapest model that still gets each task done. | T N | Partial | `openagents/auto` router on the gateway | 219, 287 | #11079 |
| C11 | You can bring your own keys, subscriptions, models, and computers. | T W | Partial | Saved Claude key (encrypted), `/models`, gateway pay-with-mine | 219, 244, 277 | #11111 |
| C12 | When one of your accounts hits its limit, it moves to the next. | T D | Partial | Coder engine adapters; not a stated launch behavior | 225, 246, 250 | — |
| C13 | It understands your whole repository without you picking files. | T | Partial | Project map / Code finder measured in hosted evals (2 → 5 of 6 tests) | 023, 107, 122, 156, 160 | — |
| C14 | You run Coder on a computer you've connected, from your phone or the web. | M W | Partial | Run Coder on a computer (iOS beta); reply from the web while Coder is open | 144, 145, 193, 281 | #11093 |
| C15 | You can rent cloud computers for your agents and pay by the hour. | W T | Partial | Environments under Pro; shown only on a local address | 106, 276, 281 | #11101 |
| C16 | Your agent can drive a computer made for agents (browser, screen, phone emulator). | D T | Partial | CoderOS workspace | 270, 280, 281, 283 | #11124 |
| C17 | Each agent gets its own workspace, with shared build caches, so many agents don't overload your machine. | T D | Partial | Worktree and lease crates; not a user-facing setting | 188, 281 | — |
| C18 | Another model reviews the work and writes tests in the background. | T | Missing | — | 029, 286 | — |
| C19 | You see every change as a diff before you accept it. | T D W | Partial | Coder shows edits; no review screen on the web | 117, 119 | — |
| C20 | You see how much CPU and memory each helper agent is using. | T D | Partial | Coder Terminal measured it per helper (posted 2026-09-04); not checked in 1.0 | X only | — |
| C21 | Coder adds no refusals of its own; when a model provider refuses, it tells you which one and offers another model. | T W | Partial | Coder acts without asking (C3); refusal reporting not built | X only | #11132 |

## D. Across your devices

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| D1 | Your terminal chats show on the website with live status, and you can reply there while Coder is open. | T W | Launching | #11046–#11048; `/coder/sync` | 187, 275, 279 | #11089 |
| D2 | Sync is off until you turn it on, per computer. | T | Launching | `/sync on\|all\|off\|delete` | 279, 281 | #11046 |
| D3 | Keys and passwords are caught before a message leaves your computer, and again on our side. | T W | Launching | `crates/secret-screen` | 245, 272 | #11046 |
| D4 | Your phone shows the same chats as your account. | M | Partial | Phone has its own and paired-computer chats | 187, 193, 275, 281 | #11107 |
| D5 | You pair your phone with your computer by scanning a code. | M D T | Partial | QR pairing on desktop and phone; iOS in review | 193, 194 | #11093 |
| D6 | You get a phone notification when an agent needs you. | M | Partial | `crates/push-gateway` (APNs/FCM) built | 160, 183, 190 | — |
| D7 | Your progress, levels, and rewards are the same in every app. | all | Missing | XP is hidden on the phone | 284 | — |

## E. Models and the OpenAgents API

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| E1 | One OpenAI-compatible API key reaches many models, and any coding tool can point at it. | N T | Partial | Gateway built, 17/17 acceptance on staging; production not deployed | 241, 242, 243, 289 | #11094 |
| E2 | The API has a free tier with real limits. | N | Partial | Built on the gateway | 119, 242 | — |
| E3 | Agents can pay per request without an account. | N | Partial | x402 on Lightning built on the gateway (`c383d9fcd1`), not live; the `Payment` scheme (MPP) on the same invoice is next | 062, 063, 070, 086 | #11077, #11136 |
| E4 | You set your own spending limits. | N W | Partial | User-set limits on the gateway | 147, 206 | #11077 |
| E5 | You say what you care about (cost, speed, privacy) and the agent picks models to match. | all | Partial | Auto router; no preference setting | 269, 286 | #11079 |
| E6 | We ask model providers not to train on or keep your chats, and usage records are deleted after 30 days. | W T | Launching | Privacy policy updated 10-09 | 046, 245 | #11040–#11044 |
| E7 | A public counter shows tokens served and which models served them. | W | Missing | — | 243, 244 | #11081 |
| E8 | The app never claims a model it didn't use; you get the model you picked or a visible failure. | all | Partial | Effective-model reporting in Coder; not checked across surfaces | 250 | — |
| E9 | You can run open models on your own machine or a friend's. | T D | Partial | Local Psionic upstream (P2); Pylon free text jobs | 145, 194, 219 | #11080 |
| E10 | You can pay more for private processing of sensitive work. | N | Missing | — | 242 | — |

## F. Plugins and new abilities

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| F1 | When the agent can't do something, it builds the ability with you: it asks what it should do, writes tests, runs them, and you approve. | all | Partial | Extension evals from chat (`docs/extensions/evaluation.md`, roadmap M10) | 211, 289 | — |
| F2 | An approved ability goes into a shared registry, so every user's agent gets it. | N | Missing | Sample plugins removed (#11096); no public registry | 049, 053, 087, 211, 269, 289 | #11096 |
| F3 | Plugin authors are paid in Bitcoin every time someone pays to use their plugin. | N | Missing | Paid-plugin flow designed in `docs/payments` | 054, 085, 097, 102, 165, 289 | — |
| F4 | The agent finds the tools it needs itself, ranked by proven results. | all | Partial | Measured tools in hosted evals; no ranked registry | 087, 102, 285, 286 | — |
| F5 | Plugins run sandboxed, with only the access you grant. | all | Partial | Packet-ABI host (`crates/plugin`) | 048, 102, 214 | — |
| F6 | Developers write plugins in many languages. | N | Partial | `crates/plugin-pdk` | 048, 053, 068 | — |
| F7 | You connect outside tools through MCP. | T D | Partial | Docs MCP server live in code (#11086); OAuth for MCP pending | 144, 165, 168, 195 | #11084 |

## G. Gym, evals, and proof

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| G1 | Gym measures agents and plugins, and anyone can inspect the results. | W T | Partial | `docs/gym.md`, hosted extension evals; not a public page | 065, 287, 289 | — |
| G2 | We publish benchmark results with the setup, costs, and failures, not only scores. | W | Partial | `docs/terminal-bench/README.md` | 120, 121, 186, 287 | — |
| G3 | You earn experience, levels, and leaderboard places for verified work, never for wasted spending. | all V | Partial | Gym/XP built, hidden on the phone | 185, 270, 284 | — |
| G4 | Leveling up unlocks free credits, better models, and hosting. | all | Missing | — | 284 | — |
| G5 | Improvements to how the agent works are published with their evidence, so everyone's agent gets better. | N | Partial | Measurement records in `docs/extensions/measurements/` | 270, 286, 287 | — |
| G6 | A public list says which of our claims work today, with proof, and what's coming. | W | Partial | Registry and pages in progress | 234, 237, 246, 248 | #11122 |
| G7 | You can report something that doesn't work, and it becomes a tracked fix. | M W T | Partial | Report a problem on the phone (NIP-17) | 234, 246, 254 | — |
| G8 | We publish head-to-head results against the leading coding agents and local runtimes (cost, pass rate, time, speed), with scripts anyone can re-run. | W | Partial | `docs/terminal-bench/README.md` (8 tasks); no public page | 120, 287 | #11131, #11129 |
| G9 | We publish accepted outcomes per dollar, and later per kilowatt-hour, as our main measure of work. | W | Missing | — | X only | — |

## H. Traces and your data

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| H1 | Every run is recorded on your computer, and you can open any step. | T | Live | Local ATIF traces (`docs/coder/runtime/traces.md`) | 033, 034, 061, 080 | — |
| H2 | You can upload a trace to your account; it stays private unless you share it. | T W | Launching | `/trace`, `/traces`, `/settings/traces` | 228, 275 | #11109 |
| H3 | You can sell scrubbed traces and data, and your agent asks before each sale. | N | Missing | — | 200, 213, 215, 230, 245, 275 | — |
| H4 | Free use in exchange for sharing scrubbed traces, with pay when they're reused. | W T | Missing | Logged as planned in the old registry | 244, 245 | — |
| H5 | You choose how much of a project is public: nothing, activity, summaries, or everything. | W | Missing | — | 272 | — |
| H6 | A plain-words changelog links each change to the work behind it. | W | Partial | Release notes drafted per release | 272 | #11104 |

## I. Wallet and payments

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| I1 | You get a Bitcoin wallet where only you hold the keys, for small amounts. | M D | Partial | Spark wallet on the phone (beta in review); out of launch copy | 143, 153, 169, 173, 207, 212 | #11093 |
| I2 | You can send sats just by asking. | all | Partial | Shown in 289; not in launch | 212, 289 | — |
| I3 | Your agent can pay and get paid, under rules and budgets you approve. | M N | Partial | Agent payment approvals on the phone; budgets and approvals designed in PR #11088 (SOV/CAP/POL) | 042, 047, 141, 147, 212 | #11146 |
| I4 | You can pay by card. | W N | Partial | Stripe Pro subscription built; agents by card through MPP `stripe`, ACP, and UCP are planned | 037, 064, 116 | #11142, #11143, #11144 |
| I5 | Earnings go to your Lightning address automatically. | N | Partial | Splits and payouts built on the pay host; owner-only so far | 064, 092, 096, 098 | — |
| I6 | The revenue split between authors, providers, and OpenAgents is published. | W | Missing | No current split | 037, 097, 098 | — |
| I7 | Bring someone in and earn a share of what they pay, for as long as they pay. | N | Missing | Red in the old registry | 037, 125, 150, 153, 229, 239 | — |
| I8 | There is no OpenAgents token. Bitcoin is our own money. | all | Live | Policy across every payment path. Revised 2026-10-09 (owner: "pay in any different way"): agents may also pay by card and dollar stablecoins, which settle to dollars; we never issue a token. Was "Bitcoin is the only money." | 001, 200, 220, 230 | #11141 |
| I9 | You can hold dollar-pegged balances backed by Bitcoin. | N | Missing | Taproot Assets work lives in `tap-ldk` | 096, 173, 200 | — |
| I10 | Your agents can pay any service that asks for payment (x402, MPP, L402, Cashu), from your wallet and within your budget. | T M N | Missing | Designed in [agent payments](../payments/agent-payments.md); `openagents inference --pay x402` pays our own API | — | #11146 |
| I11 | Every payment, any method, gets the same receipt, and shows in your usage and on `/stats`. | W N | Missing | Receipt model in [agent payments](../payments/agent-payments.md) §3 | — | #11138 |

## J. Markets, compute, and earning

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| J1 | You can sell your computer's spare power for Bitcoin with one "Go Online" switch. | D T N | Partial | Pylon runs free text jobs and feeds the gateway; paid earning is paused | 144, 174, 178, 201, 214, 221 | #11080 |
| J2 | Providers are paid for accepted work, not for being online. | N | Partial | Principle in the gateway plan | 224, 232, 237 | #11082 |
| J3 | Anyone can join the markets by following open specs, without our permission. | N | Partial | NIPs written; relay running | 203, 215, 266, 267, 288 | — |
| J4 | Anyone can run a relay, and ours works with other Nostr apps. | N | Live | `openagents-nostr-relay` carries the chat worker's jobs | 066, 177, 203, 266 | — |
| J5 | Your idle agent can take paid jobs from other people and agents. | N | Missing | — | 213, 215, 230, 246 | — |
| J6 | A public stats page shows the network's size, work done, and money paid. | W | Partial | `/stats` | 203, 221, 224, 227 | #11081 |
| J7 | Paid bounties fund outside contributors. | W | Missing | No bounty page | 001, 043, 088, 216 | — |
| J8 | Fan one job out to many machines at once. | N | Missing | — | 202, 203, 214 | — |
| J9 | Sellers on the network, starting with compute providers, are paid straight to their own wallet; OpenAgents never holds their money. | N | Missing | Non-custodial merchant profile merged as a proposal (PR #11088), with BuyerAttestation test vectors | — | #11149 |

## K. Agent-ready and open

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| K1 | Any agent can learn to use OpenAgents from one file on the website. | W N | Launching | `llms.txt`, agent card, skills, `/openapi.json`; blocked on the deploy | 230, 231, 235, 237 | #11083 |
| K2 | Agents can read our docs through MCP. | N | Launching | Docs MCP server | 165, 168 | #11086 |
| K3 | Everything you can do in an app, you can do from the command line or API. | T N | Partial | `openagents-cli` over Nostr; `/docs/api` | 067, 085, 100, 203, 289 | — |
| K4 | All of it is open source and built in public. | all | Live | This repository | 001, 047, 125, 173, 242 | — |
| K5 | You can run your own copy of the whole thing. | N | Partial | Open source, but no self-host guide | 129, 242, 289 | — |
| K6 | Agents can pay us any way they already know: x402, MPP, L402, Cashu, ACP, UCP, AP2, Lightning, Nostr, card, and stablecoins. | N | Partial | Owner, 2026-10-09: support everything. x402 and the `Payment` scheme on Lightning built (gateway, pay front); the rest planned in [agent payments](../payments/agent-payments.md) | 062, 070 | #11085, #11136, #11139–#11145 |
| K7 | One open license: everything we publish is under Apache 2.0. | all | Live | `LICENSE` (Apache 2.0) | X only | — |
| K8 | Agents find every way to pay us where they look: OpenAPI, the API and AI catalogs, the agent card, `/.well-known/ucp`, `/.well-known/acp.json`, MCP, and Nostr. | N | Partial | `/docs/api/for-agents` and `/auth.md` name the methods; generated discovery is next | — | #11137, #11147 |
| K9 | Agents can sign in with their own Nostr key. | N | Missing | NIP-98 checked on the pay host only | — | #11148 |

## L. Surfaces

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| L1 | The website is a full chat app at openagents.com. | W | Live | Production `coder-web-w11` revision; 1.0 build on staging | 002, 076, 164, 289 | #11094 |
| L2 | Terminal 1.0 installs on Mac, Linux, and Windows. | T | Launching | rc.5; stable 10-12 | 277, 289 | #11091 |
| L3 | A desktop app for Mac, Linux, and Windows that updates itself safely. | D | Partial | Built; `DESKTOP_RELEASED=false`; Windows unsigned | 002, 175, 208, 248, 256, 289 | #11092, #11120 |
| L4 | The desktop shows many chats and terminals side by side in panes. | D | Partial | Desktop Grid | 118, 123, 179, 185 | #11124 |
| L5 | Fast, game-like controls with hotkeys that stay put. | D T | Partial | Rebindable keys in Coder; desktop varies | 170, 171, 246, 249, 251 | — |
| L6 | An iPhone app on TestFlight. | M | Partial | Build 54 in Apple's review | 139, 149, 193, 289 | #11093 |
| L7 | An Android app. | M | Partial | Built; deferred past launch | 149, 151, 289 | #11093 |
| L8 | Light and dark themes that follow your system. | W D M | Launching (web) | #11100; desktop and phone ship with their releases | — | #11100 |
| L9 | A crash in one feature never takes down the app or your agents. | D T | Partial | No measured claim | 258 | — |
| L10 | The desktop opens a file in under a second and switches chats in under 50 ms. | D | Partial | Claimed for the July desktop; not in the release smoke test | X only | #11092 |

## M. The Verse

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | A shared 3D world where you meet other people and agents. | V W D M | Partial | Everglade on the web (`/everglade`); hidden on the phone; `docs/verse/status.md` | 116, 177, 189, 240, 284 | — |
| M2 | You watch your agents do real repository work there, and answer them or approve merges. | V | Partial | Agent Studio (`docs/verse/agent-studio.md`) | 116, 240, 284 | — |
| M3 | You own named agents with jobs and a place to work in the world. | V | Partial | Alice workshop agent, crew plan | 270, 284 | — |
| M4 | You can see your network's work and payments moving in the world. | V | Missing | Training board demo (240) is gone | 240, 241, 243, 289 | — |
| M5 | Guilds, quests, and trades tied to real verified work. | V | Partial | Parties, guilds, and trades exist in the game; not tied to work | 116, 200, 284 | — |
| M6 | You command agents the way you command units in a strategy game. | V D | Partial | Grid and Studio controls | 170, 171, 178, 199, 283, 284 | — |
| M7 | A terminal inside the world that you can share. | V | Partial | `docs/verse/in-world-terminal.md` (spec) | 116, 284 | — |

## N. Dropped on purpose

These were promised, then set aside. None of them is in V1. Bring one back only
by a new issue with demand behind it.

| ID | Promise | Episodes | Why it was dropped |
| --- | --- | --- | --- |
| X1 | A drag-and-drop visual builder for agent workflows | 038, 058, 059, 061, 073 | Set aside for chat (076); the agent now writes and tests the ability itself (F1). |
| X2 | A store of many separate agents to browse and rate | 090, 092, 094, 100 | Replaced by one conversation backed by a network of agents and plugins (086, 289). |
| X3 | An open replacement for WordPress, and sites built by asking | 126, 127, 129, 180, 229 | OpenPress and Sites were retired. A site is a coding task for Coder. |
| X4 | A built-in CRM and an all-in-one business suite | 135, 237, 239, 247 | Outside the current product focus; Coder and the general agent come first. |
| X5 | Code hosting to replace GitHub | 270, 272, 273, 274 | "Too big an apple" (281). Back to GitHub. |
| X6 | Paid distributed training on home machines | 216–238 | Paused; not part of the current product. Psionic stays as the local inference engine and for research. |
| X7 | Liquidity and risk markets | 213, 214, 230 | Speculative, with no buyers. |
| X8 | Ocean-powered data centers | 194, 227 | Out of scope. |
| X9 | One-click security scans of open-source projects | 264, 265 | Stood down in 266. |
| X10 | A named voice agent that runs things for you (Onyx, Sarah) | 139, 151, 260, 270 | Replaced by the one OpenAgents agent (289). Voice returns as B12. |
| X11 | Pay-per-message pricing in sats | 097, 118 | Replaced by a free tier, Pro, and credits. |
| X12 | Paying providers per second online | 214–223 | Dropped in 224 for paying for accepted work. |
| X13 | Hand-gesture "Jarvis" control | 176, 179, 183 | Teaser only. |
| X14 | Desktop with no OpenAgents account | 248, 251 | Replaced by one account everywhere (A1). Local-only use of Coder stays. |
| X15 | Small models running on the phone | 145, 149 | Too slow on Android (149). |
| X16 | Public investigation agents (Sleuth, OSINT) and public knowledge graph | 043, 044, 146, 154 | Domain experiments with no follow-up. |

## T. References from the X archive

Posts on [@OpenAgentsInc](https://x.com/OpenAgentsInc) where each promise was made or
boasted, from the [second pass](twitter.md). Rows marked "X only" in the
Episodes column came from the posts alone. Each link is labeled by its date.

| ID | Posts |
| --- | --- |
| A1 | [2025-11-19](https://x.com/OpenAgentsInc/status/1991260906211164529), [2026-03-14](https://x.com/OpenAgentsInc/status/2032635511991341122) |
| A3 | [2024-04-12](https://x.com/OpenAgentsInc/status/1778822995261141143), [2025-06-12](https://x.com/OpenAgentsInc/status/1933145402712195498) |
| A6 | [2024-12-05](https://x.com/OpenAgentsInc/status/1864528026765062439), [2026-01-28](https://x.com/OpenAgentsInc/status/2016423268564001059), [2026-04-04](https://x.com/OpenAgentsInc/status/2040481532750492072) |
| A7 | [2025-12-21](https://x.com/OpenAgentsInc/status/2002589906527531353), [2026-01-30](https://x.com/OpenAgentsInc/status/2017108809748005274), [2026-02-10](https://x.com/OpenAgentsInc/status/2021217246090014922) |
| A8 | [2026-02-11](https://x.com/OpenAgentsInc/status/2021629941272477809) |
| B1 | [2024-03-14](https://x.com/OpenAgentsInc/status/1768298996441759994), [2024-05-11](https://x.com/OpenAgentsInc/status/1789360474087117004), [2026-02-20](https://x.com/OpenAgentsInc/status/2024764526294343793), [2026-10-02](https://x.com/OpenAgentsInc/status/2105903502060859718) |
| B2 | [2024-03-28](https://x.com/OpenAgentsInc/status/1773436254555709783), [2024-04-06](https://x.com/OpenAgentsInc/status/1776665794048409999), [2025-03-18](https://x.com/OpenAgentsInc/status/1901807373980758366) |
| B3 | [2023-10-20](https://x.com/OpenAgentsInc/status/1715371396090507618), [2024-06-04](https://x.com/OpenAgentsInc/status/1798030930764001624), [2024-08-08](https://x.com/OpenAgentsInc/status/1821597874238423077), [2026-07-17](https://x.com/OpenAgentsInc/status/2078190096486895860) |
| B8 | [2024-04-10](https://x.com/OpenAgentsInc/status/1778150350383440316) |
| B9 | [2023-11-17](https://x.com/OpenAgentsInc/status/1725349984952827929), [2024-08-09](https://x.com/OpenAgentsInc/status/1821751383227347101) |
| B10 | [2024-07-25](https://x.com/OpenAgentsInc/status/1816269923414327630), [2024-08-09](https://x.com/OpenAgentsInc/status/1821751383227347101) |
| B12 | [2024-12-24](https://x.com/OpenAgentsInc/status/1871390476705947913), [2025-07-20](https://x.com/OpenAgentsInc/status/1947035291316887925), [2026-07-12](https://x.com/OpenAgentsInc/status/2076389242767368497) |
| B14 | [2024-04-09](https://x.com/OpenAgentsInc/status/1777496991099998302), [2024-12-06](https://x.com/OpenAgentsInc/status/1865112576830693650), [2026-06-04](https://x.com/OpenAgentsInc/status/2062626257443909886) |
| B15 | [2024-10-11](https://x.com/OpenAgentsInc/status/1844783790293422548), [2026-06-04](https://x.com/OpenAgentsInc/status/2062626257443909886) |
| B16 | [2024-05-20](https://x.com/OpenAgentsInc/status/1792360190689386537), [2024-10-21](https://x.com/OpenAgentsInc/status/1848211700450726214), [2024-12-30](https://x.com/OpenAgentsInc/status/1873821809386340369) |
| C1 | [2026-08-27](https://x.com/OpenAgentsInc/status/2092836555756823023) |
| C2 | [2024-01-03](https://x.com/OpenAgentsInc/status/1742609184875544613), [2025-02-14](https://x.com/OpenAgentsInc/status/1890317831784333595), [2026-09-03](https://x.com/OpenAgentsInc/status/2095552147433587011) |
| C3 | [2024-12-13](https://x.com/OpenAgentsInc/status/1867596460553822300), [2026-09-03](https://x.com/OpenAgentsInc/status/2095596118080151936) |
| C5 | [2026-10-02](https://x.com/OpenAgentsInc/status/2106032170699534676) |
| C6 | [2023-11-17](https://x.com/OpenAgentsInc/status/1725597044981617119), [2024-08-27](https://x.com/OpenAgentsInc/status/1828511026410655781), [2025-02-08](https://x.com/OpenAgentsInc/status/1888019235063927061), [2025-03-09](https://x.com/OpenAgentsInc/status/1898610506035782072) |
| C7 | [2025-11-11](https://x.com/OpenAgentsInc/status/1988348979072082230), [2026-07-21](https://x.com/OpenAgentsInc/status/2079678476836122916), [2026-09-05](https://x.com/OpenAgentsInc/status/2096058017133261230) |
| C8 | [2024-08-20](https://x.com/OpenAgentsInc/status/1825938434063741309), [2025-12-21](https://x.com/OpenAgentsInc/status/2002757002863546853), [2025-12-23](https://x.com/OpenAgentsInc/status/2003362087955730508) |
| C9 | [2025-02-24](https://x.com/OpenAgentsInc/status/1894108039503806831), [2026-09-23](https://x.com/OpenAgentsInc/status/2102773483109335209) |
| C10 | [2026-06-19](https://x.com/OpenAgentsInc/status/2068102703092543974), [2026-08-31](https://x.com/OpenAgentsInc/status/2094475625306423657) |
| C11 | [2024-10-11](https://x.com/OpenAgentsInc/status/1844783790293422548) |
| C12 | [2026-05-19](https://x.com/OpenAgentsInc/status/2056603642229039416), [2026-06-13](https://x.com/OpenAgentsInc/status/2065823704861294932), [2026-09-12](https://x.com/OpenAgentsInc/status/2098638228429365425) |
| C13 | [2024-07-25](https://x.com/OpenAgentsInc/status/1816269923414327630), [2024-12-06](https://x.com/OpenAgentsInc/status/1865112576830693650) |
| C14 | [2025-10-24](https://x.com/OpenAgentsInc/status/1981533017999814688), [2025-10-30](https://x.com/OpenAgentsInc/status/1983960929575338281) |
| C15 | [2026-08-31](https://x.com/OpenAgentsInc/status/2094539323374698817) |
| C16 | [2026-09-08](https://x.com/OpenAgentsInc/status/2097173500079346163) |
| C17 | [2026-08-27](https://x.com/OpenAgentsInc/status/2092961098378997790) |
| C18 | [2026-07-03](https://x.com/OpenAgentsInc/status/2072943871005282406), [2026-07-14](https://x.com/OpenAgentsInc/status/2076953270052852166) |
| C20 | [2026-09-04](https://x.com/OpenAgentsInc/status/2095902069374763426), [2026-09-05](https://x.com/OpenAgentsInc/status/2096357223253389345) |
| C21 | [2026-09-03](https://x.com/OpenAgentsInc/status/2095596118080151936), [2026-09-14](https://x.com/OpenAgentsInc/status/2099561282944860387) |
| D1 | [2025-07-26](https://x.com/OpenAgentsInc/status/1948994586459705695), [2026-09-09](https://x.com/OpenAgentsInc/status/2097558311184842942) |
| D4 | [2026-07-28](https://x.com/OpenAgentsInc/status/2082182272199897351), [2026-09-09](https://x.com/OpenAgentsInc/status/2097558311184842942) |
| D5 | [2025-10-30](https://x.com/OpenAgentsInc/status/1983960929575338281) |
| E1 | [2023-10-16](https://x.com/OpenAgentsInc/status/1713990270872641927), [2023-10-29](https://x.com/OpenAgentsInc/status/1718766857388204090), [2023-11-18](https://x.com/OpenAgentsInc/status/1725727843311579231), [2026-06-24](https://x.com/OpenAgentsInc/status/2069922012428914696) |
| E2 | [2024-08-12](https://x.com/OpenAgentsInc/status/1823109640357339628), [2026-08-27](https://x.com/OpenAgentsInc/status/2092836555756823023), [2026-09-03](https://x.com/OpenAgentsInc/status/2095596118080151936) |
| E3 | [2023-11-09](https://x.com/OpenAgentsInc/status/1722650919500759422), [2023-12-21](https://x.com/OpenAgentsInc/status/1737951685270491262), [2026-10-03](https://x.com/OpenAgentsInc/status/2106270735752675797) |
| E5 | [2024-05-12](https://x.com/OpenAgentsInc/status/1789636246387450357), [2026-09-06](https://x.com/OpenAgentsInc/status/2096606065441862041) |
| E7 | [2026-06-25](https://x.com/OpenAgentsInc/status/2070013895683506637), [2026-06-27](https://x.com/OpenAgentsInc/status/2070986088915583010) |
| E9 | [2024-12-14](https://x.com/OpenAgentsInc/status/1867815868131836103), [2025-08-07](https://x.com/OpenAgentsInc/status/1953546661076357463), [2026-03-10](https://x.com/OpenAgentsInc/status/2031255043903549942), [2026-08-28](https://x.com/OpenAgentsInc/status/2093464091952071131) |
| E10 | [2026-06-23](https://x.com/OpenAgentsInc/status/2069540884622786585) |
| F1 | [2026-10-02](https://x.com/OpenAgentsInc/status/2105903502060859718) |
| F2 | [2024-04-21](https://x.com/OpenAgentsInc/status/1782188994094100815), [2026-01-07](https://x.com/OpenAgentsInc/status/2009032724443484301), [2026-09-30](https://x.com/OpenAgentsInc/status/2105372666995868125) |
| F3 | [2024-01-18](https://x.com/OpenAgentsInc/status/1747994309549318228), [2024-04-17](https://x.com/OpenAgentsInc/status/1780642250411679938), [2025-11-11](https://x.com/OpenAgentsInc/status/1988293182942228779), [2026-10-02](https://x.com/OpenAgentsInc/status/2105903502060859718) |
| F4 | [2025-04-03](https://x.com/OpenAgentsInc/status/1907805725797052867), [2026-10-02](https://x.com/OpenAgentsInc/status/2105903502060859718) |
| F5 | [2024-01-12](https://x.com/OpenAgentsInc/status/1745918872866173125) |
| F6 | [2024-06-11](https://x.com/OpenAgentsInc/status/1800665114573521029) |
| F7 | [2025-03-13](https://x.com/OpenAgentsInc/status/1900282953244303440), [2025-03-27](https://x.com/OpenAgentsInc/status/1905107323279855799) |
| G2 | [2024-03-16](https://x.com/OpenAgentsInc/status/1769011829378847194), [2024-08-13](https://x.com/OpenAgentsInc/status/1823455135143518470), [2026-09-23](https://x.com/OpenAgentsInc/status/2102773483109335209) |
| G3 | [2026-09-15](https://x.com/OpenAgentsInc/status/2099943679444357241), [2026-10-02](https://x.com/OpenAgentsInc/status/2105907276749943119) |
| G5 | [2026-09-23](https://x.com/OpenAgentsInc/status/2102773483109335209) |
| G6 | [2026-06-09](https://x.com/OpenAgentsInc/status/2064390975267480060), [2026-06-15](https://x.com/OpenAgentsInc/status/2066601306668810615) |
| G8 | [2026-03-10](https://x.com/OpenAgentsInc/status/2031255043903549942), [2026-03-28](https://x.com/OpenAgentsInc/status/2037717730707542232), [2026-09-23](https://x.com/OpenAgentsInc/status/2102773483109335209), [2026-09-26](https://x.com/OpenAgentsInc/status/2103695213680091618) |
| G9 | [2026-06-08](https://x.com/OpenAgentsInc/status/2064074384881463459), [2026-07-20](https://x.com/OpenAgentsInc/status/2079311647068283131) |
| H2 | [2026-08-06](https://x.com/OpenAgentsInc/status/2085364327427547552) |
| H3 | [2025-12-19](https://x.com/OpenAgentsInc/status/2002057459574452667), [2026-03-07](https://x.com/OpenAgentsInc/status/2030132739672887561), [2026-09-10](https://x.com/OpenAgentsInc/status/2098051068605215146) |
| H5 | [2026-08-19](https://x.com/OpenAgentsInc/status/2090129708520235096) |
| H6 | [2024-04-15](https://x.com/OpenAgentsInc/status/1779907555977769160), [2024-04-19](https://x.com/OpenAgentsInc/status/1781455136675762461) |
| I1 | [2024-05-28](https://x.com/OpenAgentsInc/status/1795535732032831719), [2024-12-12](https://x.com/OpenAgentsInc/status/1867070611928846640), [2025-05-13](https://x.com/OpenAgentsInc/status/1922303008617984363) |
| I2 | [2026-04-14](https://x.com/OpenAgentsInc/status/2044072290380333348) |
| I3 | [2024-01-04](https://x.com/OpenAgentsInc/status/1742952006166225330), [2026-02-16](https://x.com/OpenAgentsInc/status/2023499214995775810), [2026-02-18](https://x.com/OpenAgentsInc/status/2024259092810703136) |
| I4 | [2023-09-26](https://x.com/OpenAgentsInc/status/1706812003258347630), [2023-12-21](https://x.com/OpenAgentsInc/status/1737946716190740950) |
| I5 | [2024-01-29](https://x.com/OpenAgentsInc/status/1752049402359754789), [2024-04-20](https://x.com/OpenAgentsInc/status/1781703101327757410), [2024-06-03](https://x.com/OpenAgentsInc/status/1797738481097077001) |
| I6 | [2023-09-25](https://x.com/OpenAgentsInc/status/1706321126970802366) |
| I7 | [2023-09-25](https://x.com/OpenAgentsInc/status/1706377244883443852), [2026-06-19](https://x.com/OpenAgentsInc/status/2068102703092543974) |
| I8 | [2024-02-18](https://x.com/OpenAgentsInc/status/1759278173148135788), [2025-01-08](https://x.com/OpenAgentsInc/status/1876869977581527550), [2026-06-12](https://x.com/OpenAgentsInc/status/2065535602905448870) |
| I9 | [2025-05-05](https://x.com/OpenAgentsInc/status/1919419077887410389), [2026-01-02](https://x.com/OpenAgentsInc/status/2006956979298685216) |
| J1 | [2023-09-12](https://x.com/OpenAgentsInc/status/1701628445648736626), [2026-03-12](https://x.com/OpenAgentsInc/status/2032108547333304421), [2026-04-08](https://x.com/OpenAgentsInc/status/2041970265471480298) |
| J2 | [2026-06-08](https://x.com/OpenAgentsInc/status/2064074384881463459) |
| J3 | [2024-06-14](https://x.com/OpenAgentsInc/status/1801653533810319867), [2026-03-12](https://x.com/OpenAgentsInc/status/2032108547333304421) |
| J4 | [2024-08-30](https://x.com/OpenAgentsInc/status/1829632437573259433), [2026-08-03](https://x.com/OpenAgentsInc/status/2084392773701063053) |
| J5 | [2026-02-24](https://x.com/OpenAgentsInc/status/2026438188198207601), [2026-04-09](https://x.com/OpenAgentsInc/status/2042041267207458964) |
| J6 | [2026-04-07](https://x.com/OpenAgentsInc/status/2041622770811842943), [2026-04-08](https://x.com/OpenAgentsInc/status/2041992244089999796), [2026-05-13](https://x.com/OpenAgentsInc/status/2054663224612487506) |
| J7 | [2023-10-03](https://x.com/OpenAgentsInc/status/1709300332453371930), [2023-10-19](https://x.com/OpenAgentsInc/status/1714816527227146342), [2026-04-17](https://x.com/OpenAgentsInc/status/2045235776716411062), [2026-06-11](https://x.com/OpenAgentsInc/status/2065092297641824747) |
| J8 | [2023-10-05](https://x.com/OpenAgentsInc/status/1710055873572110558) |
| K1 | [2026-01-30](https://x.com/OpenAgentsInc/status/2017307294585827833), [2026-02-17](https://x.com/OpenAgentsInc/status/2023659999268864077) |
| K3 | [2024-04-17](https://x.com/OpenAgentsInc/status/1780658055820034099), [2026-10-07](https://x.com/OpenAgentsInc/status/2107913192232178136) |
| K4 | [2023-10-05](https://x.com/OpenAgentsInc/status/1709873601380323495), [2023-11-07](https://x.com/OpenAgentsInc/status/1721942435125715086), [2026-01-08](https://x.com/OpenAgentsInc/status/2009142870775644644), [2026-06-15](https://x.com/OpenAgentsInc/status/2066601306668810615) |
| K5 | [2026-04-04](https://x.com/OpenAgentsInc/status/2040237117276586204), [2026-08-27](https://x.com/OpenAgentsInc/status/2092836555756823023) |
| K6 | [2024-01-26](https://x.com/OpenAgentsInc/status/1750729304504213964), [2026-02-25](https://x.com/OpenAgentsInc/status/2026754692873646279), [2026-06-23](https://x.com/OpenAgentsInc/status/2069212010131112441) |
| K7 | [2024-04-09](https://x.com/OpenAgentsInc/status/1777692495012405439), [2025-07-24](https://x.com/OpenAgentsInc/status/1948214009615765765), [2026-08-27](https://x.com/OpenAgentsInc/status/2092991734577819698), [2026-10-02](https://x.com/OpenAgentsInc/status/2105903502060859718) |
| L1 | [2026-06-04](https://x.com/OpenAgentsInc/status/2062555990864564373) |
| L2 | [2026-08-27](https://x.com/OpenAgentsInc/status/2092991734577819698) |
| L3 | [2026-07-17](https://x.com/OpenAgentsInc/status/2078193847209742468), [2026-07-18](https://x.com/OpenAgentsInc/status/2078499949105344645), [2026-09-30](https://x.com/OpenAgentsInc/status/2105372666995868125) |
| L4 | [2026-01-29](https://x.com/OpenAgentsInc/status/2016787108900335736) |
| L5 | [2026-01-31](https://x.com/OpenAgentsInc/status/2017485395512930730), [2026-03-07](https://x.com/OpenAgentsInc/status/2030156760275677420), [2026-07-11](https://x.com/OpenAgentsInc/status/2075869047560835291) |
| L6 | [2025-10-30](https://x.com/OpenAgentsInc/status/1983960929575338281), [2026-07-08](https://x.com/OpenAgentsInc/status/2074723427873702347) |
| L7 | [2024-12-20](https://x.com/OpenAgentsInc/status/1870030269916340610) |
| L9 | [2026-07-19](https://x.com/OpenAgentsInc/status/2078921380305768953) |
| L10 | [2026-07-10](https://x.com/OpenAgentsInc/status/2075680613731078265), [2026-07-19](https://x.com/OpenAgentsInc/status/2078687966210204011) |
| M1 | [2024-08-01](https://x.com/OpenAgentsInc/status/1819071289740644563), [2026-10-06](https://x.com/OpenAgentsInc/status/2107294286328787243) |
| M2 | [2026-10-07](https://x.com/OpenAgentsInc/status/2107909649299030348) |
| M4 | [2026-06-21](https://x.com/OpenAgentsInc/status/2068792528481173980) |
| M5 | [2024-07-11](https://x.com/OpenAgentsInc/status/1811507069398483192) |
| M6 | [2025-04-28](https://x.com/OpenAgentsInc/status/1916692323150311573), [2025-05-25](https://x.com/OpenAgentsInc/status/1926762604703129707) |
| X1 | [2024-02-22](https://x.com/OpenAgentsInc/status/1760765338193453150) |
| X2 | [2024-05-14](https://x.com/OpenAgentsInc/status/1790500162491523138) |
| X3 | [2024-10-03](https://x.com/OpenAgentsInc/status/1841710204691288356) |
| X4 | [2024-10-31](https://x.com/OpenAgentsInc/status/1851854221504635125) |
| X5 | [2026-08-20](https://x.com/OpenAgentsInc/status/2090295429640396967), [2026-08-24](https://x.com/OpenAgentsInc/status/2091887799137866064) |
| X6 | [2026-04-10](https://x.com/OpenAgentsInc/status/2042450069857693926), [2026-06-11](https://x.com/OpenAgentsInc/status/2065196586817216622) |
| X7 | [2026-03-07](https://x.com/OpenAgentsInc/status/2030132739672887561) |
| X8 | [2025-01-24](https://x.com/OpenAgentsInc/status/1882786018836820055), [2025-11-20](https://x.com/OpenAgentsInc/status/1991587456953774451) |
| X9 | [2026-08-03](https://x.com/OpenAgentsInc/status/2084193142970949903) |
| X10 | [2026-07-22](https://x.com/OpenAgentsInc/status/2080041664832311499), [2026-08-13](https://x.com/OpenAgentsInc/status/2087924381682933787) |
| X11 | [2023-09-26](https://x.com/OpenAgentsInc/status/1706812003258347630), [2024-05-30](https://x.com/OpenAgentsInc/status/1796251292798464265) |
| X13 | [2025-05-17](https://x.com/OpenAgentsInc/status/1923548136762466798) |
| X15 | [2024-12-14](https://x.com/OpenAgentsInc/status/1867815868131836103) |
| X16 | [2025-01-04](https://x.com/OpenAgentsInc/status/1875380960658681918) |
