# The books as one library: a holistic review

Written 2026-11-09 against `main` at `d1fd1c1c6e`.

This review looks at the seven books together. It asks what they add up to,
which themes they share without saying so, which themes are missing, and what
to do next. It does not check chapter numbers, counts or links. The
[per-book notes](2026-11-09-per-book-notes.md) go one level down.

## How this review was made

I read the repository README, the shared book documents in `docs/books/`,
every book's README, contents and plan headings, the chapter registries, and
the opening and closing of sample chapters from each book. I did not read all
of the roughly 2.5 million words. Claims about a whole book come from its
front matter, its chapter titles and its registry. Claims about chapter
voice come from the samples. Where I counted something, the count is from a
script over the files, and I say what it counted.

## The library today

| Book | Chapter files | Words (about) | Registry status |
| --- | --- | --- | --- |
| The Coax Programming Language | 47 | 160,000 | 21 implemented, 10 partly, 16 planned |
| Psionic: ML From Scratch | 238 | 835,000 | 134 implemented, 104 partly |
| Building Aethyr, World Engine | 101 | 185,000 | 14 implemented, 54 partly, 33 design drafts |
| Bitcoin From Scratch | 103 of 125 | 625,000 | 100 implemented, 3 partly, 22 planned |
| Nostr From Scratch | 89 | 570,000 | 88 implemented, 1 partly |
| Coder: Coding Agents From Scratch | 38 | 120,000 | 8 implemented, 18 partly, 12 surveys |
| Building Everglade | 16 of 32 | 21,000 | 16 partly, the rest planned |

Three things stand out.

- **The method is the library's best asset.** Every book uses the same rule
  to promote a chapter: a calculation worked by hand, runnable source with an
  exact transcript, a broken case with its refusal, and an independent check.
  Every book keeps a registry that separates what is implemented, which
  targets have evidence, and what code outside the book uses it. I know of
  few technical book series that hold themselves to this.
- **The books already reorganize themselves by topic.** Physics moved from
  Psionic to Aethyr, the agent harness moved from Psionic to Coder, and the
  cryptography moved from Nostr to Bitcoin. Each move put a topic in the
  book that owns it. Keep doing this.
- **The finished books are not the ones the releases depend on.** Bitcoin and
  Nostr are nearly complete. Aethyr has 14 implemented chapters of 101,
  Coder has 8 of 38, and Everglade has none complete. The README says the
  releases are anchored by the Verse. The books a Verse visitor would reach
  for first are the least finished.

## The main finding: the library has no stated argument

The README states a mission: open agents that people can own, inspect,
improve and earn from, on open protocols and Bitcoin. Each book states what
it builds. No page says how seven books add up to that mission, or why a
reader should care that they share one language and one method.

The argument is there. It runs through every book, and I think it is this:

1. **Bounded.** Work is admitted before it runs. A budget that is exceeded
   refuses, and a refusal is a result.
2. **Deterministic.** The same inputs give the same bytes, so a session, a
   frame, a model call or an event can be replayed.
3. **Checkable.** Because work replays, someone else can check it against a
   vector, a second implementation or a recorded trace.
4. **Payable.** Because work can be checked, you can pay for it without
   trusting the worker.
5. **Ownable.** Keys, not accounts, say who did the work and who is paid.

Each book teaches several of these steps in its own words:

| Step | Coax | Psionic | Aethyr | Bitcoin | Nostr | Coder |
| --- | --- | --- | --- | --- | --- | --- |
| Bounded | 41 Compile budgets | 116 Train within a memory budget | 16 Budgets that refuse | 71 Misbehavior limits | 44 Limits, policy and floods | 1 The simplest loop |
| Deterministic | 16 Determinism, 33 Replay | 70 Replay the device program | 5 Replay a session exactly | 35 Txids and malleability | 21 Events, IDs and signatures | 2 Model-visible means logged |
| Checkable | 11 Writing checks, 23 Verification boundaries | 17 Qualify honestly, 234 Exact replay as a verification class | 37 Measure the engine honestly | 116–120 Interop and verification | 93–97 Interop and verification | 21 Measure the harness |
| Payable | wallet rules in the introduction | 238 Pay for verified work | — | 104–115 Lightning | 80 Jobs, labor and markets; 86 Lightning payments for requests | — |
| Ownable | — | — | 55 Fork a realm; 88 A seat joins with a signed ticket | 121 One seed for Nostr and Bitcoin; 122 Agent treasuries | 104 Agent treasuries and a network of keys | 18 Consent before sharing |

