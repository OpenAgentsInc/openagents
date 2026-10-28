# Engine name migration

The engine is named **Verse**. It was named Metareal (and before that Aethyr),
and this registry records the Aethyr-to-Metareal ID migration. New prose and
examples use Verse. The registry's canonical v1 IDs (`metareal-grid-v1`,
`metareal-town-v1`, `metareal.record`, `metarealSave`,
`METAREAL-SIGNATURE-V2`, `openagents.metareal.*`) keep their published
spelling: renaming them again needs a new registry version with old readers,
not a text replacement. The owner approved migrating IDs as well
as branding. [metareal-v1.json](metareal-v1.json) is the explicit legacy-reader
registry; old spellings appear there only to identify actual archived data.
Normalized historical source paths are display aliases; the registry's
`historical_path_prefixes` maps them back to actual pre-reset Git paths.
These do not claim that renamed product crates currently exist.
Frozen binary snapshots also retain original IDs. Neither is current branding.

## What is implemented

Current `profiles.json` and `data/asset-profiles.pb` use `metareal-grid-v1` and
`metareal-town-v1`. Their scene IDs, files, formats, dependencies and buffer
strides are identical to the original catalog. Shared protobuf package/type
names are unchanged: they were already `openagents.engine.*`.

With the pinned tools configured as in the protocol guide:

```sh
scripts/proto.sh canonical-id metareal-grid-v1
scripts/proto.sh migrate-profile-catalog \
  proto/compatibility/asset-profiles-v1-20261027.pb /tmp/metareal-profiles.pb
```

`canonical-id` accepts an exact registered old or current ID and returns the
canonical metadata ID. Unknown IDs fail. It does not edit a record body.
The catalog command parses the actual typed legacy protobuf, changes only the
approved IDs, preserves unknown fields, refuses collisions, and writes a new
file exclusively. It never overwrites its input or an existing output. Running
it on canonical input is idempotent.

This does not mutate or deploy the external engine's pinned bundle. Existing
consumers remain on their original pins until their own reader transition.

## Other record and API IDs

The registry gives canonical targets for old world, UI, profile, authoring,
release/channel, NPC, save and bridge IDs. Those consumers were deleted in the
reset and are not recreated here. Their migrations are **specified, not
implemented**. `legacy/<key>` in recovered catalogs refers to a registry entry,
not a literal schema ID or a new signature domain.

For each restored Coax consumer:

1. Install explicit old-version readers and canonical metadata projection.
2. Verify old data, fixtures, unknown fields and ownership/revision checks.
3. Add a new-version writer and transport negotiation where needed.
4. Prove old/new interoperability before switching the default writer.
5. Retain durable old readers while any old data exists. Never reinterpret an
   old signed record as newly authored under a renamed ID.

The native save target changes its format identity and schema version together
(legacy schema 2 → canonical schema 3). Do not simply replace a string in an
old saved file. The actual data/schema converter must be tested when that
consumer is restored.

## Signed releases and channels

The old signature-domain bytes are recorded exactly in the registry, including
the newline. Verify an old signature against the original bytes and original
domain. Preserve those bytes when displaying canonical metadata.

New publications use the proposed `METAREAL-SIGNATURE-V2\n` domain and the
registry's v2 contract IDs only after a real signer/verifier and versioned
admission pass. They need new signatures; the old signature cannot be carried
over as proof. `canonical-id` deliberately refuses signature-domain migration.
No tool here signs, republishes or re-labels signed content.
