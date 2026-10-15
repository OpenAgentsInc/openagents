# Curated plugin registries

`plugin-registry` implements the publisher, immutable mirror, and reader roles of
[NIP-REG](../../nips/openagents/NIP-REG.md). The CLI is available as
`openagents registry` and the standalone `plugin-registry` binary. Selection
returns a verified release pin; the existing plugin installer and host still
decide installation, supported bindings, permissions, and execution.

Enroll an exact curator identity with explicit sources:

```sh
openagents registry add CURATOR:catalog --sources sources.json
openagents registry refresh CURATOR:catalog
openagents registry browse CURATOR:catalog
openagents registry select CURATOR:catalog --package PUBLISHER:tool
openagents registry compare
openagents registry remove CURATOR:catalog
```

The source file is an array containing one to eight independently admitted
sources. Relative mirror paths always use the NIP-REG layout:

```json
[
  {"type":"directory","path":"/absolute/path/to/mirror"},
  {"type":"https","base":"https://example.com/plugins"},
  {"type":"github","owner":"owner","repository":"repo","reference":"main","directory":"mirror"},
  {"type":"nostr","relay":"wss://relay.example.com","blobs":["https://blossom.example.com"]}
]
```

GitHub reads use the [repository contents API](https://docs.github.com/en/rest/repos/contents?apiVersion=2022-11-28)
with its raw media type. HTTPS redirects and URL credentials refuse. Catalog
URLs and NIP-19 relay hints never add sources. Public relay reads work without
a challenge; challenged connections use a temporary reader key, without reading
an owner's key. Restricted relays that require a particular member key are
outside this reader's role. No private installed inventory is sent.

The default policy bounds one operation to 128 source requests, 64 MiB of
received content, and 60 seconds. Failed attempts count. Cached artifacts and
events deduplicate requests within that operation. Catalog and publisher
checkpoint age is at most one day, with 300 seconds of clock skew. Signed
checkpoints remain mandatory for fresh selection. Local policy can lower the
bounds; supported maxima are 4,096 requests, 512 MiB, and 120 seconds.

`--policy FILE` accepts this closed configuration:

```json
{
  "limits":{"requests":128,"bytes":67108864,"seconds":60},
  "max_age":86400,
  "skew":300,
  "allow_cached_browse":true,
  "allow_pinned_use":false,
  "require_current_membership":true
}
```

`browse --cached` labels a stale snapshot explicitly. `pin --package ID` reads
a previous selection without fetching: default policy requires its current
recommendation and fresh remembered publisher checkpoints. Explicitly allowing
pinned use and disabling current membership permits offline reuse, including
after removal. Known publisher revocations still refuse; cached bytes and the
whole dependency closure are checked again. This does not approve execution.

An enrollment accepts `naddr` for a `30184` address. `add ID --release EVENT`
accepts an exact `3184` ID or `nevent` as a frozen snapshot, with the curator ID
still explicit. Frozen catalogs are never labeled current. New frozen selection
requires the explicit pinned-use policy above and fresh publisher checkpoints.
Changing sources, removing, or re-adding a registry preserves its remembered
heads, accepted revision, and verified revocations. State defaults to the
owner-only `~/.openagents/registries`; use `--state DIR` for isolated operation.

Publish a catalog document conforming to the bundled schema:

```sh
openagents registry publish catalog.json --mirror /absolute/path/to/mirror \
  --license MIT --key-file curator.key
```

The key file must be readable only by its owner. The publisher builds an inert
EXT package, retains immutable releases and artifacts first, then switches
checkpoint and listing heads under a publication lock. Repeated publication
reuses identical release bytes. Changed catalog content requires an increased
revision. The mirror directory can be served over HTTPS, committed to GitHub,
or transported with the same signed events to Nostr and admitted blob stores.
This command writes a mirror; it does not upload to an external service.
Retain member releases, closures, and checkpoint heads in that layout or enroll
other admitted sources that provide them.

`compare` groups identical pins and keeps all curator origins; different pins
remain separate choices. Browse fetches only the inert catalog closure.
Selection fetches exactly one member closure with original publisher signatures,
config-schema confinement, and independent revocation state for each package.
Evaluation references remain unverified data; this reader does not present them
as evaluation evidence. JSON output escapes terminal controls.

The fixtures exercise directory, real local TLS HTTPS/GitHub, and authenticated
Nostr adapters with the same signed catalog and member pins. They also cover
withdrawal replay, revision conflicts, partial revocation downloads, source
changes, frozen snapshots, bounds, mirror retries, unavailable dependencies,
and corrupt caches. They do not qualify third-party service uptime, hidden
newer state, host execution bindings, or GitHub account authentication.
