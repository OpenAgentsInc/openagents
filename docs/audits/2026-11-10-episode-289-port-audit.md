# Audit: porting Episode 289's OpenAgents to Coax

Written 2026-11-10. This audit lists what [Episode 289](../transcripts/289.md)
promised, with [Episode 288](../transcripts/288.md) and the two essays behind it
as context. It compares that with the code at the time and with today's Coax
tree, and proposes the shape and order for rebuilding all of it on the new
system.

The owner's direction: everything from 289 ships, rebuilt on Coax, Aethyr and the
one protocol. Nothing is dropped. The Verse, Everglade and the engine work are
the base the rest stands on.

## Sources

- **Episodes:** [288, Three DevDays Later](../transcripts/288.md), [289, OpenAgents](../transcripts/289.md),
  and for context [284](../transcripts/284.md) to [287](../transcripts/287.md) and
  [290, Enter the Verse](../transcripts/290.md).
- **The essays**, at tag `coder-v1.0.0` (`git show coder-v1.0.0:docs/essays/...`):
  - *Test-Time Capabilities* (2026-09-29)
  - *The Return of the General Agent* (2026-10-01)
- **The code at the time:** tag `coder-v1.0.0` (3230ded511, 2026-10-15), the last
  Rust tree before the reset. It has 229 crates under `crates/`, apps under
  `bins/` and `swift/`, plugins, programs, recipes, knowledge, capabilities, NIPs
  and the `os/` Nix image. It is not in `main`'s history: the repository was reset
  on 2026-10-27, and `main`'s root commit is from 2026-11-01. To browse it, run
  `git worktree add ../oa-1.0 coder-v1.0.0`.
- **Today's tree:** `main` at the time of writing.

## 1. What Episode 289 promised

Episode 289 defines OpenAgents as **a composable general agent**. One agent sits
behind every surface, and the words "everything, everything, everything" mean
the whole CLI and every capability are reachable through it. The promises:

| # | Promise | Where it was said |
| --- | --- | --- |
| P1 | One agent behind openagents.com, the terminal, desktop and mobile apps | 289 at 0:30 |
| P2 | A shared knowledge base the agent answers from | 289 at 0:30 |
| P3 | It loads the Coder agent for anything about your repos, and delegates to subagents and other engines ("do it with Codex") | 289 at 0:30 and 1:00 |
| P4 | Visible tools and subagents, added to first by us, then by anyone | 289 at 0:30 |
| P5 | Plugins and an eval system, adapted from Claude Code's plugins and evals | 289 at 0:30 |
| P6 | Routes beyond coding: policy, small talk, general, Gym news | 289 at 1:00 |
| P7 | A typed front that routes each message cheaply through a decision model (System One, Jev) | 289 at 1:00; the general-agent essay |
| P8 | The whole CLI behind the one agent: "send a thousand sats" opens the wallet, "show the deck" shows a deck | 289 at 1:00 to 1:30 |
| P9 | When the agent can't do something, it offers to build it as a plugin: an interview, drafted tests, evals, a run | 289 at 1:30 |
| P10 | A plugin that passes enters a registry, and every OpenAgents gets it | 289 at 2:00 |
| P11 | Authors are paid Lightning micropayments for all paid use of their plugins | 289 at 2:00; 288's revenue-sharing argument |
| P12 | Open protocols: signed JSON over WebSockets (Nostr), open source, self-hostable, so other projects can join with no permission | 289 at 2:30; 288's NIPs |
| P13 | Simple interfaces: openagents.com, the mobile app, partner apps on the OpenAgents API or a self-hosted subset of it | 289 at 3:00 |
| P14 | Reputation, identity and payments solved once for every plugin | 289 at 3:30 |
| P15 | Test-time capabilities: a component is a candidate; evidence makes a claim; adoption is policy; credit goes to verification | 289 at 4:00; the essay |
| P16 | Version 1.0 on Windows, Mac, Linux, iOS, Android, web and terminal, from openagents.com/download | 289 at 4:00 |

