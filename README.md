# OpenAgents

OpenAgents builds open agents that people can own, inspect, improve and
earn from, on open protocols and Bitcoin.

Since November 2023, in public and on video
([290 episodes](docs/transcripts/README.md)), we have built and tested the
pieces this needs:
- coding agents, from issue-to-change loops to Coder;
- plugin registries, an agent store and revenue sharing;
- Lightning payments and wallets;
- Nostr identity and markets;
- Pylons, a network that sells spare compute;
- Psionic, our machine-learning framework for inference and training;
- verification, so accepted work can be checked;
- web, desktop and mobile apps.

That research and infrastructure is now coming together into product
releases anchored by **the Verse**: a real-time 3D world on the open web.
In it you walk among the projects we're building, play and make realms,
put agents to work, and pay or get paid in Bitcoin
([Episode 290: Enter the Verse](docs/transcripts/290.md)).

The repository was reset on 2026-10-27 to a small, checked core that every
release builds on:

- **Coax**, the language;
- **Verse**, the real-time engine, written in it;
- **one protocol** that every OpenAgents surface speaks;
- **openagents.com**, drawn in Coax in the browser;
- **native desktop and iOS apps**.

## What we're building

- **Coax** ([`coax/`](coax/README.md)) is a systems language for real-time
  and money-handling code.
  - `coax0` is its bootstrap compiler, written in C++20. It emits plain C11,
    so a Coax library links into Swift, C or C++ hosts. It can also emit
    Metal shader functions.
  - Compile-time code runs in a bounded evaluator, and checks are part of
    the language. `#test` and `#forall` run while compiling. `#authority`
    makes every procedure in a file need a check. `coax mutate` checks that
    the checks catch real changes.
  - The [specification](coax/spec.md) and a
    [book](coax/book/README.md) describe the language and say which parts
    exist today.
- **Verse** ([`verse/`](verse/)) is the engine. The home scene, its
  graph, its HUD and its resource admission are Coax (`home.coax`), and so is
  its shader math (`light.coax`). Start opens the Grid (`grid.coax`), where
  you walk (`player.coax`) as an animated character among patrolling robots
  (`cast.coax`). A thin C and Swift layer owns storage and
  the GPU. The same name covers the world and experience the engine runs:
  the Verse, made of realms
  ([Verse protocol](docs/verse/openagents-verse-protocol.md),
  [engine notes](docs/verse/engine.md)). It is web-first, so a realm is a
  link you can drop anywhere, and it is moving onto native GPUs too
  ([native GPU plan](docs/verse/native-gpu.md)).
- **The OpenAgents Protocol**
  ([spec](docs/protocol/openagents-protocol-spec.md), schemas in
  [`proto/`](proto/README.md)) defines the interchange for web, mobile,
  desktop, agents, cloud hosts, payments and Verse.
  - Published contracts keep their identity.
  - New schemas are alpha.
  - Coax reads selected descriptors with `#import_proto` and generates bounded
    codecs from them.
- **Nostr** is the open transport. [`nips/`](nips/README.md) pins the
  official and external NIPs we follow, next to our own.
