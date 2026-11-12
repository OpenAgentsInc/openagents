# Episode 295 show notes: Nostr From Scratch

Drafted 2026-11-11 for Episode 295, as the follow-up to
[Episode 294, *Bitcoin From Scratch*](294-show-notes.md). This is a talk
outline, not a script. The lines are written to be said out loud, in the
voice of Episodes 290, 293 and 294. Read them, change the words, and say them
your own way.

Episode 294 walked a world, Bitcoin Lab. **Nostr Lab exists now, with four
stations** (key pair, event, ID, signature; #13360, `6bd6a5c82a`). Section 6
gives two tours: one in a terminal and the browser, and one in Nostr Lab.
[Section 7](#7-the-nip-list) is the list of NIPs to read out, with what Nostr
Lab shows for each one and what is still to build before recording (updated
2026-11-12). [Check before recording](#check-before-recording) lists what to
decide.

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

### Tour B: Nostr Lab (four stations built)

The Episode 294 format: a world on the Grid at
`openagents.com/worlds/nostr-lab`, walked with your Everglade character. It
is unlisted: open it by the direct link, or with `?realms=all`.

Built today: a key pair; an event taking shape field by field; the bytes
going into SHA-256 and the ID coming out; the signature, and a changed
letter breaking it. Each value is computed live by the book's own code.

The rest of the walk is one station per NIP in
[section 7](#7-the-nip-list). Those stations are not built yet.

## 7. The NIP list

Read down the list. For each one: "we support this, it's implemented", and
the Nostr Lab station that shows it.

> Here's the official list of NIPs. I'm going to go down it. Everything I
> read out is implemented, in Coax, from scratch, and you can walk up to it
> in Nostr Lab.

The official list is `nostr/nips/official/` (101 files, pinned). "Implemented"
below means the library code exists in `coax/lib/nostr/`, with a book chapter,
a runnable example and a separate check. Chapter numbers are Nostr From
Scratch's.

### Implemented, with a station in Nostr Lab today

| NIP | What it is | Chapter | Station |
| --- | --- | --- | --- |
| 01 | Events, IDs and signatures | 17, 21 | The event, its bytes into SHA-256, the signature, the changed letter |
| 19 | `npub`, `nsec`, `note` and the other bech32 names | 16 | The key pair shown as an npub |

### Implemented and worth showing: stations to build

The code is done. What is missing is the station. Listed in walking order.

**The core: relays**

| NIP | What it is | Chapter | What the station shows |
| --- | --- | --- | --- |
| 01 | `EVENT`, `REQ`, `OK`, `EOSE` over a WebSocket | 23, 24, 26 to 31 | A relay tower: send the event, get `OK`, ask for it back with a filter, check the signature yourself |
| 11 | Relay information document | 37 | The tower's sign: its name and the NIPs it serves |
| 42 | Logging in to a relay | 38 | The relay sends a challenge, you sign it, the gate opens |
| 65 | Relay lists and the outbox model | 32 | A second tower holding the same event; your list says where to find you |
| 09, 40, 62 | Delete, expire, vanish | 39 | An event removed on request, and one that expires by itself |
| 77 | Negentropy sync | 43 | Two towers compare what they hold and trade only what's missing |
| 13 | Proof of work | 56 | A nonce counting up until the ID starts with enough zeros |

**Keys and logging in**

| NIP | What it is | Chapter | What the station shows |
| --- | --- | --- | --- |
| 06 | Keys from seed words | 46 | Twelve words become a Nostr key (and the same words can give a Bitcoin wallet) |
| 49 | Password-encrypted key backup (`ncryptsec`) | 47 | The key locked with a password, and a wrong password refused |
| 46 | Remote signing | 48 | A signer in another place signs for the app; the app never holds the key |
| 98 | HTTP auth | 49, 99 | A signed web request: how you log in to openagents.com with a key |
| 05 | Names like `you@openagents.com` | 45 | A name looked up and matched to a key |

**Private messages and encrypted group chat**

| NIP | What it is | Chapter | What the station shows |
| --- | --- | --- | --- |
| 44 | Encrypted payloads | 14, 20 | A message crossing between two keys; the relay sees noise |
| 59 | Gift wrap | 25 | The message sealed, then wrapped with a one-time key, so the relay can't see who sent it |
| 17 | Private messages and group chat | 60 | Alice writes to Bob and Carol: three wraps, one each, and an outsider's key opens none |

Say "encrypted group chat" for NIP-17. A group is the set of people the
message names; each one gets their own wrapped copy. The MLS group chat NIP
(NIP-EE) is marked `unrecommended` upstream and is out of scope below.

**Lightning payments**

| NIP | What it is | Chapter | What the station shows |
| --- | --- | --- | --- |
| 57 | Zaps | 71 | A zap request, a Lightning invoice that commits to it, and the receipt checked against both |
| 47 | Nostr Wallet Connect | 72 | An app asks a wallet to pay over encrypted events; the wallet answers |
| 75 | Zap goals | 71 | A goal filling as receipts are counted |
| A3 | `payto` payment targets | 73 | A profile's Bitcoin and Lightning targets (others ignored: Bitcoin only) |

Run these with test invoices. Don't present them as live payments.

**What people build on it**

| NIP | What it is | Chapter | What the station shows |
| --- | --- | --- | --- |
| 02 | Follow list | 45 | A follow list, and the newest one replacing the old |
| 10, 22, 25 | Threads, comments, reactions | 50, 52 | A note, a reply and a like, linked by tags |
| 23 | Long-form articles | 53 | An article that can be edited: same address, newer event |
| 51 | Lists | 54 | A bookmark list with a public part and an encrypted part |
| 29 | Relay-based groups | 59 | A group the relay runs: join, post, and a moderator removing a post |
| 34 | Git on Nostr | 63, 98 | A repository, an issue and its status: what the project pages on openagents.com read |
| 90 | Data vending machines | 65 | A job request, a result and the feedback: paying a machine for work |
| 58 | Badges | 62 | A badge defined, awarded and worn |
| 88 | Polls | 58 | A poll and one vote per key |
| 50, 45 | Search and count | 40, 41 | Ask a relay to search, and to count |

That is 29 stations after the four built ones. If time is short, build in
this order and stop where the day ends: relays (01, 11, 42, 65), private
messages (44, 59, 17), Lightning (57, 47), keys (06, 49, 98), then the rest.

### Not implemented, and we should have it

Small ones from the official list that a listener would expect. Each is a
codec over events we already handle.

| NIP | What it is | Why |
| --- | --- | --- |
| C7 | Chats (kind 9) | The message kind NIP-29 groups use; the group station needs it |
| 7D | Forum threads (kind 11) | Same: the other thing people post in groups |
| A4 | Public messages (kind 24) | Chapter 60 describes it and stops |
| 39 | Linking a profile to other platforms | Chapter 45 says "not implemented" |
| C0 | Code snippets (kind 1337) | Chapter 63 lists it as not built; it fits the Git station |
| 34 | Pull requests (kinds 1618, 1619) | The rest of NIP-34; chapter 63 lists them as not built |

Also open in the book, outside the NIP list:

- **Chapter 99** (the website and sign-in) cites a file that was deleted
  with the old login, so the book's check fails on it (#13362).
- **Chapter 104** (agent treasuries, NIP-KEYS) is partly implemented.
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
- **Implemented, but not worth a station:** 03, 18, 21, 24, 27, 30, 32, 35,
  36, 37, 38, 48, 52, 53, 54, 56, 5A, 64, 66, 67, 68, 69, 70, 71, 73, 78,
  84, 85, 86, 89, 92, 94, 99, A0, B0, B7, CC, CD and F4. One line covers
  them: "and about forty more: calendars, wikis, live streams, statuses,
  podcasts, chess."
- **Block's NIPs** (`nostr/nips/block/`, chapters 87 to 92), except one
  station: **NIP-OA**, an owner key authorizing an agent key.
- **The OpenAgents NIPs** (`nostr/nips/openagents/`, chapters 75 to 86),
  except one station: **NIP-PYLON**, a Pylon's beacon and a signed receipt
  for work it did. Section 8 names the others without showing them.

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

- **Pick the tour.** Tour A works today. Tour B has four stations; the rest
  are listed in section 7 and are not built yet.
- **The NIP list (section 7):** read out only the NIPs whose station is
  built by the time you record. The tables say which those are.
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
