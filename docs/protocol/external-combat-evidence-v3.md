# External combat evidence v3

Identity: `external-combat-evidence-v3`,
`openagents.engine.combat.evidence.v3`. V1/V2 retain their published meanings.
This is an explicitly selected compact storage envelope for control v4/v5,
with the same capture identities, exact event/sensor fields and wire trace.

Each observation and pre-input recheck retains a full-resolution WebP scene
at quality 85 and a checksummed lossless PPM.zst of the original top-left
sensor region (up to 1024×64). The writer compares every decoded combat,
navigation and loot field with the full source before admitting the archive.
A UI scale that makes the crop insufficient refuses rather than silently
dropping sensor data. Both named artifacts and their exact byte counts are
required. Analysis checks both files and compares the lossless sensor region
against the journal. Candidate sets/decisions remain exact JSONL.

Scene pixels are lossy: do not claim bit-exact full-scene replay or recompute
the original colour-threshold vision result from WebP. The independent 4 fps
video retains its own capture IDs, timing and rate. V2 is still available
when every full-scene RGB byte must be preserved. No old recording is
rewritten, removed or downgraded by selecting V3 for a new run.
