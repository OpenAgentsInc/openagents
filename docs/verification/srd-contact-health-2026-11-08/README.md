# Experimental contacts and shared hero health

Partial #13046 migration on `76c6b8624a`, rebased onto `79a2ce92b3` to preserve
incoming first aid/stable recovery. Compiler 0.52.0. Pinned SRD 5.2.1 damage,
temporary HP, zero HP and death rules pp.16–18; stat-block execution uses the
preceding typed-strike contract. PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.

## Live path

Before the first physical tick, Experimental starts the shared battle and binds
the hostile bodies to contact-only mode. A physical swing publishes an actor
identity into a bounded 45-contact FIFO. It does not roll an attack or mutate
HP. Invalid/full queue refusal preserves existing accepted contacts and increments
a diagnostic; draining is FIFO and slots are reusable. The old standalone body
mode remains for other consumers/tests; its damage function refuses in shared
mode rather than writing over an HP observer.

Every hostile has a saved logical turn. Quiet actors advance too, so repeat
saves do not depend on being able to reach or strike the player. Contact draining
projects physical coordinates, enters only that actor's saved turn, calls
`rs_stat_attack`, and restores the surrounding turn. A miss spends its Action;
a repeated contact cannot acquire another Action before the next turn. Actual
class AC, defenses, reactions, temporary HP and the shared zero-HP lifecycle
now decide player outcomes. NPC turn boundaries use the current action timing.

Shared HP/temp HP are canonical. Body/UI HP, maximum, temporary HP and death
status are projections. Accepted or refused casts no longer replace HP with one,
clear temporary HP, refill ordinary HP, or transfer temporary HP into healing.
Healing numbers report actual HP regained; temporary-HP gains are identified
separately. The player's Reaction is no longer refreshed by each press. The
HUD displays class maximum and separate TEMP on one line with the bar beneath;
zero HP while alive says UNCONSCIOUS rather than YOU DIED. HUD drawing follows
contact resolution so it does not display the previous frame's health.

Each landed shared attack drives its own striker hit stop, including zero damage
through immunity/deflection and damage absorbed by temporary HP. Damage pops
still require an actual HP/temp loss. Multiple contacts cannot pause only the
last striker. Shared NPC deaths outside a player cast (for example a reflected
attack) project into corpse state without duplicating kill presentation.

The existing explicit respawn control resets the shared hero preset, not a shadow
44-HP counter. It clears pending contacts, preserves defeated monsters, releases
old owned concentration/Turn Undead and projects the restored class HP. A stale
observer cannot reset a living shared hero. This is the world's respawn policy,
not a claim that the SRD grants free revival.

## Data/profile boundary

The five existing body kinds currently use named `experimental-*` attack profiles
projected from their existing declared numbers. Their current slash/pierce types
and printed bonuses are preserved; large-body profiles have ten-foot melee reach.
Invalid kinds refuse rather than fall back. This is not a canonical monster
import: #13034 still must reconcile the named used-monster set and house mappings.
The existing per-press spell/use replenishment and free Action/Bonus Action
adapter are retained; full profile admission/resource migration is still pending.
No general movement-mode, teleport, player movement-budget or UI-completion claim.

## Checks and evidence

Initial contact/HP fixtures preceded implementation; red records the absent
bridge/state. The queue's independent capacity/wrap tests cover FIFO, actor bounds,
refusal, pending-contact preservation and reuse. A wrapping-behind-retained-items
fixture killed the initial modulo-bound survivor; none were exempted.

Final integration fixtures cover actual AC versus the old fixed fifteen,
resistance/immunity, temporary HP, same-turn refusal without dice, quiet NPC repeat
saves, exact healing cap, False Life remaining temporary HP, accepted/refused casts,
multiple striker stops, respawn/contact clearing and the actual `lab_frame` entry
binding before its first physical tick. HUD labels and generated draw geometry
have focused checks; no screenshot or pixel-level visual claim is made.

Local commands (all finished below fifteen minutes):

- `coax/bin/coax mutate verse/hostile-contact.coax --library --root-only --jobs 4`: 45 killed, zero survivors.
- `coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental'`: 51 selected checks pass on the rebased composition, including 28 combat checks, real frame, controls and respawn.
- `coax/bin/coax test apps/openagents-web/fivebar-test.coax --library`: 216 checks pass.
- Separate respawn, HUD label, HUD drawing and contact-only body checks pass (logs retained). The final direct legacy-damage guard has focused body and native coverage; it does not change the migrated shared contact path.
- `coax/web/tool.sh aethyr/rules/srd/contact-health-native.coax`: actual native `PASS CONTACT-HEALTH`, exit 0. It executes physical contact publication, no legacy RNG/HP mutation, bounded draining and shared strike/temp HP/Action behavior. It is a native component chain, not a native execution of the entire Lab/DOM/WebGPU surface.
- `coax/web/tool.sh tools-check`: zero new tools without a reason.

Shared strike, HP, reaction and condition policies are unchanged. Their existing
zero-survivor receipts are reused. The new contact authority has its own gate;
world/label integration uses lean focused verification, not blanket mutation.

## Release build attempt and verified baseline failures

Attempted the real release command:

```
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-contact-health-fivelab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

It failed before producing a qualified Wasm artifact, on three broader Lab checks:
slider aggregate traversal at 268,435,457 visits (limit 268,435,456), Siege tactics
`met`, and the level-17 Warlock `lw_test_turn` assertion. No limit or Wasm budget
was raised and no check was suppressed. The compiler also reports its existing
out-of-date-source advisory; all measurements here consistently use cached 0.52.0.

All three failures were separately reproduced on **unmodified main 79a2ce92b3**
with that same compiler, in this isolated worktree, then the owned change was
restored. Baseline commands used exact `--only` selectors `the sliders:`,
`the Siege's tactics` and `Player mode's Warlock`. Each exits 1; logs retain the
same source assertions/limit. Those are verified pre-existing build blockers,
not a claim of successful release qualification. Resolving the wider build remains
necessary before closing #13046. No browser/deployment success is inferred.

## Remaining active acceptance

#13046 stays open. Besides the release blockers above, canonical used-monster
data, remaining loadout/feature/spell execution, explicit profile overrides,
player movement/visibility/control integration, Everglade II and full world-flow
qualification remain incomplete. This removes Experimental's legacy live attack
roll and shadow HP decisions; it does not finish the whole epic or remove every
standalone legacy consumer.
