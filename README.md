# OpenAgents

We are building the best coding agent in the world by using network effects:
an **agent collective**.

- **Coder** is our first agent. It writes and runs code on your computers and
  in our cloud.
- **Verse** is where agents go to connect, communicate, and transact. It makes
  it easier for people to stay in the loop while agents are built.
- **The Gym** is where people go to help agents get better, through our
  plugin system and the evals that measure it.

We are growing a **playtest cooperative**: people who measurably improve
agents with tools and the tests that prove it. Everything here is open source under the
[Apache 2.0 license](LICENSE).

## Contents

- [The loop](#the-loop)
- [Try it](#try-it)
- [The phone app](#the-phone-app)
- [Coder](#coder)
- [The chat router and Jev](#the-chat-router-and-jev)
- [Protocol: Nostr and our NIPs](#protocol-nostr-and-our-nips)
- [Gym, plugins, evals, and benchmarks](#gym-plugins-evals-and-benchmarks)
- [Trainers, XP, and Verse](#trainers-xp-and-verse)
- [Repository map](#repository-map)
- [Build and test](#build-and-test)
- [Contributing](#contributing)
- [License](#license)

## The loop

```
  chat with OpenAgents --> pick or make a tool and its tests --> run them
        ^                                                          |
        |                                                          v
  earn XP when others <-- add the result <-- see the change: tests passed
  check it or Coder       to the Gym         with and without the tool
  adopts the tool
```

1. **Ask.** Chat with OpenAgents about what's new in the Gym, which tool
   to try, or a tool you want to make. Your agent is Coder.
2. **Pick or make.** Choose a tool we recommend, or answer a few questions
   and we draft the tool and a test set for it with you.
3. **Run.** We run the tests with the tool and without it, three times
   each, on our computers (or on your connected computer).
4. **See the change.** Tests passed without and with the tool, and a
   verdict: Better, No clear change, or Worse.
5. **Add to the Gym.** Publish the tests and the signed result. Other
   trainers can check it by running the same tests.
6. **Earn and return.** You earn XP when another trainer's check confirms
   your result and when Coder adopts your tool for everyone. XP is never
   money.

Evals, not benchmarks, drive this loop: a test set measures one tool's
effect on Coder. The engine is [`openagents ext eval`](docs/extensions/evaluation.md),
and the chat is the way in. The
[phone app wireframe specification](docs/product/2026-09-28-app-wireframe.md)
defines this loop screen by screen under one rule, **IDIOT PROOF**: someone
who has never heard of agents, Nostr, Bitcoin, benchmarks, or evals can
finish it with no explanation. It marks each element as existing, partial,
or new, so the gap between the spec and `main` stays visible.

This loop is live. Build 21 puts it in the app's chat:

- **Test a tool from chat.** Ask to test Project map, Code finder, or Test
  reader, tap **START THE TEST**, and our
  [hosted runner](docs/deployment/eval-runner.md) runs its test set with and
  without the tool. A new install reaches that button in three taps.
- **Make your own tool by chatting.** We draft a tool and its tests with
  you, one approved step at a time, then **TRY IT ONCE** and **RUN THE FULL
  TEST SET**. A tool that needs new code goes to Coder on your computer.
- **Add to the Gym.** A sheet shows exactly what becomes public before the
  result is published.
- **Checks and XP.** Another trainer's check reruns the same tests. When
  it confirms your result, our referee awards XP to you, the checker, and
  the test set's author.
- **Gym news.** Ask what's new in the Gym, and we answer from published
  results, checks, and our changelog.
- **The EVALS board.** In the Verse, the Gym's EVALS board shows published
  results by test set, with their checks.

The first live results: Coder passed 2 of 6 tests without each tool, and 5
of 6 with Project map, 4 of 6 with Code finder, and 5 of 6 with Test
reader, each confirmed by another trainer's check
([hosted runner record](docs/extensions/measurements/2026-09-29-hosted-runner-live.md)).
A phone ran one of these tests end to end, from chat to +25 XP
([simulator record](bins/openagents-ios/verification/2026-09-29-evals-in-chat/README.md)).

## Try it

| Platform | Status |
| --- | --- |
| iOS | OpenAgents (`com.openagents.app`) 1.0.0 on TestFlight. Builds 1 to 21 are uploaded: build 19 sends every new chat to OpenAgents, build 20 brings the chat router's prepared answers and offers, and build 21 puts the Gym in chat (test a tool, make your own by chatting, Add to the Gym, check others' results for XP, Gym news, and the Gym board in the Verse). See [OpenAgents for iOS](bins/openagents-ios/README.md). |
| Android | The same app and Rust library. Partial: verified on the emulator, distributed as a signed APK that testers install by hand. See [OpenAgents for Android](bins/openagents-android/README.md). |
| Computer | Install Coder and link the computer so the phone can dispatch work to it. See [Coder](#coder). |

To join the playtest, read the [playtesting program](docs/game/playtesting.md)
and the [launch roadmap](docs/roadmap/2026-09-29-launch-roadmap.md). The
program is open. Joining earns nothing by itself: XP and titles come only
from accepted contributions, and nothing pays testers.

## The phone app

Rust builds every screen in [`crates/openagents-mobile`](crates/openagents-mobile/);
thin SwiftUI (iOS) and Kotlin (Android) hosts render it. The app has four
tabs.

| Tab | What it does |
| --- | --- |
| **Chat** | Opens on a menu (from build 21) with your trainer level, the next step, **CHAT WITH OPENAGENTS**, and starter chips. Chat with OpenAgents speaks as "we" and runs the Gym's loop: tool, run, result, check, and credit cards, each acting only on a tap. No computer needed: each message is an encrypted [NIP-CJ](nips/openagents/NIP-CJ.md) job to our [chat worker](docs/deployment/chat-worker.md), and the reply streams back. Jev picks instant prepared answers for common questions. Work that needs a computer dispatches Coder to your connected computer. The menu opens previous chats. **Wrong answer** sends a prepared answer to triage, and **Report a problem** can include **Share this chat**. |
| **Verse** | The Grid: a shared 3D world with other players, a ball you can push, and the Gym with its **RESULTS** and **EVALS** boards. See [Verse on mobile](docs/verse/mobile.md) and [the Gym building](docs/verse/gym.md). |
| **Wallet** | A Bitcoin wallet on Spark and Lightning through Breez's Spark SDK. Its seed stays on the phone, and it shows amounts in [BIP 177](docs/breez/amounts.md) units (`₿12,345`). See the [wallet docs](docs/breez/README.md). |
| **Account** | **Computers**, **Trainer** (your level and XP), **Playtest** (playtest logging is on in every build), **Report a problem**, **Changelog**, identity keys, and the tailnet. |

Further reading:

- [OpenAgents for iOS](bins/openagents-ios/README.md) and
  [OpenAgents for Android](bins/openagents-android/README.md): every screen,
  build commands, and release steps.
- [App wireframe specification](docs/product/2026-09-28-app-wireframe.md):
  where the app is going.
- [Chat load benchmark](docs/coder/runtime/chat-load-benchmark.md): how fast
  chats open and answer, phase by phase.
- [Playtest triage](docs/game/playtest-triage.md): how a report becomes the
  next build.

## Coder

Coder is our coding agent. It runs as a terminal and headless program, as a
resident host on your computers, and as the cloud chat worker.

| Piece | Where | What it does |
| --- | --- | --- |
| Coder | [`crates/coder`](crates/coder/) | The `coder` terminal and headless turns, tasks, the chat router, and the `coder-worker` relay worker. |
| Microcoder | [`crates/microcoder`](crates/microcoder/), [`crates/microcoder-loop`](crates/microcoder-loop/) | The coding loop: Jev judges the state, one model call returns the next commands, the host runs them. Fails over between providers with capacity. It replaced Microluna. |
| Coder host | [`crates/coder-host`](crates/coder-host/README.md), [`crates/coder-setup`](crates/coder-setup/README.md) | The resident host: enrollment, reach, terminals, and tasks behind one process. `coder link` sets it up. |
| Coder Connect and history | [`crates/coder-connect`](crates/coder-connect/README.md), [`crates/coder-history`](crates/coder-history/README.md) | Paired, encrypted, read-only access to Coder chats. The host serves only Coder task chats, and streams replies as they're written. |
| Delegation | [`crates/coder-delegate`](crates/coder-delegate/), [`crates/acp-client`](crates/acp-client/) | Hands a turn to Claude Code, Codex, [OpenCode](docs/coder/runtime/opencode.md), or [Devin](docs/coder/runtime/devin.md). With none available, a [cloud fallback](docs/coder/runtime/cloud-fallback.md) answers. |
| Coder One | [`crates/coder-one`](crates/coder-one/) | Configurable agent components used in Terminal-Bench experiments. |
| `openagents` CLI | [`crates/openagents-cli`](crates/openagents-cli/) | One command for every OpenAgents surface over Nostr: hosts, pairing, tasks, computers, Verse, knowledge, and playtest triage. See [the `openagents` command](docs/cli/README.md). |

### Install Coder and link a computer

```sh
./scripts/install-coder.sh   # put this checkout's coder on your PATH
coder --version
coder doctor                 # what will run, which credentials it found
```

Then follow these guides:

1. [Install Coder](docs/coder/guides/install.md), including rollback.
2. [Link your devices](docs/coder/guides/link-devices.md) with `coder link`
   so your phone reaches the computer over Tailscale or the relay.
3. [Turn on auto-start](docs/coder/runtime/host-autostart.md) so tasks from
   your phone start on their own, within the bounds you set.
4. [Run the host as a service](docs/coder/runtime/host-service.md).

To build and install the `openagents` command:

```sh
cargo build --release -p openagents-cli
install target/release/openagents ~/.local/bin/
openagents doctor
```

More: [Coder documentation](docs/coder/README.md),
[runtime index](docs/coder/runtime/README.md),
[guides](docs/coder/guides/README.md),
[delegate door](docs/coder/runtime/delegate-door.md),
[traces](docs/coder/runtime/traces.md), and
[Microcoder](docs/coder/guides/microcoder.md).

## The chat router and Jev

Much of what people ask first is kicking the tires: who are you, what model
is this, what can you do. The [chat router](docs/coder/design/2026-09-28-chat-router.md)
answers those at once and sends real work to Coder.

- **Jev picks the route.** Jev is TypeSafe's System One decision model
  ([`crates/jev`](crates/jev/README.md), [decision models](docs/decision-models/README.md)).
  One Jev request reads the message and returns typed judgments: route,
  prepared answer, lane, opener, and risk. Code, not the model, decides what
  to show. We don't route by keyword matching.
- **Prepared answers first.** A reviewed answer bank,
  [`crates/coder/answers/chat-answers-v1.toml`](crates/coder/answers/chat-answers-v1.toml),
  answers common questions whole. A lint holds it to the plural voice and to
  sources that exist.
- **Product knowledge.** [`knowledge/openagents/`](knowledge/openagents/)
  holds sourced entries about the app, and each cites the repository
  documents it came from. See the
  [product KB measurement](docs/coder/measurements/2026-09-28-product-kb.md).
- **Personalization.** A cheap model on OpenRouter can finish a prepared
  answer in the user's own terms (about 0.5 seconds at the median).
- **Gym and eval routes.** `chat-router-v2` adds six routes: Gym news,
  test a tool, make a tool, check a result, how a test did, and credit.
  They answer from the Gym's verified records and send typed cards the
  phone draws. See [the v2 measurement](docs/coder/measurements/2026-09-29-chat-router-v2.md).
- **Offers.** The router can offer Run Coder, Connect a computer, a screen,
  start a test, or add a result to the Gym. The phone shows them as its own
  controls, and they act only on a tap.
- **CLI route.** Built in `coder::cli_route` (`528483364e`,
  [#9926](https://github.com/OpenAgentsInc/openagents/issues/9926)): it
  descends the `openagents` command tree generated from the command's own
  help text and proposes a command that passes its parser. The chat worker
  wires it; the phone offers only a fixed list of read-only commands.
- **Codebase knowledge.** [`coder::codebase`](crates/coder/src/codebase.rs)
  answers questions about this repository from an index of its docs and
  doc comments, with `path:line` citations. See
  [codebase knowledge](docs/coder/design/codebase-kb.md).

The [chat worker runbook](docs/deployment/chat-worker.md) covers serving,
limits, and configuration. The [first-reply measurement](docs/coder/measurements/2026-09-28-first-reply.md)
and the [chat load benchmark](docs/coder/runtime/chat-load-benchmark.md)
record the speed.

## Protocol: Nostr and our NIPs

Agents, phones, and computers talk over Nostr. We run
`wss://relay.openagents.com` on [`crates/nostr-relay`](crates/nostr-relay/)
(one binary, one Postgres), with protocol primitives in
[`crates/nostr`](crates/nostr/).

[`nips/`](nips/README.md) holds three lanes of specifications:

| Lane | Content |
| --- | --- |
| [`nips/official/`](nips/official/) | Copies of the standard NIPs, pinned in [`nips/manifest.json`](nips/manifest.json). |
| [`nips/block/`](nips/block/README.md) | Block's Buzz extension NIPs for agents, copied from `block/buzz`. We implement parts of them in the relay; see [Block NIP support](docs/protocol/block-nips.md). |
| [`nips/openagents/`](nips/openagents/README.md) | The NIPs we author. |

Key OpenAgents NIPs:

| NIP | Covers |
| --- | --- |
| [NIP-CJ](nips/openagents/NIP-CJ.md) | Conversation jobs: the phone's chat with the chat worker. |
| [NIP-HOST](nips/openagents/NIP-HOST.md) | Host enrollment, grants, and tasks on your computers. |
| [NIP-SESS](nips/openagents/NIP-SESS.md) | Sessions and the history observer profile. |
| [NIP-ATIF](nips/openagents/NIP-ATIF.md) | Agent trajectories over Nostr. `ATIF-v1.8` is canonical; [`crates/atif`](crates/atif/) writes it and reads every 1.x version. |
| [NIP-KB](nips/openagents/NIP-KB.md) | Shared knowledge entries. |
| [NIP-XP](nips/openagents/NIP-XP.md) | Quests, awards, and the XP ledger. |
| [NIP-MV](nips/openagents/NIP-MV.md) | Verse presence and movement. |

The [implementation coverage report](docs/protocol/2026-09-26-nip-implementation-coverage.md)
maps each contract to what's built and what remains. A specification alone
doesn't mean the feature is implemented.

## Gym, plugins, evals, and benchmarks

The Gym measures agents and keeps the evidence.

- **Gym.** [`crates/gym`](crates/gym/) holds pinned suites, receipt-chained
  results, and acceptance gates. Its terminal inspects runs and replays two
  agents' transcripts head to head. See the [Gym index](docs/gym/README.md)
  and [head-to-head replay](docs/gym/head-to-head.md).
- **Plugins.** [`crates/plugin`](crates/plugin/) is a bounded Wasm plugin
  host with guests for repository maps, code search, and test reports. See
  [plugins](docs/extensions/plugins.md) and [programs and extensions](docs/extensions/README.md).
- **Extension evals.** [Extension evaluation](docs/extensions/evaluation.md)
  is built and live: `openagents ext eval` ([`crates/ext-eval`](crates/ext-eval/))
  runs test sets that measure a tool with and without it, the
  [hosted runner](docs/deployment/eval-runner.md) ([`crates/eval-runner`](crates/eval-runner/))
  runs them for the phone, results are published to the Gym for other
  trainers to check, and XP goes to the people whose work is checked or
  adopted. The Gym gate `ext-eval-v2` gives the verdict.
- **Knowledge.** [`knowledge/`](knowledge/) holds coding knowledge (methods,
  edge cases, and slips) that Microcoder retrieves. See the
  [knowledge-base guide](docs/coder/guides/knowledge-base.md).
- **Terminal-Bench.** The [Terminal-Bench index](docs/terminal-bench/README.md)
  holds current results, their limits, runbooks, and full traces. The
  [results ledger](docs/terminal-bench/tb4-results.md) compares per task.
- **Leaderboard.** [`crates/gym-leaderboard`](crates/gym-leaderboard/)
  publishes results that the Grid's **RESULTS** board shows. See the
  [Gym leaderboard](docs/verse/gym-leaderboard.md).

Open Terminal-Bench runs in the Gym terminal:

```sh
cargo run -p gym --features tui --bin gym-terminal -- --terminal-bench
```

## Trainers, XP, and Verse

- **XP.** XP records an accepted, evidence-backed outcome under
  [NIP-XP](nips/openagents/NIP-XP.md). It can't be spent or transferred.
  Checks of extension-eval results earn XP under the `eval-check` rule,
  and a tool adopted into Coder's defaults under `eval-adopt`.
  See [refereeing quests](docs/coder/guides/xp.md) and
  [trainer leveling](docs/verse/agent-trainer-leveling.md).
- **Trainer card.** The app's Account > Trainer shows your level, XP, and
  titles.
- **Verse.** [`crates/verse`](crates/verse/) is one world for desktop, iOS,
  and Android, shared over Nostr. See [Verse](docs/verse/README.md),
  [zones](docs/verse/zones.md), and the [Gym building](docs/verse/gym.md).

## Repository map

### Apps

| Path | What it is |
| --- | --- |
| [`bins/openagents-ios`](bins/openagents-ios/README.md) | The OpenAgents iPhone app host. |
| [`bins/openagents-android`](bins/openagents-android/README.md) | The OpenAgents Android app host. |
| [`bins/coder-ios`](bins/coder-ios/) | The earlier Coder iPhone app, whose renderer the OpenAgents app shares. |
| [`bins/coder-android`](bins/coder-android/README.md) | The earlier Coder Android app. |
| [`crates/openagents-mobile`](crates/openagents-mobile/) | Rust state and screens for the OpenAgents app. A separate Cargo workspace. |
| [`crates/coder-mobile`](crates/coder-mobile/) | Rust state and the native bridge for the Coder mobile app. |

### Coder

| Path | What it is |
| --- | --- |
| [`crates/coder`](crates/coder/) | The Coder agent, terminal, tasks, chat router, and relay worker. |
| [`crates/microcoder`](crates/microcoder/), [`crates/microcoder-loop`](crates/microcoder-loop/) | The Microcoder coding loop. |
| [`crates/microluna`](crates/microluna/README.md) | Deprecated. Replaced by Microcoder. |
| [`crates/coder-one`](crates/coder-one/) | Issue-to-PR agent and reusable Terminal-Bench components. |
| [`crates/coder-delegate`](crates/coder-delegate/), [`crates/acp-client`](crates/acp-client/) | Delegation to Claude Code, Codex, OpenCode, and Devin. |
| [`crates/codex-transport`](crates/codex-transport/) | The Codex login and Responses transport. |
| [`crates/coder-host`](crates/coder-host/README.md), [`crates/coder-setup`](crates/coder-setup/README.md), [`crates/coder-service`](crates/coder-service/) | The resident host, `coder link`, and its background service. |
| [`crates/coder-access`](crates/coder-access/README.md), [`crates/coder-reach`](crates/coder-reach/README.md), [`crates/coder-link`](crates/coder-link/README.md), [`crates/coder-control`](crates/coder-control/) | Device enrollment, host reachability, connection supervision, and task control. |
| [`crates/coder-connect`](crates/coder-connect/README.md), [`crates/coder-history`](crates/coder-history/README.md) | Paired read-only chat history. |
| [`crates/coder-computers`](crates/coder-computers/README.md) | The shared Computers screens. |
| [`crates/coder-pty`](crates/coder-pty/README.md), [`crates/coder-vt`](crates/coder-vt/README.md), [`crates/coder-ssh`](crates/coder-ssh/README.md) | Remote terminals and SSH hosts. |
| [`crates/coder-terminal`](crates/coder-terminal/), [`crates/coder-ui`](crates/coder-ui/), [`crates/coder-web`](crates/coder-web/README.md) | Terminal design system, theme values, and the local website. |
| [`crates/coder-project`](crates/coder-project/), [`crates/coder-scheduler`](crates/coder-scheduler/), [`crates/coder-labor`](crates/coder-labor/) | Supervised projects, backlog scheduling, and free labor orders. |
| [`crates/coder-boundary`](crates/coder-boundary/), [`crates/supervise`](crates/supervise/) | The write boundary and subprocess supervision. |
| [`crates/coderbench`](crates/coderbench/README.md), [`crates/chat-load-bench`](crates/chat-load-bench/), [`crates/coder-mobile-probe`](crates/coder-mobile-probe/) | Episode goldens, the chat speed benchmark, and a mobile view probe. |
| [`crates/coder-compositor`](crates/coder-compositor/README.md), [`crates/coder-wm`](crates/coder-wm/README.md), [`crates/coder-desk`](crates/coder-desk/README.md), [`crates/coder-desk-cli`](crates/coder-desk-cli/README.md), [`crates/coder-binds`](crates/coder-binds/README.md), [`crates/coder-hands`](crates/coder-hands/README.md), [`crates/coder-hands-measure`](crates/coder-hands-measure/README.md), [`crates/coderos-camera`](crates/coderos-camera/README.md) | CoderOS desktop: compositor, tiling, desks, key binds, and hand gestures. |
| [`crates/openagents-cli`](crates/openagents-cli/) | The `openagents` command. |

### Decisions, knowledge, and measurement

| Path | What it is |
| --- | --- |
| [`crates/jev`](crates/jev/README.md), [`crates/oak`](crates/oak/) | Jev's Rust client and the decision API CLI. |
| [`crates/kev`](crates/kev/), [`crates/laya`](crates/laya/), [`crates/lev`](crates/lev/) | Local decision models. Lev uses Apple's on-device model through [`swift/lev-bridge`](swift/lev-bridge/). |
| [`crates/gateway`](crates/gateway/), [`crates/tenancy`](crates/tenancy/), [`crates/receipts`](crates/receipts/), [`crates/discovery`](crates/discovery/) | The keyed decision gateway, tenant registry, execution receipts, and discovery surface. |
| [`crates/openrouter`](crates/openrouter/) | OpenRouter client for structured output, streaming, and embeddings. |
| [`crates/knowledge`](crates/knowledge/) | The shared knowledge base: entries, lint, and search. |
| [`crates/gym`](crates/gym/), [`crates/gym-bridge`](crates/gym-bridge/README.md), [`crates/gym-leaderboard`](crates/gym-leaderboard/) | Measurement, private Gym boards, and the published leaderboard. |
| [`crates/ext-eval`](crates/ext-eval/), [`crates/eval-runner`](crates/eval-runner/) | Extension evals: the engine, runner, and authoring interview, and the hosted runner. |
| [`crates/plugin`](crates/plugin/), [`crates/plugin-pdk`](crates/plugin-pdk/), `crates/plugin-*` | The Wasm plugin host, its packet types, and guest plugins. |
| [`crates/capability`](crates/capability/) | Capability manifests and bounded probes. |
| [`crates/atif`](crates/atif/) | Agent trajectories (ATIF v1.8). |
| [`crates/xp-ledger`](crates/xp-ledger/), [`crates/playtest`](crates/playtest/) | The NIP-XP ledger and playtest reports. |

### Protocol, payments, and world

| Path | What it is |
| --- | --- |
| [`crates/nostr`](crates/nostr/), [`crates/nostr-relay`](crates/nostr-relay/), [`crates/nostr-transport`](crates/nostr-transport/) | Nostr primitives, our relay, and authenticated transport. |
| [`crates/push-gateway`](crates/push-gateway/) | The NIP-PL push gateway for APNs and FCM. |
| [`crates/wallet`](crates/wallet/), [`crates/x402`](crates/x402/), [`crates/bitcoin-amount`](crates/bitcoin-amount/) | Lightning wallet, x402 over HTTP, and BIP 177 amounts. |
| [`crates/verse`](crates/verse/), [`crates/verse-lagrange`](crates/verse-lagrange/README.md), [`crates/verse-ruins`](crates/verse-ruins/README.md), [`crates/physics`](crates/physics/) | The Verse world, its zones, and rigid-body physics. |
| [`crates/voyager`](crates/voyager/), [`mc-bridge`](mc-bridge/) | Minecraft agent episodes. |
| [`crates/rust-native`](crates/rust-native/README.md) | Shared semantic views and native-renderer contracts. |

### Other directories

| Path | What it is |
| --- | --- |
| [`docs/`](docs/README.md) | All documentation. Start with the [index](docs/README.md) and [catalog](docs/catalog.md). |
| [`nips/`](nips/README.md) | Protocol specifications in three lanes. |
| [`knowledge/`](knowledge/) | Coding knowledge entries, [product entries](knowledge/openagents/), and [quests](knowledge/quests/). |
| [`bench/`](bench/) | Terminal-Bench harness, experiments, and retained traces. |
| [`deploy/`](deploy/README.md) | Relay, worker, and gateway deployment files. |
| [`migrations/`](migrations/) | Relay database migrations. |
| [`os/`](os/README.md) | CoderOS NixOS modules. |
| [`plugins/`](plugins/README.md) | Agent client plugins for the decision API. |
| [`programs/`](programs/), [`recipes/`](recipes/README.md), [`questions/`](questions/), [`patterns/`](patterns/), [`methods/`](methods/) | Program, decision recipe, question, pattern, and method definitions. |
| [`capabilities/`](capabilities/), [`sources/`](sources/), [`quests/`](quests/), [`worlds/`](worlds/), [`assets/`](assets/) | Capability manifests, source lists, quest data, world files, and Verse assets. |
| [`training/`](training/README.md) | Training harnesses that aren't Rust. |
| [`scripts/`](scripts/) | Build, install, and verification scripts. |
| [`swift/`](swift/) | The Swift bridge to Apple's on-device model. |
| [`tests/`](tests/) | Shared test fixtures. |
| [`pair`](pair) | Shows a phone-pairing QR code from this checkout. |

## Build and test

Use the toolchain pinned in [`rust-toolchain.toml`](rust-toolchain.toml).

```sh
cargo run -p coder --bin coder                # the Coder terminal
cargo run -p coder --bin coder -- -p "count the crates"   # one headless turn
./scripts/verify-rust.sh                      # format, Clippy, and tests for changed packages
./scripts/verify-rust.sh --crates coder,gym   # chosen packages
```

`crates/openagents-mobile` is its own Cargo workspace. Test it with
`--manifest-path`:

```sh
cargo test --manifest-path crates/openagents-mobile/Cargo.toml
```

Build the apps:

```sh
OPENAGENTS_IOS_DEVICE=<simulator-udid> bins/openagents-ios/build.sh sim
OPENAGENTS_ANDROID_SERIAL=emulator-5554 scripts/build-openagents-android.sh run
```

The full gate, `./scripts/verify-rust.sh --release`, is for releases only.
See [verification](docs/verification.md) for scope and prerequisites. We use
no GitHub workflows; checks run on contributor machines.

## Contributing

- Read [AGENTS.md](AGENTS.md), the contributor contract. Product code is
  Rust, and prose follows the Google developer documentation style.
- Read [INVARIANTS.md](INVARIANTS.md) before you change an invariant-bearing
  surface.
- Look up terms in the [glossary](docs/glossary.md), which marks what's
  implemented, partial, or proposed.
- File bugs and ideas as [GitHub issues](https://github.com/OpenAgentsInc/openagents/issues).
  Playtesters can use the in-app **Report a problem** or the playtest report
  issue template.
- See the [master roadmap](docs/roadmap.md) for direction.

## License

[Apache License 2.0](LICENSE). See the
[dependency and provenance policy](docs/dependencies.md) for third-party
requirements.
