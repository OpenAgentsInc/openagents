# Verse private assets

This crate holds Verse's private asset registry: licensed content that this
public repository must never hold. Read
[Private assets](../../docs/verse/private-assets.md) for the design and
[the GCP resources](../../docs/deployment/verse-private-assets.md) for
operations.

- `manifest`: the `openagents.verse.private-asset.v1` manifest each asset
  carries in the private bucket: source, license, readers, pack, vendor
  archive, and provenance.
- `placements`: the owner-local `private-assets.json` in Verse's home, which
  places private assets in a zone. Committed code never names one.
- `phones`: the owner-local `private-phones.json`, where the owner's host
  notes each paired phone's Verse key when it asks for the placements
  (NIP-HOST `verse.private`), so the owner can grant it.
- `auth`: the NIP-98 grant request a reader's Verse key signs, and the grant.
- `signed_url`: Cloud Storage V4 signed URLs over an injected signer.
- `broker` (feature `broker`): the `verse-assets` Cloud Run service. It checks
  the request's signature, the URL, the body digest, the 60-second window,
  and replays, then grants a 300-second signed URL for the pack only to a
  reader the manifest names. Every other request gets the same `403`.
- `client` (feature `client`): asks a broker for a grant, blocking, HTTPS
  only, with no redirects.

Nothing here holds a key. The broker signs through the IAM Credentials API's
`signBlob` with its service account's Google-managed key, using the metadata
server's token. Tests use a fake bucket and a fake signer.

The owner's command, `verse-private`, lives in `crates/verse-zone-everglade`
beside the pack compiler it runs:

```sh
cargo run --release -p verse-zone-everglade --bin verse-private -- add SOURCE \
  --name NAME --license fab-standard
```

Run the broker's tests with:

```sh
cargo test -p verse-private --features broker,client
```
