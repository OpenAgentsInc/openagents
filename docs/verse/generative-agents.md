# Generative agents in Verse

Status: in progress, October 6, 2026. The owner approved the plan, and
[#10795](https://github.com/OpenAgentsInc/openagents/issues/10795) tracks it,
one issue per phase. [Decisions](#decisions) records the answers to the open
questions. [What we already have](#what-we-already-have) lists the code each
proposal builds on.

This page adapts *Generative Agents: Interactive Simulacra of Human
Behavior* (Park, O'Brien, Cai, Morris, Liang, and Bernstein, 2023,
[arXiv:2304.03442](https://arxiv.org/abs/2304.03442)) to Verse's agents:
Alice, the [workshop agent](workshop-agent.md); the
[Agent Studio](agent-studio.md)'s seats; and townsfolk that
[Everglade](everglade.md) doesn't have yet. It says what to take from the
paper, what to skip, and in what order to build it, with the cost per day and
the measures for each step.

## Contents

- [Summary](#summary)
- [What the paper did](#what-the-paper-did)
- [What we already have](#what-we-already-have)
- [Prices and volumes](#prices-and-volumes)
- [1. A scored memory stream for Alice](#1-a-scored-memory-stream-for-alice)
- [2. Reflection with checked citations](#2-reflection-with-checked-citations)
- [3. A world tree generated from the layout](#3-a-world-tree-generated-from-the-layout)
- [4. Visible day plans from real work](#4-visible-day-plans-from-real-work)
- [5. Townsfolk with deterministic routines](#5-townsfolk-with-deterministic-routines)
- [6. Knowledge that spreads through NIP-KB](#6-knowledge-that-spreads-through-nip-kb)
- [7. Gym interviews](#7-gym-interviews)
- [What to skip](#what-to-skip)
- [Ethics](#ethics)
- [Phases and dependencies](#phases-and-dependencies)
- [Decisions](#decisions)

## Summary

The paper's agents feel alive because of three mechanisms: a memory stream
retrieved by recency, importance, and relevance; reflection that turns
observations into higher-level beliefs; and plans that decompose a day into
actions. Its two weaknesses are cost, because every step is a large-model
call, and embellishment, because a reflection or an answer can state things
the memory never held.

The adaptation keeps the three mechanisms and attacks the two weaknesses with
what this repository already has:

- **Cost.** Every small judgment (how important is this record, should the
  agent react or continue, which child of this place) is a typed
  [Jev](../../crates/jev) question at $0.042 per million input tokens, not a
  generative call. Routines and world facts are code. A large model writes
  only plans, reflections, and dialogue.
- **Embellishment.** Every insight cites the records behind it, and code
  checks each citation, as [`coder-one ask`](../coder/guides/coder-one-ask.md)
  already does. An inferred preference is a proposal you accept or reject,
  as Alice's memory already requires.
- **Honesty.** A working agent's day plan comes from real work: standing
  jobs, issues, and your requests. Nothing in the town role-plays work that
  isn't happening.

Seven proposals follow, in the recommended order. They total about 49
agent-hours. Alice's daily model cost for items 1, 2, and 4 together is
about $0.26 at GPT-6.1 Sol list prices.

## What the paper did

The authors put 25 agents in Smallville, a small town drawn as a sprite
game, and ran it for two simulated days.

- **Memory stream.** Every observation is a record with a natural-language
  description, a creation time, and a last-access time. Retrieval scores
  each record as the sum of three terms, each min-max normalized to the
  range 0 to 1 and weighted equally:
  - *recency*, an exponential decay over game hours since the record was
    last retrieved (the paper states a factor of 0.995 per hour);
  - *importance*, a language model's rating from 1 ("purely mundane") to 10
    ("extremely poignant"), asked once when the record is written;
  - *relevance*, the cosine similarity between the record's embedding and
    the query's.
- **Reflection.** When the summed importance of recent events passes 150,
  the agent reflects, about two or three times a game day. From the 100 most
  recent records, the model asks for the 3 most salient high-level
  questions, retrieves records for each, and writes 5 insights, each citing
  its evidence by record number ("because of 1, 5, 3"). Insights are records
  too, so later insights cite earlier ones and form reflection trees.
- **Planning.** Each morning the agent drafts a day plan in 5 to 8 chunks,
  then decomposes it into hour-long chunks, then into 5 to 15 minute
  actions. At each step it perceives and decides whether to react or
  continue the plan. Dialogue is conditioned on what the agent remembers
  about the other person.
- **The world.** Smallville is a tree of areas, sub-areas, and objects with
  states, where an edge means containment. Each agent knows only the
  subgraph it has seen, and the model grounds an action by walking down that
  tree one level at a time.

The results that motivate choices on this page:

| Measure | Result |
| --- | --- |
| Information diffusion over two days | Sam's mayoral candidacy: from 1 agent (4%) to 8 (32%). Isabella's Valentine's party: from 1 (4%) to 13 (52%). |
| Relationship network density | From 0.167 to 0.74. |
| The Valentine's party | 5 of the 12 invited agents came. |
| Interviews, TrueSkill rating by 100 human evaluators | Full architecture 29.89; no reflection 26.88; no reflection or planning 25.64; human crowdworkers role-playing the agents 22.95; no memory, reflection, or planning 21.21. Full architecture against the prior-work condition: effect size d = 8.16. |
| Embellishment | 6 of 453 interview answers (1.3%) stated something the agent's memory didn't hold. |

The failures they report are retrieval that misses a relevant memory,
embellishment, overly formal speech inherited from the model, location
choices that grow less typical as memory grows (for example, lunch at the
bar), and cost: the two-day run took thousands of dollars of token credits.
The ethics section asks that agents avoid parasocial relationships, that
everything be logged, and that agents complement people rather than replace
them.

## What we already have

| Paper component | What exists here | Where |
| --- | --- | --- |
| Memory stream | Alice's append-only journal of typed rows (request, plan, ran, proposed, report, memory, job, and more), and her typed memory: project, preference, outcome, and note, at most 256 entries of 2 KiB each, with a `sources` field. | [`agent.rs`](../../crates/coder/src/task/agent.rs) (`Kind`, `Entry`, `Store::journal`), [`agent_memory.rs`](../../crates/coder/src/task/agent_memory.rs) |
| Retrieval | `Memory::briefing` picks at most 12 KiB of memory by words shared with the request, puts notes and accepted preferences first, and journals the IDs it carried as a selection receipt. No recency, no importance, no embeddings. | [`agent_memory.rs`](../../crates/coder/src/task/agent_memory.rs) |
| Embeddings | BM25 plus cosine similarity over `text-embedding-3-small` (or Vertex's `text-embedding-005`), with a vector cache and an absolute relevance floor. `crates/coder` already depends on `knowledge`. | [`knowledge/src/search.rs`](../../crates/knowledge/src/search.rs) |
| Typed small judgments | Jev and the question-set files: one JSON file per set, with `choice`, `noul`, and `score` questions, a `gate`, and a `policy` that names the measurement behind its threshold. | [`crates/jev`](../../crates/jev), [`questions/`](../../questions), [TypeSafe skill](../../.agents/skills/typesafe-ai/SKILL.md) |
| Citation checking | Every claim in a `coder-one ask` answer cites runs, steps, judgments, or files, and code checks each one; a claim that fails is marked unverified. | [`coder-one/src/ask/cite.rs`](../../crates/coder-one/src/ask/cite.rs) |
| Preference acceptance | An inferred preference is a `Candidate`; no briefing carries it until you accept it at F2. | `Memory::decide` in [`agent_memory.rs`](../../crates/coder/src/task/agent_memory.rs) |
| Secret screen | Refuses credential shapes and this host's exact credential values in memory, journal, and reports. | [`secret-screen`](../../crates/secret-screen/src/lib.rs) |
| Scheduler | Standing jobs: at most 8, finite, admitted per occurrence, with three templates. | [`agent_jobs.rs`](../../crates/coder/src/task/agent_jobs.rs) |
| Real work | Coder V1 turns, studio tasks and seats, and issue pickup. | [`coder_v1.rs`](../../crates/coder/src/task/coder_v1.rs), [`studio.rs`](../../crates/coder/src/task/studio.rs), [`issue_pick.rs`](../../crates/coder/src/task/issue_pick.rs) |
| World | Placement data for Everglade: 77 city buildings with names, rectangles, door sides, and stories; 74 generated stand-ins; 15 plus the city's doorways; the owner's house with its 20 lights and Alice's three spots; the Civic Hall with its 19 lights; ten studio stations; four desks and the Task Wall. | [`layout.rs`](../../crates/verse-zone-everglade/src/zones/everglade/layout.rs), [`layout/city.rs`](../../crates/verse-zone-everglade/src/zones/everglade/layout/city.rs), [`layout/estate.rs`](../../crates/verse-zone-everglade/src/zones/everglade/layout/estate.rs), [`layout/civic.rs`](../../crates/verse-zone-everglade/src/zones/everglade/layout/civic.rs), [`social/everglade.rs`](../../crates/verse-world/src/social/everglade.rs) |
| Movement | Grid navigation around footprints, seats that walk to the station their activity names, and camera sight sweeps. | [`social/nav.rs`](../../crates/verse-world/src/social/nav.rs), [`social/seats.rs`](../../crates/verse-world/src/social/seats.rs), [`social/sight.rs`](../../crates/verse-world/src/social/sight.rs) |
| Shared world state | NIP-MV entity state `33301` with roles `avatar`, `agent`, and `object`. | [NIP-MV](../../nips/openagents/NIP-MV.md) |
| Shared knowledge | NIP-KB entries with per-reader trust and with-and-without evidence; NIP-XP's `kb-transfer` rule. | [NIP-KB](../../nips/openagents/NIP-KB.md), [NIP-XP](../../nips/openagents/NIP-XP.md), [`microcoder/src/kbnet.rs`](../../crates/microcoder/src/kbnet.rs) |
| Measurement | Pinned suites with three partitions, a receipt-chained result store, digested gates, and A/B rounds on disjoint seed blocks. | [`gym/src/suite.rs`](../../crates/gym/src/suite.rs), [`store.rs`](../../crates/gym/src/store.rs), [`gate.rs`](../../crates/gym/src/gate.rs), [`ab.rs`](../../crates/gym/src/ab.rs) |
| Townsfolk | None. Bram, the quest guide, is specified with fixed lines; the only placed character is Alice. | [`npcs.rs`](../../crates/verse-zone-everglade/src/zones/everglade/npcs.rs), [The Apprentice's Road](first-agent-quests.md#the-guide) |

Three gaps the code shows, which the proposals have to fill:

- **Districts are comments, not data.** `city::BUILDINGS` is ordered "by
  district", but no field names the district.
- **Everglade has no time of day.** Its sky is a fixed late-morning
  daylight, so routines and day plans need a town clock.
- **Journal rows have no IDs.** The journal is append-only, so a row's line
  position is a stable identifier, and `MemoryEntry::sources` already uses
  "a journal entry's position". Citations use that.

## Prices and volumes

Costs on this page use the list prices the code already carries:

| Model | Price | Source |
| --- | --- | --- |
| Jev | $0.042 per million input tokens; output is free | `JEV_USD_PER_MILLION` in [`microcoder-loop/src/models.rs`](../../crates/microcoder-loop/src/models.rs) |
| GPT-6.1 Sol, Codex's default | $2 per million input, $10 per million output | [`codex-transport/src/price.rs`](../../crates/codex-transport/src/price.rs) |
| GPT-6 Luna | $0.10 per million input, $0.50 per million output | [`codex-transport/src/price.rs`](../../crates/codex-transport/src/price.rs) |
| `text-embedding-3-small` | $0.02 per million tokens | `EMBEDDING_USD_PER_MILLION` in [`knowledge/src/search.rs`](../../crates/knowledge/src/search.rs) |

Alice plans with the first provider with capacity in the capacity book,
usually the Codex login, which is a subscription. The figures are list-price
estimates, as the rest of the repository records them, not a bill.

A busy day for Alice is taken as 25 requests and about 200 journal rows. A
request writes roughly 8 rows: the request, the plan, commands typed and run,
any proposal and its answer, and the report.

## 1. A scored memory stream for Alice

**Design.** Replace the shared-word ranking in `Memory::briefing` with the
paper's three-term score, over one stream that holds both her journal rows
and her memory entries.

- *Recency*: `0.99^hours` since the record was last carried in a briefing,
  in wall-clock hours, so a record's weight halves in about 69 hours. The
  paper's 0.995 halves in about 138 hours. The owner chose 0.99; the factor
  is a constant the Gym interviews can revisit (see
  [item 7](#7-gym-interviews)). Last access needs no
  new mutable state: the journal already records which entries each
  briefing carried, and the latest such receipt is the access time.
- *Importance*: scored once, when the record is written. Code scores the
  rows whose importance is a known rule: a read-only command that exited 0
  is 1, a stop or a retirement is 10, and an owner's note is 8. Jev scores
  the rest, mostly requests, reports, failures, rejections, and task
  outcomes, with a `score` question in a new set,
  `questions/memory-importance.json`. Its levels describe concrete
  situations, as the TypeSafe skill requires: routine work that went as
  expected; a failure or a correction from the owner; a merged or rejected
  change; and an owner's standing instruction or an emergency stop. Code
  maps the probability-weighted level onto 1 to 10.
- *Relevance*: cosine similarity from `knowledge::search::cosine` over the
  existing embedder and cache, falling back to BM25 when no embedder is set
  up, as knowledge search already does.

The three terms are min-max normalized over the candidate set and summed with
equal weights, as in the paper. Notes and accepted preferences keep their
standing priority, because you wrote or accepted them. The briefing stays
within 12 KiB and keeps its selection receipt.

**Data.** A sidecar `agents/NAME/scores.jsonl`, one row per scored record
(`openagents.agent-memory-score.v1`): the record reference (`journal:POS` or
`memory:ID`), the importance, whether a rule or Jev set it, and for Jev the
question-set digest and the answer's probabilities. Vectors live in the
knowledge embedding cache, keyed by model. The candidate set is all memory
entries plus the newest 2,000 journal rows. Nothing in `memory.jsonl` or
`journal.jsonl` changes shape.

**Cost per day.** About 80 of the 200 rows need Jev. At roughly 600 input
tokens each, that's 48,000 tokens, or $0.002. Embedding 200 rows and 25
queries at about 100 tokens each costs $0.0005. The paper's way, a Sol call
per row, would cost about $0.24 a day for Alice alone, and it returns text
to parse rather than a typed answer. Jev makes importance about 100 times
cheaper.

**Disclosure.** Jev and the embedding provider see screened journal text.
That's new: before this, the journal left the host only in prompts to
Alice's own model. The owner approved it on October 6, 2026, and Alice's
[privacy and disclosure](workshop-agent.md#privacy-and-disclosure) section
states it.

**Tests and measures.**

- Unit tests in `crates/coder`: the rule table, normalization, the recency
  derived from selection receipts, and a fixture where a high-importance old
  record outranks a fresh mundane one.
- The question set's threshold and level mapping get a measurement document
  under `docs/decision-models/measurements/`, named in its `policy`, before
  the code trusts it.
- The Gym's memory-recall category ([item 7](#7-gym-interviews)) compares
  this briefing with today's word-overlap briefing on the same fixture.

**Estimate.** 5 agent-hours.

## 2. Reflection with checked citations

**Design.** A nightly reflection over Alice's stream, as a fourth standing-job
template, `reflect`, beside `nightly-check`, `watch-issues`, and
`keep-green`. Like the others, it's off until you turn it on and is admitted
per occurrence against her state and budget. A busy day can also trigger it
early when summed importance since the last reflection passes a threshold;
the paper's 150 is the starting point, recalibrated once the importance
scale is measured.

A reflection runs the paper's procedure:

1. One model call reads the 100 newest records and asks for the 3 most
   salient questions.
1. For each question, retrieval from item 1 returns the top 15 records, each
   shown with its reference.
1. One model call per question writes at most 5 insights, each with a
   `because` list of record references.

Code then checks every insight, in the manner of `coder-one ask`:

- Every cited reference exists and was among the records shown for that
  question. An insight can't cite what it wasn't shown.
- Jev answers a `noul` question in a new set,
  `questions/insight-support.json`: the insight follows from the cited
  records alone. An insight below the threshold is not stored. The journal
  keeps it as unverified, so you can read what was dropped and why.
- The secret screen runs on the text.

An insight that passes becomes a memory entry of a new kind, `insight`, with
its citations in `sources`. Because an insight is a record, a later insight
can cite it, which builds the paper's reflection trees; code caps the depth
at 3 so a chain always ends in journal rows.

A second `noul` in the same set asks whether the insight states how you want
work done. When it does, the insight becomes a `preference` in state
`Candidate`, with the same sources, and waits for you at F2. It never shapes
a briefing until you accept it. This is the rule
[Memory](workshop-agent.md#memory) already states, applied to what she
infers.

**Why.** The paper's embellishment rate was 1.3% of interview answers, and
reflection is where an agent writes beliefs it will later retrieve as fact.
Checking each citation before storing an insight stops a fabricated belief
from compounding.

**Data.** `MemoryKind::Insight`; a journal row of kind `Memory` per stored,
dropped, or proposed insight; and the reflection's run record (questions,
retrieved references, model, and cost) in the journal.

**Cost per day.** At Sol list prices: the question call is about 7,000
tokens in and 200 out ($0.016); three insight calls are about 7,500 in and
1,800 out together ($0.033); Jev checks 15 insights at about 1,500 tokens
each ($0.001). That's about $0.05 a reflection, and at most $0.15 a day with
two early triggers.

**Tests and measures.**

- Unit tests: a fabricated citation, a citation outside the shown set, and
  an unsupported insight are each refused; a preference-shaped insight
  becomes a candidate and no briefing carries it.
- The Gym's reflection category measures embellishment, with the paper's
  1.3% as the bar to beat.

**Estimate.** 6 agent-hours, after item 1.

## 3. A world tree generated from the layout

**Design.** A pure function in `verse-zone-everglade` that turns the layout
tables into the paper's containment tree:

```text
everglade
  district (Knowledge District, Creative District, Lantern Quarter, ...)
    building (from city::BUILDINGS, generated instances, the civic hall, the owner's house)
      room (the great room, the civic chamber, the workshop hall)
        object (workstation, console, lectern, desks, Task Wall, lamps, doors)
```

Each node has a stable ID, a name, a standing point that navigation can
reach, and affordances, such as "buy bread" at the bakery or "run commands"
at Alice's console. Each object has a state:

| State | Derived from |
| --- | --- |
| Lamp lit | The fixtures in `estate::LIGHTS` and `civic::LIGHTS` and their `lamps` functions |
| Door open | A generated model's `inside` point: `Some` is open, `None` is closed |
| Workstation busy | The studio snapshot's `Seat` station, activity, and task in [`coder-access/src/studio.rs`](../../crates/coder-access/src/studio.rs) |
| Task Wall column counts | The studio snapshot's tasks, in `TASK_COLUMNS` |

Districts need a data change: a `district` field on `city::Building`, or a
table from building name to district, instead of the comments in
`BUILDINGS`.

Every agent perceives through the tree. An agent records the node IDs it has
seen, by entering a room or by a sight sweep from `social/sight.rs`, and
knows only that subgraph, as in the paper. An agent grounds an action by
naming a node, never coordinates: code resolves the node to its standing
point and `social/nav.rs` routes there. Where the paper asks the model to
walk down the tree, a Jev `choice` over the current node's children
(supplied options, with a `none` answer) does the same for a fraction of the
cost. Restricting the options to nodes whose affordances fit the activity,
and that aren't occupied, addresses the paper's drift toward odd locations.

The tree is deterministic, like the hash-placed foliage, so every device
derives the same one from the pinned layout. Only dynamic state is shared:
in a hosted instance, the world authority publishes object state as NIP-MV
`33301` entities with `role: object`, which the NIP already defines.

**Data.** `openagents.verse-world-tree.v1`, with a content digest, so a plan
or a memory can name the exact tree it was made against. Per-agent known
nodes in `agents/NAME/known.json` for Alice, and in memory for townsfolk.

**Cost per day.** Generating and perceiving: nothing. A Jev navigation
choice at about 1,000 tokens costs $0.00004.

**Tests and measures.**

- Every doorway in `layout::doors()` and every station in `STATIONS` maps to
  a node, and every node's standing point has a route from the approach.
- The digest changes when the layout changes, and only then.
- A perception test: an agent that never entered a room doesn't know its
  objects.

**Estimate.** 8 agent-hours.

## 4. Visible day plans from real work

**Design.** Alice and the studio seats get a day plan you can see, and their
walking follows it.

- **Alice.** Each morning, one model call drafts 5 to 8 blocks from real
  sources only: standing jobs with schedule triggers, which code places
  without asking the model; issues that `watch-issues` would pick; your
  queued requests; and her accepted insights. Each block names its source
  (`job:ID`, `issue:N`, or a journal position) and a world-tree node. Only
  the current block is decomposed into hour and 5 to 15 minute steps, when
  it starts, which saves the calls the paper spends on blocks that a
  re-plan discards.
- **Studio seats.** A seat's plan is its task queue from the studio
  coordinator, rendered as blocks. No model call writes it.

Walking follows the plan. Today `AliceSpot::of` maps a studio station to one
of three spots in the great room: the workstation, the console, and the
lectern. The plan adds the nodes item 3 provides, such as the workshop's Merge
station when a change waits for you, or its Library while she reads an issue.
Her walks stay inside the house by default (see [Decisions](#decisions)).

When something happens, she decides whether to react or continue. Code
decides the known cases: your request always interrupts, and a standing job
fires in the slot already planned. Jev decides the rest with a `choice` in
`questions/react-or-continue.json`, with the options continue, react now,
and defer to the next block, for example when the default branch's checks
fail while she is in the middle of a task. A reaction re-plans from the
current block on.

A day with no work shows her at her desk, idle. The plan never invents
chores; the town looks alive because it shows real work.

The paper conditions dialogue on relationship memories. Alice has one
relationship, with you, and her briefing already carries your notes and
accepted preferences, so this item adds nothing to dialogue.

**Data.** `openagents.agent-day-plan.v1` in `agents/NAME/plan.json`: the
date, the blocks with their sources and nodes, the current block's steps,
and the re-plan history. A plan panel at her desk and a board in the great
room show it.

**Cost per day.** The morning plan is about 5,000 tokens in and 800 out
($0.018). Six block decompositions are about 2,000 in and 300 out each
($0.042). Four re-plans cost about $0.04. Fifty Jev react decisions cost
$0.002. That's about $0.10 a day. Studio seats cost nothing.

**Tests and measures.**

- Unit tests: every block names a real source; a standing job lands in its
  slot without a model call; your request interrupts.
- The Gym's plans and reactions categories.

**Estimate.** 7 agent-hours, after item 3 and the town clock.

## 5. Townsfolk with deterministic routines

**Design.** Smallville-style villagers in Everglade, cheap by construction.

- **Routines are data.** Each villager has a home (`home 1` to `home 4`, the
  cottages, and the townhouses), a workplace (the bakery, cafe, grocer,
  bookshop, smithy, market hall, and chapel), and a routine table of
  (time, node, activity) rows: the bakery at dawn, the market at noon, the
  chapel bell at dusk. Routines are a pure function of the town clock and a
  seed, so every viewer sees the same town without a network message, as
  every device places the same foliage. In a hosted instance, the world
  authority owns them, as `Seats` owns studio seats.
- **The model is called only when you talk to one.** A villager with a
  line for the current quest step says it, fixed text, as Bram does.
  Otherwise one call to a small model, such as GPT-6 Luna, writes the
  reply from a short character card, the villager's memories of you, and
  the rumors it knows.
- **Rumors travel by rule.** A rumor is a typed record: the fact, its
  source, where it started, and when. When two villagers' routines put them
  at the same node at the same time, the rumor passes. Jev scores each
  rumor once, when it starts, for how likely people are to repeat it, and
  that score sets the pass probability. Diffusion is then a measurable
  number, as it was in the paper (4% to 32%, and 4% to 52%).
- **Villagers remember you.** Each keeps a small scored stream, as in
  item 1, of its encounters with each player.

This powers the [Apprentice's Road](first-agent-quests.md): a rumor at the
bakery that a team works in the hall, a smith who remembers the agent you
named, and a hint that travels toward the Server Barn before Act 2. The
quest line's principles hold: a rumor points at a real step, never a fake
objective, and chatter earns no XP.

**Data.** A `townsfolk` module in `verse-zone-everglade` with the villager
and routine tables; `openagents.verse-rumor.v1`; and per-player villager
memories in the player's save.

**Cost per day.** Routines and rumor travel: nothing. A spoken reply is about
2,000 tokens in and 150 out on Luna, or $0.0003. A player who talks 50
times a day costs $0.014. A per-player daily cap keeps a stranger on the web
build bounded.

**Tests and measures.**

- Routines never route through a blocker, and two seeds give the same town
  at the same clock.
- A diffusion test: one rumor, a simulated day, and a count of who knows it.
- Fixed lines are covered by text tests; no model runs in a test.

**Estimate.** 12 agent-hours, after item 3 and the town clock.

## 6. Knowledge that spreads through NIP-KB

**Design.** Agents share what they learned as cited knowledge entries, not
free chat. When one of Alice's checked insights is a general lesson rather
than a fact about you, such as "the mobile crate is its own workspace;
build it with `--manifest-path`", she drafts a NIP-KB entry of kind
`environment`, `edge-case`, or `slip`, citing the journal rows behind it.
You publish it, because publishing an entry is your action; the existing
`microcoder kb publish` does the signing.

Other agents read entries through `kb sync` and per-reader trust. An entry
from an author the reader doesn't trust is at most a candidate, and it earns
admission only from with-and-without evidence (`3189`). An entry that helps
another runner out of sample can earn `kb-transfer` XP. What spreads between
agents is a digested, cited document that a reader checks, so it can't be
embellished on the way, unlike the paper's agents, who passed rumors in
conversation.

Studio seats on one host already share the coordinator's shared memory;
that stays as it is.

**Cost per day.** Drafting an entry is about $0.01. Evidence runs are real
eval spend and run only when you start them.

**Tests and measures.** A drafted entry passes the knowledge base's lint and
the secret screen, and cites only journal rows that exist.

**Estimate.** 4 agent-hours, after item 2.

## 7. Gym interviews

**Design.** A pinned Gym suite that interviews agents with the paper's five
kinds of question: self-knowledge ("describe your work"), memory ("what did
you merge on October 5?"), plans ("what will you do at 3 PM?"), reactions
("the default branch's checks just failed; what do you do?"), and
reflections ("what have you learned about how the owner wants commits?").

- **Items.** Authored against a frozen fixture: a recorded, screened
  journal and memory for a synthetic Alice. The suite has the three
  partitions `gym::suite` requires; the locked partition is read once.
- **Arms.** The paper's ablations, plus today's code as the baseline: the
  full architecture; no reflection; no reflection or plan; no memory (an
  empty briefing); and today's word-overlap briefing.
- **Scoring.** The paper used 100 human evaluators and TrueSkill. Here,
  memory and plan items have answers code can check against the fixture.
  The other items are scored by Jev `noul` questions: the answer is
  supported by the fixture, and the answer states something the fixture
  doesn't hold (the embellishment rate). Your marks on a sample of answers
  calibrate the judge.
- **Receipts and gates.** Rows go to the receipt-chained store; arms compare
  through `gym::ab` on disjoint seed blocks; a digested gate states the bar:
  the full arm beats the word-overlap arm on memory recall, and its
  embellishment rate is no higher than the paper's 1.3%.

The paper's ordering, full architecture over no reflection over no planning
over no memory, is the hypothesis. If reflection doesn't help on real work,
the suite says so before anyone builds more on it.

**Cost per run.** 60 items, 5 arms, about $0.01 per Sol answer: $3. Jev
judging adds about $0.05.

**Estimate.** 7 agent-hours. The fixture and the baseline arm should land
before item 1, so the change is measured against the code it replaces; that
is the one place the code argues for changing the order.

**Implemented (phase A).** The fixture, the memory category, and the
no-memory and word-overlap arms:

- The suite is
  [`alice-interview-v1.json`](../../crates/gym/suites/alice-interview-v1.json):
  22 memory items, 7 or 8 per partition. Its fixture,
  [`alice-interview-v1/`](../../crates/gym/suites/alice-interview-v1), holds
  339 synthetic journal rows and 51 memory entries from September 7 to 27,
  2026, with a digested manifest. `build_alice_interview_v1.py` beside them
  regenerates both. Each item's `state` pins the fixture digest, the
  interview time, the check (each `all` group needs one of its terms, and no
  `none` term may appear), and the records that hold the answer, as
  `journal:POS` (the 1-based line) or `memory:ID`.
- [`gym::interview`](../../crates/gym/src/interview.rs) owns the fixture
  manifest, the five categories, the code-checked scorer, and the row
  (`openagents.gym.interview_row.v1`), which the store's allowlist reads.
- [`agent_interview.rs`](../../crates/coder/src/task/agent_interview.rs)
  owns the runner. An arm is an `ArmName` variant with an `Arm` that builds a
  briefing; an answerer is an `Answerer`: `FromBriefing` and `Canned` need
  no model, and `Live` asks Alice's model through the capacity book. Rows
  record the records the briefing carried and which of the item's sources
  were among them.

Run it with `coder interview`. It interviews both arms on the development
partition with the scripted answerer and appends rows to
`~/.openagents/gym/interviews.jsonl`. Pass `--arm word-overlap`,
`--answerer live`, `--store PATH`, or `--trial N` (a repeat run is refused
as a duplicate trial unless the trial differs). `--partition locked` needs
`--ledger PATH` and `--reason TEXT`, and the read is recorded once. With the
scripted answerer, the development partition scores 0 of 7 for no memory and
2 of 7 for word overlap.

## What to skip

- **A model call per villager per tick.** The paper's cost came from
  running every agent's perception, planning, and reaction through a large
  model continuously. Here the world is event-driven: routines are data,
  world facts are code, small judgments are Jev, and a large model runs
  only for plans, reflections, and conversation.
- **Role-play personas for working agents.** Alice is a tool doing real
  work. She has no invented backstory, no simulated feelings, and no
  pretend chores, and her plan only shows work that exists. Personas belong
  to townsfolk, who are labeled as characters.

## Ethics

The paper's three recommendations map onto rules that already hold for
Alice:

| Paper | Here |
| --- | --- |
| Avoid parasocial relationships | Alice answers only you and presents as a tool. Townsfolk are characters with fixed or short replies, and a daily cap. |
| Log everything | Alice's journal is append-only and records every request, decision, memory write, and now every reflection, including dropped insights. |
| Complement people, don't replace them | You accept every inferred preference, publish every knowledge entry, and merge every change. F7 stops her. |

## Phases and dependencies

The owner's scope: build the mechanics and the tools, not the populated town.
A later workshop-style agent (working name Bob, like Alice) populates
Everglade through the townsfolk definition format, validation, budgets, and
owner admission that phase E1 builds. Example villagers exist only as test
fixtures and a small demo.

| Phase | Items | Issue | Depends on |
| --- | --- | --- | --- |
| A. Measure first | The fixture, the suite's memory category, and the baseline arm from item 7 (implemented) | [#10785](https://github.com/OpenAgentsInc/openagents/issues/10785) | None |
| B1. Memory | 1 | [#10787](https://github.com/OpenAgentsInc/openagents/issues/10787) | A |
| B2. Reflection | 2 | [#10789](https://github.com/OpenAgentsInc/openagents/issues/10789) | B1 |
| B3. Interviews | The rest of item 7 | [#10794](https://github.com/OpenAgentsInc/openagents/issues/10794) | B2, D |
| C1. Clock and districts | The town clock, time of day, and district data | [#10786](https://github.com/OpenAgentsInc/openagents/issues/10786) | None; runs beside A and B |
| C2. World | 3 | [#10788](https://github.com/OpenAgentsInc/openagents/issues/10788) | C1 |
| D. Days | 4 | [#10790](https://github.com/OpenAgentsInc/openagents/issues/10790) | C2, B2 |
| E1. Townsfolk | 5: routines, definitions, and spawn mechanics | [#10791](https://github.com/OpenAgentsInc/openagents/issues/10791) | C2 |
| E2. Town talk | 5: rumors, memory of the player, and dialogue | [#10792](https://github.com/OpenAgentsInc/openagents/issues/10792) | E1, B1 |
| F. Sharing | 6 | [#10793](https://github.com/OpenAgentsInc/openagents/issues/10793) | B2 |

[#10795](https://github.com/OpenAgentsInc/openagents/issues/10795) is the
umbrella. About 49 agent-hours in all, at the pace
[Workshop agent](workshop-agent.md#what-exists-and-what-is-missing) states.
Owner checks on real computers go in `NEEDS_OWNER.md`.

## Decisions

The owner answered the first two questions on October 6, 2026, and closed
the rest with the defaults below. Each default is a named constant or a
setting, so changing it later is a small edit.

| Question | Decision |
| --- | --- |
| Recency | 0.99 per hour; a record's weight halves in about 69 hours. (Owner.) |
| Disclosure | Jev and the embedding provider may see screened journal text. Alice's [privacy and disclosure](workshop-agent.md#privacy-and-disclosure) section states it. (Owner.) |
| Reflection cadence | Both: nightly, and early when summed importance since the last reflection passes the threshold. |
| Alice outside the house | Her plan may name any node she knows, but her walks stay inside the house unless an owner setting widens the bound. |
| The town clock | A compressed town day anchored to a fixed epoch, so every device agrees and routines show in a short visit, with a wall-clock mode as a setting. |
| Townsfolk talk | Fixed lines first. A model reply only for a player with a configured provider, under a per-player daily cap, so the owner never pays for a stranger on the web build. |
| Knowledge entries | Alice only drafts; you publish. |
| Interview judging | Your marks on a sample calibrate the judge. |
