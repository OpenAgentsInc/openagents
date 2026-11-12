# Root glossary

Shared vocabulary for the repository and the books. Each shared term has one
meaning and one owning context; the Conflicts column names what the code does
today and which move renames it. For Coax language terms, see the
[Coax glossary](../../coax/book/appendix-e-glossary.md).
For the bounded contexts that own these words, see [Bounded contexts and context map](../architecture/contexts.md).

**Rule for new code:** a new identifier, table, registry ID or wire field uses
the glossary's word. Old names are not swept; a rename lands only with a move
that already touches the file (#13267 to #13269, #13280).

## Shared domain terms

| Term | One meaning | Context | Conflicts |
| --- | --- | --- | --- |
| Aethyr | The engine | Engine | `proto/README.md` called the Verse the engine (fixed in #13312); `verse/` holds older engine files |
| Verse | The set of realms on the open web that Aethyr runs | Realms | See Aethyr |
| Realm | One world: a document and a link | Authoring | |
| Zone | A named area of one realm's world, loaded as cells | Everglade | Also a Wasm page (`everglade/features.coax:66-81`, rename to pages in #13268/#13280), the AAA audit's server unit, and a GCP zone |
| Page | A Wasm module the browser loads | Web client | Called "zone" (`EfZone`) in `everglade/features.coax:66-81`; rename to pages when #13268/#13280 touch it |
| Room | One server-owned simulation of a zone with up to 16 seats | Room session | SQL says "instance" (`verse.room_instances`, `0115_room_directory.sql:1-2`, rename to rooms when #13267/#13280 touch it); also a shard's URL path (`/api/room/s1/`) and the `oa-room` binary |
| Instance | Don't use. Say room | Room session | `verse.room_instances` (`0115_room_directory.sql:1-2`); rename to rooms with the room/world moves (#13267, #13280) |
| Shard | One room server process | Room session | |
| Host (VM) | A machine that runs shards | Operations | |
| Seat | A player's slot in a room (`SeatId`), proved by a seat ticket | Room session | `verse.seats` (`0110_realm_seats.sql`) is an agent's place in a made realm; rename to agent seats when touched (#13267–#13269, #13280) |
| Agent seat | An agent's place in a made realm | Authoring | Rename target for `verse.seats` (`0110_realm_seats.sql`, `seats.c`), picked up when a move touches those files (#13267–#13269, #13280) |
| Seat ticket | Signed admission to one seat: identity, character, class and level | Room session | Class and level not in all tickets yet (#13259) |
| Account | A login identity with authenticators (`auth.accounts`) | Identity | `verse.accounts` (game account) and `inference.account_day` also use "account" |
| Game account | The game's owner of characters and realms (`verse.accounts`) | Everglade | Spelled `verse.accounts` today |
| Player key | A browser's random secret for a guest (`verse.players`) | Identity | "Player" also means a person and the engine's `Player` body-plus-camera struct |
| Player | A person playing. Not a key, not a struct | Everglade | SQL `verse.players` is a player key; engine `Player` holds body and camera |
| Body | The server-owned movable entity of a player (#13271) | Engine | Today part of `Player` |
| Character | The persistent hero: class, level, XP, abilities, items by reference | Everglade | Rig runtime (`aethyr/character/`), appearance, talk persona (`aethyr/talk/character.coax`), checkpoint capture, and `0130_item_ledger.sql` all use the word |
| Appearance | A character's look from Character Create | Everglade | Sometimes called character |
| Rig | An animated skeleton and its parts (`aethyr/character/`) | Engine | Folder and types called "character" |
| Persona | An NPC's identity for talk | Talk | `aethyr/talk/character.coax` calls it character |
| Class | An SRD class. Spelled `class` in code | Rules | Spelled `class`, `klass` and `kind` in one function (`room-everglade.coax:789,867`, extracted in #13267), with level folded into strings like `srd-fighter-5` |
| Level | Derived from XP by the rules' table | Rules | Two copies of the XP table; client hello can set it |
| Ability | One action a character can take: spell, class feature, attack | Rules | #13285 defines it |
| Aura | A lasting effect on a creature, with stacking rules | Rules | #13285 |
| Creature | Any rules participant: character, monster, NPC | Rules | |
| Creature template | A stat block by content ID | Rules | Monsters addressed by index today |
| Content ID | Stable ID of an ability, spell, template or spawn | Rules | #13286 |
| Combat profile | A world's declared departures from the SRD (`world-profile.coax`) | Rules | "Profile" has seven meanings across wire, combat, timing, checkpoint, execution, compat and books |
| Wire profile | A JSON file pinning a hand-packed frame's bytes (`proto/verse/*.json`) | Protocol | Shares "profile" with six other uses |
| Checkpoint | A durable save of a character or a realm's simulation | Room session | Two different things today (character checkpoint vs simulation checkpoint); always qualify it |
| Item | A game object in the ledger. Never money | Items | |
| Operation ID | A caller-chosen key that makes a write idempotent | Items, Room session | `OperationId` (`aethyr/ids.coax`) in Coax; `text`, 1 to 80 bytes, on the wire and in SQL ([0006](../decisions/0006-item-ledger-rules-in-coax.md)) |
| Domain event | A past-tense fact a context publishes (`RsEvent`) | All | Shares "event" with world, analytics, Nostr, `chat.events` and `projects.events` |
| World event | A scheduled in-world happening, such as a raid | Everglade | `verse.room_events` and `room-events.coax` |
| Session (login) | An authenticated browser or app session (`auth.sessions`) | Identity | Six SQL tables named `sessions` (`analytics`, `projects`, `drops`, `signin`, `auth`, plus `verse.pack_sessions`), plus `aethyr/session/` and `openagents.session.v1` |
| Transcript log | Coder's JSONL log, id `openagents.session.v1` | Coder | Clashes with NIP-SESS (compliance plan C1) and login/room sessions |
| Engine (Coder) | What runs a Coder task | Coder | Shares "engine" with Aethyr and SRD `rules/srd/engine.coax` |
| Golem | A Coder agent session, shown in the Everglade desktop | Golemancy (ACL over Coder) | |
| Everglade | The game. Today Everglade II | Everglade | Legacy v1 (`everglade.wasm`, until #13269), `everglade2`, `.e2`, `everglade-ii` (#13267–#13269), and web domain `everglade.world` |

## Book terms

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
| Room | One server-owned simulation that every seated player sees the same way. An Everglade room seats 16 players (`EG_SEATS`). | Aethyr 86 |
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