Episode 288 adds the *why*. OpenAI's marketplace shares no revenue and needs
permission, so ours is open, permissionless and pays its authors.

## 2. What existed then (`coder-v1.0.0`)

Most of this was built and partly live in the Rust tree:

- **The front and chat:** `openagents-chat`, `openagents-chat-app`, `route-contract`,
  `jev`, `jev-hosted`, `brainstorm-client`, and the chat router with Gym and eval
  routes.
- **Coder and delegation:** `coder`, `coder-*` (about 60 crates), `coder-delegate`,
  `codex-transport`, `claude_agent_sdk`, `acp-client`, `microcoder`, `briefed-agent`,
  `agent-fleet`, `supervise`.
- **Plugins:** `plugin`, `plugin-pdk`, and nine shipped plugins (`plugin-repo-map`,
  `plugin-code-search`, `plugin-test-report`, and others), plus `plugins/` for
  claude, codex, brainstorm, meeting notes and skills.
- **Evals and the Gym:** `ext-eval`, `eval-runner`, `gym`, `gym-bridge`,
  `gym-leaderboard`, `coderbench`. One capability, Project map, went from claim to
  externally validated adoption on 2026-09-29.
- **Knowledge:** `knowledge`, `memory-stream`, `code-graph`, `code-index`, and
  `knowledge/` entries.
- **Payments:** `wallet`, `spark-wallet`, `pay-ledger`, `pay-host`, `x402`,
  `receipts`, `commercial-spend`, `xp-ledger`.
- **Protocol:** `nostr`, `nostr-relay`, `nostr-transport`, `atif`, `atif-nostr`,
  and 33 OpenAgents NIPs (`nips/openagents/`).
- **Compute:** `pylon`, `psionic`, `inference`, `model-access`, `openrouter`,
  `oa-computers`, `boat`, `solver-box`.
- **Apps:** `openagents-desktop`, `openagents-mobile`, `openagents-terminal`,
  `openagents-cli`, `openagents-web`, `openagents-deck`, with iOS and Android
  shells in `bins/`.
- **Programs and recipes:** `programs/*.json` (solve-issue, review-changes,
  delegate-fan-out and more), and 20 `recipes/` built on decision models.

The weak points, as the essays themselves say: no independent contributors at
scale, no compounding across a network, nothing proven outside software work.
Version 1.0 shipped for Coder (`coder-v1.0.0`), not for the whole OpenAgents
app on every platform.

## 3. What exists today in Coax

The reset kept a small, checked core and has grown it fast. These parts of 289
already have a Coax base:

| Area | Today | Where |
| --- | --- | --- |
| The language and tools | Coax, compiling to C and Wasm, with a book | `coax/`, `coax/book/` |
| Engine and world | Aethyr, the Verse, Everglade, Experimental, with a book | `aethyr/`, `verse/`, `everglade/` |
| Protocol | Nostr core and about 80 NIPs, every OpenAgents NIP (now 37, adding AGENT, DEPLOY, KEYS and SKILL), a relay, and a book | `coax/lib/nostr/`, `nostr/nips/openagents/`, `nostr/book/` |
| Bitcoin and Lightning | Lightning primitives (BOLT 1, channels, onion, offers, pay, invoices), an explorer lab, and a book | `coax/lib/lightning/`, `bitcoin/` |
| x402 | settle and verify | `coax/lib/x402/` |
| Models | a gateway (Anthropic, OpenAI, Gemini, Vertex, OpenRouter, Psionic, Pylon, and others), System One, and the Psionic engine with a book | `coax/lib/gateway/`, `coax/lib/systemone/`, `psionic/` |
| Coding agent | the Coder book (38 chapters: the loop, tools, sandbox, plugins, protocols, driving Claude Code, Codex and Cursor), Microcoder, and agent drivers | `coder/book/`, `coax/lib/microcoder/`, `coax/lib/agent/` |
| Delegation | Golemancy: a model tier per issue, Autopilot, and the Golems board | `apps/everglade-desktop/next.coax`, `coax/lib/agent/tier.coax` |
| Apps | the web realms and chat, the CLI, desktop, iOS, and Everglade for Mac, iOS and Android | `apps/` |
| OS | the CoderOS desk, compositor and capture pieces | `os/` |

