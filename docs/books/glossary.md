# Glossary for the books

Words the seven books share. Each entry names the book that owns the topic.
For Coax language terms, see the
[Coax glossary](../../coax/book/appendix-e-glossary.md).

## The stack

| Word | Meaning | Owner |
| --- | --- | --- |
| Coax | The language every book is written in. | Coax |
| `coax0` | The bootstrap compiler. It emits C11, which also builds to WebAssembly, and shader functions for Metal and WGSL. | Coax 27 |
| Coaxial | The Coax web framework that draws openagents.com and realm pages. | [coax/web.md](../../coax/web.md) |
| Aethyr | The engine: the frame loop, the world, rendering, physics, rooms and the realm format. Named on 2026-10-31. | Aethyr |
| Aethyrforge | The realm builder and world-creation tools on Aethyr. | Aethyr Parts VI and X |
| The Verse | The world that Aethyr runs: a set of realms on the open web. The `verse/` directory holds engine and realm code from before the engine had its own name. | Aethyr Part VII |
| Psionic | The machine-learning library: tensors, model execution, serving and training. | Psionic |
| Coder | The coding agent. It speaks as "I". | Coder |
| OpenAgents | The general agent and the company. It speaks as "we". | Coder plan |

## Worlds

| Word | Meaning | Owner |
| --- | --- | --- |
| Realm | One world in the Verse. A realm is a document and a link. | Aethyr 51 |
| The Grid | The bare shared world you walk into from the home page. | Aethyr 56 |
| Zone | A named place inside a realm, such as the glade. | Everglade |
| Room | One server-owned simulation that every seated player sees the same way. | Aethyr 86 |
| Seat | One player's place in a room. A signed ticket proves who sits in it. | Aethyr 88 |
| Shard | One server process that hosts rooms. | Aethyr 101 |
| Step | One fixed advance of simulation time. Drawing happens between steps. | Aethyr 1 |
| Replay | Running recorded inputs again and getting the same state. | Aethyr 5 |
| Deed | One action an actor asks to take: ask, act, present. | Aethyr 102 |
| Bearing | How a body holds itself and what it may do, from its conditions. | Aethyr 102 |
| SRD | The D&D 5E System Reference Document 5.2.1, licensed CC BY 4.0. | Everglade Part V |
| Maker | The person a realm's tips go to. | [docs/verse/makers.md](../verse/makers.md) |
| Fork | Your own copy of a realm, with a record of where it came from. | Aethyr 55 |

## Models and agents

| Word | Meaning | Owner |
| --- | --- | --- |
| Decision model | A model that returns one choice from a typed set, with a probability, instead of text. | Psionic 12–17 |
| Elephant, Mammoth | The two decision models the Psionic book executes from their original weights. | Psionic 14 and 15 |
| System One | The typed decision interface that Psionic serves at `/v1/systemone`. | Psionic 20 |
| Pylon | A machine that sells spare compute. It announces itself with a Nostr key and answers decision jobs. | Psionic 24 |
| Mesh | Many Pylons behind one API. | Psionic 25–27 |
| Harness | The program around a model that sends requests, runs tools and decides when to stop. | Coder 1 |
| Engine (Coder) | What runs a Coder task: Microcoder, a Psionic-served model, Claude Code or Codex. | Coder 9–13 |
| Microcoder | Coder's own loop, on any model it can reach. | Coder 9 |
| Router, Worker | The Router decides what each incoming event becomes. A Worker does one task. | Coder 17 |

## Money and identity

| Word | Meaning | Owner |
| --- | --- | --- |
| Key | A secp256k1 key pair. One seed gives a person or an agent both a Nostr identity and Bitcoin keys. | Bitcoin 121 |
| Event | A signed Nostr record with an ID that is the hash of its contents. | Nostr 21 |
| Relay | A server that stores and forwards events. | Nostr Part V |
| NIP | A Nostr protocol document. OpenAgents follows the official set and adds its own. | Nostr Part VI |
| The OpenAgents Protocol | The protobuf schemas in [`proto/`](../../proto/README.md) that every OpenAgents surface speaks. Nostr is one transport for it. | Coax 18 |
| Treasury | Keys and rules that let an agent hold and spend Bitcoin. | Bitcoin 122, Nostr 104 |
| Receipt | A signed record that work was done or paid for. | Nostr 82, Psionic 238 |

## Words about evidence

| Word | Meaning |
| --- | --- |
| Check | A `#test` or `#forall` that the compiler runs. |
| Mutation | Changing the code on purpose to confirm that a check fails. |
| `#authority` | A file whose every procedure must be exercised by a check. |
| Profile | The named scope a chapter builds and checks, such as "native C, 16 seats". |
| Implemented | The chapter meets the four promotion rules for its profile. |
| Qualified target | A target with recorded evidence for that profile. |
| Adopted | Code outside the book uses the chapter's code today. |
| Oracle | An independent source of the right answer: a vector, another implementation or a hand calculation. |
| Refusal | A defined result that says work was not admitted, and why. |
| Route | A set of commands from a clean checkout with an exact expected output. See [paths](paths.md). |
| Capstone | A route across several books that ends in something a person can see. |
