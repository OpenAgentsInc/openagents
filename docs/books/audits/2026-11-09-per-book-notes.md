# Per-book notes

Written 2026-11-09 against `main` at `d1fd1c1c6e`. Read the
[holistic review](2026-11-09-holistic-review.md) first. It explains the
method, the two groups of books and the capstones that these notes refer to.

Each section says what the book does well, what it is missing at the level
of themes and structure, and what to do next. These notes come from each
book's front matter, chapter titles, registry and sample chapters, not from
a full read.

## The Coax Programming Language

**What works.** This is the best front door in the library: a foreword that
says where the language came from, an introduction that shows real code from
the app, a "who this book is for" section, appendices for Jai and C++
readers, a glossary and an error-code index. It is the only book that says
it is written for agents as well as people.

**What is missing.**

- **Money and secrets.** The introduction uses the wallet's `#authority`
  file as its second example, and the cryptography lives in `coax/lib/`.
  The book never links to the Bitcoin or Nostr books. Constant-time rules
  for secret data (Bitcoin chapter 3) are a language concern as much as a
  Bitcoin one. A short section here, pointing there, would connect the
  language to the code that most needs its checks.
- **The web.** openagents.com is drawn in Coax through the Coaxial
  framework. The book has no chapter where you build a page. For many
  readers that is the fastest way to see Coax do something.
- **Agents.** Chapter 22 covers tooling for agents. The larger claim, that
  most Coax is written by models and that the language was shaped for that,
  deserves more room: what an agent gets wrong, which error codes it
  recovers from, and how checks and mutation keep generated code honest.

**Structure.** Parts VII–IX are 16 planned chapters, one per roadmap phase.
They are a roadmap in book form. The robot and freighter scenarios (34–39)
overlap Aethyr Parts IV–V and Psionic's robot chapters. Decide which book
owns the scenario and let Coax teach only the language features it needs.
Consider moving the planned parts to a clearly separate "Road ahead"
volume, so the book a reader finishes is the book that runs.

**Next.** Add the secrets section and the page chapter. Add outbound links
to Bitcoin, Nostr and Coder where their libraries appear in chapter 26's
recipes.

## Psionic: ML From Scratch

**What works.** Chapter 1 is the best opening chapter in the library: one
number, every multiplication checked by hand, a bad request, and something
to try. The book runs original published weights early, before it trains
anything. That choice is unusual and worth stating as a design decision.

**What is missing.**

- **Visible structure.** The plan has 25 parts. The contents page shows 238
  chapters in unnamed groups. At about 835,000 words this is several books:
  tensors and executing original models; serving and the Pylon mesh;
  training; images, motion, audio, fields and robots; and the late
  chapters on programs in weights, verification and optimization. Name the
  parts in the contents now. Decide later whether to publish volumes.
- **The decision-model thread.** Elephant, Mammoth and System One are what
  make this book different from other ML texts. The idea is spread across
  parts II–IV and XXII. Give it a short stated thesis near the front: what
  a typed decision is, when it replaces text generation, and how you know
  its probabilities are any good. Coder and Everglade both depend on it.
- **The most original chapters are at the end.** Chapters 226–237 (exact
  execution compiled into weights), 238 (pay for verified work) and 239–249
  (gates that can say "unverifiable", optimization loops) carry ideas that
  the rest of the library relies on. A reader has to pass 225 chapters to
  find them. They would work as their own volume with a short on-ramp.
- **Money and identity.** The book has no link to the Bitcoin or Nostr
  books. The Pylon chapters (24–27) and chapter 238 are where those links
  belong.

**Structure.** This book has been the library's incubator: physics and the
agent harness both started here and moved out. Keep using it that way, and
keep moving finished topics to the book that owns them.

**Next.** Psionic work is paused, so limit this to editing: named parts in
the contents, a core path of about 20 chapters, the decision-model
statement, and reverse links from the Pylon chapters.

## Building Aethyr, World Engine

