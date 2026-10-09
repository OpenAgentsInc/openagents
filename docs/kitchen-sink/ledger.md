# Kitchen Sink promise ledger

Draft, 2026-10-09, for [#11125](https://github.com/OpenAgentsInc/openagents/issues/11125).
This is the full list behind the [Kitchen Sink spec](README.md). The
promises registry ([#11122](https://github.com/OpenAgentsInc/openagents/issues/11122))
is meant to be seeded from it.

Each row is one promise from the [video archive](../transcripts/README.md),
written as what OpenAgents does for a person. Duplicates across eras are
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
| E3 | Agents can pay per request without an account. | N | Partial | x402 on the gateway (`c383d9fcd1`) | 062, 063, 070, 086 | #11077, #11085 |
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
| I3 | Your agent can pay and get paid, under rules and budgets you approve. | M N | Partial | Agent payment approvals on the phone | 042, 047, 141, 147, 212 | — |
| I4 | You can pay by card. | W | Partial | Stripe Pro subscription built | 037, 064, 116 | — |
| I5 | Earnings go to your Lightning address automatically. | N | Partial | Splits and payouts built on the pay host; owner-only so far | 064, 092, 096, 098 | — |
| I6 | The revenue split between authors, providers, and OpenAgents is published. | W | Missing | No current split | 037, 097, 098 | — |
| I7 | Bring someone in and earn a share of what they pay, for as long as they pay. | N | Missing | Red in the old registry | 037, 125, 150, 153, 229, 239 | — |
| I8 | Bitcoin is the only money. There is no token. | all | Live | Policy across every payment path | 001, 200, 220, 230 | — |
| I9 | You can hold dollar-pegged balances backed by Bitcoin. | N | Missing | Taproot Assets work lives in `tap-ldk` | 096, 173, 200 | — |

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

## K. Agent-ready and open

| ID | Promise | Surfaces | Status | Evidence or reason | Episodes | Issues |
| --- | --- | --- | --- | --- | --- | --- |
| K1 | Any agent can learn to use OpenAgents from one file on the website. | W N | Launching | `llms.txt`, agent card, skills, `/openapi.json`; blocked on the deploy | 230, 231, 235, 237 | #11083 |
| K2 | Agents can read our docs through MCP. | N | Launching | Docs MCP server | 165, 168 | #11086 |
| K3 | Everything you can do in an app, you can do from the command line or API. | T N | Partial | `openagents-cli` over Nostr; `/docs/api` | 067, 085, 100, 203, 289 | — |
| K4 | All of it is open source and built in public. | all | Live | This repository | 001, 047, 125, 173, 242 | — |
| K5 | You can run your own copy of the whole thing. | N | Partial | Open source, but no self-host guide | 129, 242, 289 | — |
| K6 | Agents can buy from us using standard agent payment protocols. | N | Missing | x402 only | 062, 070 | #11085 |

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
