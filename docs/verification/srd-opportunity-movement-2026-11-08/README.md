# Opportunity reactions before ordinary grid movement

SRD 5.2.1 p.15, pinned PDF SHA-256
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

`rs_step_ready` shares ordinary grid movement admission without mutating the
turn: logical actor, true active context, movement capability/Speed, Steady
Aim, map/occupancy, adjacency and repriced movement payment. `rs_step_to`
uses the same admission. Invalid actors/coordinates are rejected before
reading backing creatures or computing distance.

`rs_step_reactions` first validates the proposed paid step, then resolves
caller-selected opportunities in order before placement. It rechecks movement
after each strike and finally uses the normal payment/place/Spirit Guardians
path. There is no outer transaction: a lethal accepted attack stops movement
without refunding the Reaction, rolling back damage or soliciting later
reactors. Failed movement does not spend movement. Legal movement pays once.

The Lab's ordinary grid walks, player token-to-body steps and Siege walks now
use this boundary. Its default choices are nearby foes with ordinary sight,
using their held represented melee weapon. The shared engine does not impose
that hostility selection. Blinded observers, Invisible movers, unavailable
Reactions and non-departures are excluded before expensive strike execution.
The grid's eight neighboring squares bound the selection without scanning the
whole army. This is the open Lab grid path, **not** Experimental's separate
continuous physical controller or Everglade II's network movement path.

## Evidence

Missing-API regressions preceded implementation. Full composed entry passes
200 checks. Policy gate kills all 53 mutants, zero survivors/no exemptions.
A subsequent ordered two-reactor kill regression passes separately: the first
attack stops departure, the second keeps its Reaction. Native execution prints
PASS OPPORTUNITY-MOVEMENT for both surviving and lethal departures. Hotbar
passes 228 checks. New Lab regressions verify actual adapter kill interruption,
allies and ordinary sight. A combined multi-sight fixture initially exceeded
its work budget because invalid proposals entered the full strike transaction;
filtering unusable choices fixed it without increasing budgets.

```
coax/bin/coax test aethyr/rules/srd/opportunity-movement-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/opportunity-movement-checks.coax --library --unit aethyr/rules/srd/opportunity-movement.coax --jobs 4
coax/web/tool.sh aethyr/rules/srd/opportunity-movement-native.coax
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat opportunity'
coax/web/tool.sh tools-check
```

## Remaining scope

Continuous physical and network adapters must bind their own authoritative
visibility, attack choice, movement causes and before-departure boundary. This
API handles paid ordinary grid steps; teleportation, forced displacement and
Action/Bonus/Reaction movement need their explicit provenance. Special senses,
obscured terrain, extended monster attack choices and optional reactor follow-up
movement are not inferred. Accepted strikes still use the existing whole-battle
transaction: large-army cost is not claimed qualified. General timing and every
world's integration remain open. No issue closure is inferred from these checks.

The complete checked Lab Wasm build passes in 37.0 seconds, including the
ordinary army/Horde/Siege checks, without new step/traversal overrides.
Output `/tmp/srd-opportunity-movement-fivelab.wasm` is 2,523,674 bytes;
source/artifact hashes and complete build log are retained. Existing near-limit
and newer-compiler-source advisories remain. The temporary output name has no
registered size budget; it is not a new shipped target. Tools-check/diff check
pass. The earlier in-flight build was stopped when adapter source changed;
this receipt is from the completed build of the final adapter.

```
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-opportunity-movement-fivelab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```
