# Invariants — nostr/INVARIANTS.md

> Scope: Nostr relays, NIPs, events, signing, NIP-44 (5 invariants). Indexed from root [INVARIANTS.md](../INVARIANTS.md).

## Unconfirmed observations preserve confirmed state and their validation label

Chapter 82's explorer profile uses its own ephemeral table and separately
versioned projection. Fees derive from own-node confirmed coins or earlier
observed parents. Failed additions leave observations unchanged. Conflicts,
missing/immature inputs, bad money and exhausted bounds omit candidates;
they never change confirmed coins, balances or history. Confirmed IDs are
excluded, and tip changes rebuild observations. The response always labels
validation as unvalidated and completeness as false. A missing or malformed
feed is unavailable data. Full script and relay acceptance remain planned.
The 238 library and 57 feed mutants are all killed, with zero survivors.

## Native relay TLS verifies the peer before exposing plaintext

`coax/platform/tls-native.c` is the owner-approved TLS effect: OpenSSL 3 on
Linux, Network.framework and Security on Apple. `cxtls_connect` owns TCP and
TLS under one deadline, with TLS 1.2 minimum, SNI, chain and DNS-name checks.
There is no verification bypass or plaintext fallback. Test anchors require
a separate build macro; production ignores that environment variable.
The scalar bridge has one caller thread per process. Apple callback state is
serialized and generation-fenced, including pending reads after timeout and
callbacks delivered after cancellation. Buffers and connection slots are bounded.

Apple reports `CXTLS_END_UNVERIFIED` rather than claiming that opaque receive
completion proves TLS close_notify. `relay_conn_rules.coax` refuses that end
during ordinary polling; only a parsed authenticated WebSocket close permits
it during the closing handshake. `last_read` preserves the native evidence.
Coax owns URL/transport choice, framing, entropy policy and relay behavior;
the host owns encryption, trust evaluation, clocks and platform effects.

Checks: `coax/tests/net/tls.sh` and `relay_e2e.py` cover certificates, version
floor, exact stream bytes, timeouts/reuse and actual relay sessions under
ASan/UBSan. Apple pending-callback reuse also passes ThreadSanitizer. iOS
compilation is separate from runtime qualification. Producer hashes, actual
results, Linux regression and the owner-requested focused-check scope are in
the [Apple TLS record](docs/verification/2026-10-30-apple-tls.md).

## Published contracts and signed originals retain identity

All `proto/compatibility/` snapshots and vendor source/license bytes are
immutable. Published names, numbers, types and presence must pass every frozen
descriptor baseline. Nostr/HOST/REACH and other signed/framed protocols retain
their original bytes and authority owners. A decoded DTO grants no authority.

Check: recovery hashes, compatibility mutation tests, isolated vendor builds,
and alpha missing-versus-zero/limits tests in `scripts/proto.sh check`.
These checks do not claim that deleted runtime admission code is installed.

## Engine rename has explicit readers and versions

Legacy IDs live in the migration registry, frozen evidence and byte-exact
recovered or upstream NIP source specifications. Current
asset catalogs use canonical IDs; their other required data is unchanged.
Migrations preserve original input, unknown fields and signatures, reject
collisions and never reinterpret an old signature under a new domain. Other
record/bridge/signature migrations remain gated on actual consumers.

Check: real frozen asset catalog round trip, idempotence, collision tests and
signature-domain refusal in `proto/tools/test_contracts.py`.
The NIP recovery source/tree identity is recorded in `proto/nostr/recovery.json`;
restoring historical text does not restore its retired implementation mandates.
Current official/external mirrors may advance beyond that historical tree only
with exact upstream pins and per-file hashes; `nostr/nips/openagents` is not synced.
Check both online pin parity (`scripts/check-nips-pin.sh --latest`) and offline
inventory/hash parity (`--offline`), including non-Markdown fixtures.

## Nostr projections cannot create authority or replace signed bytes

New `nostr.*.v1alpha1` values are source-pinned projections. A SignedEvent keeps
original JSON alongside ordered string tags and exact content; the published
common.v1 wrapper stays intact behind an explicit adapter. JSON null, missing,
zero and unknown fields remain distinguishable where the source requires it.
No DTO, parsed owner claim, label, command, revision link or decoded JWS counts
as signature verification, owner membership, relay identity, grant, CAS
acceptance or encryption. Decrypted memory/telemetry/metrics and identity
credentials must not be published, logged or placed in a public snapshot.

Check: `proto/tools/check_nostr_sources.py` and `scripts/proto.sh check` reject
stale mirror pins/hashes and schema inventory drift. Reference tests check real
event ID/original round trips, unknown fields, missing/null/zero cases, token
classes, byte bounds and selected-profile imports. Coax tests consume the actual
Event protobuf fixture through both the narrow reader and actual descriptor-
generated typed codecs. `coax/tests/proto_import.py` checks presence/oneofs,
repeats, unknown retention, writer capacity/alias errors, policy bounds and
native ASan/UBSan; its explicit protoc oracle checks interoperability. Codecs
borrow input/output storage, allocate no target memory, and reject unsupported
descriptors and duplicate known singular/oneof fields. Domain migration and all
cryptographic/relay admission remain explicit unimplemented gates.


