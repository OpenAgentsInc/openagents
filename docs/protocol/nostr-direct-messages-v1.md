# Nostr direct-message consumer

Implementation in progress for #13629, after #13628. Existing custody (21),
store (22), authenticated relay (23) and typed conversation-reference
contracts remain unchanged. This document describes the consumer composition;
it does not add a new signing authority or declare the app delivered.

## Projection and identity

The NIP-17 projection is shared by native and browser consumers. It uses the
existing NIP-59 unsigned-rumor adapter, NIP-01 event reader/ID computation,
NIP-17 membership, room hashing and chronological ordering. Those operations
are separated from signing and key-provider effects, rather than copied into
a second parser. Parsing an opened rumor does not prove a seal. Only an exact
successful custody `open` response from the current authenticated provider
can supply an inbox row.

Each response must match the current account-generation owner, canonical
identity, outstanding operation, service and action. An unrelated callback
cannot consume that outstanding read. A matching refusal ends it without
publishing content. Account changes clear the volatile messages, ownership
references and pending read; generations never wrap. Even two accounts using
the same identity have different local lifetimes and durable namespaces.

The current bounded text projection keeps the most recent 128 messages of at
most 16,384 UTF-8 content bytes. A newer arrival evicts only the oldest row;
an older arrival cannot displace recent history. Original ciphertext stays in
the durable journal. It deduplicates inner rumor IDs across gift wraps and
orders by signed rumor timestamp then rumor ID, never outer randomized time.
Capacity is explicit; it does not manufacture an empty or complete history.
The DM consumer requires a two-member kind-14 room. Later encrypted-group and
attachment issues own their respective contracts.

Each new seal intent adds an encrypted `oa-op` extension tag to its rumor,
using the stable operation reference. Two deliberate identical messages in the
same second remain distinct inner IDs. This tag never appears on a public wrap.
Retry still returns the committed originals; it never encrypts a second batch.

Private plaintext stays in memory. The durable source remains the account's
NCJ2 verified ciphertext originals and exact outbox. Cold-start recovery
reopens these through the actual provider; a label or public journal row
cannot substitute for decryption and seal/rumor identity checks. Private
content cannot become agent/tool actions or executable markup.

## Installed resource capacity

The mobile compiler output now exceeds the old per-program 8 MiB ceiling.
The installed program carrier admits at most 16 MiB, through the shared Coax
capacity rule. The package keeps its independent 32 MiB bound and exact runtime
fingerprint check. This requires a new TestFlight runtime; the larger bundle
must never be published to an older runtime. The browser's complete message
codecs use the same 8 MiB call-stack reservation as the mobile assembly.

## Remaining integration gates

### Private read positions

The companion [read-marker profile](../../proto/nostr/read-marker-v1.json)
stores only each account's monotonic `(rumor timestamp, rumor ID)` position
per NIP-17 conversation. This is private account metadata, never published to
relays and never presented as the recipient's read receipt. It uses the
existing journal namespace, account lifecycle and canonical-identity gate.
An old device cannot roll it back. Unread excludes authored messages; unknown
read positions remain unknown until reconciled. Marking follows the admitted
visible conversation, with Earlier navigation preserving its previous position.
`coax/native/nostr-read-policy.coax` owns mark admission, capacity and monotonic
ordering. The API supplies journal-presence/count facts and persists its
decision under the existing account lock; SQL and C do not choose the position.

### Recovering send status

`POST /api/auth/nostr/delivery/v1alpha1` enumerates immutable successful seal
receipts from the existing account-owned operation table. Its companion
[profile](../../proto/nostr/delivery-v1alpha1.json) defines closed request/page
shapes. A fresh read operation is echoed exactly with the identity and cursor;
a response for an older read cannot replace a newer projection. Read retries
retain that same reference. Pages contain seal operation IDs and the existing complete sealed receipt;
they never contain plaintext, private intent digests, or custody keys. The
sender's self wrap locates the verified opened rumor; its other outer IDs
locate recipient ACK masks. An acceptance is one real accepting relay for
each recipient copy, not a read receipt. The sender-copy ACK is tracked
separately. Missing, expired or pruned originals remain unknown.

The exclusive lexicographic cursor only describes the current forward pass.
New random operation IDs can sort before it. Automatic reconciliation therefore
starts another pass from the beginning and deduplicates immutable operation
IDs. It never calls this an atomic or permanently complete snapshot. Account
merge ambiguity refuses just as the underlying journal does. Journal wipe
removes these existing operation rows in the same transaction.

This endpoint is being implemented as part of #13629; the published custody,
store, result, sealed receipt and authenticated relay envelopes stay frozen.

- Compose live relay reads, authentication, kind-10050 discovery, outbox
  publishing, acknowledgements and durable replay through their existing owners.
- Use typed `nip17` conversation references in the existing history, transcript
  and simple composer; keep account/agent/environment/project destinations intact.
- Complete replies, optimistic/exact retry states, unread/read-watermark
  persistence and chronological bounded navigation. Any new stored/read-marker
  shape starts in proto with a versioned codec; RXE, account settings and
  existing journal records are not repurposed.
- Qualify phone ↔ web ↔ independent-client interoperability, crash/reconnect,
  second-client sender history, forgery/wrong-recipient refusal, UI regressions,
  actual renders and delivery before closing #13629.