**What works.** The chapter titles are the best in the library. Each one is
a statement you can check: "A frame has a fixed step", "Budgets that
refuse", "Effects are state, events are cues". The "What it replaced"
sections show how real code moved into the engine. Part XII builds
multiplayer the direct way, with one server that owns the scene.

**What is missing.**

- **Order for a game maker.** A reader who wants to make a playable realm
  finishes the core (Parts I–III) and then meets 31 chapters of advanced
  simulation: snow on a grid, differentiable simulation, ephemerides, burns
  and heat rejection. Realms, publishing and multiplayer come after. Those
  simulation parts were inherited from the Psionic book. Group the book
  into volumes: the core; realms and Aethyrforge; multiplayer; sound; then
  simulation and space as an advanced volume.
- **Ownership and payment.** The README says makers claim a realm with a
  Nostr key, players tip them, and forks pay residuals. Part VI covers
  publishing and forking. The book has no link to the Nostr or Bitcoin
  books. Chapter 88 (a seat joins with a signed ticket) and chapters 54–55
  (publish and fork) are where identity and payment enter the engine.
- **The maker who doesn't code.** Aethyrforge builds a realm from speech.
  The book builds the engine underneath. Nothing explains to a maker what
  the engine promises them: what a fork preserves, what a published realm
  pins, what they own.

**Structure.** Only 14 of 101 chapters are implemented, and the releases
depend on this engine. Everglade is told here twice (chapters 58 and 101)
and again in its own book. Let the Everglade book own the game. Keep the
engine's side here: what the room server guarantees to any game.

**Next.** This book and Everglade should get most of the chapter-writing
effort. Finish Parts I–III, then VI and XII. Add the identity and payment
links when capstone 2 ("a maker is paid") is drafted.

## Bitcoin From Scratch

**What works.** This is the most consistent book. Every chapter has the same
shape: what it teaches, where the code is, a worked example, how the code
is checked, a summary, dependencies and sources. It has moved fast because
Bitcoin has specifications, published vectors and a reference node to
compare against. The rule that the node "decides no money for OpenAgents on
its own" until three named chapters are done is exactly the right kind of
boundary.

**What is missing.**

- **Why Bitcoin, and why for agents.** The book starts at bytes. It never
  argues the choice. One short opening chapter should say why agents need
  money that no account owner can withhold, and why OpenAgents accepts only
  Bitcoin rails.
- **How the book relates to the product.** The README says the wallet for
  every person and agent uses Breez's Spark SDK, with Taproot Assets among
  the rails. I found neither named in the book. A reader will ask whether
  the from-scratch node and Lightning stack replace the SDK, check it, or
  only teach. Say which.
- **Part XIX, Bitcoin in OpenAgents.** Two of four chapters are written.
  This part is the book's bridge to the rest of the library. It should end
  in capstone 1: a payment for checked work.
- **A short path.** With 125 chapters planned, offer a path from a
  signature to a payment: about 15 chapters through hashes, the curve,
  Schnorr, a transaction, Taproot and a channel.

**Next.** The book can coast on new protocol chapters. Spend effort on the
opening argument, the product-relationship page and Part XIX.

## Nostr From Scratch

**What works.** Client and relay are both built, which few Nostr texts do.
The interop and verification part compares against other implementations.
The "direct consumer" table in the README, with the scope each consumer
does not cover, is a model of honest reporting.

**What is missing.**

- **The OpenAgents NIPs are the real subject.** Chapters 76–92 cover work
  coordination, capabilities, runs and sessions, agents and skills, jobs
  and markets, decisions, Pylons, attested work, knowledge, worlds, and
  payments for requests. Together they are the agent economy's protocol.
  They sit inside Part VI, "Every NIP", after 31 chapters on the official
  set. Give them their own part with a narrative: how an agent finds work,
  does it, proves it and is paid, one event kind at a time.
- **Two protocols or one.** The README says there is one OpenAgents
  protocol with protobuf schemas, and that Nostr is the open transport.
  This book specifies many OpenAgents behaviors as NIPs. Explain the
  relationship in the introduction.
