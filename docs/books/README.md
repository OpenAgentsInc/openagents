# The OpenAgents books

OpenAgents builds open agents that people can own, inspect, improve and earn
from, on open protocols and Bitcoin. These eight books build the pieces that
needs, from scratch, in one language, with every claim checked.

| Book | What you build | Read it when you want to |
| --- | --- | --- |
| [The Coax Programming Language](../../coax/book/README.md) | Programs in Coax, the language every other book uses | write or review any code here |
| [Building the OpenAgents Metaverse Engine](../../metaverse-engine/book/README.md) | A real-time engine: the frame loop, a world, a renderer, physics, realms and a multiplayer server | make a realm or work on the engine |
| [Building Everglade](../../everglade/book/README.md) | An MMORPG on the D&D 5E SRD, on OpenAgents Metaverse Engine | see a whole game built on the stack |
| [Psionic: ML From Scratch](../../psionic/book/README.md) | A machine-learning library that runs, serves and trains models | run a model, serve a decision or train one |
| [Coder: Coding Agents From Scratch](../../coder/book/README.md) | A coding agent and its harness | build or drive an agent |
| [Bitcoin From Scratch](../../bitcoin/book/README.md) | Bitcoin and Lightning, from SHA-256 to channels | understand or check how money moves |
| [Nostr From Scratch](../../nostr/book/README.md) | A Nostr client, a relay and the NIPs OpenAgents uses | understand identity, events and markets |
| [The OpenAgents Book](../../openagents/book/README.md) | OpenAgents Desktop (`apps/openagents-desktop/`), iOS, and Coax Web/UI SSR apps on compiled Coax, `openagents/core/` (`oa-worker`/`Inky`), WorkOS hybrid auth, Breez Spark Bitcoin/Lightning wallet, and the Episode 289 capability economy | build the product apps or the agent |

## The argument

The books share one line of reasoning. Each book teaches several of its
steps.

1. **Bounded.** Work is admitted before it runs. A budget that is exceeded
   refuses, and a refusal is a result you can read.
2. **Deterministic.** The same inputs give the same bytes, so a frame, a
   session, a model call or an event can be replayed.
3. **Checkable.** Because work replays, someone else can check it against a
   published vector, a second implementation or a recorded trace.
4. **Payable.** Because work can be checked, you can pay for it without
   trusting the worker.
5. **Ownable.** Keys say who did the work and who is paid. No account
   holder stands in between.
6. **Compounding.** Because verified work produces provably correct traces and
   redacted trajectories, Pylons can run bounded decentralized training windows
   that pass Byzantine verification and quarantine gates before shipping back
   to every user's agent.

| Step | Where each book teaches it |
| --- | --- |
| Bounded | Coax 9 and 41, the Metaverse Engine 16, Psionic 116, Coder 1, Bitcoin 71, Nostr 44 |
| Deterministic | Coax 16 and 33, the Metaverse Engine 5 and 36, Psionic 70 and 226–233, Coder 2, Bitcoin 35, Nostr 21 |
| Checkable | Coax 11 and 23, the Metaverse Engine 37, Psionic 17 and 234, Coder 21, Bitcoin 116–120, Nostr 93–97 |
| Payable | Psionic 238, Nostr 80, 82 and 86, Bitcoin 104–115 and 123 |
| Ownable | The Metaverse Engine 54, 55 and 88, Coder 18, Bitcoin 121 and 122, Nostr 104 |
| Compounding | Psionic 224, 235–241, 247 and Part XXVIII (272–274: DiLoCo, Byzantine verifier & Quarantine Optimizer), Coder 8, 12 and 18, OpenAgents 52, 54–55 and 70 |

### Why from scratch

Four of the titles say "From Scratch", and the Bitcoin book allows no
outside cryptography library. The reasons:

- **One toolchain.** A person or an agent can read every layer, from the
  hash function to the game rule, in one language.
- **Checks cover the whole path.** A dependency you can't check is a place
  where the argument above stops.
- **No hidden behavior.** No allocation, thread or rounding mode comes from
  code you didn't read. Replay depends on that.
- **Small enough to audit.** Each piece is built for the profile that uses
  it.

Other implementations appear in every book as oracles. The code is compared
with them and never built on them.

## Reading order

Start with the Coax book's Parts I–III. Every other book assumes them.

```text
Coax ──┬── Metaverse Engine ──── Everglade
       ├── Psionic ─── Coder
       └── Bitcoin ─── Nostr
```

- Metaverse Engine Part IV onward also needs Psionic chapters 3–6, 37 and 38.
- Nostr needs Bitcoin Parts I–V.
- Everglade's guard uses Psionic's decision chapters (28–32).
- Coder's own engine uses models that Psionic serves.

[Paths through the books](paths.md) gives a short core path in each book
and the routes you can run from a terminal today.

## How every book works

A chapter becomes a teaching chapter only when it has all four of these:

