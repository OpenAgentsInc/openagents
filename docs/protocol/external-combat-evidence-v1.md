# External combat evidence v1

Identity: `external-combat-evidence-v1`, `openagents.engine.combat.evidence.v1`.
This composes the published combat observation v1 and control v2 without
changing either contract. The SDK remains read-only. Evidence does not
replay the external game's simulation.

Every controller capture is retained losslessly as `observe-N.ppm.zst` or
`recheck-N.ppm.zst`, using Zstandard with content checksums. The JSONL image
row binds its observation, phase, relative path, compressed byte count and
monotonic capture/archive interval. Recording failure stops further input;
it is never silently counted as a complete recording. An eight-GiB archive
budget requests encounter drain before new pulls, preserving the current
fight's remaining evidence. This budget is not a free-disk guarantee.

`wire.jsonl` records exact serialized desk requests and raw reply strings,
including refusals, send/read failures and monotonic start/end times. Each
input's down/up/release exchanges can therefore be checked against the
proposed pulse. It contains only the game controller's traffic. No account
credentials or unrelated desktop sessions are collected.
Requested pulses and observed wire intervals are distinct: transport,
logging and scheduling take time. The controller subtracts elapsed down-call
and logging time from its remaining sleep. The broker lease is the final
bound; neither an OS scheduler nor a reply timestamp guarantees an exact
physical key duration.

`journal.jsonl` retains all public sensor fields, navigation, bounded visual
candidate sets (including repeated-row counts and capacity reached), the
baseline proposal and final decision, decision reason, controller state,
fresh pre-input recheck facts and admission result, actual effect completion,
kill/XP outcomes and terminal reason. An action row is a proposal; only its
effect row and wire replies show whether it was applied.

Post-run analysis must verify every referenced archive, its checksum and
decoded public sensor against the journal; report missing/corrupt evidence,
not infer it. A timestamped visual replay contains the controller's sampled
images, not continuous unseen frames. A separate game-window recording, if
available, has its own rate/timing and integrity manifest. Keep source and
executable hashes, settings, logs, failures and aggregate search/combat/rest
timing alongside the run. Large archives stay in persistent local storage;
the repository retains their manifest, reports and selected fixtures.