- **openagents.com** ([`apps/openagents-web/`](apps/openagents-web/README.md))
  runs on **Coaxial**, the Coax web framework ([design](coax/web.md),
  tools in [`coax/web/`](coax/web/)). Coax compiles to WebAssembly, a small
  generic host draws with WebGPU, and every visible thing on a page is Coax.
  - The homepage shows our projects as a live network, with the XP
    leaderboard and the Pylons online; WALK AROUND opens the Grid, a
    multiplayer world.
  - Realms ([`/realms`](https://openagents.com/realms)) include:
    - [Everglade](https://openagents.com/realms/everglade), a D&D-rules
      town (SRD 5.2.1 spells) whose buildings break and burn and whose
      guards chase you down ([notes](docs/verse/everglade.md));
    - Fairy Forest, built from a friend's game idea;
    - Dead Gears, Ice Cream Man, the Haunted House and Hecate's Cemetery;
    - Orbit League, Belt 2200 and Hold the Stair, pitched by the audience
      of Episode 290 ([design documents](docs/verse/game-ideas/README.md)).
  - [`/realms/new`](https://openagents.com/realms/new) makes a realm from
    what you say or type ([realm builder](docs/verse/realm-builder.md)), and
    you edit it by voice ([realm edit](docs/verse/realm-edit.md)). FORK
    makes your own copy of any realm with your own rules
    ([fork](docs/verse/realm-fork.md)), and REPLY names a building after a
    post so you can smash it and share the clip ([replies](docs/verse/replies.md)).
  - Makers get paid: a maker claims their realm with a Nostr key and a
    Lightning address, players tip them, and makers list upgrades, such as
    a better fairy, that players buy in one tap
    ([makers](docs/verse/makers.md)).
  - To build a realm with Claude Code, clone the repo one level deep
    (`git clone --depth 1`), point your agent at
    [`verse/SKILL.md`](verse/SKILL.md), and open a pull request.
  - [`/projects`](https://openagents.com/projects) lists projects.
    [`/projects/coax`](https://openagents.com/projects/coax) shows the Coax
    roadmap and Nostr work as signed Nostr events: the queue, who is on what,
    their plans, and each task's detail
    ([protocol](docs/protocol/2026-10-28-projects.md),
    [how to work the queue](coax/web/project/QUEUE.md)).
    [`/projects/verse`](https://openagents.com/projects/verse/index.md) tracks the
    Verse engine and realms.
  - Each page has an `index.md` beside it for agents.
  - Releases go through `coax/web/release.sh`: a clean build of `origin/main`
    goes to staging, is smoked and walked, promoted to production and
    checked again (briefly by default, the full walk with `--thorough`),
    with automatic rollback.
- **The iOS app** ([`apps/openagents-ios/`](apps/openagents-ios/README.md))
  is a minimal native home: the animated graph and HUD, running in Coax.
  Start opens the Grid, the bare Verse world, to walk around in. It
  ships to the owner's TestFlight group as `com.openagents.app`.

- **Pylons** sell spare compute on gaming computers for Bitcoin. A Pylon
  runs GPT-OSS for small tool use and the Elephant and Mammoth decision
  models on **Psionic**, our machine-learning framework
  ([book](psionic/book/README.md), being ported from Rust into Coax). Everglade's
  guards are the first game NPCs to ask Pylons for decisions
  ([guard decisions](docs/verse/pylon-guard-decisions.md)).

## What's next

- **A Bitcoin wallet for every person and agent**, on Breez's Spark SDK,
  with the payment rules in a checked Coax file
  ([`docs/lightning/`](docs/lightning/README.md)). Money is Bitcoin only:
  Lightning, on-chain, Spark and Taproot Assets.
- **Fast multiplayer**: a WebSocket room service with an authoritative
  tick, prediction and rollback for Orbit League, and deterministic
  lockstep for Belt 2200.
- **Agents that moonlight**: point your agent at our docs, set your price
  and hours, and it works overnight for Bitcoin.
- **An open metaverse protocol on Nostr**
  ([NIP-MV](nips/openagents/NIP-MV.md)), so anyone can build realms without asking
  our permission, and a store that pays makers residuals when people use
  or remix their work.
- **Verification** grows with the code
  ([plan](docs/verification/2026-10-27-formal-verification-brainstorm.md)):
  - fuzzing with emitted coverage;
  - pinned test vectors for Nostr and BIPs;
  - sanitizer runs.
- **More of the engine and app in Coax.** Each piece is ported only when
  its checks exist first.

## Try it

```sh
coax/build.sh test                              # build coax0 and run its tests
coax/bin/coax run coax/examples/mandelbrot.coax
coax/bin/coax test coax/examples/money.coax     # checks written before the code
coax/bin/coax mutate coax/examples/money.coax   # every mutant must fail a check
apps/openagents-ios/build.sh                    # the iOS app, for the simulator
coax/web/serve.sh apps/openagents-web          # openagents.com locally, with hot reload
```

Agents working here start with [AGENTS.md](AGENTS.md) and
[INVARIANTS.md](INVARIANTS.md). Talks and episode notes are in
[docs/transcripts/](docs/transcripts/README.md).
