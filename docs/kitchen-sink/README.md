# Project Kitchen Sink: OpenAgents version one

First draft, 2026-10-09, for [#11125](https://github.com/OpenAgentsInc/openagents/issues/11125).
For the owner to review. Not a commitment until the owner signs off.

This spec gathers every product promise from the 289-episode
[video archive](../transcripts/README.md) into one product: **OpenAgents 1.0**,
on the website, the terminal, the phone, the desktop, and the Verse. The full
list, with status, evidence, and episode numbers for each promise, is the
[promise ledger](ledger.md). The new promises registry and the `/promises` and
`/roadmap` pages ([#11122](https://github.com/OpenAgentsInc/openagents/issues/11122))
should be seeded from that ledger.

## The vision in one paragraph

OpenAgents is an open network of agents that you work with through one
conversation, from anywhere: the website, your terminal, your phone, your
desktop, and a shared world. You sign in once, and your chats, computers, and
projects follow you. Behind that one conversation, the network does real work,
starting with code: it shows every step, uses whichever model gets the job
done for the least money, and hands each part to the agent, model, or
computer that does it best. When the network can't do something yet, it
builds that ability with you,
tests it, and shares it with every user, and the person who built the ability
gets paid in Bitcoin each time it's used. Everything is open source and runs
on open protocols, so anyone can check our claims, run their own copy, or
join the network without asking us.

## Where we stand today

From the [ledger](ledger.md) (134 promises):

| Status | Count | Meaning |
| --- | --- | --- |
| Live | 5 | In production with evidence |
| Launching | 22 | Built and on staging; ships Monday 10-12 if the smoke test passes |
| Partial | 68 | Part of it is built, or it's built but not in front of users |
| Missing | 23 | Nothing a person can use yet |
| Dropped on purpose | 16 | Set aside, each with a reason ([ledger §N](ledger.md#n-dropped-on-purpose)) |

The pattern: much is built and little has shipped. Most "Partial" rows are
finished code waiting on one of three things: the production account service
([#11094](https://github.com/OpenAgentsInc/openagents/issues/11094)), the
production API host, or an app store review.

## What the archive keeps promising

Eight ideas come back in every era, from episode 001 to 289. V1 should honor
all of them, even where a feature isn't built yet.

| # | Principle | First and latest episodes |
| --- | --- | --- |
| 1 | **One conversation, a network behind it.** One account and one conversation on every surface; behind it, a network of agents, models, and computers that grows as people add abilities, not a store of separate bots. | 086 → 289 |
| 2 | **You see everything.** Every step, every model used, every cost. No hidden routing. | 033 → 277 |
| 3 | **No claim without proof.** A public list of what works, with evidence, and a place to report what doesn't. | 120, 234 → 288 |
| 4 | **Contributors get paid.** Plugin authors, compute providers, data owners, and referrers earn Bitcoin when their work is used. | 037 → 289 |
| 5 | **Neutral across AI labs.** Use the best or cheapest model for each job; hand work to other agents. | 067 → 287 |
| 6 | **Open source and open protocols.** Anyone can read the code, run a copy, or join the network without permission. | 001 → 288 |
| 7 | **Your keys, your data.** Self-custody wallet, private by default, sync only when you turn it on, delete means delete. | 143 → 279 |
| 8 | **Demand first.** Supply-side markets failed when buyers were missing (GPUtopia, 174; training payouts, 224). Useful paid work comes before markets. | 174 → 247 |

## The five surfaces

Each surface does one job well. They share one account, one chat history, one
agent, and the same abilities.

### Website (openagents.com)

**Job:** the front door. Try it, sign in, chat, manage your account, and see
everything you have running on your other devices.

| Area | Full intended feature set | Ledger |
| --- | --- | --- |
| Chat | Chat with OpenAgents and the network behind it; pick or switch models; streaming; pin, rename, archive, search, delete | B1–B7 |
| Account | GitHub sign-in; guest chats carry over; signed-in computers; privacy and deletion | A1, A3, A4, B6, E6 |
| Projects | Group chats by GitHub repo; issue in, pull request out | B7, C6 |
| Your other devices | Terminal chats with live status; reply from the browser; run Coder on a connected computer | D1, C14 |
| Abilities | Build a new ability with the agent; browse the shared registry; Gym results | F1, F2, G1 |
| Money | Pro plan by card; later wallet, earnings, and payouts | B14, I1–I7 |
| Proof | `/promises` (what works, with proof), `/roadmap` (what's next), changelog, stats | G6, H6, J6 |
| For agents | `llms.txt`, agent card, OpenAPI, docs MCP, pay-per-request API | K1–K3, K6, E1–E4 |
| Window into the Verse | `/everglade` in the browser | M1 |

### Terminal (`coder`)

**Job:** the power tool. Do real work on your own machine, with every step
visible, and nothing leaving your computer unless you turn sync on.

| Area | Full intended feature set | Ledger |
| --- | --- | --- |
| Install | One command on Mac, Linux, and Windows | C1, L2 |
| Work | Shows every step; acts without asking; queue messages; % done; understands the whole repo | C2–C5, C13 |
| Delegate | Hand pieces to Codex, Claude, Devin, and others; walk away and let it keep going | C7, C8 |
| Cost | Cheap look-around before expensive models; cheapest model per task; your own keys and subscriptions | C9–C12 |
| Account | `coder login`; `/sync`; key screening; trace upload | A2, D2, D3, H2 |
| Computers | Run on connected computers and rented cloud computers | C15–C17 |

### Mobile (iPhone and Android)

**Job:** the remote. Talk to the agent and keep an eye on and steer your work
from anywhere, and hold your wallet.

| Area | Full intended feature set | Ledger |
| --- | --- | --- |
| Chat | Chat with the agent; your account's chats from every device | B1, D4 |
| Your computers | Pair by scanning a code; run Coder there; notifications when an agent needs you | D5, D6, C14 |
| Wallet | Self-custody Bitcoin wallet; approve agent payments | I1–I3 |
| Voice | Talk and listen | B12 |
| Report | Report a problem from the app | G7 |
| Later | The Verse and XP (built, hidden today) | M1, G3 |

### Desktop (Mac, Linux, Windows)

**Job:** the cockpit. Many agents, chats, and terminals on one screen, with
game-like controls.

| Area | Full intended feature set | Ledger |
| --- | --- | --- |
| Shell | Signed, self-updating app on three platforms; light and dark | L3, L8 |
| Grid | Chats and terminals side by side; command agents like units | L4, L5, M6 |
| Work | Everything Coder does, plus the agent computer (CoderOS) | C2–C17 |
| Devices | Pair your phone by QR | D5 |
| Extras | Slides and live answer pieces; Gym card | B13, G1 |

### The Verse

**Job:** the shared world. A place where you and other people can see
agents doing real work, and where progress and play run on verified work.

| Area | Full intended feature set | Ledger |
| --- | --- | --- |
| World | A shared 3D world on desktop, phone, and web | M1 |
| Work you can watch | Agent Studio: your agents work on your repo; you answer and approve merges | M2, M7 |
| Your agents | Named agents you own, each with a job and a place | M3, A7 |
| Network you can see | Work and payments flowing between agents and plugins | M4 |
| Game | Guilds, quests, trades, levels, tied to verified work | M5, G3, G4 |

### How the surfaces connect

| Connection | How it works | Status |
| --- | --- | --- |
| One account | GitHub sign-in on the web; device-code sign-in for terminal and desktop | Launching (A1, A2) |
| One chat history | Each device chooses to sync; deletes go both ways; the phone joins next | Launching on web + terminal (D1–D3); phone Partial (D4) |
| One conversation | The same conversation and the same network of abilities everywhere, through the OpenAgents API | Partial (B1, E1) |
| Your computers | Any surface can run Coder on any computer you've connected | Partial (C14, A4) |
| One wallet and one level | The same balance, XP, and unlocks on every surface | Missing (D7, I1) |
| The Verse | A view of the same account, agents, and work, not a separate account | Partial (M1–M3) |

## Systems that run under every surface

| System | What it does for a person | Today | Ledger |
| --- | --- | --- | --- |
| Accounts and identity | One sign-in; your computers; later, your own keys and your agents' keys | Launching on staging; production service in progress | A1–A8 |
| Sync | Your chats on every device, on your terms | Web + terminal launching; phone next | D1–D7 |
| Agents, Coder, delegation | Real work, every step visible, other agents brought in when better | Coder 1.0 ready; delegation partial | C1–C19 |
| Models and the API | Many models, one key, free tier, your own keys, pay per request | Built and tested on staging; not in production | E1–E10 |
| Plugins and skills | New abilities built with you, tested, shared with everyone | Evals from chat work; registry missing | F1–F7 |
| Gym, evals, Jev | Measure what helps, publish the evidence, show progress | Internal; Jev's % done in Coder | G1–G7 |
| Markets, Pylon, compute | Sell spare compute; paid jobs for your agent | Pylon runs free jobs; paid earning paused | J1–J8 |
| Payments, wallet, Lightning, x402 | Self-custody wallet; card for Pro; pay per request; payouts to authors | Phone wallet in beta; card and x402 built; payouts owner-only | I1–I9 |
| Traces, data, privacy | Every run recorded; private unless shared; later, sell your data | Local traces live; upload launching | H1–H6, E6 |
| Agent-ready web | Any agent can read our docs, API, and skills and pay us | Built; waiting on deploy | K1–K3, K6 |
| Open source | Read, fork, self-host | Live | K4, K5 |

## V1: what "OpenAgents version one" must include

V1 is the smallest set that makes principles 1, 2, 3, 6, and 7 true for a
person on day one, and starts principle 4. A surface is in V1 only if a
stranger can install and use it.

### V1 core (must ship)

- **One account, two doors:** web and terminal, with sync between them (A1–A4, D1–D3).
- **The agent and Coder:** chat, models, streaming, projects, Coder 1.0 (B1–B7, C1–C5).
- **Honesty built in:** `/promises` and `/roadmap` generated from this ledger, plus agent-ready files (G6, K1, K2).
- **Privacy:** provider no-training, 30-day usage deletion, key screening, delete-all (E6, D3, B6).
- **Traces:** local, and uploaded privately (H1, H2).

### V1 complete (the rest of "1.0" within about a week)

- iPhone (TestFlight public link) and Android APK, with the phone joining the account's chats (L6, L7, D4).
- Desktop 1.0 on Mac and Linux, Windows unsigned (L3).
- The public OpenAgents API in production with a free tier (E1, E2).
- The Pro plan with cloud computers (B14, C15).

### Expanded scope (1.x, after V1)

- Abilities that compound: build-with-you, the shared registry, a public Gym (F1, F2, G1).
- Money: wallet in launch copy, plugin-author payouts, published split, referrals (I1–I7, F3).
- Memory, attachments, voice (B9, B10, B12).
- Paid compute and agent jobs (J1, J2, J5).
- The Verse as a launch surface, tied to real work and the account (M1–M5).

## Sequencing

The owner has moved the launch to **Monday 2026-10-12, web + terminal**
(#11091, #11102). This draft suggests:

| When | What ships | Gate |
| --- | --- | --- |
| **Today, Fri 10-09: basic web update** | Only what doesn't need the new account service: homepage composer and cards ([#11123](https://github.com/OpenAgentsInc/openagents/issues/11123)), agent-ready files ([#11083](https://github.com/OpenAgentsInc/openagents/issues/11083)), docs pages, plain-words copy | Staging smoke passes on those pages |
| **Mon 10-12: V1 core** | Production account service, GitHub sign-in, sync, projects, signed-in computers, trace upload, privacy; Coder 1.0.0 stable; `/promises` and `/roadmap` | #11094, #11091, #11102, #11122 |
| **Week of 10-12: V1 complete** | iPhone public TestFlight link (after Apple review), Android APK, phone joins account chats, desktop 1.0, public API, Pro plan | #11093, #11107, #11092/#11120, gateway production host |
| **Late October: abilities** | Build-with-you abilities, shared registry, public Gym page, attachments, memory, issue-to-PR on the web, walk-away runs | Registry design; Gym page |
| **November: money** | Wallet in launch copy, published split, author payouts, referrals, paid Pylon jobs, trace sharing | Split decision; payout checks |
| **After: the world** | The Verse on every surface, showing real agent work, payments, and levels | Account and payments in the world |

### Dependencies

```
production account service (#11094)
  ├── sign-in, sync, projects, computers  → Mon 10-12
  ├── phone joins account chats (#11107)  → V1 complete
  └── one level / one wallet across apps  → 1.x
production API host
  ├── public API + free tier              → V1 complete
  ├── Pro capacity and cloud computers    → V1 complete
  └── Pylon as a paid source (#11080)     → November
promises registry (#11122) ← this ledger
shared ability registry (F2)
  └── author payouts (F3) ← published split (I6) ← pay host checks
Apple review → iPhone link; Windows signing → Windows desktop
```

## Top 10 gaps

| # | Gap | Why it matters | Ledger |
| --- | --- | --- | --- |
| 1 | Production account service isn't running | Gates sign-in, sync, and projects for Monday | A1, #11094 |
| 2 | The public API isn't in production | Blocks the API, Pro capacity, and agent buyers | E1–E4 |
| 3 | No shared registry for abilities | The 289 promise: one user's new ability helps everyone | F2 |
| 4 | No payouts to plugin authors, and no published split | The oldest promise in the archive (037) | F3, I5, I6 |
| 5 | The phone doesn't show the account's chats; iPhone waits on Apple; Android deferred | Breaks "one account everywhere" on mobile | D4, L6, L7 |
| 6 | Desktop 1.0 not released | One of the five surfaces is missing | L3 |
| 7 | No public promises or roadmap page | Principle 3 has no home since August | G6 |
| 8 | No memory about you | Promised since 005; expected of any agent | B10 |
| 9 | No file or image attachments in web chat | The very first product (008) did this | B9 |
| 10 | The Verse isn't connected to real work or the account | The fifth surface is a separate game today | M1–M5 |

## Open decisions for the owner

| # | Decision | Recommendation |
| --- | --- | --- |
| 1 | Is Monday web + terminal only? | **Yes.** Call phone and desktop "1.0 beta" with links on `/download` that week. |
| 2 | What goes out in today's basic web update? | **Homepage, agent-ready files, and docs only.** Hold sign-in and sync for Monday so they launch with the account service. |
| 3 | Is the Pro plan in Monday's copy? | **No.** Announce Pro the same week as the public API, since both depend on production capacity. |
| 4 | What is the split for plugin authors? | **Publish one simple split before any payout**, starting from the archive's 80% author / 20% OpenAgents (097), and put it on `/promises`. |
| 5 | Is the wallet in V1? | **Keep it in the phone beta, self-custody only, out of launch copy** until payouts exist to put money in it. |
| 6 | Is the Verse a V1 surface? | **No.** Keep it as a playtest. Make it a launch surface when it shows your own agents' real work under your account. |
| 7 | Should the old dropped promises appear on `/roadmap`? | **No.** List them only in the ledger, so the roadmap stays a short list of things being built. |

## How this draft was made

- Six readers covered the episodes in ranges (001–060, 061–120, 121–180,
  181–230, 231–262, 263–289) and pulled out each promise, with episode numbers
  and what later happened to it. The theme map in the
  [archive index](../transcripts/README.md#theme-finder) organized the merge.
- The old registry (`product-promises.ts`, 145 records: 34 green, 21 yellow,
  7 red, 78 planned, 5 withdrawn) was recovered from
  `git show d613b8ea22^:apps/openagents.com/workers/api/src/product-promises.ts`.
  Its states map to this ledger: green → Live, yellow → Partial, red and
  planned → Missing, withdrawn → Dropped.
- Current state comes from the code on `main` at `4e14c405e7`, the
  [1.0 launch drafts](../launch/1.0/README.md), the
  [Verse status](../verse/status.md), the
  [gateway doc](../inference/gateway.md), and the open issues on the V1 board.
- Episode transcripts are machine transcriptions. Use the ledger's episode
  numbers to find the source, and check the video before quoting.