So the protocol, payment primitives, model access, the coding agent, the engine
and the apps' shells are already in Coax. What is missing is the layer 289 was
actually about: the general agent, and the open economy of capabilities around
it.

## 4. Promise by promise: the gap and the shape

| # | Promise | Status in Coax | What to build | Where it belongs |
| --- | --- | --- | --- | --- |
| P1 | One agent behind every surface | Partial: web chat and CLI exist; no one shared agent runtime | **The OpenAgents agent runtime**: one Coax library every surface links (web Wasm, CLI, desktop, mobile) and one hosted service | `openagents/agent/` (new top-level, see section 5) |
| P2 | Knowledge base | Missing as a runtime; NIP-KB exists | A knowledge store with retrieval by embedding search (per the semantic-routing rule), sourced from docs, books and transcripts, published as NIP-KB events | `openagents/knowledge/` |
| P3 | Coder and delegation | Strong: the Coder book, drivers, tiers | Expose Coder as a member of the composition; route "work on my repo" to it | `coder/` (exists) |
| P4 | Visible tools and subagents | Partial | A members registry the UI lists: tools, subagents, plugins, each with its claim status | the runtime |
| P5 | Plugins and evals | Partial: Coder book ch. 15 (plugins), ch. 21 (measuring the harness); NIP-EVAL and NIP-EXT exist | **A plugin host** (a Coax plugin interface, Wasm-sandboxed, with grants), and **the eval engine** (cases, graders, two-arm runs, reports), ported from `ext-eval` and `eval-runner` | `openagents/plugins/`, `openagents/eval/` |
| P6 | Routes beyond coding | Missing | Members for answers, policy, small talk, the Gym, the Verse, and so on | the runtime |
| P7 | Typed front on a decision model | Partial: System One and Decisions are in Coax | **The front**: one typed router whose input is the message plus context, whose output is a typed plan (members, grants, budget). Calls go through the Decisions API (Clef first). No keyword routing | `openagents/agent/front` |
| P8 | The whole CLI behind the agent | Partial: the CLI exists; no wallet in Coax | Every CLI command becomes a typed member the front can call, with an approval step for money. **A wallet** (Lightning send and receive, balances, history) on `coax/lib/lightning` | `openagents/wallet/`, `apps/openagents-cli` |
| P9 | Build a missing plugin from chat | Missing | The authoring interview: spec, draft tests, eval, run, publish. It runs as a Coder program | `openagents/eval/author` |
| P10 | Registry; everyone gets it | Missing: NIP-REG exists | **A registry**: plugin releases as signed NIP-REG events on our relay, mirrored from any relay; installs pinned by hash; adoption into defaults by recorded policy | `openagents/registry/` |
| P11 | Authors paid in Lightning | Missing | **Metering and payouts**: each paid call writes a receipt; a payout runner splits revenue to authors over Lightning on a schedule; a public ledger | `openagents/pay/` on `coax/lib/lightning` |
| P12 | Open protocol and self-hosting | Strong: Nostr and NIPs in Coax | Make every step above a NIP event (request, judgment, member, grant, run, eval, reproduction, adoption, credit). Ship a self-host bundle | `nostr/nips/openagents/` |
| P13 | Partner apps and an API | Partial | **The OpenAgents API** (HTTP and WebSocket), the same one our apps use, with keys, rate limits and an OpenAPI description; a self-host image | `openagents/api/` |
| P14 | Reputation and identity | Partial: keys, NIP-05, XP ledger | Author and plugin reputation from reproduced claims, not stars; one identity across web, apps and the Verse | `openagents/registry/` with `coax/lib/nostr` |
| P15 | Test-time capabilities | Missing in code; the essay is the spec | The claim lifecycle: candidate, admission, controlled delta, reproduction, external validation, adoption, credit, revalidation, with claims as NIP-EVAL and NIP-CAP records | `openagents/eval/` |
| P16 | 1.0 on every platform | Partial: Mac, iOS, Android (Everglade), web, Linux tooling; **no Windows host** | Hosts per platform under Aethyr's host contract, including a **Windows host** (Win32, D3D12 or Vulkan); one release pipeline that builds, signs and publishes all of them to openagents.com/download | `aethyr/host/`, `coax/platform/`, release scripts |

