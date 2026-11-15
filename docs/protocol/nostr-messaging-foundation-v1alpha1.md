# Nostr messaging foundation

2026-11-15, #13628. Builds on the installed public reader in
[Nostr app services](nostr-app-services-v1.md). Published read operation 20,
clipboard, identity, account-chat and conversation references remain unchanged.
This alpha is an implementation contract; readiness is reported by the actual
installed provider, rather than inferred from the presence of an account key.

## Durable state

The shared client journal remains `coax/lib/nostr/client_journal.coax`.
Its original NCJ version 1 keeps its exact bytes, 4096-byte record limit,
64-event/16-publish/32-tombstone bounds and unscoped 36-byte header.
The explicit version-2 entry point selects a new format and larger bounds;
there is no in-place reinterpretation of a v1 file.

NCJ2 starts with `NCJ`, byte 2, the 32-byte canonical public identity and a
32-byte account namespace digest. Its 68-byte header is followed by records:
four-byte big-endian type/body length, its four-byte ones' complement,
type/body, then the first four SHA-256 bytes of that type/body. The original
record body types retain their meaning. Version 2 additionally admits the
255/255 EVENT marker for a locally staged original with its actual signing key
and timestamp, without advancing an incoming cursor. Its new type-7 queue
reference freezes an existing original's event ID and eight-byte relay mask.
Gift wraps retain their ephemeral outer signer rather than acquiring the
account's key as a fictional author. Maximum type/body length is 65,611
bytes (a complete 65,536-byte original plus the event record's 75-byte fixed
part). The app file is at most 8 MiB; the v2 indexes hold 512 events, 64
unresolved outgoing events and 256 tombstones. A final OK releases its active
publish slot; the retained original keeps its frozen targets and separate
accepted/refused observations. Checkpointing replays those observations without
occupying completed publish slots. NCJ1 retains its original behavior. Capacity is an explicit result.

The namespace is SHA-256 over `OpenAgents Nostr Store v1\0`, a two-byte
big-endian account-ID byte length, its UTF-8 bytes, and the binary canonical
public key. A copied journal from another account or
identity refuses, including two accounts using the same linked public key.
Native effects derive these facts from the authenticated account/provider;
the generic app cannot select an account or path. The first production adapter
stores the journal as account-owned bytes in the existing authenticated account
service. Phone, browser and desktop share that journal instead of racing
independent outboxes. Local projections are bounded memory caches; encrypted
history and pending sends recover through the account service after restart.
This does not claim cold-start offline plaintext history. A future local mirror
uses the same codec and namespace rather than a second event store.

Only original, ID/signature-verified events enter the production journal.
New arrivals also enforce the client's 900-second future window and explicit
NIP-40 expiration. Replay re-verifies signatures without making a shifted wall
clock turn an already committed original into corrupt storage. Relay ingress
tag quotas and relay-only retention are separate from client history rules.
Parsing a replayed journal is structural integrity, not cryptographic admission:
the session re-verifies originals before indexing or rendering them. Pending
outgoing events retain their exact signed bytes, target relays and separate
accepted/refused/pending observations. The paged outbox projection includes originals that have expired
from ordinary history. Its rows carry the complete stable relay-pool URLs,
so pending masks remain usable across machines and restarts. Eight frozen
targets per event can be serviced in batches within six active SDK sockets.
Transient authentication/rate-limit
responses do not become final refusals. An uncertain retry resends those bytes.

Writes are prepared in Coax and committed atomically by the account service's
existing transactional storage SDK,
then acknowledged to Coax. No publish, visible delivery or cursor advance occurs
before the persistence receipt. Failed writes do not acknowledge an event.
A file mirror can cut a torn trailing record using the journal's explicit
recovery result. An atomic database row cannot legitimately be torn: its
adapter refuses CJO_CUT rather than silently repairing committed state.
Interior corruption and wrong owner/version refuse. An atomic checkpoint keeps
pending outbox bytes, relay outcomes, required replacement/addressable heads,
deletion tombstones and inclusive cursors. Pruning cannot manufacture an empty
history or discard an unresolved outgoing operation. Projections page within
explicit caps and report incomplete/pruned history. Ordinary checkpoint history
keeps at most 256 entries within 3 MiB; live replacement/addressable heads and
every unresolved outgoing original stay pinned separately. The account row
retains a monotone pruning flag, so compaction cannot make a shorter projection
claim complete history. An exact-revision, fresh-strong-proof wipe clears it.

