# Neverwinter's Ranger action combat, and what Everglade takes from it

This note studies the Ranger's combat in Neverwinter, Cryptic Studios' D&D
MMORPG, and lists what Everglade should adopt. Everglade follows the SRD 5.2.1
rules and adapts them for action combat (see
[chapter 18 of the book](../../../everglade/book/ch018-turns-become-time.md)).
The owner wants Everglade to adopt much of Neverwinter's action combat, so each
recommendation below names the SRD rule it must stay faithful to.

## Where the information comes from

- **The video.** "Neverwinter - Combat Concepts" by the channel Classic_DM
  (uploaded 2026-08-17, 46 minutes). A new Hunter Ranger plays through the
  tutorial: the Valindra Siege start, the bridge boss, and the combat training
  room in Protector's Enclave. The video has English auto-captions. This note
  paraphrases them and quotes only a few words, with timestamps. Timestamps are
  approximate, to within about 45 seconds.
- **Public sources.** The video covers a level-1 tutorial character, so it
  doesn't show the full power tray, stamina, or the daily-power gauge in detail.
  Those facts come from fan wikis and guides, listed in [Sources](#sources).
  Every such fact is marked **(public)**. Facts with a timestamp come from the
  video. Facts marked neither come from Everglade's code.

## Neverwinter's action combat model

### Targeting and aim

- **No tab-targeting.** A centered reticle picks the target. The reticle is
  white over nothing, blue over a friendly NPC, and red over an enemy (10:30).
- **Aim assist.** You don't need shooter precision. If the reticle highlights an
  enemy, the character pivots in place and the shot corrects onto it, even when
  the reticle drifts off to one side (11:15).
- **Range gating.** A power whose target is out of range is greyed out on the
  bar. The bar lights up as you close in. The Ranger's at-wills have an 80-foot
  range; a movement power has none (19:30).
- **Interact by reticle.** The same reticle shows a door or use icon on an
  interactive object, grey when you are too far and lit when you are close
  enough (13:30).

### Movement while attacking

- **You stop to shoot.** The defining rule: using a power, including the basic
  attack, roots you for its animation. Auto-run stops, and a held movement key
  resumes only after the shot (1:30, 6:45). Neverwinter's combat skill is
  choosing when to stand still.
- **Combat stance.** A drum beat and a crouched stance tell you that you are in
  combat. Movement in combat is slightly slower (10:30).
- **Strafe to change targets.** Enemies step in front of each other. Because you
  shoot what is ahead of you, you sidestep or dodge to get a clean line instead
  of turning in place (24:00).

### Dodge, shift and stamina

- **Shift is the class's dodge.** For the Ranger it is a short evasive slide
  (10:45, 27:00). Each class has its own version: dodge, sprint, block or
  teleport **(public)**.
- **Stamina.** Each shift spends part of a stamina bar, and you can't shift
  without enough stamina **(public)**.
- **Invulnerability frames.** A dodge timed to an attack avoids it. The video
  calls these i-frames (22:30). The training room's first lesson asks you to
  dodge three attacks "just before the enemy strikes" (36:45).
- **Step away.** You can also avoid a melee blow without a dodge: watch the
  wind-up animation and walk out of reach (2:15, 23:15).

### Powers and the slot layout

- **At-wills** have no cooldown. The basic attack is held down to repeat (11:15).
  The Ranger starts with Rapid Shot and Split Shot (39:45).
- **Encounter powers** have cooldowns shown as a countdown on the slot (4:30).
  The tutorial Ranger has a snare that roots, and a leap-back power (3:45).
  There are three encounter slots, on Q, E and R by default **(public)**.
- **Daily powers** cost a full Action Point gauge (41:15). The Ranger's are
  Seismic Shot, a shock wave that pulls enemies together, and a stealth power
  that hides you for 5 seconds and strikes foes within 15 feet as you pass
  (41:15, 42:00). There are two daily slots on 1 and 2, around a d20-shaped
  gauge **(public)**.
- **Mount power.** In combat the mount key fires the mount's combat power, an
  area blast that works like a trinket (34:30, 40:30).
- **Class features** are passives. The Ranger's shift applies a root through one
  of them (42:00).

### Action points

The gauge fills as you fight and empties when you use a daily power. The video
shows a tutorial that grants a full gauge (41:15). Classes fill it at different
rates, and some encounter powers add to it **(public)**.

### The Ranger's stance switch

The class mechanic switches between a ranged stance and a melee stance (38:15).
The Ranger draws its blades, and each at-will and encounter power becomes its
melee twin: Rapid Shot becomes Rapid Strike and Split Shot becomes Split Strike
(39:00). On the character sheet you click a power to see its twin (39:45). The
twins play differently: Rapid Strike is a four-hit combo whose fourth hit is
stronger, while Rapid Shot fires the same shot every time (40:30). On PC the
switch is the Tab key, and the swap covers the encounter tray too **(public)**.

### Aimed and charged shots

One Ranger at-will is held to charge: the longer you focus, the more damage it
deals, and you let go to fire (40:30). The Split Shot volley is a tapered cone
in front of you (39:45).

### Area telegraphs

- **Red zones.** An area attack shows a red shape on the ground before it lands,
  and there is "usually" enough time to step out (21:45). The video calls them
  denial areas.
- **Unmarked attacks.** Not every attack is telegraphed. A boss's pull and
  point-blank burst have only an animation tell, and a single-target spell
  follows you unless you break line of sight (21:45, 25:30).
- **Charges.** A mounted knight's charge has a wind-up you can read; take it and
  you are trampled and knocked down (23:15, 24:00).

### Crowd control

The tutorial Ranger already has two control powers: the root and the leap that
puts distance between you and the enemy (3:45). A boss is immune to the root,
but the power still deals its damage (26:15).

### Combat advantage and flanking

Combat advantage is Neverwinter's flanking: you gain it when the enemy stands
between you and an ally (37:30). A hit with combat advantage shows a small
"behind" arrow on its hit mark (6:00, 38:15). The tutorial notes that enemies
can flank you too. The video contrasts this with Pathfinder 2e's stricter
flanking-line rule (37:30).

### Threat

A small meter on the target's name plate shows how much of the enemy's aggro
you hold (25:30). Tanks hold aggro; ranged classes watch the meter.

### How it reads on screen

- **Hit marks.** A normal hit shows a yellow sword icon, a critical hit a
  different mark, and a hit with combat advantage a "behind" arrow (5:15–6:00).
- **Damage numbers.** Numbers float off targets (5:15). The video recommends
  turning off other players' numbers to cut clutter (17:15).
- **Health bars on demand.** Show an enemy's name on mouseover and its health
  when targeted or recently damaged. You then see at a glance which foes you
  have hurt without a screen full of full bars (17:15–21:00).
- **Boss bar.** Bosses get a large health bar at the top of the screen (25:30).
- **Sound.** A drum hit tells you that combat has started (9:45).
- **Recovery.** Health regenerates a few seconds after combat. Campfires heal
  you, and potions matter only mid-fight (2:15, 3:00, 20:15).
- **The core lesson.** Don't be a "block of hit points": use range, timing and
  movement instead of standing and trading blows (0:45, 2:15).

## How Neverwinter maps D&D rules onto real-time play

Neverwinter launched in 2013 on D&D 4th Edition structure and later took 5E
names and flavor. Its mapping:

| Tabletop | Neverwinter | Departure |
| --- | --- | --- |
| 4E at-will, encounter and daily powers | Same names: no cooldown, cooldown, and a gauge | Encounter and daily become timers and a meter, not "per fight" and "per day" |
| Action points (4E) | A gauge filled by combat | Spent on dailies, not on an extra action |
| Attack roll against AC | Hit if the reticle's target is in range; damage scales with stats | No visible d20. Crits and defenses are percentages |
| Combat advantage (4E) and flanking | Ally on the far side of the target | Given automatically by position |
| Shift (4E: a 1-square move with no opportunity attack) | A stamina dodge with i-frames | A defensive reflex, not a careful step |
| Rounds and turns | Continuous time | No action economy. Animation lock is the only limit |
| Area spells with templates | Red ground telegraphs | Enemies' areas are dodgeable; yours aren't shown to them |
| Hit points | Tens of thousands (55,000 at level 1, 21:00) | Item level drives numbers far above tabletop |
| Short and long rests | Out-of-combat regeneration and campfires | No rest rules |
| Death | Injuries that need injury kits (45:00) | Not death saving throws |

The pattern: Neverwinter keeps D&D's vocabulary and drops its math. Everglade
does the opposite. It keeps the SRD's math and changes only time, as chapter 18
describes. So Everglade can borrow Neverwinter's feel (aim, telegraphs, dodge
timing, stance swap, feedback) as long as the SRD still decides each outcome.

