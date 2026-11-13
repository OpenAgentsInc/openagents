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
| `attach-overlap.coax` | The attachment overlap check (#13073): a pose's body bones and a held item's pieces as capsules; `overlap_pose` finds the deepest overlap (a piece may allow the hand that grips it) and `overlap_keep` keeps the deepest over a sweep of every clip and frame. Experimental's bow checks run on it (`everglade/scene/charactercreator.coax` `cc_bow_sweep`) |
| `deed.coax` | The action pipeline (docs/metaverse/action-pipeline.md, #13072): ask, act, present. `deed_ask` asks whether an actor can act now from its bearing (one pose and one set of move/act/react/speak locks, from its conditions), its cooldown and its feel; the authority's answer (`deed_answer`) or a 1.5 s timeout decides; only an accepted answer presents (`deed_show`). A refusal greys the slot (`deed_look`) and plays nothing. `DeedFeel` is the presentation contract: animation, effect, sound and movement, checked by `deed_feel_missing` |
| `floats.coax` | The numbers over whoever a blow struck: damage taken (red), dealt, a critical hit (larger), a small "miss", a heal or a word, each queued through `react_present`'s per-source cap from the realm's own authority's events (a room's hit lines, a local battle's log); Everglade II's |
| `area-aim.coax` | Ground targeting for a placed area: armed by the first press, a square or circle that follows the crosshair on the ground, held to the ability's range (stopping at its edge, marked far), cast at exactly the point shown on confirm, dropped on cancel; Everglade II's area spells and v1's Entangle |
| `break.coax` | Breakables (debris, kinematic, structural cost profiles) that break once and hand back the declared fact patch, a sound stimulus and a bounded cosmetic debris burst; attachments with one parent authority, parent-before-child order and refused detach when no placement is free |
| `bot-battle.coax` | Bot battles (#13505): machines fight with the cards they hold, in real time. A bot holds up to four cards, each a move (keys 1 to 4) with a power, a charge cost, a cooldown, a reach and a stun; its battery is its energy and refills over time; a bot's health is the engine's vitals (`vitals.coax`), so at zero it powers down until repaired (`bb_repair`). A side fields one of its crew at a time and a swap (`bb_swap`, a core swap) takes 0.75 s. A press goes through the deed bar (`deed_ask`, answered at once by the battle's own rules), so a stunned bot's presses are refused and grey. `bb_pick` is a rule-based foe's choice. Splice Bots' battles run on it (`metaverse/splice-bots/splice-battle.coax`) |
| `hoverboard.coax` | A hoverboard (#13485) on the mount (`mount.coax`): called up under your feet and ridden at once, its casual and very fast speeds the mount's walk and gallop, so acceleration, carving, the jump and collision are `mount_step`'s. It adds the hover and bob, the lean into a carve, the nose lift, and the rush past casual that a realm turns into a wider view, a camera pulled back, a shake and speed lines (`board_fov`, `board_back`, `board_shake`, `board_lines`). A realm with its own colliders hands back where they pushed it (`board_resolve`). A world gives it with the board table (`BOARD_TABLE`), set by `openagents realm world-board`. Splice Bots rides it; the mesh is `kit_hoverboard` (`metaverse-engine/kit/machines.coax`) |

## Order within one tick

1. `action_advance` to the tick: crossed events, whether the sensor is open.
2. The realm evaluates the pose and attachments (`attach_order`) and samples
   the sensor's poses across the tick (`sensor_samples_needed`).
3. `sensor_resolve`: contacts, filtered and deduplicated.
4. The ruleset applies accepted contacts; `react_decide` names the reaction.
5. Presentation (`react_present`) reads accepted reactions only.

Interpolated drawing never senses.

## Adoption

- **Everglade Experimental** (`everglade/zone-combat.coax`):
  every weapon blow (the slash and the owned two-handed cleaves) is an action
  built from its clip (`xc_swing_def`, window ±6 ticks round the clip's
  impact). Each sensing tick samples the blade's actual pose (the swing clip's
  baked hands between frames, through the same `weapon_matrix` /
  `two_hand_matrix` that draws it) and sweeps the monsters' hurt capsules; the
  SRD Attack resolves once against the first accepted contact. The former
  press-time arc pick and pending impact time are removed.

- **State Lab** (`metaverse-engine/state/fixture-strike.coax`, #13027): [B] swings a
  32-tick action whose blade sweeps an arc ahead of you through its window. A
  contact with the gate's hurt capsule is the only way the gate breaks:
  `react_decide` names the reaction (a slash with a 4-tick hit stop that holds
  only you) and `break_hit` breaks it once, handing back the `gate.broken` patch
  (published through the ordinary command, so the collision box and navigation
  owner swap with the fact), a sound stimulus the NPC notes through
  `metaverse-engine/ai/stimulus.coax` when it is there, and 12 pieces of cosmetic debris.
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

## The action pipeline (#13072)

- **Everglade II** (`everglade/client/cast.coax`): a press asks with your bearing from the room's row of
  you (`rs_bearing_of`) and plays only when the room's `e cast` for your seat comes back. A refusal
  or a timeout greys the slot. The bow's draw asks before it starts (`e2k_ready`).
- **Everglade Experimental** (`everglade/zone-combat.coax xc_press`): the local rules answer in the
  same frame. A blow swings only once `fb_ready` takes it, and the bow draws only for a bearing
  that can act.
- **Everglade v1** (`everglade-spells.coax sp_cast`): in a room, the cast bar and its sparks start
  on the room's `e cast` for your seat (`sp_taken`), never on the press.
- The SRD mapping is `rules/srd/condition-bearing.coax`. Everglade's feel for every class
  ability and monster action is `everglade/client/class-feel.coax`.

```sh
coax/bin/coax test metaverse-engine/action/deed.coax --library
coax/bin/coax test metaverse-engine/action/action.coax --library
coax/bin/coax test metaverse-engine/action/sensor.coax --library
coax/bin/coax test metaverse-engine/action/react.coax --library
coax/bin/coax test metaverse-engine/action/break.coax --library
coax/bin/coax test metaverse-engine/state/fixture.coax --library
coax/bin/coax test --library apps/openagents-web/old/experimental/fivelab.coax --only "experimental combat"
```
