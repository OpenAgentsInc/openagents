# OpenAgents

We are rebuilding OpenAgents from a small, checked core. The repository was
reset on 2026-10-27. What is here now is the base everything else will be
built on:

- **Coax**, the language;
- **Metareal**, the real-time engine, written in it;
- **one protocol** that every OpenAgents surface speaks;
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
- **Metareal** ([`metareal/`](metareal/)) is the engine. The home scene, its
  graph, its HUD and its resource admission are Coax (`home.coax`), and so is
  its shader math (`light.coax`). A thin C and Swift layer owns storage and
  the GPU. **Verse** is the world and experience being built on Metareal
  ([Verse protocol](docs/verse/openagents-verse-protocol.md),
  [engine notes](docs/verse/metareal-engine.md)).
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
```

Agents working here start with [AGENTS.md](AGENTS.md) and
[INVARIANTS.md](INVARIANTS.md). Talks and episode notes are in
[docs/transcripts/](docs/transcripts/README.md).
