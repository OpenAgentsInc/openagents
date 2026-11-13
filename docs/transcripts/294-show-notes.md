# Episode 294 show notes: Bitcoin From Scratch

Drafted 2026-11-11 for Episode 294. This is a talk outline, not a script.
The lines are written to be said out loud, in the voice of Episodes 290 and
293. Read them, change the words, and say them your own way.

The Bitcoin Lab web version goes live with the episode at
`openagents.com/worlds/bitcoin-lab`. Its redesign is being finished today. Walk
the live build before you record and fix the tour below to match what you
see. [Check before recording](#check-before-recording) lists what to confirm.

## The short version

- **The idea:** we built our own game engine, our own ML library, our own
  programming language, our own cryptography and our own Nostr library. The
  next step is our own Bitcoin.
- **The joke:** zero CVEs, zero hacks. So far.
- **Why bother:** we want to own every line, and Coax lets us check that code
  in ways the old stacks can't. Be honest about what Coax is still missing.
- **The tour:** Bitcoin Lab. Walk a block with the Everglade character, see
  what's inside it, and watch the hashing happen.
- **The ask:** point your agents at it. Check our work, add stations, break
  things.

## 1. Cold open: we build everything

> Folks, so here's where we're at. Over the last weeks, months, years, we've
> built our own game engine. Our own machine learning library. Now our own
> programming language, Coax. Our own cryptography library. Our own Nostr
> library.
>
> So the logical next step is: our own Bitcoin implementation.
>
> Maybe Bitcoin. Maybe Lightning. Maybe Ark. Maybe whatever the heck we want.
> But we want to own all the code in here.

On screen: the repo tree, one folder at a time. `aethyr/` (the engine),
`crates/psionic` (ML), `coax/` (the language), `nostr/`, then `bitcoin/`, last
and largest.

## 2. The CVE joke

> You know how many CVEs there have been for our stuff? Zero. Zero hacks.
>
> There are CVEs every day for Bitcoin software and all these services, and
> outages. Ours? None.
>
> Now, that won't last, of course.

Keep the "won't last" line. It's the honest part, and it sets up the ask at
the end: help us find the first one before someone else does.

## 3. Why build our own

Talk to two people at once: a Bitcoiner who doesn't code, and a Bitcoin
developer who'll ask "why go to all that effort?"

> Everyone's vibe coding their own stuff these days anyway. So we may as
> well vibe code our own stuff. Also known as: building on all the
> foundational work we've put in over the last weeks, months and years.

The points, in plain words:

- **We know every line.** When something breaks, there's no upstream to wait
  on. The engine, the language and the Bitcoin code are all in one repo, all
  open source.
- **It's written to be checked.** The book builds Bitcoin from bytes up:
  hashes, keys, signatures, transactions, scripts, then the node and
  Lightning. Each chapter comes with its checks, and the checks are checked
  against the official test vectors and against Bitcoin Core.
- **One codebase, browser to node.** The same Coax code runs in Bitcoin Lab
  in your browser and natively on your machine, checked once.

For the technical listener, three facts from
[the why-Coax analysis](../research/bitcoin/2026-11-11-why-coax-for-bitcoin.md):

- In Coax, every procedure in a checked file must run in a test, and a
  failing check fails the build. Arithmetic is overflow-checked and behaves
  the same in the browser and natively.
- Mutation testing plants deliberate bugs to see whether the tests catch
  them. On the consensus chapter, the tests caught 425 out of 425.
- Coax runs its checks two ways: its own evaluator, and compiled C. The two
  agreed on all 479,316 checks.

## 4. What Coax is missing, and what we're adding

Say this part plainly. It earns trust with the technical crowd.

> Coax doesn't have everything yet. Some things are missing, and we're going
> to add them.

- **Speed.** Our signature checks run about 1.4 times slower than
  libsecp256k1, the library Bitcoin Core uses (measured 2026-11-13). They
  were about 45 times slower when we recorded; the cause was how our field
  arithmetic was written, and we rewrote it.
- **No threads yet.** A node checks scripts in parallel. Coax will get
  parallel checks whose result can't depend on timing.
- **Secrets.** Coax should know which values are secret keys and refuse to
  compile code that leaks them through timing.
- **Fuzzing.** Throwing millions of random inputs at the parser, built into
  the language.
- **Saving channel state safely.** For Lightning, sending a message before
  its state is saved to disk should be a compile error, not a lost-funds bug.

The full ranked list of ten is in the analysis.

## 5. "You want to make Bitcoin? You're going to earn Bitcoin"

> You want to make Bitcoin? Yeah. You're going to earn Bitcoin over here.

Before you record, settle what this means, and say the concrete version.
What pays out today is Pylon compute. Contributor tips and bounties are not
live, so don't promise them as live.

## 6. The Bitcoin Lab tour

> Let's go in. This is Bitcoin Lab. It's live right now; the link's below.
> Click around.

Record it in the browser at full screen, so viewers see exactly what they'll
get from the link.

What's in the world:

| Where you walk | What you see | What to say |
| --- | --- | --- |
| Spawn on the Grid | Your Everglade character on a dark grid, a lit lane straight ahead, glass panels along it. Top left, the station you're at and one line about it. | "Same character, same controls as Everglade. WASD to walk, drag to look." |
| 1. A byte | Eight bits that light up, with their value as a number, as hex and as a letter. | "Everything in Bitcoin is bytes. This is one of them." |
| 2. Hex | The genesis block's 80 header bytes, two hex digits each, coloured by field, with one byte split into its two halves. | "This is what a block header actually looks like on the wire. Eighty bytes." |
| 3. Base64 | Three bytes regrouped into four six-bit values and their letters, animated. | "Same bytes, different way of writing them down." |
| 4. Byte order | The header's time field as it's stored, its bytes reversed into the number, and the date it means: 3 Jan 2009. | "Bitcoin stores numbers backwards. This trips up everybody once." |
| 5. SHA-256 | The 80 header bytes flow into SHA-256 and 32 bytes come out. | "Our own SHA-256, written in Coax, running live in your browser. Not a JavaScript library." |
| 6. SHA-256d | The hash of the hash, its leading zeros lit green, and the target under it. | "That's mining. Find a hash under the target. That's the whole game." |
| 7. The chain | Recorded regtest blocks 95 to 102 as solid cubes along the lane, each face showing its height and hash, each joined to the one before by its previous-block hash. | "Every block points back at the one before it. Change one, and every link after it breaks." |

The stations follow chapter 1 of the book (bytes, hex and base64), then
chapters 4 and 5 (SHA-256 and double SHA-256). Say so, and show the book's
contents for a second: there are 103 chapters written so far.

## 7. How to help

> If you're a developer, or you've got agents: point them at this. Here's
> what they can do.

- **Check our work.** "Have your agents read the book and the code and try
  to prove us wrong." Every chapter lists the vectors it's tested against.
  Find a case we don't cover.
- **Break the parser.** Feed it weird blocks and transactions. If you find a
  crash, that's our first CVE, and we want to hear it from you first.
- **Add a station to Bitcoin Lab.** One per chapter is the dream: keys,
  signatures, addresses, scripts, Taproot. Each one is a small Coax file in
  the world.
- **Compare against Bitcoin Core.** Run the same inputs through both and
  diff the answers.
- **Pick a Coax addition.** The ten in the analysis are sized as issues.

> The repo's open. PRs welcome. Change anything you want.

## 8. Close

> We built a game engine to make games. Now we're building Bitcoin the same
> way: from scratch, in the open, with you. Go walk around Bitcoin Lab. Stay
> tuned.

End card: `openagents.com/worlds/bitcoin-lab` and
`github.com/OpenAgentsInc/openagents`.

## Check before recording

- Walk the live Bitcoin Lab and rewrite the tour table to match it.
- Make sure it plays well on a phone, since many viewers will tap the link
  there.
- Settle what "earn Bitcoin" means (section 5).
- "Maybe Ark": make sure you mean Ark, the layer-two protocol. None of the
  plans cover it yet, so present it as a maybe.
- The numbers here (425 of 425, 479,316 checks, 103 chapters) come from
  the analysis and the repo as of 2026-11-11; the speed was measured again
  on 2026-11-13. Check them again if you record later.

## Sources

- [Bitcoin From Scratch, the book](../../bitcoin/book/README.md)
- [Build-everything audit](../research/bitcoin/2026-11-11-build-everything-audit.md)
- [Why Coax for Bitcoin](../research/bitcoin/2026-11-11-why-coax-for-bitcoin.md)
- [Outside implementation research](../research/bitcoin/chatgpt-pro-implementation-research.md)
- The world's code: `apps/openagents-web/bitcoin-lab*.coax`