## 5. The proposed shape

Three layers, each built on the one below:

1. **Foundation (exists, keeps growing):** Coax, Aethyr and the Verse, the Nostr
   protocol, Lightning, the gateway and Psionic.
2. **The general agent (new):** a front, members, plugins, knowledge, evals,
   claims, a registry, payouts and an API. This is 289's OpenAgents. It belongs
   in one new top-level directory, `openagents/`, the same way `coder/`,
   `nostr/`, `bitcoin/` and `psionic/` each own a domain.
3. **Surfaces (exist as shells):** web, CLI, desktop, mobile, the Verse and
   partner apps. They are thin clients of the agent runtime and the API, and
   none has its own agent logic.

The Verse is a surface, not a separate product. Agents you hire appear in the
world. Golems show agent work live, as they now do on the Mac. Plugins and
realms are both members of the same registry, and both are paid the same way.

### Should we write another book?

Yes, one: **The OpenAgents Book**, at `openagents/book/`. It follows the pattern
of the Coax, Aethyr, Nostr, Bitcoin, Psionic and Coder books: each chapter
explains one idea and ships the checked Coax that implements it, so the book is
the spec, the tutorial and the test plan in one.

The other books already cover most of the foundation. The gap is the
composition and the economy, and no existing book owns them. The Coder book is
about one member, and the Nostr and Bitcoin books are about transport and money.
A proposed outline:

1. **Part I, one agent.** The simplest front; members; a typed plan; grants and
   approvals; routing through the Decisions API; an answer member over the
   knowledge base.
2. **Part II, members.** Coder as a member; delegation to other engines;
   subagents; the CLI's commands as members; the wallet as a member with
   spending approval.
3. **Part III, plugins.** The plugin interface; the Wasm sandbox; grants;
   writing one; the authoring interview from chat.
4. **Part IV, evidence.** Cases and graders; a two-arm run; a claim's key;
   reproduction by another key; adoption policy; revalidation when a model
   changes. This part is the essay made executable.
5. **Part V, the market.** Releases as NIP-REG events; installs pinned by hash;
   metering and receipts; Lightning payouts to authors; reputation from
   reproduced claims.
6. **Part VI, everywhere.** The API; partner apps; self-hosting; each surface as
   a thin client; agents in the Verse.

Each part ends with something that runs. Part IV, for example, re-runs the
Project map adoption from 2026-09-29 in Coax and gets the same verdict.

### What else is needed besides the book

- **A port map** of all 229 old crates: port, fold or drop. Group-level
  dispositions are in section 6; each crate needs a one-line entry in
  `openagents/book/PORT.md`.
- **A Windows host** for Aethyr and `coax/platform`, the only platform 289
  promised that has no host today.