The first store retains public originals and encrypted NIP-59 originals,
never decrypted rumors, plaintext previews, private drafts, secret keys or
conversation keys. Plaintext is bounded in memory and erased on logout,
identity/account change and view retirement. OS file protection and browser
origin isolation supplement relay encryption; they do not imply protection
against a compromised logged-in origin or the managed-key custodian.

## Installed effects

Operation 21 adopts the frozen custody envelope from #13626. Its action,
operation ID and expected identity do not assert authority. Each action has a
closed, separately registered purpose payload. A result echoes the action,
operation and identity with an explicit status and bounded complete payload.
Unknown fields, incomplete UTF-8, unsupported purposes and mismatched current
identity refuse. Unsupported capabilities are reported honestly.

Managed identities use the authenticated account service and existing sealed
secret. Independently held identities use their actual local key or an explicitly
authorized provider, and refuse when it is absent. The generic UI sees no key
or ephemeral secret. Managed encryption remains readable by the account's
custodian. Login keys, refresh credentials and wallet seeds stay separate.

Supported purposes are capabilities, current relay-challenge authentication,
one complete NIP-17 send (all peer envelopes and the sender copy), verified
wrap opening, and supported read-before-write list edits. Group actions remain
unsupported until their own feature/provider ships. No raw-sign/hash/export or
caller-selected kind/signing template is exposed. NIP-44, NIP-59, NIP-17,
NIP-42, NIP-65 and list parsing use the existing shared Nostr library.

The published `seal` action is a complete send coordinator. It uses
`nip17_plan` and `nip17_send`, reads verified inbox heads for every peer and the
sender, and refuses the whole request before crypto when anyone is not ready.
Every envelope and frozen relay target commits in one NCJ2 transaction with
the operation's first sealed manifest. Linked providers submit only this
encrypted batch through `nostr-send.v1alpha1`; managed providers construct and
commit it internally. No single-recipient callback is a successful `seal`.
Uncertain retries return that manifest and recover the exact queued originals.
The manifest contains outer IDs and its queue revision, with no inner rumor
or room ID or private preview. Kind-15 attachment opening remains unsupported
until its SDK retains the file keys instead of returning them to generic UI.

Outgoing custody operations are keyed by authenticated account, current identity
and the preselected operation ID, with a private-provider HMAC of the complete canonical request. A plain
message hash is not stored. Reuse
with different content conflicts. A committed result is returned again after an
uncertain response; no second random envelope is created. Private opening
results are not cached as plaintext. Source/provider revocation or changed
identity refuses even when an old operation is known.

Operation 22 is the account-scoped durable store effect. Its SDK owns
authenticated HTTP and transactional storage, while Coax owns
admission, format, indexing, bounded pages, checkpoint contents and outcomes.
Operation 23 is the versioned authenticated/publishing relay session. It reuses
the installed socket SDK and queue ledger. Public read v1 remains read-only;
it cannot use a v2 authenticated socket as a way around that restriction.

The publishing session validates complete signed frames and requires the exact
outgoing event to be durably staged. AUTH binds the current native account,
canonical identity, exact saved WSS URL, connection generation and observed
challenge. Replaced challenges and retired owners never acquire permission.
Account changes and logout first retire owners and clear private memory, then
cancel SDK work. Late completions cannot populate another namespace. Reading
from a relay, an accepted write and a successful local commit remain separate
facts.

## List and session coordination

One account session coordinates feed and messaging subscriptions through the
existing Relay/Outbox state machines and socket carrier. Verified metadata and
replaceable/addressable heads are selected by timestamp, then lowest complete
event ID on a tie. Lists preserve unfamiliar tags and content when changing a
supported item. A failed/partial read is never interpreted as an absent list;
the write waits for the current generation's required reads. No unsolicited
follow, relay-list overwrite, key minting or default-group join occurs.

Qualification includes frozen old/new format fixtures, complete event-size
boundaries, independent signatures/encryption vectors, exact-byte recovery,
interrupted persistence, corruption, authentication and stale callbacks,
two-account isolation, real kill/relaunch and network-loss behavior. The
foundation build installs the qualified effects before DM screens use them.
Its matching signed update feed preserves the earlier runtime feeds.