- **Why Nostr.** As with Bitcoin, the choice is never argued. A page on
  keys instead of accounts, and what that gives an agent, would tie the
  book to the mission.

**Structure.** The first chapter a reader sees is 14, because 15 numbers
moved to the Bitcoin book. That is correct bookkeeping and a confusing
first impression. Renumber when the book next has a stable release. Part
VI's official NIPs are reference material; label them that way and keep the
core path to Parts I–V.

**Next.** Like Bitcoin, this book can coast. The valuable work is the new
part for the OpenAgents NIPs and the links from Aethyr, Coder, Psionic and
Everglade back into it.

## Coder: Coding Agents From Scratch

**What works.** The split between OpenAgents ("we", the router) and Coder
("I", the worker) is clear and useful. The rule that every chapter ends at
an `openagents coder` verb keeps the book tied to the product. Part VII,
which reads other coding agents' source and records what Coder takes from
each, is generous and unusual.

**What is missing.**

- **The agent on the network.** The README's "What's next" says an agent
  will set its price and hours and work overnight for Bitcoin. This book
  has no link to the Nostr or Bitcoin books and no chapter on identity,
  finding paid work, proving it or being paid. Add a part for it. It is
  capstone 3.
- **Coder and the books.** Coder is the agent that reads the other six
  books. Psionic chapters 54–57 build search over the Coax book. This book
  should close that loop with a measured chapter: Coder implements a
  planned chapter from another book, from the book alone.
- **Judgment.** The book teaches mechanisms: loops, logs, tools, sandboxes.
  It says Coder is "opinionated about code: small diffs, tests first,
  honest failures". No chapter teaches those opinions or shows how the
  harness enforces them.

**Structure and voice.** The sample chapter opens with a status block, two
profile identifiers and an evidence table before it says what a harness
is. Of all the books, this one most needs the reader's problem first. The
survey chapters name tool versions and will age quickly. Date each survey
and separate what Coder took (durable) from what the other tool did at
that version (dated). The book is two books in one binding: building a
harness, and a field guide to other agents. That is fine if the contents
say so.

**Next.** Rewrite chapter openings on the practical path the README
already names (1, 2, 9–11, 14 and 23). Draft the network part.

## Building Everglade

**What works.** This is the most reader-friendly design in the library.
Every chapter starts from a question a player asks. Every chapter has a
"Mistakes" section that says what was built wrong first. The tension the
book names, follow the SRD faithfully and play as action combat, is a real
design problem, and "every difference from the SRD is a decision with a
reason" is a strong rule. Part IX, "Run an MMORPG", promises the
operations story that every other book tells only in pieces.

**What is missing.**

- **Its role as the capstone.** Everglade is where every other book meets:
  Coax for everything, Aethyr for the engine, Psionic for the guard's
  decisions, Nostr for identity and tickets, Bitcoin for anything a player
  pays or earns. The book links to Coax once and Aethyr three times, and
  to nothing else. The guard chapters (20–22) should use the Psionic
  decision chapters. A later part should cover what players and makers own
  and earn.
- **The game.** The plan covers land, light, rules, combat, a guard, a
  city and operations. It has little on why someone keeps playing: quests,
  progression, loot, an economy, parties and guilds, and what the city
  remembers. An MMORPG book needs a part on that, even a short one.
- **Assets and sound.** The look comes from a reference image and a
  third-party asset kit. The book should say how assets enter, under what
  license, and how sound (Aethyr Part VIII) is used.
- **SRD attribution.** SRD 5.2.1 is published under CC BY 4.0, which
  requires an attribution statement. I didn't find one in `everglade/book/`.
  Check this and add it to the book's front matter if it is missing.

**Structure.** The book is 16 drafted chapters, all partly implemented.
The contents skip Part IV (chapters 12–13, the room and coming back),
which is the multiplayer part and the one Aethyr Part XII already builds
toward.

**Next.** Everglade deserves the most attention of any book, because it is
the one a new visitor will want to read and it proves the rest. Complete
the first chapters to the promotion standard so the README can stop saying
that no chapter is complete. Draft capstone 1 here.
