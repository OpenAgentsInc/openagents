# NIP sources

Source copies of the Nostr Implementation Possibilities used to design
OpenAgents' protocol. The [root protocol specification](../docs/protocol/openagents-protocol-spec.md)
and [proto tree](../proto/README.md) define OpenAgents interchange; vendoring a
NIP does not implement its behavior or grant authority to a decoded message.

| Lane | Upstream | Current inventory |
| --- | --- | --- |
| [Official](official/README.md) | [nostr-protocol/nips](https://github.com/nostr-protocol/nips) | 100 specifications and upstream index |
| [Block](block/README.md) | [block/buzz docs/nips](https://github.com/block/buzz/tree/main/docs/nips) | 20 specifications and two JSON fixture files |
| [OpenAgents](openagents/README.md) | Authored in this repository | 36 specifications, shared contracts and JSON Schemas |

[manifest.json](manifest.json) records exact commits, branches, source paths,
counts, timestamps and browsable tree URLs. [upstream-lock.json](upstream-lock.json)
records the SHA-256 of every upstream file. The official lane copies the
upstream repository tree; the Block lane copies the complete `docs/nips`
subtree, including supporting fixtures and future nested files. Sources are
read from Git blobs without newline, formatting or name transformations.

The Block lane has a local index because upstream does not supply one. Only
that `README.md` may differ from the upstream inventory; an upstream README
replaces it if one appears. The OpenAgents lane is never overwritten by sync.

## Sync and verify

Requires Git and Python 3; no packages or credentials are needed for these
public sources. Run from the repository root:

```sh
scripts/sync-nips.sh
scripts/check-nips-pin.sh --latest
scripts/check-nips-pin.sh --offline
python3 scripts/test_nips.py
```

Sync fetches the current `master` of the official repository and `main` of
Block, reads both complete trees before replacing either lane, then writes the
pins and hashes. Review every spec change and commit the sync separately.

The default pin check downloads the pinned commits and compares every source
byte and the complete inventory. `--latest` also checks that those pins equal
the live branch heads. `--offline` checks local inventory, hashes, counts and
JSON syntax without a network connection. Missing, extra or edited files fail;
only the explicitly local Block index is exempt. `--cache PATH` may reuse Git
clones under `PATH/official` and `PATH/block`; even a cached latest check asks
the upstream for its current branch head.

## Review and provenance

The [2026-10-27 sync review](../docs/protocol/2026-10-27-upstream-nip-sync.md)
records the changes and implications for future Coax consumers. Runtime
admission, original signed bytes, privacy, permission checks and compatibility
vectors remain separate implementation gates. Never advertise a feature just
because its specification is present. Official and Block names remain separate
lanes; matching draft identifiers do not merge their meanings.

The original 191-file pre-reset recovery remains identified by
[its historical Git tree](../proto/nostr/recovery.json). That record is preserved;
the current official/Block lanes intentionally advance beyond it. The
OpenAgents-authored lane remains unchanged by this sync. Historical text in
that lane may describe deleted Rust services, old tools and former deployment
status; those statements do not restore the old implementation or override the
current Coax direction.
