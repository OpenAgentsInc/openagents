# OpenAgents

OpenAgents is a **composable general agent**: one agent you reach from
anywhere, made of many specialized members that it brings in for each
request. Ask a question, plan some work, change some code, pay for a
service, or build something it can't do yet. Each step goes to whichever
member, model, or computer serves it best.

- **One agent, every surface.** [openagents.com](https://openagents.com),
  the phone and desktop apps, the terminal, and any other app through
  [our API](https://openagents.com/docs/agents). They're the same agent and
  the same chats.
- **A cheap decision layer in front.** A typed decision model, System 1,
  decides what each step needs before any big model runs. It goes through
  our [Decisions API](docs/api/decisions.md), Clef first.
- **Members are admitted on evidence.** A member can be an answer, a
  knowledge entry, a plugin someone wrote, another agent, or another
  computer. It joins the defaults only after a with-and-without measurement
  shows it helps. We call these
  [test-time capabilities](docs/essays/2026-09-29-test-time-capabilities.md).
- **Open all the way down.** Everything here is open source under the
  [Apache 2.0 license](LICENSE). Every step is a signed record on Nostr
  ([our NIPs](nips/openagents/README.md)), so other apps can build on the
  same agent, self-host a part of it, or extend it. Payments are Bitcoin
  over Lightning.

**Coder is the first member, not the whole picture.** It's our coding
metaharness: one Coder in front of the coding agents you already use
(Claude Code, Codex, OpenCode, Grok Build, Devin CLI, Cursor), deciding
what each step needs. [Coder 1.0](https://openagents.com/blog/coder-1-0)
shipped on 2026-10-15. We started with coding because it's the work with
the clearest checks: a change passes its tests or it doesn't. The same
front, members, and evidence carry over to other work.

Why general agents can work now, when they stalled before, is in
[The Return of the General Agent](docs/essays/2026-10-01-the-return-of-the-general-agent.md).
The short version is in [Episode 289](docs/transcripts/289.md): "It's a
composable general agent."

## Contents

- [The bigger picture](#the-bigger-picture)
- [Try it](#try-it)
- [Coder](#coder)
- [Alice and the crew](#alice-and-the-crew)
- [Verse and Everglade](#verse-and-everglade)
- [The Gym, plugins, and XP](#the-gym-plugins-and-xp)
- [The phone app and chat](#the-phone-app-and-chat)
- [How OpenAgents makes money](#how-openagents-makes-money)
- [Protocol: Nostr and our NIPs](#protocol-nostr-and-our-nips)
- [Many agents on one machine](#many-agents-on-one-machine)
- [Repository map](#repository-map)
- [Build and test](#build-and-test)
- [Contributing](#contributing)
- [License](#license)

## The bigger picture

OpenAgents is several parts that make one agent. Each part is useful
alone, and they compound together.

| Part | What it is | Status |
| --- | --- | --- |
| **The front** | Typed decisions on every step: which member, which model, which computer, and whether to ask you first. The [Decisions API](docs/api/decisions.md) is Clef first, with Jev as the backup | In production |
| **Members** | Answers and knowledge ([NIP-KB](nips/openagents/NIP-KB.md)), plugins ([plugins](docs/plugins/README.md)), agents such as Coder and the agents it fronts, and computers | Coder 1.0 shipped; plugins and knowledge are live |
| **Evidence** | [The Gym](docs/gym/README.md) measures each member with and without it; adoption into the defaults is a recorded decision. XP credits accepted contributions | Live; contributors at scale aren't proven yet |
| **Routing work** | You describe an outcome; the [router for agentic execution](docs/api/2026-10-02-agentic-execution-router.md) picks an admitted agent or tool, a computer you've granted, and the checks that prove the result, then records what ran and what it cost. Today's paths are [agent assignments](docs/sdk/agents.md) (`/v1/agent-assignments`), asking a saved agent such as Foreman from chat, and Coder runs on your computers | Assignments and local routing are live; the public runs API and paid execution are planned ([status](docs/api/2026-10-02-agentic-execution-router.md#15-what-changed-2026-10-05-to-2026-10-17)) |
| **Computers** | Where work runs: your own Mac or PC, our cloud, and fast sandboxes, behind one lifecycle layer ([computers](docs/cloud/computers.md)) and the public [Computers API](docs/sdk/computers.md) with its Rust client | Your machines and our cloud work, and the Computers API is live; fast sandboxes are in private testing, and paid computers aren't on sale yet |
| **Surfaces** | The website, iOS and Android, the Mac app, the terminal, the [`openagents` command](docs/cli/README.md), and the [API at `api.openagents.com/v1`](https://openagents.com/docs/agents) for other apps and agents | The website is live and Coder is 1.0; mobile and desktop are in testing |
| **Protocol** | Nostr and [our NIPs](nips/openagents/README.md) carry requests, runs, trajectories, evaluations, credit and payments as signed records, so no single app owns the agent | Specified; coverage is tracked in the [coverage report](docs/protocol/2026-09-26-nip-implementation-coverage.md) |
| **Money** | Bitcoin only: Lightning, [x402](nips/openagents/NIP-X402.md) paid calls, and our own LND node ([LND plan](docs/payments/2026-10-15-lnd-sdk-plan.md)) | Paid calls work; our own node is waiting on funding |
| **Verse** | The world where people and agents meet, work and trade, on our own Rust engine ([Verse](#verse-and-everglade)) | Playable on desktop, phone and web |
| **Self-improving work** | Every accepted or rejected change makes the next one cheaper: the outcome loop in [self-improving codebases](docs/product/self-improving-codebases.md) and [outcomes](docs/research/outcomes/README.md) | We run it on this repository; selling it is next |

What we haven't shown yet: independent contributors at scale, gains that
compound across a network, and the composition working well outside
software. [Part V of the essay](docs/essays/2026-10-01-the-return-of-the-general-agent.md#part-v-what-we-have-shown-and-what-we-havent)
keeps that list.

Where it goes next: [the roadmap](docs/roadmap.md), the
[OpenAgents SDK](docs/sdk/README.md) for building on all of this from your
own code, and [verification](docs/verification/README.md) for the parts
that guard money and authority.

## Try it

| Platform | Status |
| --- | --- |
| Web | Chat at [openagents.com](https://openagents.com), and Everglade in the browser at [openagents.com/everglade](https://openagents.com/everglade) (WebGPU or WebGL2). See [Everglade on the web](crates/everglade-web/README.md). |
| Terminal | **Coder 1.0.0** on macOS, Linux and Windows. Install with `curl -fsSL https://openagents.com/cli/install.sh \| bash` (macOS and Linux) or `irm https://openagents.com/cli/install.ps1 \| iex` (Windows PowerShell), then run `coder` in a project folder. See [the download page](https://openagents.com/download) and [the GitHub release](https://github.com/OpenAgentsInc/openagents/releases/tag/coder-v1.0.0). |
| API | `https://api.openagents.com/v1`: models (OpenAI-compatible), decisions, projects, computers, saved agents and their assignments, and agent self-signup. See [the docs for agents](https://openagents.com/docs/agents) and the [Rust SDK](docs/sdk/README.md). |
| iOS | OpenAgents (`com.openagents.app`), the UIKit app over the Rust mobile core. See [OpenAgents for iOS](apps/openagents/apps/ios/README.md). |
| Android | The current mobile core is shared; the Android shell is planned. See [the app guide](apps/openagents/OPENAGENTS.md). |
| Mac | OpenAgents for desktop: chat, Coder on this computer, and phone pairing. See [OpenAgents desktop](crates/openagents-desktop/README.md). |

To join the playtest, read the [playtesting program](docs/game/playtesting.md).
The program is open. Joining earns nothing by itself: XP and titles come only
from accepted contributions, and nothing pays testers.

## Coder

Coder is the first member of OpenAgents, and the one with a 1.0 release.
Coder runs as a terminal and headless program, as a resident host on your
computers, and as the cloud chat worker. It routes work to the agents you
already pay for and measures the result.

| Piece | Where | What it does |
| --- | --- | --- |
| Coder V1 | [`crates/coder-new`](crates/coder-new/README.md) | The current Coder terminal: live chat, plugins (Microcoder, Jev, the `openagents` CLI, ACP subagents, OpenRouter BYOK), model selection, and ATIF session export. `openagents coder` drives the same runtime from scripts. |
| Coder | [`crates/coder`](crates/coder/) | The `coder` terminal and headless turns, durable tasks, the chat router, the host's agent and studio services, and the `coder-worker` relay worker. |
| Microcoder | [`crates/microcoder-loop`](crates/microcoder-loop/), [`crates/microcoder`](crates/microcoder/) | The coding loop: Jev judges the state, one model call returns the next commands, and the host runs them, failing over between providers with capacity. |
| Delegation | [`crates/coder-delegate`](crates/coder-delegate/), [`crates/acp-client`](crates/acp-client/) | Hands work to Claude Code, Codex, [OpenCode](docs/coder/runtime/opencode.md), [Devin](docs/coder/runtime/devin.md), or [Grok Build](docs/coder/runtime/grok.md) over the Agent Client Protocol and native adapters. With none available, a [cloud fallback](docs/coder/runtime/cloud-fallback.md) answers. |
| Coder host | [`crates/coder-host`](crates/coder-host/README.md) | The resident host: device enrollment, reach, terminals, tasks, and the Agent Studio behind one process. |
| `openagents` | [`crates/openagents-cli`](crates/openagents-cli/) | One `--json`-first command for hosts, pairing, tasks, Coder, agents, Verse, plugins, leases, wallets, and sales records. See [the `openagents` command](docs/cli/README.md). |

Steer Coder V1 from a script or another agent:

```sh
openagents coder chat -p "Review the parser" --session parser-review --json
openagents coder delegate microcoder --task "Add a parser regression test" --json
openagents coder export parser-review --output parser-review.atif.json
```

Install this checkout's `coder` and `openagents`, then link a computer:

```sh
./scripts/install-coder.sh   # installs into ~/.openagents/bin; --rollback restores
coder doctor                 # what will run, and which logins it found
```

Then follow [Install Coder](docs/coder/guides/install.md),
[Link your devices](docs/coder/guides/link-devices.md), and
[auto-start](docs/coder/runtime/host-autostart.md). More:
[Coder documentation](docs/coder/README.md),
[Coder V1 research](docs/coder-new/README.md), and the
[delegate door](docs/coder/runtime/delegate-door.md).

## Alice and the crew

**Alice** is the [workshop agent](docs/verse/workshop-agent.md): a persistent
agent you own, with a standing desk in your house in Everglade. You walk up
and talk to her, or reach her with `openagents agent ask alice ...` or `@alice`
in the terminal. The host plans, checks, and journals each request. The
[Alice runbook](docs/verse/alice-runbook.md) shows how to delegate to her,
let her code on autopilot, and supervise her.

- **Her own identity.** Alice has her own Nostr key, attested by the owner's
  key under [NIP-OA](nips/block/NIP-OA.md), and a signed profile. The host can
  rotate, retire, move, and snapshot her.
- **Her own memory.** Scored memories, reflections with checked citations,
  and a consolidated core persist as encrypted [NIP-AE](nips/block/NIP-AE.md)
  engrams, locally and, when the owner turns it on, on relays. She plans her
  day from real work. Her spend is recorded under
  [NIP-AM](nips/block/NIP-AM.md).
- **She steers Coder.** Each request becomes plain prompts to her own Coder V1
  session, which you can follow and take over. Coder asks before any command
  that isn't read-only, and that question waits for your **CONFIRM** or
  **REJECT** at her lectern. `openagents agent engine alice codex` has Coder
  delegate the coding to Codex and check the result itself.

See [agent identity and engrams](docs/verse/agent-identity-and-engrams.md).
[The crew](docs/verse/crew.md) extends the same machinery to a cast of named
agents after the cryptography cast. Bob, the town builder, runs on it today;
most other members are proposed. The sales roles are presets with a
drafting-only charter; see [How OpenAgents makes money](#how-openagents-makes-money).

The [Agent Studio](docs/verse/agent-studio.md) runs a team of coding agents on
a repository (`openagents studio up`), each at a station in Everglade's
workshop: at a desk while it writes, at the workbench while it runs commands,
and at the podium when it needs your answer.

## Verse and Everglade

Verse is one Rust program ([`crates/verse`](crates/verse/)) on our own engine
(`wgpu`, `winit`, and a custom renderer). The same world runtime runs on
desktop, in the phone apps, and in the browser through WebAssembly. Players see
each other over Nostr ([NIP-MV](nips/openagents/NIP-MV.md)).

| Place | What you do there | Open it |
| --- | --- | --- |
| **The Grid** | The shared plaza: other players, chat, portals to the zones, and the Gym with its RESULTS and EVALS boards. | `verse` |
| **Everglade** | The town where your agents work. See below. | `verse --everglade`, or [the web](https://openagents.com/everglade) |
| **The Grove** | A druid's training field with dummies to test spells on. | `verse --grove` |
| **Water Lab** | A cove with a sea, a river, a waterfall, floating bodies, and water spells. | `verse --water-lab` |
| **Demolition yard** | Knock down cottages with a sledgehammer and Meteor Swarm. | `verse --demolition` |
| **Crypt lab** | A candlelit laboratory hall. | `verse --crypt` |
| **Lagrange 1, Physics Lab** | An EVA construction station at the Sun–Earth L1 point, and live physics mechanisms. | Portals on the Grid |

The Ruins zone was removed on October 5, 2026. See
[zones](docs/verse/zones.md) and [zone rules](docs/verse/zone-rules.md).

### Everglade

[Everglade](docs/verse/everglade.md) is a town in a forest glade, grown toward
[its city map](docs/verse/everglade-map.png).

- **A living town.** The [town clock](crates/town-clock/) runs a compressed day
  by default, with districts and times of day. [Townsfolk](crates/townsfolk/)
  follow deterministic routines, talk, and spread rumors. A generated
  [world tree](crates/world-tree/) names every place. See
  [generative agents](docs/verse/generative-agents.md).
- **Workplaces.** The workshop hall holds the Agent Studio; Alice works in the
  owner's house on Library Way. Buildings in the [Greco-futurism](docs/verse/greco-futurism.md)
  style include the owner's house, the belvedere, and the Agora, the sales
  floor's trading hall.
- **Water.** Ponds and Glade Run are swimmable, with breath. The
  [water system](docs/verse/water.md) has shipped phases W1 to W6: physics
  coupling, shaders per tier, gameplay, refraction and reflection, and
  ripples, wakes, splashes, debris, and rowboats.
- **Spells.** The hotbar holds movement spells only: Levitate, Feather Fall,
  Wind Wall, Reverse Gravity, and Wall of Stone. Combat elsewhere follows the
  [combat model](docs/verse/combat-model.md).
- **Destruction.** The town's buildings can break, but no player spell does it
  in Everglade. A local build with the `dev-destruction` feature puts Meteor
  Swarm and the sledgehammer back for testing; production, web, and phone
  builds can't enable it. See [destructible buildings](docs/verse/destructible-buildings.md).
- **In progress.** The owner approved a
  [rebuild on a modular medieval town kit](docs/verse/everglade-medieval-refactor.md)
  ([#10903](https://github.com/OpenAgentsInc/openagents/issues/10903)). The
  kit's assets stay out of git and ship through the
  [private asset pipeline](docs/verse/private-assets.md).

```sh
cargo run --release -p verse -- --everglade   # straight into Everglade
scripts/build-everglade-web.sh OUT            # the browser build
```

More: the [Verse documentation](docs/verse/README.md) and
[the engine](docs/verse/engine/architecture.md).

## The Gym, plugins, and XP

```
  chat with OpenAgents --> pick or make a plugin and its tests --> run them
        ^                                                            |
        |                                                            v
  earn XP when others <-- add the result <-- see the change: tests passed
  check it or Coder       to the Gym         with and without the plugin
  adopts the plugin
```

A plugin is anything you add to Coder: skills, workflows, knowledge, Wasm, and
tests ([plugins](docs/plugins/README.md)). A test set measures one plugin's
effect: `openagents plugin test` ([extension evaluation](docs/extensions/evaluation.md))
runs it with and without the plugin, and the
[hosted runner](docs/deployment/eval-runner.md) runs it for the phone. Other
trainers check published results by running the same tests. The
[first live results](docs/extensions/measurements/2026-09-29-hosted-runner-live.md)
record the change for three plugins.

- **Gym.** [`crates/gym`](crates/gym/) holds pinned suites, receipt-chained
  results, and acceptance gates. See the [Gym index](docs/gym/README.md).
- **Leaderboard.** [`crates/gym-leaderboard`](crates/gym-leaderboard/)
  publishes Terminal-Bench results, recomputed from committed evidence, that the Grid's RESULTS board
  shows. See the [Gym leaderboard](docs/verse/gym-leaderboard.md) and the
  [Terminal-Bench index](docs/terminal-bench/README.md).
- **XP.** [NIP-XP](nips/openagents/NIP-XP.md) records an accepted,
  evidence-backed outcome. XP can't be spent or transferred, and it is never
  money. See [refereeing quests](docs/coder/guides/xp.md) and
  [trainer leveling](docs/verse/agent-trainer-leveling.md).

## The phone app and chat

The OpenAgents mobile app lives in [`apps/openagents`](apps/openagents/OPENAGENTS.md).
Its UIKit shell uses the Rust mobile core, with sessions shared through
`coder-sync`. See [the iOS guide](apps/openagents/apps/ios/README.md) for builds
and [the release guide](apps/openagents/apps/ios/release/README.md) for TestFlight.
The previous SwiftUI and Android app was removed in #11901.

## How OpenAgents makes money

Status: a plan with growing foundations. No customer offer is active yet, and
nothing below pays out today. The [sales plan](docs/sales/README.md) and the
[revenue roadmap](docs/sales/revenue-roadmap.md) own the details.

Coder is free to use on your own computer with your own subscriptions. Pro
($20 a month) is for running on our cloud and isn't on sale yet
([pricing](https://openagents.com/docs/pricing)). Beyond that, OpenAgents
earns on the paid resources and services it adds, priced by usage, with no
seat bundles or multi-year contracts:

- **Paid plugins and paid calls** over [x402](nips/openagents/NIP-X402.md).
  Paying plugin authors a declared per-call fee is designed
  ([split contract](docs/payments/2026-10-02-central-receive-and-splits.md))
  but not built; we don't pay plugin authors today.
- **Cloud computers** for work your machine is too busy for, metered in sats
  ([retail cloud](docs/cloud/retail-contract.md)).
- **Decision access** through the keyed [gateway](docs/decision-models/service/gateway.md).
- **Assisted pilots and business accounts.** The
  [first workflow offer](docs/sales/README.md#first-workflow-offer-v1) is one
  checked change to a public repository, delivered with Coder. Its price is
  proposed and waits for the owner's approval.

The roadmap's order is to prove an offer, collect the first payment, earn
repeat use, then grow through referrals, partners, and teams.

### How you can earn

- **Affiliates.** A share of the Pro revenue from the accounts you refer,
  with named attribution, is built (#11698). It isn't open yet; its terms
  are published when it opens.
- **Partners** fulfill parts of a buyer's order, such as work we don't do
  ourselves, with referrals in both directions. The private sales pipeline
  records accepted partner assignments.
- **The coding agent pool.** We plan to pay contributors for accepted work
  that customers need, with explicit checks and receipts.

### Selling in public, and in Everglade

We built in public, and we sell in public: launches, results, and lessons, in
the same channels where we show the product. Our own agents do much of the
sales work. The [agent sales floor](docs/sales/agent-sales-floor.md) plans a
sales leader, Paul, and a few hires who train against simulated buyers and
Gym suites before they sell. They work at standing desks in the Agora in
Everglade, which is built. Every hire, message, price, and agreement waits for
the owner's approval, and agents disclose that they are AI.

Accounts, workspaces, roles, sessions, sandbox billing, team budgets, and
reviewed commercial attribution live in [`crates/tenancy`](crates/tenancy/),
[`crates/gateway`](crates/gateway/), and
[`crates/commercial-accounts`](crates/commercial-accounts/). Owner-only steps,
such as live payment runs, are tracked in [NEEDS_OWNER.md](NEEDS_OWNER.md).

## Protocol: Nostr and our NIPs

Agents, phones, and computers talk over Nostr. We run
`wss://relay.openagents.com` on [`crates/nostr-relay`](crates/nostr-relay/)
(one binary, one Postgres), with protocol primitives in
[`crates/nostr`](crates/nostr/). [`nips/`](nips/README.md) holds three lanes:
[official NIPs](nips/official/), [Block's agent NIPs](nips/block/README.md)
(including NIP-OA, NIP-AE, and NIP-AM), and
[the NIPs we author](nips/openagents/README.md).

| NIP | Covers |
| --- | --- |
| [NIP-CJ](nips/openagents/NIP-CJ.md) | Conversation jobs: the phone's chat with the chat worker. |
| [NIP-HOST](nips/openagents/NIP-HOST.md) | Host enrollment, grants, tasks, and agents on your computers. |
| [NIP-SESS](nips/openagents/NIP-SESS.md) | Sessions and the history observer profile. |
| [NIP-ATIF](nips/openagents/NIP-ATIF.md) | Agent trajectories. [`crates/atif`](crates/atif/) writes `ATIF-v1.8`. |
| [NIP-KB](nips/openagents/NIP-KB.md), [NIP-XP](nips/openagents/NIP-XP.md) | Shared knowledge entries, and the XP ledger. |
| [NIP-MV](nips/openagents/NIP-MV.md) | Verse presence and movement. |
| [NIP-X402](nips/openagents/NIP-X402.md) | Lightning payments for paid HTTP and Nostr calls. |

A specification alone doesn't mean the feature is built. The
[coverage report](docs/protocol/2026-09-26-nip-implementation-coverage.md)
maps each contract to what exists.

## Many agents on one machine

Many agents build here at once, so the host brokers shared resources
([Many agents on one machine](docs/coder/design/many-agents-one-machine.md)):

- **[Leases](docs/coder/runtime/leases.md)** (`openagents lease`) give out
  build slots, memory, disk, the quiet machine, the screen, and the GPU in
  turn.
- **[Placement](docs/coder/runtime/placement.md)** sends release gates,
  benchmarks, and builds to another computer by class.
- **[The artifact queue](docs/coder/runtime/artifact-queue.md)**
  (`openagents artifact`) lands changes to single-digest artifacts, such as
  the Everglade pack, one at a time.
- **[Scratch](docs/coder/guides/scratch.md)** and
  **[browser checks](docs/coder/guides/browser.md)** give each session its own
  durable directory and its own Chrome profile and port.
- **[The capacity book](docs/coder/runtime/capacity.md)** records provider
  usage limits so agents wait for a reset instead of failing.

## Repository map

AGENTS.md has the full crate list. The main groups:

| Area | Paths |
| --- | --- |
| Apps | [`apps/openagents`](apps/openagents/OPENAGENTS.md) (the current desktop and mobile apps, in their own Cargo workspace), [`crates/openagents-desktop`](crates/openagents-desktop/README.md), [`crates/openagents-terminal`](crates/openagents-terminal/), [`crates/openagents-web`](crates/openagents-web/README.md), and the earlier Coder apps in [`bins/coder-ios`](bins/coder-ios/) and [`bins/coder-android`](bins/coder-android/README.md) |
| Coder | [`coder-new`](crates/coder-new/README.md), [`coder`](crates/coder/), [`microcoder-loop`](crates/microcoder-loop/), [`microcoder`](crates/microcoder/), [`coder-delegate`](crates/coder-delegate/), [`acp-client`](crates/acp-client/), [`codex-transport`](crates/codex-transport/), [`coder-one`](crates/coder-one/), and [`microluna`](crates/microluna/README.md) (deprecated; kept for retained benchmark evidence) |
| Host and devices | [`coder-host`](crates/coder-host/README.md), [`coder-service`](crates/coder-service/), [`coder-access`](crates/coder-access/README.md), [`coder-reach`](crates/coder-reach/README.md), [`coder-link`](crates/coder-link/README.md), [`coder-pty`](crates/coder-pty/README.md), [`coder-vt`](crates/coder-vt/README.md), [`coder-ssh`](crates/coder-ssh/README.md), [`coder-connect`](crates/coder-connect/README.md), and [`coder-computers`](crates/coder-computers/README.md) |
| Machine | [`coder-lease`](crates/coder-lease/), [`supervise`](crates/supervise/), [`coder-boundary`](crates/coder-boundary/), and the CoderOS desktop in [`os/`](os/README.md) with [`coder-compositor`](crates/coder-compositor/README.md) |
| Verse | [`verse`](crates/verse/), its zone crates (`crates/verse-zone-*`), [`verse-core`](crates/verse-core/), [`verse-pbr`](crates/verse-pbr/), [`verse-net`](crates/verse-net/), [`physics`](crates/physics/), [`town-clock`](crates/town-clock/), [`townsfolk`](crates/townsfolk/), [`world-tree`](crates/world-tree/), [`memory-stream`](crates/memory-stream/), [`verse-private`](crates/verse-private/), and [`everglade-web`](crates/everglade-web/README.md) |
| Decisions and measurement | [`jev`](crates/jev/README.md), [`oak`](crates/oak/), [`kev`](crates/kev/), [`laya`](crates/laya/), [`lev`](crates/lev/), [`gym`](crates/gym/), [`gym-leaderboard`](crates/gym-leaderboard/), [`ext-eval`](crates/ext-eval/), [`eval-runner`](crates/eval-runner/), [`plugin`](crates/plugin/) and the `plugin-*` guests, [`knowledge`](crates/knowledge/), and [`atif`](crates/atif/) |
| Accounts and money | [`gateway`](crates/gateway/), [`tenancy`](crates/tenancy/), [`commercial-accounts`](crates/commercial-accounts/), [`receipts`](crates/receipts/), [`pay-ledger`](crates/pay-ledger/), [`retail-cloud`](crates/retail-cloud/), [`retail-service`](crates/retail-service/), [`wallet`](crates/wallet/), [`spark-wallet`](crates/spark-wallet/), and [`x402`](crates/x402/) |
| Protocol | [`nostr`](crates/nostr/), [`nostr-relay`](crates/nostr-relay/), [`nostr-transport`](crates/nostr-transport/), [`push-gateway`](crates/push-gateway/), and [`nips/`](nips/README.md) |
| Docs and data | [`docs/`](docs/README.md) (start with the [index](docs/README.md)), [`knowledge/`](knowledge/), [`bench/`](bench/), [`deploy/`](deploy/README.md), [`capabilities/`](capabilities/), [`programs/`](programs/), [`questions/`](questions/), [`sources/`](sources/), [`methods/`](methods/), [`artifacts/`](artifacts/), and [`scripts/`](scripts/) |

## Build and test

Use the toolchain pinned in [`rust-toolchain.toml`](rust-toolchain.toml), and
run heavy Cargo commands through the machine's build lease so builds take
turns:

```sh
openagents lease build --keep-target-dir -- cargo test -p coder-new
openagents lease build --keep-target-dir -- cargo run -p coder-new        # Coder V1
openagents lease build --keep-target-dir -- cargo run -p coder --bin coder -- -p "count the crates"
```

If your `openagents` has no `lease` command, build it once with
`cargo build -p openagents-cli --bin openagents`. The default check for a
change is `cargo test -p` for the crates you edited, plus `cargo fmt`.
`./scripts/verify-rust.sh --crates coder,gym` runs formatting, Clippy, and
tests for chosen packages.

The current desktop and mobile apps have their own workspace. See
[`apps/openagents/OPENAGENTS.md`](apps/openagents/OPENAGENTS.md) for their checks.

The full gate, `./scripts/verify-rust.sh --release`, is for releases only. See
[verification](docs/verification.md). We use no GitHub workflows; checks run
on contributor machines.

## Contributing

- Read [AGENTS.md](AGENTS.md), the contributor contract. Product code is Rust,
  and prose follows the Google developer documentation style.
- Read [INVARIANTS.md](INVARIANTS.md) before you change an invariant-bearing
  surface.
- Look up terms in the [glossary](docs/glossary.md), which marks what's
  implemented, partial, or proposed.
- Claim an issue before you work it (`openagents issue claim N`), and keep the
  [project board](docs/project-board.md) current.
- File bugs and ideas as [GitHub issues](https://github.com/OpenAgentsInc/openagents/issues).
  Playtesters can use the in-app **Report a problem**.
- See the [master roadmap](docs/roadmap.md) for direction.

## License

[Apache License 2.0](LICENSE). See the
[dependency and provenance policy](docs/dependencies.md) for third-party
requirements.
