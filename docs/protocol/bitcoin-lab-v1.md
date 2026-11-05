# Bitcoin Lab read-only projection v1

**Status: alpha read-only Lab adapter.** The deterministic fixture projections
are rebuilt by `bitcoin/lab/record.coax`. The optional `own-node` source is
configured by the local service operator; clients select its name only.
Explorer HTTP v1 remains pow-only. The recorder's internal JSON remains a
build artifact, separate from this HTTP contract. Unknown observation time
remains absent; a response's receipt time is not invented source freshness.

Typed source: `proto/openagents/bitcoin/lab/v1alpha1/lab.proto`. This is a
new contract for #12803/#12804; it changes neither explorer HTTP v1 nor the
mempool observation contract. The realm stays unlisted. No endpoint signs,
broadcasts, controls a node, relays cookies, or decides an OpenAgents payment.

HTTP uses snake_case JSON, rather than protobuf JSON. Hashes are full
lowercase display-order hex. Scripts are forward-order hex. Amounts are
integer satoshis, weight is integer weight units, and size is serialized
bytes. Missing optional numbers are JSON null, never a fabricated zero.
Tickets/cursors are bounded strings. Header time and source observation time
have distinct fields. A fixed fixture observation time is not a live clock.

## Sources and snapshot identity

The server configures the source allowlist; a browser cannot supply an
upstream URL, filesystem path, RPC method or credential. `fixture` uses
retained public regtest blocks, replayed through our own validator/indexer.
An explicitly configured `own-node` source uses our read-only service. A
source's `mode` is fixture/live, separately from `profile`: pow-only,
validated-v1, snapshot-background, unavailable. The existing explorer's
L/I/E node journals retain pow-only. An acknowledged V/X origin retains
validated-v1 through its disposable reader view. No profile is inferred from
the compatible query table. Snapshot/background mode is representable but
this adapter does not create or run a background validator.

Every response identifies source, mode, network, exact committed tip
height/hash, publication cursor, observation time, validation frontiers,
completeness, availability and implemented capabilities. Unknown background
progress is null. Snapshot-background is not full historical validation.
Unavailable/incomplete data has no admitted block/transaction/amount view.
Capabilities are promises only for the current complete snapshot.

Source, mode, network, profile, cursor or tip changes invalidate every related
view together, including a same-height hash change. Observation-time refresh
alone does not change the identity. Requests echo a strictly increasing
client ticket. Responses to canceled, superseded or other-source requests
are refused even when an effect slot has been reused. Client exhaustion
refuses instead of wrapping. At most two effects are pending; replies are
bounded to 2 MiB. A failure clears readiness. Retries use a bounded delay;
there is no unbounded refresh loop.

## Durable validated index, schema 2

Validated `V` state and its indexes are built from genesis and authenticated
blocks. They are never created by relabeling an `L` cursor or migrating its
coins. The index marker is `X || 0x02`, containing schema=2, network, tip
height, next archive sequence and the exact validated tip hash (48 bytes).
`L`, `I` and `E` are foreign metadata and refuse in this representation.
`J/T` positions and `F/G` script/output/spend archive encodings reuse their
checked serializers; active membership uses validated 116-byte `H` receipts,
not light-state receipts. `C` coins retain the validator's exact coin format.

The validated connector/reorganizer runs first in disjoint private scratch.
Only accepted bodies supply positions, original input-coin receipts and
archive entries. Coins, undo, receipts, positions, script histories and the
new marker share one KV frame. A failure poisons scratch and returns no
publishable batch. The caller syncs authenticated block bytes before the
state frame, appends/syncs the complete frame, then publishes the new view.
The existing write-back cache may coalesce that complete body, but neither
its private view nor an unacknowledged flush is servable. Readers do not
repair or expose partial frames. A new indexed profile rebuilds authenticated
blocks; an old V-only store is not silently assigned indexes.

To reuse the existing checked explorer query code, a bounded disjoint
read-only view may project validated H receipts to their compatible first
80 bytes and synthesize the old query metadata from the checked V/X origin.
This disposable view is never serialized, accepted by the validator/cache,
or used to infer validation: the original V/X snapshot supplies the Lab's
identity/profile. Input L/I/E records refuse before creating the view. This
is a one-way reader adapter, not a promotion of a pow-only cursor.

## Bounded reads

The same-origin prefix is `/api/lab/v1/`. Only GET routes explicitly admitted
by the Coax adapter are supported. Data requests name source, ticket and
the expected snapshot cursor (or `latest` for a new window). An old cursor
returns 409; malformed requests 400; unknown objects 404; unavailable/pruned
bodies, corruption and local capacity failures 503. Responses are no-store.
The local service binds loopback and can sit behind the existing /api proxy.

Routes are `snapshot`, `window`, `transactions`, `transaction`, `script`,
`outputs` and `search` under that prefix. Query fields are `source`
(`fixture` or `own-node`), a decimal `ticket` (1–9223372036854775807),
`cursor`, optional fixture `phase` (0–2), optional `offset`, and `key` for
object queries. Fields cannot repeat; unknown fields, URL escapes, fragments,
credentials, request bodies and non-GET methods refuse. Public identifiers
use their literal alphanumeric/hex encoding. `key` is bounded to 256 bytes.
The server never accepts an upstream URL, RPC verb or filesystem path.

`latest` is allowed for snapshot, window and search requests. Subsequent
object/page requests use the returned exact snapshot cursor. A page's `next`
is `CURSOR~OFFSET`, so an ordinal from a different publication cannot silently
continue a new view. The client sends its cursor and ordinal separately.
Fixture phase is part of source identity. The own-node reader opens one
complete committed journal into private bounded memory per response. It
checks size/mtime before and after the read and after projecting the response;
a concurrent publication refuses with 409. Torn/corrupt frames, foreign V/X
metadata and unavailable authenticated bodies refuse without serving amounts.
Windows include genesis and retain archive sequence order. They page blocks
without downloading or returning the complete block archive. At most 24
blocks, 32 transactions and 16 inputs/outputs per transaction page are returned.
The query table is bounded to 64 MiB, the journal to 256 MiB and a response to
2 MiB. Exceeding any local bound refuses rather than returning a partial page.

`transaction` accepts `TXID-BLOCKHASH` for an exact archived occurrence. Plain
txids retain active-first search semantics. Fixture inputs name their exact
known parent occurrence. A node input whose receipt is unavailable remains
unknown and is not followed to a different active occurrence. Node fees and
cumulative work remain unknown when the journal does not retain them. Script
keys are exact hex (up to 128 bytes) or a standard address; no ownership or
clustering is inferred. The node index reports active script history; fixtures
also retain stale occurrences with explicit active/stale membership.

Windows contain at most 24 block records. Transaction pages contain at most
32 transactions; each input/output page contains at most 16 links. Counts
distinguish returned, total, omitted and unknown, and a next cursor binds
both the publication and ordinal. A valid bounded page is never described
as the whole chain/block/history. Raw block identity and full transaction
boundaries are checked before data is projected. Unavailable statistics are
null; live pow-only data cannot acquire validated capabilities by adaptation.

Fixture replay has three complete states: branch A, branch B, then branch A
extended. A transition is admitted in full before branch/amount/selection
changes. Stale occurrences remain historical; active membership and exact
outpoints determine spend links. Address views mean exact scripts only:
there is no ownership clustering, inferred change or unique satoshi flow.

The initial native profile and every later promotion record their actual
fixtures, source commits, refusals, mutation and independent raw-block/Core
comparisons. This contract grants no mainnet or money qualification; book
chapters 80, 93 and 118 remain separate gates.
