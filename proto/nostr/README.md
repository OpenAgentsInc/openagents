# Nostr mirror namespace

`nostr.*` is reserved for source-faithful mirrors of pinned official and Block
NIPs. OpenAgents-specific bodies remain under their owning `openagents.*`
packages. Neither tree changes NIP-01 signed bytes, NIP-44 encryption or native
HOST/REACH admission/framing by introducing a protobuf projection.

The [protocol spec](../../docs/protocol/openagents-protocol-spec.md) catalogs
coverage and dependencies. All pre-reset specification sources are restored in
root [`nips/`](../../nips/README.md):

| Lane | Recovered source |
| --- | --- |
| [Official](../../nips/official/README.md) | 99 Nostr specifications and their index |
| [Block](../../nips/block/README.md) | 17 extension specifications and their index |
| [OpenAgents](../../nips/openagents/README.md) | 36 specifications, shared contracts and 33 JSON Schemas |

The full 191-file tree matches pre-nuke commit `853417f290` exactly, including
the [upstream manifest](../../nips/manifest.json). Its source commit and Git tree
identity are recorded in [recovery.json](recovery.json). No compiled `nostr.*`
mirror is claimed yet.
The recovered common/HOST/REACH contracts already carry original signed JSON,
refs and exact native operation data without inventing a second authority path.

Before adding a mirror, pin its upstream text, record source hashes/licenses,
map every field and extension, and test original JSON round trips and actual
consumers. Signed originals remain separate from a typed projection. Select
only the mirrors a consumer needs; do not import the whole catalog into every
Metareal, phone or Wasm profile.

The recovered text retains historical implementation status, tool links,
names and IDs. It does not restore deleted services or override the current
Coax implementation direction. Legacy names remain source evidence; new
Metareal identities follow the [explicit migration](../migrations/README.md).
Do not rewrite signed-format definitions as part of a source recovery.
