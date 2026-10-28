# OpenAgents

We are rebuilding OpenAgents from a small, checked core. The repository was
reset on 2026-10-27. What is here now is the base everything else will be
built on:

- **Coax**, the language;
- **Verse**, the real-time engine, written in it;
- **one protocol** that every OpenAgents surface speaks;
- **openagents.com**, drawn in Coax in the browser;
- **a native iOS app** that runs Coax today.

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
  the Verse, made of realms such as the Haunted House
  ([Verse protocol](docs/verse/openagents-verse-protocol.md),
  [engine notes](docs/verse/engine.md)).
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
  - [`/realm/haunted-house`](https://openagents.com/realm/haunted-house):
    the Verse engine's Haunted House realm in the browser, the same Coax as
    on iOS.
  - [`/projects`](https://openagents.com/projects) lists projects.
    [`/projects/coax`](https://openagents.com/projects/coax) shows the Coax
    roadmap and Nostr work as signed Nostr events: the queue, who is on what,
    their plans, and each task's detail
    ([protocol](docs/protocol/2026-10-28-projects.md),
    [how to work the queue](coax/web/project/QUEUE.md)).
    [`/projects/verse`](https://openagents.com/projects/verse/index.md) tracks the
    port of the playtest infrastructure.
  - Each page has an `index.md` beside it for agents.
  - Releases go through `coax/web/release.sh`: a clean build of `origin/main`
    goes to staging, is smoked, promoted to production and smoked again,
    with automatic rollback.
- **The iOS app** ([`apps/openagents-ios/`](apps/openagents-ios/README.md))
  is a minimal native home: the animated graph and HUD, running in Coax.
  Start opens the Grid, the bare Verse world, to walk around in. It
  ships to the owner's TestFlight group as `com.openagents.app`.

## What's next

- **A Lightning wallet in the iOS app**, on Breez's Spark SDK, with the
  payment rules in a checked Coax file
  ([`docs/lightning/`](docs/lightning/README.md)). Money is Bitcoin only:
  Lightning, on-chain, Spark and Taproot Assets.
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
