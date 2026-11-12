# Nostr Lab

An unlisted learning world at https://openagents.com/worlds/nostr-lab, made
for Episode 295, *Nostr From Scratch*. You walk your Everglade character down
a lit lane on the Grid. Forty-two stations stand beside it.

## The first four: an event from nothing

1. **Key pair**: a secret key's 32 bytes, never shown, and the public key it
   makes, as hex and as an `npub`.
2. **An event**: a kind 1 note taking shape field by field: `pubkey`,
   `created_at`, `kind`, `tags` and `content`.
3. **The ID**: the event serialized as NIP-01's
   `[0, pubkey, created_at, kind, tags, content]`, those bytes into SHA-256,
   and the 32-byte event ID out.
4. **The signature**: the BIP-340 signature over the ID, and its check. Then
   one letter of the content changes: the bytes hash to a different ID, and
   the same signature fails.

## Then a station for each NIP

Each station shows who sends what to whom, the values the library worked
out, what held, and what was refused.

**Relays**

5. **NIP-01, a relay**: send the event, get `OK`, ask for it back with a
   filter, and check the signature yourself. A changed copy is refused.
6. **NIP-11, relay information**: the document a relay serves about itself
   and the NIPs it supports.
7. **NIP-42, relay login**: the relay sends a challenge and you sign it. An
   old answer to a new challenge is refused.
8. **NIP-65, two relays**: your relay list, the same note on two relays, and
   the note still read after one relay is lost.
9. **NIP-09, delete** (and NIP-40): the author deletes a note, and a note
   expires. Someone else's deletion request does nothing.
10. **NIP-77, sync**: two relays compare what they hold with Negentropy and
    learn only what each lacks.
11. **NIP-13, proof of work**: a nonce counted up until the ID starts with
    enough zero bits.

**Private messages**

12. **NIP-44, encryption**: a message between two keys. The relay sees noise.
13. **NIP-59, gift wrap**: the message sealed, then wrapped with a one-time
    key, so a relay cannot see who sent it.
14. **NIP-17, group chat**: one message to Bob and Carol, a wrapped copy
    each. An outsider's key opens none.

**Lightning** (test invoices; nothing is paid)

15. **NIP-57, zaps**: a zap request, an invoice that commits to it, and the
    receipt checked against both.
16. **NIP-47, Wallet Connect**: an app asks a wallet to pay over encrypted
    events.
17. **NIP-75, zap goals**: a goal filling as receipts are counted.
18. **NIP-A3, payment targets**: a profile's Bitcoin and Lightning targets.
    Other kinds of money are ignored.

**Keys and logging in**

19. **NIP-06, seed words**: twelve words make a Nostr key.
20. **NIP-49, key backup**: the key locked with a password. A wrong password
    is refused.
21. **NIP-46, remote signer**: a signer elsewhere signs, and the app never
    holds the key.
22. **NIP-98, HTTP auth**: a signed web request, which is how you log in to
    openagents.com with a key.
23. **NIP-05, names**: a name like `you@openagents.com` matched to a key.

**What people build on it**

24. **NIP-02, follows**: a follow list, and the newest one replacing the old.
25. **NIP-10, threads** (with NIP-25): a note, a reply and a like, linked by
    tags.
26. **NIP-23, articles**: an article you can edit: same address, newer event.
27. **NIP-51, lists**: a bookmark list with a public part and a private part.
28. **NIP-29, groups**: a group the relay runs: join, post, moderate.
29. **NIP-34, Git**: a repository, an issue and its status.
30. **NIP-90, jobs**: a job request, a price, and the result matched to its
    request.
31. **NIP-58, badges**: a badge defined, awarded and worn.
32. **NIP-88, polls**: a poll, and one vote per key.
33. **NIP-50, search** (with NIP-45): a relay searching and counting.

34. **NIP-22, comments**: a comment on anything: a note, an article, a web
    page.
35. **NIP-C7, chats**: a chat message, and a reply that quotes it.
36. **NIP-7D, forum threads**: a thread with a title, and its replies.
37. **NIP-A4, public messages**: a message to someone that anyone can read.
38. **NIP-39, profiles**: a profile, and the other accounts it claims.
39. **NIP-34, pull requests**: a pull request on a repository, and its update.
40. **NIP-C0, code snippets**: a code snippet, with its language.

**Agents**

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
right side to look. WORLDS (top right) goes back to [Worlds](/worlds).
