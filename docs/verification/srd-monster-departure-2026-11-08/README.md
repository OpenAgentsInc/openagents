# Player Reactions before monster locomotion publication

SRD 5.2.1 p.15, pinned PDF SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

Experimental's Lab now owns its shared-combat AI advance. Character Create's
explicit `hostile_external` flag leaves its normal standalone AI path intact;
the Lab sets the flag only for its Experimental combat view. Character Create
still advances its clock, while the Lab stages the same bounded hostile tick
before combat/render publication. Quiet/held/nonmoving enemies retain their
normal AI updates and the existing contact FIFO is preserved.

`xc_hostile_tick` evaluates the AI in a local Hostiles proposal. Each candidate
passes `xc_monster_move` while the actual body still occupies its prior place.
A locomotion departure is measured in terrain-adjusted feet and checked through
the shared opportunity query. Ordinary sight uses the existing dungeon sight
query plus shared Blinded/Invisible state. The default automatic choice uses
the player's held represented melee weapon, or an Unarmed Strike while holding
a ranged weapon. This selection policy is an adapter choice, not a new source
restriction or an Attack Action.

The moving monster's saved seat supplies the true active turn and its
Disengage/slot history. The player borrows an off-turn strike frame, spends
exactly one available Reaction even on a miss, and leaves player/NPC Action
budgets intact. Accepted damage, displayed numbers, hit/voice cues and shared
permission/death state survive interrupted movement. A lethal strike keeps the
monster's old body position; survivors publish their proposed position. A
second mover cannot reuse the spent player Reaction. Non-locomotion placement
is not relabeled as the mover's own movement.

Per-enemy permission projection is shared with the existing full projection,
avoiding a whole-army permission scan for every candidate. The AI remains the
same pursuit/home logic; this does not invent a retreat just to force an
opportunity in normal play.

## Evidence and composition

Missing-API regressions preceded implementation. The focused adapter fixtures
pass for lethal interruption, miss/second mover, moving-seat Disengage versus
player-seat Disengage, ordinary sight, held ranged weapon/Unarmed fallback and
non-locomotion placement. An actual Lab-frame fixture confirms exactly one AI
time increment. Existing selected Experimental combat passes all 31 checks.

The initial native harness and complete checked Wasm build passed. Main then
advanced with `ae1f6e3a59` (true-turn strike features and attacker sight). This
change was rebased onto it, preserving both implementations, and the focused
five-check adapter suite passed again. Core source is unchanged here: reuse
that commit's retained core gates and the opportunity executor/trigger receipts
at `34aa86f15d`/`1f20120688`; no blanket mutation gate is added to this world
adapter. Final combined native/Wasm receipts are retained separately.

```
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental monster departure'
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental staged monster'
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat'
coax/web/tool.sh apps/openagents-web/experimental-opportunity-native.coax
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-monster-departure-fivelab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
coax/web/tool.sh tools-check
```

## Remaining scope

This connects both player locomotion and monster AI locomotion to shared
opportunities in Experimental. It is not complete #13040/#13046 qualification.
Continuous movement costs/Speed and general timing profiles, all physical
strike-distance rules beyond the new departure metric, special senses and
obscuration, source on-hit programs/used-monster catalogs, explicit optional
reactor follow-up movement and remaining feature/condition interactions still
need their own evidence. Player choice UI/preferences beyond the declared
automatic Reaction profile are not added. Forced displacement/teleport paths
remain separate from this AI locomotion producer. E2's social no-bystander-harm
profile remains unchanged.

Final rebased native execution prints PASS PHYSICAL-OPPORTUNITY, covering the
four incoming player Shield budget cases plus outgoing player lethal/nonlethal
monster departures and unspent Action budgets. The final complete checked Lab
Wasm build passes in 49.8 seconds, 2,528,981 bytes, without new step/traversal
overrides. Its actual host and the standalone native harness compile against
the updated Character Create layout. Source/artifact/compiler hashes and logs
are retained. Existing compiler-source/near-limit advisories remain. The temporary
output name has no registered size budget and is not a new shipped target.
Tools-check and diff check pass; no browser/render/audio playback claim.