## What Everglade has today

- **Aim by facing.** Bolts go straight ahead with 5 degrees of assist; melee
  hits within 45 degrees; a cone uses the OpenAgents Metaverse Engine's attention query
  (`apps/openagents-web/experimental-combat.coax:397`,
  `apps/openagents-web/class-kit.coax:143-145`). Nothing is picked for you.
- **The bow.** Hold the right button to draw, release to loose. A crosshair
  closes as the draw builds; a full draw takes 0.8 s, and you slow to a walk
  while drawn (`experimental-combat.coax:907-1060`,
  `metaverse-engine/action/bow.coax:24`, `metaverse-engine/action/bow.coax:65`,
  `docs/verse/bow-combat.md`). The arrow is a real projectile, and the SRD
  Longbow Attack decides the hit, with Disadvantage past 150 feet
  (`rules/srd/battle.coax:1589`).
- **The pose.** An IK draw pose blended by draw strength
  (`metaverse-engine/action/bow-pose.coax`).
- **The Ranger's bar.** Hunter's Mark in slot 1
  (`apps/openagents-web/fivebar.coax:295`), then Attack and Entangle
  (`class-kit.coax:42`), then Dash from the level kit (`fivebar.coax:113`).
  The Ranger holds a Shortsword and a Longbow with the Slow and Vex masteries;
  it has Archery at 2, Colossus Slayer at 3 and Extra Attack at 5
  (`rules/srd/engine.coax:802-821`).
