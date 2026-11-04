# External combat evidence v2

Identity: `external-combat-evidence-v2`,
`openagents.engine.combat.evidence.v2`. V1 remains unchanged.

Every observation and pre-input recheck is retained as checksummed lossless
PPM.zst. Each image row declares `codec: zstd-delta-v2` and
`base_observation`. Zero means an ordinary standalone Zstandard frame;
otherwise decode with `zstd --patch-from=DECODED_OBSERVATION`. An observation
uses the preceding observation, except observations 1, 31, 61 and so on are
standalone. A recheck uses its own observation. Dependencies never point to a
recheck and span at most thirty images. Rolling raw working copies are not
the evidence; all referenced archives must be present and decodable.

Exact broker traffic, timing and proposed/applied effects retain the v1
semantics. V4 also records the loot sensor on observations and rechecks and
distinct loot-action, loot-recheck, loot-effect and loot-outcome rows. A
one-GiB archive budget requests draining both combat and the pending corpse;
no previous recording is deleted. This is not a free-disk guarantee.
The analyzer must compare the reconstructed combat, navigation and loot
samples with the journal, and report any missing dependency or mismatch.
The separate 4 fps video is a convenient view, not the lossless source.
