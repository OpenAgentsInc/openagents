# M7 Media Contract

nostr-relay implements a bounded Blossom server on the relay's existing HTTP
listener. The protocol inputs are pinned `nips/official/B7.md`,
`nips/official/94.md`, and `nips/official/98.md`. NIP-B7 delegates the HTTP
contract to the external Blossom BUD specifications, so M7 also reviewed
Blossom commit `b5bd2801d1763aa635fc8fea7a76597e0eb18990`: BUD-01, BUD-02,
BUD-03, BUD-08, and BUD-11. Those external files are reviewed inputs, not a
new NIP source lane.

## HTTP surface

Setting `NOSTR_RELAY_MEDIA_ROOT` enables:

- `PUT /upload`: verifies the NIP-98 authorization (signature, kind, time,
  URL, method, and one `payload` digest) before reading any of the body,
  checks optional `X-SHA-256` against that digest, commits
  ownership/quota/replay state, then streams the exact request body to a
  private temporary file while hashing it. A body whose SHA-256 is not the
  authorized digest, or that never arrives whole, is refused and the
  reservation and temporary file are released. A whole body is atomically
  installed as the content-addressed file, marked ready, and only then
  answered with `200` or `201`;
- `GET /<sha256>[.<ext>]`: public immutable retrieval with MIME type,
  content length, ETag, SHA-256, CORS, and one bounded byte range;
- `HEAD /<sha256>[.<ext>]`: the same metadata without a body; and
- `DELETE /<sha256>[.<ext>]`: NIP-98-authenticated owner removal. Shared
  content remains until its last owner deletes it.

Uploads require `Content-Length`; chunked uploads are refused. The configured
per-blob limit is enforced before allocation, the body is streamed through a
64 KiB buffer with a 30-second I/O timeout and five-minute total timeout, and
mutation limits apply per IP and authenticated pubkey. Startup removes upload
temporary files older than one hour; the total timeout makes that safe across
processes sharing a media root. `NOSTR_RELAY_MEDIA_MAX_BYTES_PER_PUBKEY` is checked in
the same Postgres transaction that adds ownership. Authorization IDs are
one-use and stored in `media_auth_request`.

Postgres owns whether bytes are public. A new blob begins `ready = false`.
After its registration commits, the file is atomically renamed within the
configured root and a prepared update makes it ready. A crash before that
last update exposes no partial file; the owner can retry the same hash with a
new authorization or delete the pending ownership. Reads fail closed when a
ready row has a missing or size-mismatched file.

The upload descriptor includes `url`, `sha256`, `size`, `type`, `uploaded`,
and a `nip94` tag array containing `url`, `m`, `x`, and `size`. Kind 1063 file
metadata and kind 10063 Blossom server lists also have owned structural
validation and committed fixtures. NIP-11 advertises numeric NIPs 94 and 98;
the alphanumeric NIP-B7 identifier cannot be represented in NIP-11's numeric
`supported_nips` array.

## Authentication decision

The roadmap explicitly selects NIP-98 for media authorization. Current
Blossom BUD-11 instead describes kind-24242 Blossom authorization. nostr-relay
therefore deliberately uses kind 27235 with exact `u`, `method`, timestamp,
signature, and upload `payload` tags; it does not claim BUD-11 authentication
compatibility. Reads are public; upload and delete mutate owner state and are
authenticated.

## Storage adapters

The default adapter is a local POSIX filesystem rooted at
`NOSTR_RELAY_MEDIA_ROOT`, sharded by the first two hash characters. The optional
mounted-cloud adapter uses the same atomic filesystem contract and sets
`NOSTR_RELAY_MEDIA_CLOUD_BASE_URL`; successful reads return a temporary redirect
to `<base>/<storage-key>/<sha256>.<ext>`. This supports an operator-provided object-storage
mount without adding an SDK, credentials protocol, sidecar, service, cache,
broker, or dependency to nostr-relay. The mount must provide atomic same-mount
rename and must publish `<storage-key>/<sha256>.<ext>` at that URL with MIME
types inferred from the extension. The database-owned storage key prevents an
old last-owner deletion from removing a concurrent re-upload of the same
hash. If the mount cannot expose that shape, the backend is unsupported.
Local filesystem mode instead shards files by the first two hash characters
and includes the same generation key in the private filename.

The committed single-box deployment uses the filesystem adapter. A container
must bind-mount a persistent writable media directory or leave media disabled.
The Cloud Run path leaves M7 disabled because its ordinary writable filesystem
is ephemeral.

## Deletion retention and backups

A delete that removes a blob's last owner takes the row out of the database at
once but does not destroy the bytes: the file moves to `<media root>/.deleted/`
under its content hash and storage key, where it stays for 48 hours
(`DELETED_RETENTION` in `crates/nostr-relay/src/gateway/media.rs`) before the
relay removes it. The retained file is never served; `GET` answers from the
database, which no longer names it.

That window is the barrier `deploy/backup/nostr-relay-backup` rests on. The
script dumps the database first and archives the media root second, so every
blob the dump references is either at its live path or under `.deleted/` when
the archive runs, as long as the run finishes inside the window. The script
verifies that from the dump itself and writes the unit's manifest only when
the check passes. `docs/deployment/runbook-debian-vps.md` covers the unit,
the restore script, and recovery.