- **One shared cooldown.** One second between any two presses
  (`class-kit.coax:20`), shown as a dimming sweep over every slot
  (`experimental-combat.coax:1845`). Nothing runs out: the Everglade world
  declares unlimited presses (`RS_DEV_FREE_PRESS`, chapter 18).
- **Target lock.** A strike sets your target; it is held within 24 m and
  released when it falls (`experimental-combat.coax:330-367`).
- **Feedback.** Hit stops (`experimental-combat.coax:369`), floating numbers
  (`experimental-combat.coax:287`), a red off-screen wedge toward whoever hit
  you (`experimental-combat.coax:717`), positional sounds
  (`experimental-combat.coax:1270`), and visible enemy arrows and rocks
  (`experimental-combat.coax:665`).
- **Enemy tells.** A shot has a 0.6 s draw or wind-up before it leaves the hand
  (`metaverse/experimental-hostiles.coax:29-31`). Melee swings and area attacks have
  no ground telegraph.
- **Your own areas.** Hovering Thunderwave previews its footprint, and a cast
  keeps it in place (`experimental-combat.coax:1774`).
- **Movement.** Dash bursts you forward for 0.9 s
  (`experimental-combat.coax:1524`). There is no dodge roll and no stamina.
  Opportunity Attacks are real, with Disengage
  (`experimental-combat.coax:489-510`).
- **Advantage.** The SRD's Advantage sources, Vex, Sneak Attack's ally rule, and
  Disadvantage on a ranged attack with a hostile within 5 feet
  (`rules/srd/battle.coax:254`, `battle.coax:882`, `battle.coax:1646`,
  `battle.coax:1736`).

## Mechanic-by-mechanic comparison

