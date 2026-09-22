# M6 Protocol Expansion

M6 extends the relay without changing its architecture: one nostr-relay binary,
one Postgres database, and no external protocol service. The pinned official
specifications under `nips/official/` are normative; the limits and deliberate
subsets below are part of nostr-relay's public contract.

This page records the M6 deployed surface. Its subsets and absent handlers are
not permanent scope decisions: the protocol-totality program in the roadmap
targets every NIP in all three pinned lanes while preserving fail-closed
advertisement until each path is implemented and fixture-proved.

## Expiration and protected events

- NIP-40 events at or past their `expiration` timestamp are rejected. Reads
  exclude expired rows even before cleanup runs. One in-process worker deletes
  expired rows every `NOSTR_RELAY_EXPIRATION_SWEEP_SECONDS` (default 60, allowed
  range 1–86400). A failed sweep makes the process non-current and closes its
  connections.
- A NIP-70 event carrying a `-` tag is accepted only when its author is
  authenticated on that WebSocket with NIP-42. Forwarding it from another
  connection is refused. Kind 6 and 16 reposts embedding a protected event are
  also refused.

## Private-message routing

- NIP-17 gift wraps (kind 1059) must have exactly one valid `p` recipient.
  They are stored, but historical queries, live fanout, and COUNT expose them
  only on a WebSocket authenticated as that recipient. An authenticated
  non-recipient receives no result.
- Kind 10050 private-message relay lists are ordinary replaceable events. At
  least one `relay` tag is required and every relay value must be a bounded
  `ws://` or `wss://` URL. nostr-relay stores and serves the list; clients decide
  where to route messages.

## Relay-managed groups

NIP-29 support is enabled only when `NOSTR_RELAY_RELAY_SECRET_KEY` is configured.
The relay manages public-read, restricted-write groups. Every group-scoped
event has exactly one non-empty `h` tag, group existence and supported kinds
are checked before storage, and non-management authors must be members.

nostr-relay implements moderation kinds 9000, 9001, 9002, 9005, 9007, 9008,
9009, and 9010, plus join 9021 and leave 9022. Unsupported moderation kinds
9003, 9004, 9006, and 9011–9020 fail closed. Closed groups require a valid
invite code to join. Accepted joins and leaves create relay-signed 9000/9001
history events referencing the request.

After each state change, the relay atomically regenerates signed replaceable
metadata kinds 39000–39005. The supported role is `admin`; all listed roles
have full administrative authority. Group reads are public, hidden/private
groups and subgroups are not implemented, and NIP-11 advertises
`subgroups: false`. Kind 39004 is emitted as an empty participant document;
nostr-relay does not add a call/media service.

Clients may include `previous` references. Each must be an eight-character
lowercase hexadecimal prefix found among the last 50 non-self events in that
group; an unknown or malformed reference is rejected. Late group publications
use the relay's configurable global past-timestamp admission bound.

## Management API

Set `NOSTR_RELAY_MANAGEMENT_PUBKEY` and `NOSTR_RELAY_RELAY_URL` to enable the NIP-86
JSON-RPC endpoint on the same HTTP listener. Requests are `POST` with content
type `application/nostr+json+rpc`, a body no larger than 65,536 bytes, and a
NIP-98 `Authorization: Nostr ...` event signed by that exact management
pubkey. The event must be kind 27235, within 60 seconds, and bind the exact
HTTP URL, `POST` method, and SHA-256 body hash. Mutating and listing requests
consume the authorization event ID once to prevent replay.

The standard methods are `supportedmethods`, `banpubkey`, `unbanpubkey`,
`listbannedpubkeys`, `allowpubkey`, `unallowpubkey`,
`listallowedpubkeys`, `allowkind`, `disallowkind`, and
`listallowedkinds`. When relay signing is enabled, nostr-relay also exposes these
explicit NIP-29 extensions:

- `creategroup [id, name, about, picture, closed, admin_pubkey,
  supported_kinds_or_null]`
- `deletegroup [id]`
- `listgroups []`
- `putgroupuser [id, pubkey, roles]`
- `removegroupuser [id, pubkey]`

The API changes the same policy and group tables used by admission. Direct SQL
is not required for these operations.

## COUNT and search

- NIP-45 COUNT accepts the same bounded filter list as REQ, counts unique
  matching event IDs across filters, and returns an exact count. Work stops at
  the configured query-cost bound; a request too expensive to prove exactly
  is closed rather than approximated. Gift-wrap privacy applies to COUNT.
- NIP-50 `search` is limited to 256 characters and must contain at least one
  non-extension word. Unknown `key:value` extensions are ignored. One
  contract serves replay and live delivery: every remaining term must occur
  as a substring of the content, ASCII letters fold case in both the terms
  and the content, and non-ASCII text matches exactly, so `cat` finds
  `catwalk` and `Cat,` but not `chát`. Gift-wrap and access-gated kinds
  (`SEARCH_EXCLUDED_KINDS` in `crates/nostr`) never match in either path.
  Results are ordered by `created_at` like any other query, not ranked. The
  matching is independent of the database collation. Search shares the
  ordinary result and query-cost limits.

## Relay lists and watched drafts

NIP-65 kind 10002 relay-list metadata requires at least one bounded `r` tag
with a `ws://` or `wss://` URL and, when present, a `read` or `write` marker.
It uses the normal replaceable-event rules and its indexed `r` tags are
queryable. nostr-relay does not connect to, proxy, or fan out toward the named
relays; relay selection is a client responsibility. Operators should keep
relay lists compact because the normal event, content, tag-count, and frame
limits apply.

NIP-77 `NEG-OPEN`, `NEG-MSG`, and `NEG-CLOSE` reconcile id sets with the
version-1 frame in `crates/nostr/src/negentropy.rs`. A filter that matches
more than 4096 events is refused `blocked` rather than scanned. NIP-11
advertises 77 because that path is what the relay runs. NIP-91 was not present in the pinned
official source lane, so M6 could not advertise or implement it; it enters the
same process if a future reviewed sync pins it. See `source-lanes.md` for the
recorded lane decisions and their 2026-08-04 supersession.
