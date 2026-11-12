# Nostr Lab

An unlisted learning world at https://openagents.com/worlds/nostr-lab, made
for Episode 295, *Nostr From Scratch*. You come in on a plaza on the Grid
with your Everglade character. Nine avenues leave it, one per district, each
with its name in light on the plaza floor. There are forty-two stations in
all.

## Getting around

- **INDEX** (or **M**) lists every district and its stations. Choose a
  station to go straight to it.
- **NEXT** and **PREV** (or **]** and **[**) step through the stations in
  order, district by district. The plaza comes before the first and after
  the last.
- Every station has its own address, such as
  [`?at=nip-44`](/worlds/nostr-lab?at=nip-44). The address bar follows you,
  so a link opens where you were. A district's address, such as
  [`?at=zaps`](/worlds/nostr-lab?at=zaps), opens its first station, and
  `?at=start` opens the plaza.

## 1 Start here: an event from nothing

1. **Key pair** (`?at=key-pair`): a secret key's 32 bytes, never shown, and
   the public key it makes, as hex and as an `npub`.
2. **An event** (`?at=event`): a kind 1 note taking shape field by field:
   `pubkey`, `created_at`, `kind`, `tags` and `content`.
3. **The ID** (`?at=id`): the event serialized as NIP-01's
   `[0, pubkey, created_at, kind, tags, content]`, those bytes into SHA-256,
   and the 32-byte event ID out.
4. **The signature** (`?at=signature`): the BIP-340 signature over the ID,
   and its check. Then one letter of the content changes: the bytes hash to
   a different ID, and the same signature fails.

Each station after these shows who sends what to whom, the values the
library worked out, what held, and what was refused.

## 2 Relays

5. **NIP-01, a relay**: send the event, get `OK`, ask for it back with a
   filter, and check the signature yourself. A changed copy is refused.
6. **NIP-11, relay information**: the document a relay serves about itself
   and the NIPs it supports.
7. **NIP-42, relay login**: the relay sends a challenge and you sign it. An
   old answer to a new challenge is refused.
8. **NIP-65, two relays**: your relay list, the same note on two relays, and
   the note still read after one relay is lost.
9. **NIP-77, sync**: two relays compare what they hold with Negentropy and
   learn only what each lacks.
10. **NIP-50, search** (with NIP-45): a relay searching and counting.

## 3 Events and IDs

11. **NIP-09, delete** (with NIP-40 and NIP-62): the author deletes a note,
    a note expires, and a request to vanish removes all of its signer's
    notes. Someone else's deletion request does nothing.
12. **NIP-13, proof of work**: a nonce counted up until the ID starts with
    enough zero bits.
13. **NIP-23, articles**: an article you can edit: same address, newer event.
14. **NIP-51, lists**: a bookmark list with a public part and a private part.

## 4 Keys and login

15. **NIP-06, seed words**: twelve words make a Nostr key.
16. **NIP-49, key backup**: the key locked with a password. A wrong password
    is refused.
17. **NIP-46, remote signer**: a signer elsewhere signs, and the app never
    holds the key.
18. **NIP-98, HTTP auth**: a signed web request, which is how you log in to
    openagents.com with a key.
19. **NIP-05, names**: a name like `you@openagents.com` matched to a key.

## 5 Messages

20. **NIP-44, encryption**: a message between two keys. The relay sees noise.
21. **NIP-59, gift wrap**: the message sealed, then wrapped with a one-time
    key, so a relay cannot see who sent it.
22. **NIP-17, group chat**: one message to Bob and Carol, a wrapped copy
    each. An outsider's key opens none.
23. **NIP-C7, chats**: a chat message, and a reply that quotes it.
24. **NIP-A4, public messages**: a message to someone that anyone can read.

## 6 Payments and zaps (test invoices; nothing is paid)

25. **NIP-57, zaps**: a zap request, an invoice that commits to it, and the
    receipt checked against both.
26. **NIP-47, Wallet Connect**: an app asks a wallet to pay over encrypted
    events.
27. **NIP-75, zap goals**: a goal filling as receipts are counted.
28. **NIP-A3, payment targets**: a profile's Bitcoin and Lightning targets.
    Other kinds of money are ignored.

## 7 Social

29. **NIP-02, follows**: a follow list, and the newest one replacing the old.
30. **NIP-39, profiles**: a profile, and the other accounts it claims.
31. **NIP-58, badges**: a badge defined, awarded and worn.
32. **NIP-88, polls**: a poll, and one vote per key.

## 8 Threads and groups

33. **NIP-10, threads** (with NIP-25): a note, a reply and a like, linked by
    tags.
34. **NIP-22, comments**: a comment on anything: a note, an article, a web
    page.
35. **NIP-29, groups**: a group the relay runs: join, post, moderate.
36. **NIP-7D, forum threads**: a thread with a title, and its replies.

## 9 Code and agents

37. **NIP-34, Git**: a repository, an issue and its status.
38. **NIP-34, pull requests** (`?at=nip-34-pr`): a pull request on a
    repository, and its update.
39. **NIP-C0, code snippets**: a code snippet, with its language.
40. **NIP-90, jobs**: a job request, a price, and the result matched to its
    request.
41. **NIP-OA** (Block's): an owner's key authorizes an agent's key.
42. **NIP-PYLON** (OpenAgents'): a Pylon's beacon, read fresh or stale, and
    the receipt its buyer signs for the work.

Every key, byte, hash, signature and answer is computed in Coax as the page
runs, by the Nostr library (`coax/lib/nostr/`) and the shared crypto. The
keys are published test keys (BIP-340's test vectors and the ones the book's
examples use); none is a real key, and the world never shows a secret key.

Read more in [Nostr From Scratch](https://github.com/OpenAgentsInc/openagents/blob/main/nostr/book/README.md),
the book these stations follow.

Use WASD or the arrow keys to walk, Shift to walk slowly and Space to jump;
drag to turn the follow camera. On touch, drag the left side to walk and the
right side to look, and tap INDEX, PREV and NEXT under the panel at the top
left. WORLDS (top right) goes back to [Worlds](/worlds).