| Neverwinter mechanic | Everglade today | Recommendation | SRD 5.2.1 rule to stay faithful to |
| --- | --- | --- | --- |
| Reticle color: white, blue, red | Reticle is "a little warmer" over a monster (`experimental-combat.coax:1090-1108`) | **Adopt.** Red over a hostile in range, grey when out of range, blue over a friend | None (presentation) |
| Aim assist pivot onto a highlighted foe | 5-degree bolt assist (`class-kit.coax:145`); bow has none (`experimental-combat.coax:961`) | **Adapt.** Bend a full-draw arrow onto the foe under the reticle within a small cone. Keep quick shots on the ray | Attack roll against AC still decides the hit |
| Out-of-range powers greyed on the bar | Bar shows only the shared cooldown (`experimental-combat.coax:1845`) | **Adopt.** Grey a slot when no target ahead is in its range | Spell and weapon range (normal and long range) |
| Root while using a power | Bow slows you to a walk (`bow.coax:65`); casts don't root | **Adapt.** Keep walking while drawn. Root only for a power's release frames | None. The Everglade world walks unpaid (`RS_DEV_RATE_WALK`) |
| Shift dodge with stamina and i-frames | Dash only (`experimental-combat.coax:1524`) | **Adapt.** A dodge step that is the SRD's Disengage or Dodge action, not free i-frames | Dodge action (attacks against you at Disadvantage); Disengage (no Opportunity Attacks); Action economy |
| Stamina bar | None | **Adapt.** Show the turn's movement and Bonus Action as the "stamina" the dodge spends | Speed and the Bonus Action, mapped by `rs_timing_move` |
| At-will / encounter / daily tiers | Flat bar, one shared 1 s cooldown, nothing spent | **Adapt.** Cantrips and Attack are at-wills; leveled spells and limited features show their own recharge (`RS_DEV_RECHARGE`, 9 s) | Spell slots; uses per rest; recharge is a declared house rule |
| Action points for dailies | None | **Skip.** Use the SRD's own limited resources (higher slots, Action Surge) instead of a second meter | No SRD equivalent |
| Stance switch, melee and ranged twins | Ranger holds both weapons; Attack is always the bow (`experimental-combat.coax:1121`) | **Adopt.** One key swaps Longbow and Shortsword; Attack's slot becomes the melee swing | Weapon swap as part of the Attack action (equip or stow one weapon per attack) |
| Four-hit melee combo | Fighter swing is one blow per press | **Adapt.** Combo length equals the attacks the Attack action allows: one, then two with Extra Attack | Extra Attack (Ranger level 5) |
| Charged aimed shot | Draw strength sets speed only (`docs/verse/bow-combat.md`) | **Adapt.** Full draw gives range and accuracy. No extra damage | Longbow damage 1d8 + Dex; Archery +2 to hit |
| Split Shot cone | None for the Ranger | **Skip** at levels 1–6. Revisit with a SRD option that hits several foes | No such SRD Ranger feature at 1–6 |
| Root encounter power | Entangle on the bar (`class-kit.coax:42`) | **Adopt** its feel: a telegraphed ground circle you place | Entangle (Restrained, Strength save) |
| Leap-back power | Dash forward only | **Adapt.** Dash plus Disengage as a backward hop | Disengage; Dash |
| Red ground telegraphs for enemy areas | Only shot wind-ups (`experimental-hostiles.coax:29`) | **Adopt.** Draw each monster area on the ground for its wind-up | Area of effect shapes; Dexterity saving throws still roll |
| Read melee wind-ups and step away | Opportunity Attacks exist; swings have no visible wind-up window | **Adopt.** A readable wind-up; leaving reach before impact makes the swing miss | Reach; Opportunity Attacks when you leave reach |
| Combat advantage by position | Sneak Attack ally rule, Vex (`battle.coax:1646`, `battle.coax:1736`) | **Adapt.** Show a "behind" mark when Advantage applied. Do not grant Advantage for flanking | SRD has no flanking Advantage; only listed Advantage sources |
| Hit marks: normal, crit, advantage | Numbers and hit stops only | **Adopt.** Distinct marks for hit, critical hit, and Advantage | Critical hit on a natural 20 |
| Health bars when targeted or damaged | Target's HP over the bar | **Adopt.** Show bars on targeted and recently damaged foes | None |
| Threat meter | None | **Skip** for now. Revisit with grouped play | No SRD threat |
| Combat-start drum | Positional cues only | **Adopt.** A combat-start sting | None |
| Hunter's Mark (not in Neverwinter) | Slot 1 (`fivebar.coax:295`) | **Keep.** Show the mark on the foe; move it with a Bonus Action when it dies | Hunter's Mark (1d6 Force, Concentration, move as Bonus Action) |
| Out-of-combat regeneration | Not in Experimental | **Skip.** Use Short Rest and Hit Dice | Short Rest |

