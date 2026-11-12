# Episode 295 show notes: Nostr From Scratch

Drafted 2026-11-11 for Episode 295, as the follow-up to
[Episode 294, *Bitcoin From Scratch*](294-show-notes.md). This is a talk
outline, not a script. The lines are written to be said out loud, in the
voice of Episodes 290, 293 and 294. Read them, change the words, and say them
your own way.

Episode 294 walked a world, Bitcoin Lab. **There is no Nostr Lab yet.**
Section 6 gives two tours: one of what's live today, and one of a Nostr Lab
if you want it built before you record.
[Check before recording](#check-before-recording) lists what to decide.

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

### Tour B: Nostr Lab (not built)

The Episode 294 format: a world on the Grid at
`openagents.com/worlds/nostr-lab`, walked with your Everglade character.
Stations in order: a key pair; an event taking shape field by field; the
bytes flowing into SHA-256 and the ID coming out; the signature; a relay as
a tower you send the event to; a second relay holding the same event; a
changed letter breaking the signature; an encrypted message crossing between
two keys.

It would reuse Bitcoin Lab's stations for bytes, SHA-256 and the walking
controls. If you want this tour, it needs building first.

## 7. The OpenAgents NIPs

Keep this short. It's the bridge to the next episodes.

> Nostr gives you keys, events and relays. We wrote the rest of what an
> agent needs on top, as open specs anyone can use.

> How an agent finds work, does it, proves it, and gets paid. All of it
> signed events.

The book's chapters 76 to 92 cover our own NIPs. Four to name: jobs and
markets (80), Pylons (82), attested work (83) and Lightning payments for
requests (86). Chapter 102 runs one agent through all of it, offline.

Say "offline" out loud. Don't present the full market as live.

## 8. How to help

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

## 9. Close

> Bitcoin last time, Nostr this time. Money and identity, both from
> scratch, both in the open, both on keys you hold. Next we put agents on
> top of them. Stay tuned.

End card: `github.com/OpenAgentsInc/openagents`, and the link for whichever
tour you recorded.

## Check before recording

- **Pick the tour.** Tour A works today. Tour B needs Nostr Lab built first.
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
