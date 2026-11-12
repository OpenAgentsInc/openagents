# Episode 295 show notes: Nostr From Scratch

Drafted 2026-11-11 for Episode 295, as the follow-up to
[Episode 294, *Bitcoin From Scratch*](294-show-notes.md). This is a talk
outline, not a script. The lines are written to be said out loud, in the
voice of Episodes 290, 293 and 294. Read them, change the words, and say them
your own way.

Episode 294 walked a world, Bitcoin Lab. **Nostr Lab is built: 42 stations,
one for each NIP read out below** (#13360, #13366). Section 6 gives two
tours: one in a terminal and the browser, and one in Nostr Lab.
[Section 7](#7-the-nip-list) is the list of NIPs to read out, with the
station that shows each one (updated 2026-11-12).
[Check before recording](#check-before-recording) lists what to decide.

A first cut of the talk is recorded ([295.md](295.md), cut 1). Its footage
windows are placeholders. [What the cut names](#what-cut-1-names-and-the-station-for-it)
gives the station for each thing said in it.

## The short version

- **The idea:** last time, our own Bitcoin. This time, our own Nostr, built
  on the same cryptography, from the first byte to a client and a relay.
- **The line:** "No blockchain garbage, just data. Signed JSON over
  WebSockets. Very easy." (Episode 289.) This episode shows how easy.
- **Why it matters for agents:** a key is the identity. An agent signs its
  work, publishes it to any relay, and anyone can check it without asking
  us.
- **The tour:** make a key, sign an event by hand, send it to a relay, read
  it back, then log in to openagents.com with that key.
- **The ask:** point your agents at the book and the NIPs. Check our work,
  run a relay, write a client.

## 1. Cold open: same keys, next layer

> Last episode we built Bitcoin from scratch. Bytes, hashes, keys,
> signatures. Here's the thing: Nostr runs on the exact same cryptography.
> Same curve, same signatures. So we already built the hard part.
>
> This episode: Nostr from scratch. Every line ours, in Coax.

On screen: the repo tree again, ending on `nostr/` next to `bitcoin/`. Then
the two books side by side: Bitcoin From Scratch and Nostr From Scratch.

The fact behind the line: SHA-256, secp256k1, BIP-340 Schnorr signatures and
Bech32 are built in the Bitcoin book. The Nostr book starts from there.

## 2. What Nostr is, in one minute

Talk to someone who has heard the word and nothing else.

> Nostr is three things. A key. A message you sign with that key. And a
> server that passes messages along.
>
> That's it. No blockchain. No token. No company in the middle.

The points, in plain words:

- **An event** is a small piece of JSON: who wrote it, when, what kind it
  is, some tags, the content, and a signature over all of it.
- **A relay** is a server that takes events and hands them to whoever asks.
  Anyone can run one.
- **A client** connects to relays over a WebSocket, publishes events and
  subscribes to the ones it wants.

> Signed JSON over WebSockets. Very easy.

Show one real event on screen, pretty-printed, and point at each field as
you name it.

## 3. Why this matters for agents

> So why do we care? Because of what a signature gives you.

Three properties, from
[the OpenAgents book, chapter 46](../../openagents/book/ch046-signed-json-over-websockets.md):

- **It's signed.** You know who said it without trusting the relay that
  handed it to you.
- **It's plain data.** Any program in any language can read it.
- **Relays can be replaced.** The same event can sit on many. Lose one and
  you lose nothing.

> An agent has a key. Everything it does, it signs. Anybody can read it,
> anybody can check it, and nobody can turn the account off, including us.

Then the one-seed line, if you want it: the same seed can give you a Bitcoin
wallet and a Nostr identity. It's opt-in for people.

## 4. Why build our own

Same two listeners as 294: someone who doesn't code, and a Nostr developer
who'll ask why we didn't use an existing library.

> There are good Nostr libraries. We used them for years. We wrote our own
> anyway, for the same reason we wrote our own Bitcoin: we want to know
> every line, and we want it checked.

- **One codebase, browser to relay.** The same Coax code signs an event in
  your browser, in the Mac app and on the server.
- **Written to be checked.** Every chapter comes with a worked example, a
  runnable example, the refusals shown in its output, and a separate check.
- **Checked against the outside world.** libsecp256k1, the library Bitcoin
  Core uses, agrees with our code on 10,000 signatures and 300 signed
  events.

For the technical listener, from [the handoff note](../../nostr/HANDOFF.md):

- The book has 104 chapters. 102 are implemented; 103 and 104 are partly
  done.
- The last full run of every example passed, 204 runs.
- Mutation testing plants deliberate bugs to see whether the tests catch
  them. Every authority file the book added has zero survivors.
- Only one thing is not ours: the C host may link one TLS library, so
  native clients can reach `wss://` relays. WebSocket framing and everything
  above it is Coax.

## 5. What's missing

Say this part plainly, as in 294.

> It's not all done. Here's what isn't.

- **Our relay isn't the live one yet.** `relay.openagents.com` still runs
  the earlier relay. The importer and the comparison tools are built and
  checked offline; the cutover hasn't happened.
- **Two chapters are partly built:** moving the live relay (103) and keys
  (104).
- **The relay holds no signing key yet,** so relay-signed groups aren't
  wired.
- **No ecash.** Cashu and Fedimint are recognised and never built. Money on
  our Nostr is Lightning and on-chain Bitcoin only.

## 6. The tour

Pick one before recording.

### Tour A: what's live today

Record in a terminal and the browser, full screen.

| Step | What you show | What to say |
| --- | --- | --- |
| 1. A key | Make a key pair. Show the secret as `nsec...` and the public key as `npub...`. | "This is your identity. Nobody gave it to you. Nobody can take it." |
| 2. An event by hand | Write the JSON for a short note. Show the exact bytes that get hashed, then the ID. | "The ID is just a hash of the message. Same SHA-256 from last episode." |
| 3. Sign it | Sign the ID. Show the 64-byte signature. | "Same Schnorr signature Bitcoin uses for Taproot. We built it last episode." |
| 4. Send it | Open a WebSocket to a relay, send `EVENT`, get `OK` back. | "Signed JSON over a WebSocket. That's the whole network protocol." |
| 5. Read it back | Send `REQ` with a filter, get the event, verify the signature. | "I don't trust the relay. I check the signature myself." |
| 6. Break it | Change one letter of the content and verify again. It fails. | "Change anything and the signature breaks." |
| 7. A private message | Encrypt a note to another key with NIP-44 and show the ciphertext. | "Same keys, private this time. The relay sees noise." |
| 8. Log in with it | Go to `openagents.com/login` and log in with the key. | "No password. No phone number. Your key is your account." |
| 9. The Grid | Open the Grid and show the projects updating. | "Everything you see here arrives as signed events." |

Steps 1 to 7 follow the book's core path: shared secrets, the event model,
the transport, a client and a relay (chapters 14 to 44).

### Tour B: Nostr Lab (built)

The Episode 294 format: a world on the Grid at
`openagents.com/worlds/nostr-lab`, walked with your Everglade character. It
is unlisted: open it by the direct link, or with `?realms=all`.

One lane, 42 stations, left and right in turn. The first four build an event
from nothing: a key pair; an event taking shape field by field; the bytes
going into SHA-256 and the ID coming out; the signature, and a changed
letter breaking it. Then one station a NIP, in the order of
[section 7](#7-the-nip-list).

How a NIP's station reads:

- **The strip of boxes** at the top says who sends what to whom (ALICE,
  RELAY, BOB), with a light running along it.
- **Up to six rows**, each a small label and its value: the kind, the tag
  that carries the meaning, the ciphertext, the relay's answer.
- **A green line**: what held. **A red line**: what was refused.

Every value is worked out in the page by the same Coax library the book
builds. The green and red lines are only shown after the library gave that
answer. Stop walking at a station and the camera turns to it.

To get pictures or footage without walking it by hand, on a local build:

```sh
coax/web/tool.sh nostr-lab-check http://127.0.0.1:PORT OUT_DIR             # a picture of every station
coax/web/tool.sh nostr-lab-check http://127.0.0.1:PORT OUT_DIR --frames    # and every frame, for a video
```

## 7. The NIP list

Read down the list. For each one: "we support this, it's implemented", and
the Nostr Lab station that shows it.

> Here's the official list of NIPs. I'm going to go down it. Everything I
> read out is implemented, in Coax, from scratch, and you can walk up to it
> in Nostr Lab.

The official list is `nostr/nips/official/` (101 files, pinned). "Implemented"
below means the library code exists in `coax/lib/nostr/`, with a book chapter,
a runnable example and a separate check. Chapter numbers are Nostr From
Scratch's. Station numbers are Nostr Lab's, in walking order.

### Implemented, each with its station

**From nothing to a signed event**

| Station | NIP | What it shows | Chapter |
| --- | --- | --- | --- |
| 1 | 19 | A key pair: the secret hidden, the public key as hex and `npub` | 16 |
| 2 | 01 | An event taking shape field by field | 17, 21 |
| 3 | 01 | The event's bytes into SHA-256, the ID out | 21 |
| 4 | 01 | The BIP-340 signature; one changed letter and it fails | 21 |

**Relays**

| Station | NIP | What it shows | Refused |
| --- | --- | --- | --- |
| 5 | 01 | `EVENT` to a relay, `OK` back, `REQ`, the event and `EOSE`, the reader checks the signature | A copy with one letter changed |
| 6 | 11 | The relay's information document: its name and the NIPs it serves | A document cut short |
| 7 | 42 | The relay's challenge, your signed kind 22242 answer, login accepted | An old answer to a new challenge |
| 8 | 65 | Your relay list, a note on two relays, one relay lost and the note still read | A list with a bad marker |
| 9 | 09, 40 | The author deletes a note; a note expires at its time | Someone else's deletion request |
| 10 | 77 | Two relays compare what they hold and learn only what each lacks | A message cut short |
| 11 | 13 | A nonce counted up until the ID starts with 10 zero bits | Work claimed and not done |

The relay in these stations is our relay's own code (admission, store,
routing) running in the page.

**Private messages and encrypted group chat**

| Station | NIP | What it shows | Refused |
| --- | --- | --- | --- |
| 12 | 44 | Alice encrypts to Bob; the relay sees noise; Bob reads it | A changed payload, or the wrong key |
| 13 | 59 | A message sealed, then wrapped with a one-time key; Bob opens it | The wrong key |
| 14 | 17 | One message to Bob and Carol: three wraps, each a new key; all read it | An outsider's key opens none |

Say "encrypted group chat" for NIP-17. A group is the set of people the
message names; each one gets their own wrapped copy. The MLS group chat NIP
(NIP-EE) is marked `unrecommended` upstream and is out of scope below.

**Lightning payments** (test invoices: say so, nothing is paid)

| Station | NIP | What it shows | Refused |
| --- | --- | --- | --- |
| 15 | 57 | A zap request, an invoice that commits to it, the receipt checked against both | The invoice of another zap |
| 16 | 47 | An app asks a wallet over encrypted events; the wallet answers | A payment over the daily budget |
| 17 | 75 | A goal of 50,000 msat filled by two zap receipts | A receipt the server didn't sign |
| 18 | A3 | A profile's Bitcoin and Lightning targets | A target that isn't Bitcoin |

**Keys and logging in**

| Station | NIP | What it shows | Refused |
| --- | --- | --- | --- |
| 19 | 06 | Twelve test words become a key; it matches NIP-06's test vector | A wrong last word |
| 20 | 49 | A key locked with a password as `ncryptsec`, and unlocked | The wrong password |
| 21 | 46 | A signer elsewhere signs an event for the app | An app key the signer doesn't know |
| 22 | 98 | A signed web request to openagents.com, accepted | Another URL, or 61 seconds old |
| 23 | 05 | `you@openagents.com` looked up and matched to a key | A document naming another key |

**What people build on it**

| Station | NIP | What it shows | Refused |
| --- | --- | --- | --- |
| 24 | 02 | A follow list with a relay hint and a petname; a newer list replaces it | An older list arriving late |
| 25 | 10, 25 | A note, a reply and a like, linked by tags | A like with no `e` tag |
| 26 | 23 | An article and its edit at the same address | An article with no `d` tag |
| 27 | 51 | A bookmark list with a public part and an encrypted part | Another key opening the private part |
| 28 | 29 | A group the relay runs: join, post, a moderator removes the post | A post from a non-member |
| 29 | 34 | A repository, an issue, and the maintainer's status deciding it | A stranger closing the issue |
| 30 | 90 | A job request, a price, the result matched to its request | A price over the bid |
| 31 | 58 | A badge defined, awarded and worn | An award by another key |
| 32 | 88 | A poll; one vote a key, the newest counts | A vote after the end |
| 33 | 50, 45 | A relay searching, best match first, and counting | A search that is a number |
| 34 | 22 | A comment on an article, a reply to it, and a comment on a web page | A comment with no root tags |
| 35 | C7 | A kind 9 chat message and a reply that quotes it | A quote with a short ID |
| 36 | 7D | A kind 11 forum thread with a title, and a reply | A reply to a reply |
| 37 | A4 | A kind 24 public message to two people, readable by anyone | A message with an `e` tag |
| 38 | 39 | A profile, the GitHub account it claims, and where the proof must be | A platform name in capitals |
| 39 | 34 | A pull request and its update | An update signed by someone else |
| 40 | C0 | A kind 1337 code snippet with its language, name and licence | A language in capitals |

Stations 35 to 40 are NIPs that were not implemented when these notes were
first drafted. They are now, each with checks and a book section.

**One each from the other two NIP sets**

| Station | NIP | What it shows | Refused |
| --- | --- | --- | --- |
| 41 | NIP-OA (Block's) | An owner's key signs a tag that lets an agent's key act for it | The tag copied by another key |
| 42 | NIP-PYLON (ours) | A test Pylon's beacon, read fresh; the receipt its buyer signs | A beacon 301 seconds old |

At station 42, say "a test Pylon". The buyer signs the receipt, not the
Pylon. Don't present the market as live.

### What cut 1 names, and the station for it

The first cut ([295.md](295.md), 0:52 to 1:10) lists these. Each window
there is a placeholder; this is the station to film for it.

| Said | Station |
| --- | --- |
| "the keys" | 1 (and 19, 20 for seed words and backup) |
| "private messages" | 12, 13 |
| "encrypted group chat" | 14 |
| "Lightning payments" | 15 (and 16, 17, 18) |
| "follows" | 24 |
| "threads, comments, reactions" | 25, 34 |
| "long-form articles" | 26 |
| "lists" | 27 |
| "groups" | 28 |
| "git" | 29 |
| "badges" | 31 |
| "polls" | 32 |
| "searches" | 33 |
| "chats" | 35 |
| "forum threads" | 36 |
| "public messages" | 37 |
| "profiles" | 38 (and 23 for names) |
| "git pull requests" | 39 |

For "signed JSON over WebSockets" (0:05, 0:25), film stations 2 to 5.

### Still open in the book

- **Chapter 103** (replace `relay.openagents.com`) belongs to the relay
  replacement, #13361, which is in progress.

### Out of scope for this episode

Don't read these out as supported, and no station.

- **Ecash, never built:** 60 (Cashu wallet), 61 (nutzaps) and 87 (mint
  discovery). Bitcoin and Lightning only.
- **Marked `unrecommended` upstream:** 04 (old encrypted DMs; we decrypt
  only, to read old messages), 26 (delegation), 96 (HTTP file storage), EE
  (MLS group chat, replaced upstream by the Marmot protocol) and BE
  (Bluetooth). If you want MLS groups, that is a build of its own (RFC 9420),
  not a station.
- **Platform signers:** 07 (browser extension) and 55 (Android signer app).
  The book describes them; the signing happens in someone else's program.
  Logging in with a browser extension is in Tour A, step 8.
- **Relay features waiting on a relay key:** 43 (relay access requests) and
  NIP-29's relay-signed group events on the live relay.
- **Implemented, but no station:** 03, 18, 21, 24, 27, 30, 32, 35, 36, 37,
  38, 48, 52, 53, 54, 56, 5A, 62, 64, 66, 67, 68, 69, 70, 71, 73, 78, 84,
  85, 86, 89, 92, 94, 99, A0, B0, B7, CC, CD and F4. One line covers them:
  "and about forty more: calendars, wikis, live streams, statuses,
  podcasts, chess."
- **Block's NIPs** (`nostr/nips/block/`, chapters 87 to 92), except
  station 41.
- **The OpenAgents NIPs** (`nostr/nips/openagents/`, chapters 75 to 86),
  except station 42. Section 8 names the others without showing them.

## 8. The OpenAgents NIPs

Keep this short. It's the bridge to the next episodes.

> Nostr gives you keys, events and relays. We wrote the rest of what an
> agent needs on top, as open specs anyone can use.

> How an agent finds work, does it, proves it, and gets paid. All of it
> signed events.

The book's chapters 76 to 92 cover our own NIPs. Four to name: jobs and
markets (80), Pylons (82), attested work (83) and Lightning payments for
requests (86). Chapter 102 runs one agent through all of it, offline.

Say "offline" out loud. Don't present the full market as live.

## 9. How to help

> If you're a developer, or you've got agents: point them at this.

- **Check our work.** Every chapter lists what it's tested against. Find a
  case we don't cover.
- **Break the parser.** Send our relay code strange events and strange
  JSON. If you find a crash, we want to hear it first.
- **Run it against your relay or your client.** Diff the answers against
  the library you use today.
- **Write a client.** A small one is an afternoon. The book takes you
  there.
- **Read the NIPs.** Tell us where ours are wrong, or where an existing NIP
  already does the job.

> The repo's open. PRs welcome.

## 10. Close

> Bitcoin last time, Nostr this time. Money and identity, both from
> scratch, both in the open, both on keys you hold. Next we put agents on
> top of them. Stay tuned.

End card: `github.com/OpenAgentsInc/openagents`, and the link for whichever
tour you recorded.

## Check before recording

- **Pick the tour.** Both work. Tour B is 42 stations; section 7 says what
  each shows.
- **Production:** the stations are on `main`. Check that
  `openagents.com/worlds/nostr-lab` shows 42 stations before you record
  there; until a release ships them it shows four.
- **Tour A, steps 1 to 7:** choose the commands. The book's runnable
  examples are in `nostr/examples/` (`events.coax`, `filters.coax`,
  `ecdh.coax`, `gift-wrap.coax`). Run them before recording, and decide
  whether a small command-line tool would show it better.
- **Tour A, step 4:** choose the relay. Our own Coax relay is not the live
  `relay.openagents.com` yet, so say which one you're talking to.
- **Tour A, step 8:** logging in with a Nostr key shipped on 2026-11-11.
  Try it on production first, with a browser extension that holds the key.
- **Tour A, step 9:** confirm the Grid's projects still update from Nostr
  before you say so.
- **"We used them for years":** true of the earlier Rust crates (`nostr`,
  `nostr-relay`, `nostr-transport`). Change the line if you'd rather not
  say it.
- **Numbers** (104 chapters, 102 implemented, 204 runs, 10,000 signatures,
  300 events) come from the handoff note of 2026-11-02. Check them again
  before recording.
- **The Episode 294 transcript** ([294.md](294.md)) was added after these notes
  were drafted from its show notes. The recording is shorter: fix anything
  here that it changed.

## Sources

- [Nostr From Scratch, the book](../../nostr/book/README.md) and its
  [handoff note](../../nostr/HANDOFF.md)
- [The OpenAgents book, chapter 46: Signed JSON over WebSockets](../../openagents/book/ch046-signed-json-over-websockets.md)
- [Episode 294 show notes: Bitcoin From Scratch](294-show-notes.md)
- [Episode 289, *OpenAgents*](289.md): 2:30, signed JSON over WebSockets
- [The OpenAgents NIPs](../../nostr/nips/openagents/README.md)
- [libsecp256k1 cross-check](../verification/2026-11-01-libsecp-crosscheck.md)
- The library: `coax/lib/nostr/`