## Prioritized changes for the Ranger and bow combat

1. **Stance swap between Longbow and Shortsword (medium).** One key, Tab on
   desktop and a bar slot on phones, swaps the held weapon. The Attack slot
   then fires the bow or swings the sword, and its icon changes. Melee swings
   reuse the Fighter's blade sensor. Files:
   `apps/openagents-web/experimental-combat.coax` (`xc_press`, the Ranger branch
   at line 1121), `apps/openagents-web/class-kit.coax` (`xc_kit`,
   `xc_aim_for`), `apps/openagents-web/everglade2-mouse.coax`,
   `apps/openagents-web/charactercreator.coax` (held weapon).
2. **Red ground telegraphs for monster attacks (medium).** Each monster's melee
   reach and area attack shows a red zone for its wind-up. A melee swing you
   leave before impact misses, and leaving still provokes the Opportunity
   Attack the SRD allows. Files: `metaverse/experimental-hostiles.coax`,
   `apps/openagents-web/experimental-combat.coax` (draw beside the Thunderwave
   footprint at line 1774).
3. **A dodge step that is the SRD's Disengage or Dodge (medium).** Shift hops
   you a square or two away and spends your turn's Bonus Action (Cunning Action
   or Step of the Wind) or Action. Show what it spent as a small "stamina" pip.
   No free i-frames: the SRD decides through Disadvantage and no Opportunity
   Attack. Files: `apps/openagents-web/experimental-combat.coax` (next to Dash
   at line 1524), `rules/srd/disengage.coax`, `rules/srd/dodge.coax`,
   `apps/openagents-web/fivebar.coax`.
4. **Reticle that reads range and allegiance (small).** Red over a hostile in
   range of the active weapon, grey when out of range, blue over a friend. Grey
   out-of-range slots on the bar. Files: `metaverse-engine/cam/reticle.coax`,
   `metaverse-engine/cam/mouselook.coax` (`ml_tint`),
   `apps/openagents-web/experimental-combat.coax` (`xc_crosshair` at line 1093,
   the bar overlay at line 1845).
5. **Hit marks for hit, critical and Advantage (small).** Read the event book
   for the roll's edge and natural 20, and draw a distinct mark with each
   number. Files: `apps/openagents-web/experimental-combat.coax`
   (`xc_float_from`, `xc_hostile_result`), `metaverse-engine/action/react.coax`.
6. **Per-tier recharge on the bar (medium).** Replace the single 1 s cooldown
   with at-will (Attack, cantrips: the 1.5 s turn) and recharging slots (leveled
   spells and features: 9 s under `RS_DEV_RECHARGE`), each with its own sweep.
   Files: `apps/openagents-web/class-kit.coax` (`XC_COOL`),
   `apps/openagents-web/experimental-combat.coax`,
   `rules/srd/world-profile.coax`. This changes the Everglade world's
   `RS_DEV_FREE_PRESS` deviation, so update chapter 18 and its tests in the
   same change.
7. **Extra Attack as a two-shot volley (small).** At level 5, one Attack press
   looses two arrows in quick succession, each its own SRD attack. A melee press
   makes a two-hit combo. Files: `apps/openagents-web/experimental-combat.coax`
   (`xc_bow_loose`, `xc_bow_strike`), `metaverse-engine/action/bow.coax`.