1. **A calculation or trace worked by hand**, before the program runs.
2. **Runnable source** with an exact expected transcript.
3. **A broken case**, with the refusal or error it produces.
4. **An independent check**: a published vector, another implementation or
   a hand-derived value.

Each book's chapter registry (`chapters.json`) records three separate
facts about a chapter. The books use these words in one sense only:

| Word | Meaning | What it does not mean |
| --- | --- | --- |
| Implemented | The chapter meets the four rules for its named profile. | That every feature the title suggests is built. |
| Qualified target | A target, such as `cpu`, `wasm`, `cuda` or `metal`, with recorded evidence for that profile. | That other targets work. |
| Adopted | Code outside the book uses the chapter's code today. | That a whole application depends on it or is deployed. |

A chapter marked *partly implemented*, *design draft* or *planned* says
what runs and what does not. Two additional registry statuses appear where
needed: *written* (for external-system study chapters, such as Coder's
third-party harness teardowns, which cite pinned external revisions rather
than shipping Coax code) and *moved* (for chapters relocated to another book
while keeping legacy numbers traceable). When a book and the code disagree,
one of them has a bug.

## Which book owns a topic

Several topics appear in more than one book. One book owns each topic, and
the others link to it. When a topic outgrows its book, it moves: physics
moved from Psionic to the Metaverse Engine, the agent harness moved from Psionic to
Coder, and the shared cryptography moved from Nostr to Bitcoin.

| Topic | Owner | Other books link from |
| --- | --- | --- |
| Hashes, curves, signatures, Bech32 | Bitcoin Parts I–V | Nostr, Coax |
| Events, relays, NIPs, markets | Nostr | Psionic, Coder, Metaverse Engine |
| Channels, invoices, payments | Bitcoin Part XVII | Nostr 71–73 and 86, Psionic 238 |
| Agent treasuries | Bitcoin 122 for signing, Nostr 104 for the network of keys | Coder |
| Tensors, models, serving, training | Psionic | The Metaverse Engine, Coder |
| Decision models | Psionic Parts II–IV and XXII | Coder 13, Everglade, the Metaverse Engine 77 |
| Pylons | Psionic 24–27 and 234–238 for the service, Nostr 82 and 100 for the events | Everglade, OpenAgents 70 |
| Compiled weight modules & hybrid executors | Psionic Part XXIII (226–237) | Coder 8 and 12, OpenAgents 69 |
| Decentralized training & Quarantine Optimizer | Psionic Part XXVIII (272–274) and Part XXIV (239–241, 247) | Nostr 82, Coder 18, OpenAgents 52 and 70 |
| The frame loop, rendering, physics | Metaverse Engine | Everglade, Coax |
| Rooms and multiplayer | Metaverse Engine Part XII | Everglade Part IV |
| Actions, conditions and their presentation | The Metaverse Engine 102 | Everglade Parts V–VII |
| SRD rules as code | Everglade Part V | Metaverse Engine |
| Everglade, the game | Everglade | The Metaverse Engine 58 and 101, Psionic 28–32 |
| Robots and spacecraft | Metaverse Engine Parts IV–V | Coax 34–39, Psionic XVIII |
| The agent harness | Coder | Psionic XXI, OpenAgents 24–28 |
| Language features and compile limits | Coax | every book |

## Capstones

A capstone is a route across several books that ends in something a person
can see. Capstones connect the money and identity books to the world and
model books.

- [The Executor Network closes the loop](capstones/executor-network-flywheel.md):
  **Master capstone** across Coax, Psionic, Nostr, Bitcoin, Coder and OpenAgents. Verified on real Metal 4 + RTX 4080 CUDA GPUs and live Bitcoin mainnet Spark (`#13912`–`#13928`), with runnable CPU route `executor-network-flywheel` in [`routes.json`](routes.json).
- [A guard's decision is paid for](capstones/a-guard-decision-is-paid-for.md):
  Everglade, the Metaverse Engine, Psionic, Nostr and Bitcoin. Design draft.
- A maker is paid: Metaverse Engine Part VI, Nostr and Bitcoin. Not drafted.
- [Coder takes a job and gets paid](capstones/coder-takes-a-job-and-gets-paid.md):
  Coder, Nostr, Bitcoin and OpenAgents. Verified on CPU / `#authority` boundary.

## More

- [Glossary](glossary.md): words the books share.
- [Paths through the books](paths.md): core paths and runnable routes.
- [Completion milestones](completion.md): what each book still needs.
- [Delivery and publication](DELIVERY.md): how routes and publications are
  checked.
- [Reviews & audits](audits/2026-11-21-executor-network-flow-audit.md): the
  2026-11-21 Executor Network flow audit across all eight books, the
  [2026-11-20 holistic books audit](audits/2026-11-20-holistic-books-audit.md),
  and the [2026-11-09 review](audits/2026-11-09-holistic-review.md) with
  [per-book notes](audits/2026-11-09-per-book-notes.md).
