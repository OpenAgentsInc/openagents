# Consented document effect v1

`metaverse.engine.document-effect.v1` uses action **35** over the existing
bounded action outbox. `metaverse-engine/host/document-wire.coax` is its sole
writer. No new account, network or inference transport is introduced.

The JSON request has `schema`, positive signed-31-bit `token`, `op` (`open`
or `share`), ASCII basename `name` (1–64 bytes, alphanumerics, dot, underscore
or dash; no leading dot or double dot), `limit` (1–2048 bytes), and canonical
`data` base64. Open has empty data; Share contains 1–limit opaque bytes.
The producer asks once; the SDK projects a file picker or Share/download
ceremony. Importing bytes does not execute a file.

The optional reply ABI is
`coaxial_document_reply_v1(state, token:i32, status:i32, bytes:u32)`.
Before calling, the SDK copies the complete bytes into the producer's existing
bounded inbox. Status: 0 success, 1 cancelled, 2 unavailable, 3 malformed,
4 capacity. Failed replies have no bytes. A Share success has no bytes.
Open success has the complete bounded file. The producer binds the pending
token and current document operation before decoding, owns record admission
and ignores stale callbacks. SDK callbacks from a closed/replaced page are
discarded. No pointer or callback can grant hardware or account authority.

On web, Share success means the Blob download was offered to the browser;
it does not confirm a filesystem write or user acceptance. Native Share
reports the activity controller's completion. The app labels the result
**File offered**, separately from a completed byte-storage Save.

The native projection reuses `NativeDocuments` and its shared OpenAgents
file location. The browser projection uses an ordinary consented file input
or a Blob download. Byte storage Save/Open-latest remains the existing
independent opaque storage contract, not this file-transfer ceremony.
