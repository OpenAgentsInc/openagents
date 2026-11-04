# Visible Classic Era combat, 2026-11-03

Foob, level 4 warlock, Deviate Delight, Coldridge Valley (map 1426).
Native Verse OS desk 2, 2548×1428. Ordinary leased input only; SDK code
reads public state and draws observations, never casts or moves.

| Trial | Result | Journal |
| --- | --- | --- |
| First wolf | 52 XP; stopped on disappearing target before corpse sample | [01](01-first-wolf.jsonl) |
| Boar and menu | 104 XP with matched kill event; clearing absent target opened menu | [02](02-menu-after-boar.jsonl) |
| Corrected batch | Three consecutive kills, 208 XP; minimum health 83/88, final 87/88; two water uses | [07](07-three-kills.jsonl) |
| Longer batch | One boar, 104 XP; transient combat flag after kill refused the next step | [08](08-combat-lingers.jsonl) |

The final profile adds a bounded three-second idle grace after a verified
kill when combat has not yet cleared. It cannot permit attacking or moving
during that grace. The next longer trial is in progress.

Journals are raw development evidence. Trials 01/02 used the superseded
byte-sum framing, which could admit a wrong UI-scale pitch in some fields.
Trial 07 onward uses CRC-32 and omits the client's restricted nameplate
geometry API. No early incorrect sample is silently rewritten as final
profile qualification. [Screenshot after the first wolf](first-wolf.png).

`frame.ppm` is a rolling local image cache. Immutable selected PPM snapshots
are local `evidence-N.ppm` artifacts at starts and kills, not a retained image
for every JSONL observation. The journal records full sensed facts and
capture identities; it does not replay WoW's simulation.

Checks so far: sensor 134/134 entry-source mutants killed; combat policy
249/249, zero survivors. Controller closure: 106 checks passed. Protobuf
compilation, Nix flake evaluation and native bundle installation passed.
Sanitizer gameplay qualification is in progress. tools-check reports
12 unrelated header violations on the base.

[Protocol](../../protocol/external-combat-v1.md) ·
[Work journal and limits](../../os/verse-combat.md).