- **One release pipeline** for every platform, with signing and notarization,
  building on the shared build cache (#13119) and green main (#13102).
- **The hosted services:** the agent API, the registry relay, the eval runner on
  our GCP build hosts and the payout runner. They run as Coax binaries and stay
  inside the $1,500-a-month GCP budget.
- **Owner decisions,** listed in section 8.

## 6. Port map by group (from `coder-v1.0.0`)

| Old group | Crates (examples) | Disposition |
| --- | --- | --- |
| Chat and front | `openagents-chat`, `route-contract`, `jev`, `jev-hosted`, `brainstorm-client` | Port as the agent front (book Part I). Jev calls go through the Decisions API |
| Coder | `coder`, `coder-*`, `microcoder*`, `briefed-agent`, `supervise`, `agent-fleet` | Mostly done or underway in `coder/`. Port the remaining host, sync and terminal pieces as the Coder book reaches them |
| Plugins | `plugin`, `plugin-pdk`, `plugin-*` (9) | Port: the interface first, then the nine plugins as book examples |
| Evals and Gym | `ext-eval`, `eval-runner`, `gym*`, `coderbench`, `workflow-run` | Port as book Part IV and the hosted runner |
| Knowledge | `knowledge`, `memory-stream`, `code-graph`, `code-index` | Port: knowledge store and code index |
| Payments | `wallet`, `spark-wallet`, `pay-ledger`, `pay-host`, `x402`, `receipts`, `commercial-*`, `xp-ledger` | Port on `coax/lib/lightning` and `x402`. Bitcoin and Lightning only (Spark is a Lightning path; no other chains) |
| Protocol | `nostr*`, `atif*`, `oa-att` | Done in `coax/lib/nostr`; keep the NIPs current |
| Compute | `pylon`, `psionic`, `inference`, `model-access`, `openrouter`, `oa-computers`, `boat`, `solver-box` | Psionic and the gateway are done. Port Pylon and Computers as members and as a compute market |
| Apps | `openagents-desktop`, `-mobile`, `-terminal`, `-cli`, `-web`, `-deck`, `bins/*` | Rebuild as thin clients in `apps/` |
| Verse and game | `verse-*`, `everglade-web`, `game-lab`, `playtest` | Done and far ahead in Aethyr and Everglade |
| Labs | `cad*`, `robot-*`, `ship-lab`, `physics`, `laya`, `lev`, `kev`, `voyager` | Fold into Aethyr labs where they're still wanted; the owner picks |
| Small web toys | `bunny-*`, `icecream-*`, `snap-*`, `townsfolk`, `town-clock` | Drop unless the owner names one |
| Infra | `oa-auth`, `oa-tokens`, `oa-vault-web`, `secret-screen`, `tenancy`, `gateway`, `push-gateway` | Port as the API's auth, keys, vault and push |

## 7. Order of work

Each step ships something usable and is the base for the next. Steps on
different layers can run in parallel.

1. **Velocity first (now):** resident compiler #13138, shared build cache #13119,
   green main #13102, native checks #13122, and the Coax rules in #13125. Every
   later step is many agent-weeks of compiles, so this pays back first.
2. **The book's skeleton and the port map:** `openagents/book/` with its
   contract, plan and chapter list, and `PORT.md` with all 229 crates.
3. **One agent, in chat:** the front, the knowledge answer member, Coder as a
   member, on the web chat and the CLI (P1, P2, P3, P6, P7).
4. **The wallet and CLI members:** send and receive sats from chat with approval
   (P8).
5. **Plugins and evals:** the interface, the sandbox, the nine plugins, two-arm
   runs, and claims (P4, P5, P15).
6. **The authoring interview:** build a missing plugin from chat (P9).
7. **The registry and payouts:** publish, install, adopt and pay authors (P10,
   P11, P14).
8. **The API and self-host:** partner apps on our API, and a self-host image
   (P12, P13).
9. **1.0 everywhere:** the Windows host, the release pipeline, and the download
   page (P16).
10. **Agents in the Verse:** hired agents and plugins as things you meet and pay
    in the world.

Each step gets a tracking issue in project 22 with its child issues, worked
through Golemancy at the tier each one rates.

## 8. Decisions for the owner

1. **Name and place:** a new top-level `openagents/` with `openagents/book/`?
   This audit recommends it.
2. **The plugin sandbox:** Wasm with grants, which this audit recommends, or
   native processes with OS sandboxing.
3. **The revenue split:** the author's share of each paid call, and the payout
   schedule (for example, daily over Lightning above a minimum).
4. **Which labs and toys come back** (section 6).
5. **Windows graphics:** D3D12 or Vulkan for the Windows host.
