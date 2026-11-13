# The OpenAgents books

OpenAgents builds open agents that people can own, inspect, improve and earn
from, on open protocols and Bitcoin. These seven books build the pieces that
needs, from scratch, in one language, with every claim checked.

| Book | What you build | Read it when you want to |
| --- | --- | --- |
| [The Coax Programming Language](../../coax/book/README.md) | Programs in Coax, the language every other book uses | write or review any code here |
| [Building the OpenAgents Metaverse Engine](../../aethyr/book/README.md) | A real-time engine: the frame loop, a world, a renderer, physics, realms and a multiplayer server | make a realm or work on the engine |
| [Building Everglade](../../everglade/book/README.md) | An MMORPG on the D&D 5E SRD, on OpenAgents Metaverse Engine | see a whole game built on the stack |
| [Psionic: ML From Scratch](../../psionic/book/README.md) | A machine-learning library that runs, serves and trains models | run a model, serve a decision or train one |
| [Coder: Coding Agents From Scratch](../../coder/book/README.md) | A coding agent and its harness | build or drive an agent |
| [Bitcoin From Scratch](../../bitcoin/book/README.md) | Bitcoin and Lightning, from SHA-256 to channels | understand or check how money moves |
| [Nostr From Scratch](../../nostr/book/README.md) | A Nostr client, a relay and the NIPs OpenAgents uses | understand identity, events and markets |

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

| Step | Where each book teaches it |
| --- | --- |
| Bounded | Coax 9 and 41, the Metaverse Engine 16, Psionic 116, Coder 1, Bitcoin 71, Nostr 44 |
| Deterministic | Coax 16 and 33, the Metaverse Engine 5 and 36, Psionic 70, Coder 2, Bitcoin 35, Nostr 21 |
| Checkable | Coax 11 and 23, the Metaverse Engine 37, Psionic 17 and 234, Coder 21, Bitcoin 116–120, Nostr 93–97 |
| Payable | Psionic 238, Nostr 80 and 86, Bitcoin 104–115 |
| Ownable | The Metaverse Engine 54, 55 and 88, Coder 18, Bitcoin 121 and 122, Nostr 104 |

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
what runs and what does not. When a book and the code disagree, one of them
has a bug.

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
| Pylons | Psionic 24–27 for the service, Nostr 82 and 100 for the events | Everglade |
| The frame loop, rendering, physics | Metaverse Engine | Everglade, Coax |
| Rooms and multiplayer | Metaverse Engine Part XII | Everglade Part IV |
| Actions, conditions and their presentation | The Metaverse Engine 102 | Everglade Parts V–VII |
| SRD rules as code | Everglade Part V | Metaverse Engine |
| Everglade, the game | Everglade | The Metaverse Engine 58 and 101, Psionic 28–32 |
| Robots and spacecraft | Metaverse Engine Parts IV–V | Coax 34–39, Psionic XVIII |
| The agent harness | Coder | Psionic XXI |
| Language features and compile limits | Coax | every book |

## Capstones

A capstone is a route across several books that ends in something a person
can see. Capstones connect the money and identity books to the world and
model books.

- [A guard's decision is paid for](capstones/a-guard-decision-is-paid-for.md):
  Everglade, the Metaverse Engine, Psionic, Nostr and Bitcoin. Design draft.
- A maker is paid: Metaverse Engine Part VI, Nostr and Bitcoin. Not drafted.
- Coder takes a job from the queue: Coder, Nostr, Bitcoin and the book that
  supplies the task. Not drafted.

## More

- [Glossary](glossary.md): words the books share.
- [Paths through the books](paths.md): core paths and runnable routes.
- [Completion milestones](completion.md): what each book still needs.
- [Delivery and publication](DELIVERY.md): how routes and publications are
  checked.
- [Reviews](audits/2026-11-09-holistic-review.md): the 2026-11-09 review
  that this page came from, with [per-book notes](audits/2026-11-09-per-book-notes.md).
