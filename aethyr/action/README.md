# Actions, contacts and reactions (#12993)

One lifecycle for a swing, a cast or a thrown bolt: action → sensor/contact →
gameplay reaction → presentation. Rulesets keep their own hit points, turns and
authority; these modules decide timing, contact and deduplication, and hand the
ruleset's adapter a reaction to apply.

| File | What |
| --- | --- |
| `action.coax` | Versioned `ActionDef` (stable ID, clip reference, movement owner, length and events in 1/64 s ticks, sensor and cancel windows, multi-hit) admitted whole; instances advanced to a tick hand back every crossed event exactly once at any frame rate, stall or catch-up; rational time scale; sampling never emits |
| `sensor.coax` | Blade/fist sensors as swept capsules against hurt capsules, active only in the action window; motion profile 1 for rotating blades (sampled actual poses, no end-pose-only sweep), declared work exhaustion; contacts deduplicated by attack-instance ID and target identity/generation, same-tick merge ordered by sample then target; self, team, stale, blocked-line and stale-source contacts never apply |
| `react.coax` | Separate tables: `AttackAttrs`, `TargetResponse`, `Reaction` (guard arc, immunity, weakness, resistance, zero-damage flinch, environmental-only, reflection) with named hit-stop and knockback profiles; scoped hit stops that never pause the world; a presentation queue with per-source caps and counted suppression; reflected projectiles as new owned instances whose old events are refused |
| `break.coax` | Breakables (debris, kinematic, structural cost profiles) that break once and hand back the declared fact patch, a sound stimulus and a bounded cosmetic debris burst; attachments with one parent authority, parent-before-child order and refused detach when no placement is free |

## Order within one tick

1. `action_advance` to the tick: crossed events, whether the sensor is open.
2. The realm evaluates the pose and attachments (`attach_order`) and samples
   the sensor's poses across the tick (`sensor_samples_needed`).
3. `sensor_resolve`: contacts, filtered and deduplicated.
4. The ruleset applies accepted contacts; `react_decide` names the reaction.
5. Presentation (`react_present`) reads accepted reactions only.

Interpolated drawing never senses.

## Adoption

- **Everglade Experimental** (`apps/openagents-web/experimental-combat.coax`):
  every weapon blow (the slash and the owned two-handed cleaves) is an action
  built from its clip (`xc_swing_def`, window ±6 ticks round the clip's
  impact). Each sensing tick samples the blade's actual pose (the swing clip's
  baked hands between frames, through the same `weapon_matrix` /
  `two_hand_matrix` that draws it) and sweeps the monsters' hurt capsules; the
  SRD Attack resolves once against the first accepted contact. The former
  press-time arc pick and pending impact time are removed.

- **State Lab** (`aethyr/state/fixture-strike.coax`, #13027): [B] swings a
  32-tick action whose blade sweeps an arc ahead of you through its window. A
  contact with the gate's hurt capsule is the only way the gate breaks:
  `react_decide` names the reaction (a slash with a 4-tick hit stop that holds
  only you) and `break_hit` breaks it once, handing back the `gate.broken` patch
  (published through the ordinary command, so the collision box and navigation
  owner swap with the fact), a sound stimulus the NPC notes through
  `aethyr/ai/stimulus.coax` when it is there, and 12 pieces of cosmetic debris.
  A loaded broken gate breaks nothing and drops nothing; the credit stays at one.
  The recorded script breaks it by contact twice (before and after the editor
  reset) with the same trace at 30, 60, 144 FPS and uneven frames.
- **Everglade Experimental**: a landed blow holds the struck monster for a
  5-tick hit stop and a monster whose blow lands on you holds itself for 4
  (`xc_stops_step`, `Enemy.stopped`); nothing else pauses. Floating numbers go
  through `react_present` (at most 8 a source a frame; the rest counted,
  `lab_creator_probe 934/935`).

## Residual adapters and profile limits

- The blade sensor samples the swing clip's pose, not the drawn blend with the
  idle/run underneath (weight 0.8–1.0 in the window).
- Before the bake is ready (and in native checks) the blade follows a declared
  fallback arc; each such tick is counted (`lab_creator_probe 919`).
- Experimental's line of action has no colliders (the meadow fight has no walls
  between you and a monster in reach).
- Guard, reflection, immunity and weakness have no playable consumer: none of
  Experimental's SRD monsters (the world's Imp and Gremlin stat blocks) guards,
  reflects or carries an immunity, and the ruleset already applies its own.
- Root motion and attachments are checked here only (`attach_*`); no realm
  plays a root-motion action under collision-resolved movement.

```sh
coax/bin/coax test aethyr/action/action.coax --library
coax/bin/coax test aethyr/action/sensor.coax --library
coax/bin/coax test aethyr/action/react.coax --library
coax/bin/coax test aethyr/action/break.coax --library
coax/bin/coax test aethyr/state/fixture.coax --library
coax/bin/coax test --library apps/openagents-web/fivelab.coax --only "experimental combat"
```
