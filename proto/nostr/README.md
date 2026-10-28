# Nostr mirror namespace

`nostr.*` is reserved for source-faithful mirrors of pinned official and Block
NIPs. OpenAgents-specific bodies remain under their owning `openagents.*`
packages. Neither tree changes NIP-01 signed bytes, NIP-44 encryption or native
HOST/REACH admission/framing by introducing a protobuf projection.

The [protocol spec](../../docs/protocol/openagents-protocol-spec.md) catalogs
coverage and dependencies. No compiled `nostr.*` mirror is claimed yet.
The recovered common/HOST/REACH contracts already carry original signed JSON,
refs and exact native operation data without inventing a second authority path.

Before adding a mirror, pin its upstream text, record source hashes/licenses,
map every field and extension, and test original JSON round trips and actual
consumers. Signed originals remain separate from a typed projection. Select
only the mirrors a consumer needs; do not import the whole catalog into every
Metareal, phone or Wasm profile. Historical NIP sources remain accessible at
OpenAgents revision `853417f290` in Git history; the reset did not erase them.