This is my reading, not something a book states. If it is right, write it
down once, in one place, and let every book point at it. If it is wrong,
the correction is the most useful output of this review. Either way, the
library needs a first page that states the argument.

The two empty cells in the "Payable" row matter most. See
[Two islands](#two-islands) below.

## Two islands

I counted the relative links from each book to each other book.

| From ↓ / To → | Coax | Psionic | Aethyr | Bitcoin | Nostr | Coder | Everglade |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Coax | — | 8 | 25 | 0 | 0 | 0 | 0 |
| Psionic | 11 | — | 15 | 0 | 0 | 32 | 0 |
| Aethyr | 26 | 10 | — | 0 | 0 | 0 | 0 |
| Bitcoin | 6 | 0 | 0 | — | 8 | 0 | 0 |
| Nostr | 12 | 6 | 4 | 79 | — | 0 | 0 |
| Coder | 1 | 53 | 0 | 0 | 0 | — | 0 |
| Everglade | 1 | 0 | 3 | 0 | 0 | 0 | — |

The links show two groups:

- **Worlds and models:** Coax, Psionic, Aethyr, Coder and Everglade.
- **Money and identity:** Bitcoin and Nostr.

Nostr reaches across in its last part. Nothing reaches back. Aethyr, Coder
and Everglade never link to Bitcoin or Nostr. Psionic never links to either,
although Pylons are paid in Bitcoin and announce themselves on Nostr.

The bridges between the two groups are the company's thesis: an agent that
earns, a maker who is paid, a Pylon that sells compute, a realm that anyone
can publish without permission. Those bridges are what the books have not
written yet. The pieces exist as isolated chapters (Psionic 238, Nostr 80, 86
and 100–102, Bitcoin 122). No reader can follow one satoshi from a player to
a maker, or from a buyer to a Pylon, across the books.

**Recommendation:** treat the bridges as the next structural work, ahead of
new chapters inside either island. Three concrete bridges are in
[Three capstones](#three-capstones).

## Themes the library should explore more

### The economy

"Earn" is in the first sentence of the README and almost nowhere in the
books. The books explain how to sign a transaction and how to open a
channel. They don't explain why an agent would be paid, how a price is set,
who takes the risk when work fails a check, how a maker's residual is split
when a realm is forked, or what a Pylon operator earns for a night of
decisions. This is the theme with the largest gap between how much the
company says about it and how much the books teach.

### Decisions as a primitive

Psionic's most distinctive thread is the decision model: a small model that
returns a typed choice with a probability, not text. It appears in Psionic
(Elephant, Mammoth, System One, parts II–IV and XXII), in Coder (chapter 13,
choosing an engine) and in Everglade (the guard). Three books use it and
none explains the idea on its own terms: when a typed decision replaces a
prompt, how to calibrate it, what it costs, and how game code waits for it.
This could be the library's most original contribution to how people
program with models. It deserves a short treatment that all three books
cite.

### Agents as readers and authors

The Coax book says it is written to be read by agents, and that most Coax in
OpenAgents is written by models. No other book says this, and no book
develops it. Three threads already exist and don't know about each other:

- Psionic chapters 54–57 build search and answers over the Coax book.
- Coder builds the agent that would use those answers.
- `metaverse/SKILL.md` tells an outside agent how to build a realm.

Join them. The claim "these books are how our agents learn the stack" is
strong and testable: give Coder a planned chapter from another book and
measure whether it can implement it from the book alone.

### Why from scratch

Four titles say "From Scratch". The Bitcoin book forbids outside
cryptography libraries. The Nostr book allows one TLS library and nothing
else. No book argues for the rule. The reasons are good: one toolchain an
agent can read end to end, checks that cover the whole path, no hidden
allocation or nondeterminism from a dependency, and code small enough to
audit. Say so once. A reader who doesn't hear the reasons will assume the
rule is taste.

### Adversaries

Every book has a chapter about someone attacking the system: a document that
tries to become the system prompt (Coder 6), relay floods (Nostr 44),
misbehaving peers (Bitcoin 71), interest management without leaks (Aethyr
97), who can read the prompt (Psionic 194), checking a peer's work (Psionic
195). The treatment is per chapter. A reader would benefit from one question
asked in every chapter that has a boundary: what can someone on the other
side of this boundary do, and what stops them?

### Running it live

Each book has an operations part: Bitcoin XIV, Nostr 103, Psionic XIX,
Coder IV, Aethyr 98–99, Everglade IX. They share a shape: deploy, observe,
roll back, survive a failure, shadow before you promote. The library could
name that shape once. Everglade IX ("Run an MMORPG") is the natural place to
show it whole, because it runs the relay, the room server, the models and
the wallet together.

### Where it came from

The README counts 290 public episodes and three years of work. The books
barely draw on it. The places that do are among the best pages: the Coax
foreword (Rust, then C++, then Coax), Aethyr's "What it replaced" sections,
and Everglade chapter 2 ("What the prototypes taught"). A short "what we
tried first" note in more chapters would teach judgment, which is what the
from-scratch method otherwise leaves out.

### The protocol

The README lists five core pieces. Four have a book or a part of one. The
fifth, "one protocol that every OpenAgents surface speaks", has one Coax
chapter (18). Meanwhile the Nostr book's chapters 76–92 specify work
coordination, jobs, markets, Pylons, knowledge and attested work as
OpenAgents NIPs. A reader can't tell how the protobuf protocol and the
Nostr NIPs relate: which is the contract, which is the transport, and when
you use each. One page should answer that. It doesn't need a book.

## Structure and voice across the books

### Give the library a front door

Only the Coax book has a foreword, an introduction, a "who this book is
for" section and a glossary. The other six open on status counts, registry
rules and promotion criteria. Those are contributor documents. A reader's
first screen should say what they will be able to do at the end, what they
need to know first, and how long the core path is.

**Recommendation:**

- Add `docs/books/README.md` as the series front page: the argument above,
  a map of the seven books, the reading order, and the shared method stated
  once.
- In each book, split the README into a reader introduction and a separate
  status page. Keep the registry rules in the status page.

### State the honesty rules once

The books are careful about what a passing check does and does not
establish. That care is correct. Repeated on every page, it reads like a
disclaimer: "does not establish", "does not promote", "is not a claim
about". Define *implemented*, *qualified target* and *adopted* once on the
series front page, and let chapters use the words without restating the
limits.

### Lead with the reader's problem, not the evidence

Psionic chapter 1 opens with what the program will learn. Everglade opens
each chapter with a question a player asks. Those are the models. Several
other chapters open with a status block, profile identifiers such as
`cpu-harness-loop-v1`, commit hashes and a table of evidence before they say
what the chapter is about. Move that material to the end of the chapter or
into the registry. Commit hashes and tool version numbers also date a
chapter quickly.

### One chapter shape

Each book found one good habit, and no book has all of them. I counted
section headings across chapter files:

| Habit | Where it is used | Where it is missing |
| --- | --- | --- |
| "What this chapter teaches" | Aethyr, Bitcoin, Nostr, Everglade | Coax, Psionic, Coder |
| A closing summary | Bitcoin, Nostr, about half of Coax | Psionic, Aethyr, Coder, Everglade |
| "Try a change" exercises | 9 Psionic chapters, 1 Coax chapter | everywhere else |
| "Mistakes" (what we got wrong first) | 15 of 16 Everglade chapters | everywhere else |
| Opens from a reader's question | Everglade | everywhere else |
| Sources and dependencies | Bitcoin, Nostr, Aethyr, Coder | varies |

**Recommendation:** adopt one shape for new and revised chapters: the
question, what you will learn, the calculation by hand, the code, the broken
case, how it is checked, mistakes, something to try, sources. Don't
retrofit 700 chapters. Apply it to each book's core path first.

### A core path in every book

Two books are over half a million words. Psionic executes more than a dozen
named models. Nostr's Part VI has 48 chapters, one family of NIPs each. This
is valuable reference material, and it buries the teaching line. In each
book, mark 12 to 20 chapters as the core path that a person can finish in a
week. Label the rest as reference or as branches. The
[paths document](../paths.md) is the right home for this, and it needs to
grow from four terminal routes to cover all seven books.

### One owner for each shared topic

Several topics are told in more than one book:

| Topic | Told in |
| --- | --- |
| Everglade | Aethyr 58 and 101, Psionic 28–32, the Everglade book |
| Agent treasuries | Bitcoin 122, Nostr 104 |
| Pylons | Psionic 24–27, Nostr 82 and 100 |
| Robots and spacecraft | Coax 34–39, Aethyr IV–V, Psionic XVIII |
| Replay | Coax 33, Aethyr 5 and 36, Psionic 70 and 234 |

The moves between books show that the rule is already understood: one book
owns the topic and the others link. Write the owners down in a table on the
series front page so that new chapters land in the right book.

### One glossary

These words carry more than one meaning across the books and the README:
Verse (the engine and the world), Aethyr, Aethyrforge, realm and world, the
Grid, room, System One, the Decisions API, Elephant and Mammoth, Pylon,
Worker and Router, profile, route, qualified, adopted. The README itself
calls Verse "the real-time engine" in one list and says "the engine is
Aethyr" in another. Only the Coax book has a glossary. A shared glossary is
a small job with a large effect, for people and for agents.

### The shared documents are four books behind

`docs/books/paths.md` and `docs/books/completion.md` were written for four
books. There are seven now. Bitcoin, Coder and Everglade have no runnable
route, and the completion plan doesn't mention them. No route ends in
something you can play or a payment you can see. Update both when the
series front page is written.

## Three capstones

A capstone here is a route across books that ends in something a person can
see. Each one closes a gap in the link table.

1. **A guard's decision is paid for.** A player in Everglade attacks a
   guard. The guard asks a Pylon. The Pylon answers, the answer is checked,
   and the Pylon is paid over Lightning with a receipt on Nostr. This joins
   Everglade, Aethyr, Psionic, Nostr and Bitcoin. It is the README's whole
   thesis in one trace.
2. **A maker is paid.** Someone publishes a realm with a Nostr key and a
   Lightning address, another person forks it, and a player's tip reaches
   both. This joins Aethyr Part VI, Nostr and Bitcoin, and it forces the
   residual rules to be written down.
3. **Coder takes a job from the queue.** Coder reads a task published as a
   signed Nostr event, implements a planned chapter from another book,
   passes that book's promotion check and is paid. This joins Coder, Nostr,
   Bitcoin and whichever book supplies the task, and it tests the claim that
   agents can learn the stack from the books.

Each capstone should be written the way a chapter is: a trace worked by
hand first, then the runnable version, then the broken case.

## What to do next, in order

1. Write the series front page: the argument, the map, the shared method,
   the topic owners and the glossary.
2. Split each book's README into a reader introduction and a status page.
3. Mark a core path in each book and extend the paths document to all seven.
4. Write capstone 1 as a design draft, so the missing bridge chapters have
   names and owners.
5. Put chapter-writing effort where the releases are: Aethyr Parts I–III,
   VI and XII, and Everglade. Bitcoin and Nostr can coast.
6. Give Psionic visible parts and decide whether it is one book or several
   volumes. Psionic work is paused, so this is editing, not new chapters.
7. Add the reverse links: Aethyr, Coder, Everglade and Psionic chapters that
   touch identity or payment should point at the Nostr and Bitcoin chapters
   that build them.

## What I would not do

- **Don't start an eighth book.** The protocol, the web framework and the
  economy are real gaps. A page, a part or a capstone closes each one. A
  new book would add another unfinished front.
- **Don't retrofit every chapter to a new template.** Apply the shape to
  core paths and to new chapters.
- **Don't loosen the promotion rule to raise the implemented counts.** The
  rule is why the books can be trusted.

## Follow-up, 2026-11-09

The same day, these steps from the list above were done:

| Step | Done | Left |
| --- | --- | --- |
| 1. Series front page | [The OpenAgents books](../README.md) and the [glossary](../glossary.md) | — |
| 2. Reader introductions | A "Start here" section at the top of each book's README | The status tables stay in each README, because the book checks read them there. Moving them needs a checker change. |
| 3. Core paths | [Paths](../paths.md) covers all seven books | Runnable routes for Bitcoin, Coder and Everglade |
| 4. Capstone 1 | [Design draft](../capstones/a-guard-decision-is-paid-for.md) | Five owner decisions listed in the draft; capstones 2 and 3 |
| 5. Aethyr and Everglade | Everglade chapters 12 and 13; Aethyr chapter 102 on the action pipeline; 11 drifted Aethyr listings copied again from source; the SRD license statement | Listings and runnable examples for the Everglade chapters; Everglade Parts VI–IX |
| 6. Psionic parts | The contents name all 25 parts | A decision on volumes |
| 7. Reverse links | Aethyr 54, 55 and 88, Psionic 24 and 238, Coder 17, and the Everglade README link to Nostr and Bitcoin | A full pass over each book |

The book checks had been failing on `main` before these changes, which hid
the drift: a missing registry source for Aethyr 93, the 11 Aethyr listings,
three Coax excerpts and one Coax link. Those are fixed. One failure
remains and needs a person or agent to review compiler changes: the
capability claims `coax.wgsl.current` and `psionic.language.current` in
`coax/book/capability-claims.json` report changed evidence
(`coax/lib/compiler/wgsl.coax`, `coax/src/shader.cpp` and
`apps/openagents-web/gpu/everglade-v1/gpu.bindings`).