8. **Hunter's Mark you can see and move (small).** A mark over the marked foe;
   when it dies, the next press of Hunter's Mark moves the mark as a Bonus
   Action without spending a slot. Files:
   `apps/openagents-web/experimental-combat.coax`,
   `apps/openagents-web/class-look.coax`.
9. **Aim assist on a full draw (small).** A full-draw arrow bends onto the
   foe under the reticle within about 3 degrees. Quick shots stay on the ray.
   Files: `apps/openagents-web/experimental-combat.coax` (`xc_bow_point` at line
   961), `metaverse-engine/action/bow.coax`.
10. **Health bars on targeted and recently damaged foes, and a combat-start
    sting (small).** Files: `apps/openagents-web/experimental-combat.coax`,
    `metaverse/experimental-hostiles.coax`.
11. **A Ranger stealth beat (large, later).** Neverwinter's 5-second stealth
    daily has an SRD path: Hide, then attack from hiding with Advantage. Make
    Hide readable (a fade and a "hidden" mark) before adding more. Files:
    `rules/srd/hide.coax`, `apps/openagents-web/experimental-combat.coax`.

## What not to take

- **Stat inflation.** Neverwinter's 55,000-hit-point level-1 hero hides the
  dice. Everglade keeps SRD numbers so that every roll is legible.
- **Free i-frames.** A dodge that ignores the attack roll replaces the SRD's
  math with reflexes. Use Dodge and Disengage instead.
- **Positional Advantage.** Combat advantage from flanking is a 4E rule. The
  SRD 5.2.1 core rules don't grant it. Show the Advantage the SRD gives, and
  don't invent more.
- **A second resource meter.** Action points duplicate spell slots and limited
  uses.

## Sources

- Classic_DM, "Neverwinter - Combat Concepts," YouTube, 2026-08-17,
  <https://www.youtube.com/watch?v=PMXCEWw47bQ>. Timestamps used above:
  0:45 (agenda), 1:30 (powers root you), 2:15–3:00 (use range; regeneration),
  3:45–4:30 (leap back, snare, cooldown countdown), 5:15–6:00 (damage numbers
  and hit marks), 6:45 (no movement mid-shot), 9:45–10:30 (combat drum, combat
  stance, reticle colors), 11:15 (aim assist, hold to repeat), 13:30
  (interaction reticle), 17:15–21:00 (health bar and floater settings), 19:30
  (80-foot range, greyed slots), 21:45–22:30 (single-target spells, red area
  zones, i-frames), 23:15–24:00 (charge wind-up, knockdown, strafing past
  blockers), 25:30 (boss bar, threat meter), 26:15 (root immunity), 36:45
  (dodge training), 37:30–38:15 (combat advantage), 38:15–40:30 (stance switch,
  power twins, combo, charged shot), 40:30–42:00 (mount power, action points,
  dailies).
- Neverwinter Wiki, "Encounter power,"
  <https://neverwinter.fandom.com/wiki/Encounter_power> (Q, E and R slots).
- Neverwinter Wiki, "Action Point,"
  <https://neverwinter.fandom.com/wiki/Action_Point> (the d20 gauge and dailies).
- Neverwinter Wiki, "Hunter Ranger,"
  <https://neverwinter.fandom.com/wiki/Hunter_Ranger> (stance switch on Tab;
  marked historical by the wiki).
- Gamepressure, "Neverwinter: Skills,"
  <https://www.gamepressure.com/neverwinter/skills/zf4d3d> (bar layout,
  Shift and stamina).
- nwguide, "Gameplay mechanics,"
  <https://nwguide.gitbook.io/main/basics-of-neverwinter/stat-system-in-basic/gameplay-mechanics>
  (stamina and class tactic).
- SRD 5.2.1, Wizards of the Coast, CC BY 4.0, <https://www.dndbeyond.com/srd>.
