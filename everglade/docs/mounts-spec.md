# Mounts

This spec describes how mounts work in Everglade: horses you own, mounts you
summon for a short time, and how you ride, fight and travel on them. Everglade
follows the D&D 5E System Reference Document 5.2.1 (SRD 5.2.1) and adapts its
rules for action combat. Where this spec adds something the SRD does not have,
such as calling a dragon or whistling for your horse, it says so.

Mounts are built in three layers, and no zone has mount code of its own:

- **Engine (Aethyr).** The mount, the rider's seat, the gaits, mounting and
  dismounting, steering and riding collision live in
  `aethyr/action/mount.coax`; the rider's astride pose lives in
  `aethyr/action/ride-pose.coax`. Any realm can use them.
- **Game (Everglade).** Everglade's horses and the SRD's mount rules, shared
  by every Everglade zone, live in `everglade/mounts.coax`: the Riding Horse's
  tuning, the white horse's baked frames in the world pack (`glade/horse*`),
  drawing the horse and seating the rider, the shared bow from the saddle, and
  later stabling, whistling and summoned mounts.
- **Zone wiring.** A zone places its horses and connects its keys and HUD to
  the game layer. Everglade Experimental places one white horse near spawn.
  Everglade II adopts mounts by placing a horse.

## The SRD rules

These are the rules the system follows. Page numbers are SRD 5.2.1 pages.

### Mounted combat (pp. 15-16)

- **Who can be a mount.** A willing creature at least one size larger than the
  rider, with an anatomy suited to it, can serve as a mount. A Medium character
  rides a Large horse.
- **Mounting and dismounting.** During your move, you can mount a creature
  within 5 feet of you, or dismount. Either costs movement equal to half your
  Speed, rounded down. With a Speed of 30 feet, mounting costs 15 feet.
- **Controlling a mount.** You can control a mount only if it is trained to
  accept a rider. Domesticated horses, mules and similar creatures are.
- **Controlled mounts.** When you mount a controlled mount, its Initiative
  changes to match yours. It moves on your turn as you direct it, and it can
  take only the Dash, Disengage and Dodge actions. It can move and act on the
  turn you mount it.
- **Independent mounts.** A mount that lets you ride but ignores your control
  keeps its own place in the Initiative order and moves and acts as it likes.
- **Falling off.** If an effect is about to move your mount against its will
  while you ride it, you make a DC 10 Dexterity saving throw. On a failure you
  fall off and land Prone in an unoccupied space within 5 feet of the mount.
  You make the same save if you or your mount is knocked Prone.

### Saddles (p. 100)

A saddle comes with a bit, a bridle and reins. A Military Saddle gives
Advantage on any ability check you make to remain mounted. An Exotic Saddle is
required to ride an aquatic or flying mount.

### Mounts and other animals (p. 100)

| Animal | Carrying capacity | Cost |
| --- | --- | --- |
| Horse, Draft | 540 lb. | 50 GP |
| Horse, Riding | 480 lb. | 75 GP |
| Pony | 225 lb. | 30 GP |
| Warhorse | 540 lb. | 400 GP |

Feed costs 5 CP a day.

### Riding Horse (p. 360)

Large Beast, Unaligned. AC 11, HP 13 (2d10 + 2), Speed 60 ft., Initiative +1.
Str 16, Dex 13, Con 12, Int 2, Wis 11, Cha 7. CR 1/4 (XP 50; PB +2).
**Hooves.** Melee Attack Roll: +5, reach 5 ft. Hit: 7 (1d8 + 3) Bludgeoning.

### Warhorse (p. 364)

Large Beast, Unaligned. AC 11, HP 19 (3d10 + 3), Speed 60 ft., Initiative +1.
Str 18, Dex 12, Con 13, Int 2, Wis 12 (save +3), Cha 7. CR 1/2 (XP 100; PB +2).
**Hooves.** Melee Attack Roll: +6, reach 5 ft. Hit: 9 (2d4 + 4) Bludgeoning.
If the target is Large or smaller and the horse moved 20 or more feet straight
toward it immediately before the hit, the target takes an extra 5 (2d4)
Bludgeoning damage and has the Prone condition.

Both stat blocks are already in the engine
(`aethyr/rules/srd/monsters.coax`, `MON_RIDING_HORSE` and `MON_WARHORSE`).

### Find Steed (pp. 130-131)

Level 2 Conjuration (Paladin). Action, range 30 feet, V and S, Instantaneous.
You summon an otherworldly being that looks like a loyal Large mount of your
choice (a horse, camel, dire wolf or elk) in an unoccupied space in range. It
uses the Otherworldly Steed stat block: AC 10 + 1 per spell level, HP 5 + 10
per spell level, Speed 60 ft., and Fly 60 ft. only from a level 4 or higher
slot. It shares your Initiative count and is a controlled mount while you ride
it. If you are Incapacitated, it acts on its own right after your turn to
protect you. It disappears at 0 Hit Points or if you die. A Paladin's Faithful
Steed (p. 55) casts it once a Long Rest without a slot.

### Phantom Steed (p. 152)

Level 3 Illusion (Wizard). 1 minute or Ritual, range 30 feet, V and S,
duration 1 hour. A Large, quasi-real horselike creature appears with a saddle,
bit and bridle. It uses the Riding Horse stat block with a Speed of 100 feet.
When the spell ends, the steed fades over 1 minute so its rider can dismount.
The spell ends early if the steed takes any damage.

## Speeds in action combat

Everglade turns the SRD's feet per turn into metres per second. A character
with a Speed of 30 feet runs at 7.5 m/s. A Riding Horse has twice that Speed,
60 feet, and a horse at full gallop is much faster than a running person, so
the horse's gallop is set above the plain 2x ratio:

| Gait | Speed | When |
| --- | --- | --- |
| Walk | 4.0 m/s | the default while you steer |
| Gallop | 14.0 m/s | Shift held, or the stick pushed all the way on a phone |
| Phantom Steed gallop | 20.0 m/s | its Speed is 100 feet |

The horse speeds up at 6 m/s² and slows at 10 m/s². It turns at up to
2.4 rad/s at a walk and 1.4 rad/s at a gallop, so it carves wide arcs at
speed instead of spinning in place. The trot (5.5 to 8 m/s) and canter (8 to
11 m/s) play between the walk and the gallop as the speed passes through them.

## Owned horses

An owned horse is a creature in the world, not an item in your bag.

- **Getting one.** You buy a horse from a stable master (Riding Horse 75 GP,
  Warhorse 400 GP) or earn one from a quest. You own it until it dies or you
  sell it.
- **Stabling.** Every town's stable keeps your horses. A stabled horse is safe,
  fed and fully healed after a Long Rest. You take a horse out at any stable
  where you have stabled it.
- **Leaving it where you dismount.** When you dismount, your horse stays where
  you left it, as in Red Dead Redemption. It idles, grazes and turns its head,
  but it does not wander more than 3 metres, despawn or go home on its own.
- **Whistling.** Press **H** to whistle. If your horse is within 150 metres and
  can reach you, it trots to you along open ground and stops beside you. A
  horse farther away than that, or in another zone, does not hear you. A
  phone has a WHISTLE button while you own a horse and are not riding.
- **Calling from a stable.** At a stable you can call any of your horses there,
  even if you left it somewhere else. The left-behind horse returns to the
  stable.
- **Care.** A horse has its stat block's Hit Points and heals on your rests. It
  needs feed (5 CP a day, deducted at a Long Rest); an unfed horse gains one
  Exhaustion level a day, as a character does without food.
- **Death and recovery.** A horse at 0 Hit Points dies. Its saddle and
  saddlebags drop where it falls. You can bring it back with a spell that
  returns the dead to life, under that spell's rules, within its time limit.
  Otherwise it is gone and you need a new one. A horse is never lost because
  you logged out: a horse you left in the open goes back to its stable after
  you have been away for an hour.
- **Ownership.** Only you can ride your horse. Another player who presses E
  beside it sees your horse's name and nothing else. Party members can ride
  each other's horses only if the owner allows it in the party panel. This
  spec does not include horse theft.

## Summoned mounts

A summoned mount comes for a limited time and then leaves. It has no stable,
needs no feed and is never left behind.

- **Find Steed and Phantom Steed** follow the SRD rules above. A Phantom
  Steed fades over 1 minute when its hour ends or it takes damage. While it
  fades, a bar over the reticle shows the time left, and it dismounts you
  when the time runs out.
- **Dragon's Call (an Everglade addition, not SRD).** A rare item or quest
  reward calls a dragon to carry you for 2 minutes. It follows the Otherworldly
  Steed's level 4 rules: it is a controlled mount with Fly 60 ft. (18 m/s in
  flight). It lands in an unoccupied open space at least 10 by 10 metres
  within 30 metres of you. You can't call it indoors, underground, in a
  dungeon, in a town's walls or in a no-flight zone, and the call refuses with
  a plain message such as "No room to land here".
- **Flying limits.** A flying mount stays below the zone's ceiling (60 metres
  over the ground in the open world) and can't fly over a zone's edge. When
  its time runs out in the air, it glides down at 4 m/s, lands, and you
  dismount. You never fall from a summoned mount because its time ran out.
- **Where summoned mounts can't go.** No mount enters a building, a dungeon,
  a cave or a ship's hold. At an entrance you dismount and the mount waits
  outside (an owned horse) or leaves (a summoned one).
- **One at a time.** You can have one summoned mount at a time. Calling a new
  one dismisses the old one.

## Flying mounts and the red dragon

A flying mount is a mount with wings. It uses the same mount system as the
horse: you walk up, press E, ride, and get off where you want, and it stays
there. Only how it moves is different.

- **One flight module.** Flying mounts don't have flight code of their own.
  The engine's mount (`aethyr/action/mount.coax`, `mount_step_wings`) steps
  Aethyr's shared winged flight (`aethyr/action/flight.coax`, `wing_step`),
  the same module the druid's dragon form uses. Takeoff, climbing, gliding,
  diving, banking and landing behave the same whether you are the dragon or
  ride one.
- **The red dragon.** Everglade Experimental places one red dragon near
  spawn. It uses the Adult Red Dragon's speeds (SRD 5.2.1, Adult Red Dragon:
  Speed 40 ft., Fly 80 ft.) against the 30-foot pace of 7.5 m/s, so it walks
  at 10 m/s and flies at 20 m/s, about 2.7 times your run on foot and faster
  than a horse's 14 m/s gallop.
- **The saddle.** You sit at the base of its neck, on the model's `saddle`
  bone.
- **Taking off and landing.** On the ground it walks. Space takes off. In the
  air, Space beats the wings, holding Space climbs, letting go glides, and
  holding Ctrl dives and speeds it up. A and D steer and bank it into the
  turn; W flies on, and letting go of W slows it to a hover. **X** lands. It
  also lands when it glides down to the ground.
- **No getting off in the air.** E works only on the ground. In the air the
  prompt says "Land first" instead of "E  Dismount".
- **Collision.** Low over the ground its body meets walls as the horse's does.
  High enough, it flies over them. It stays under the zone's ceiling (60
  metres).
- **Camera.** While you ride the dragon the camera pulls back further than on
  the horse, enough to fit its wings.
- **Fighting from its back.** Your bow works as on the horse, through the
  shared bow system. **F** asks the dragon for its Fire Breath, aimed where
  you look (your yaw and pitch), through the SRD's dragon actions
  (`aethyr/rules/srd/dragon-actions.coax`: `rs_breath_ready`,
  `rs_in_cone3`, `rs_fire_breath_on`). The breath recharges on the SRD's
  Recharge 5-6, rolled at the start of the dragon's turn.
- **One pipeline.** Mounting, getting off, the breath and shots from the
  saddle are all deeds (`aethyr/action/deed.coax`): each declares its look
  and sound, asks with your bearing (`rs_bearing`: you can't mount while
  unconscious, paralyzed or grappled), the rules act, and only a yes plays.

Everglade's summoned dragon (Dragon's Call, above) uses this same mount once
summoning lands.

## Controls

| Action | Keyboard and mouse | Phone |
| --- | --- | --- |
| Mount | **E** within 2.5 m of the horse; the prompt "E  Mount" shows | the MOUNT button |
| Dismount | **E** while riding | the DISMOUNT button |
| Steer | **W** forward, **A** and **D** turn, **S** slows and backs up | the left stick: forward, turn, back |
| Gallop | hold **Shift** | push the stick all the way |
| Look | the mouse (mouselook) or a drag | a drag on the right side |
| Whistle | **H** | the WHISTLE button |
| Shoot | hold and release the right mouse button, or the bow's hotbar key | hold and release the bow's slot |
| Take off, beat, climb (flying mount) | **Space**: press to take off or beat, hold to climb | the JUMP button |
| Dive (flying mount) | hold **Ctrl** | the DIVE button |
| Land (flying mount) | **X** | the LAND button |
| Fire Breath (dragon) | **F**, aimed where you look | the BREATH button |

- **Mounting.** Mounting takes 0.6 seconds. You step to the horse's left side
  and swing up into the saddle. The horse stands still while you mount.
- **Dismounting.** Dismounting takes 0.5 seconds. You step down on the
  horse's left side. If that side is blocked, you step down on the right; if
  both are blocked, you can't dismount there and the prompt says "No room to
  step down".
- **The SRD cost.** In a fight, mounting and dismounting spend half your
  Speed, as the SRD says. Outside a fight there is no cost.
- **Camera.** While you ride, the follow camera pulls back 1.5 metres and
  rises 0.5 metres, so you see the whole horse. It eases in and out over half
  a second.

## Combat from horseback

- **Bows and crossbows: yes.** You draw, aim and shoot as you do on foot,
  through the shared bow system (`aethyr/action/bow.coax`). Your upper body
  takes the bow pose and your legs stay in the saddle. Arrows leave your hand
  and fly toward the crosshair. The SRD attack roll resolves as usual.
- **One-handed melee: yes.** You swing from the saddle with a reach of 5 feet
  from your seat.
- **Two-handed melee: no.** You need one hand for the reins. Greataxes,
  greatswords and other Two-Handed melee weapons are stowed while you ride.
  A Lance is the exception if Everglade adds one.
- **Spells.** You can cast spells that need V and S components. Spells with a
  casting time longer than one action, and Rituals, can't be cast while
  mounted.
- **Your mount in a fight.** A horse you control takes only Dash, Disengage
  and Dodge. A Warhorse's charge (Hooves after a 20-foot straight run) is the
  one attack a controlled horse makes, on your command, with **F**.
- **Being attacked.** Monsters can target you or your horse. The rider is the
  default target; a monster with a reason (a beast that hunts horses) can
  target the mount. A hit on you is your damage. A hit on the horse is the
  horse's.
- **Falling off.** Any effect that moves the horse against its will, or knocks
  you or it Prone, calls the DC 10 Dexterity saving throw. On a failure you
  land Prone within 5 feet of the horse.
- **Opportunity Attacks.** The horse's movement provokes Opportunity Attacks
  against the horse; your own movement doesn't, because you aren't moving
  yourself. Disengage on the horse applies to both of you.

## Multiplayer and server authority

- **The room server owns the horse.** Its position, facing, gait, rider and
  owner are server state. The client predicts its own riding the same way it
  predicts walking and accepts the server's correction.
- **Mounting is a request.** Pressing E sends a mount request. The server
  checks the range (2.5 metres), that the horse is free, that you may ride it
  and that you are alive and not Incapacitated. It answers with the new rider.
  Two players who press E on a free horse in the same tick: the first request
  the server receives wins.
- **No stealing.** A horse with an owner accepts only its owner and the
  players its owner allows. A free horse (a stable's loaner, a quest horse)
  accepts anyone.
- **Riding is a movement.** The rider's input drives the horse on the server
  with the same walking, collision and speed rules the client uses. A client
  that claims a faster horse or a horse through a wall is corrected.
- **Summons are server state.** The server starts and ends a summoned mount's
  time and refuses a call where it can't land.

## Animation

- **The rider's seat.** The saddle point is the horse rig's `Torso2` bone,
  baked per frame, so the rider rises and falls with the horse's back in
  every gait. The rider's pelvis sits on that point.
- **The rider's legs.** A keyed riding pose (`aethyr/action/ride-pose.coax`)
  turns the thighs out and forward around the horse's barrel, bends the knees
  and hangs the lower legs down the horse's sides, astride. The upper body
  plays the idle or an action (the bow, a cast, a swing) through the shared
  upper-body layer.
- **The horse's gaits.** Walk, trot, canter and gallop. The baked clips are
  Idle, Walk and Gallop; the trot plays the walk faster and the canter plays
  the gallop slower. The clip's playback rate follows the ground covered, so
  hooves don't slide.
- **Transitions.** Mounting and dismounting blend the rider from standing
  beside the horse into the seat over their 0.6 and 0.5 seconds.

## Fairness

- A mount helps you travel. It never makes your attacks stronger, except a
  Warhorse's SRD charge.
- Monsters keep their own speeds. Galloping past them provokes the horse's
  Opportunity Attacks as the SRD says.
- A horse can't enter places a horse can't fit. It can't climb ladders,
  stairs inside buildings or slopes steeper than 35 degrees.
- A horse costs gold and feed. A summoned mount costs a spell slot or a rare
  item. None of them is sold for real money.

## Data model

```
Mount (aethyr/action/mount.coax)
  x, z, face            position (metres) and heading (radians, 0 is +Z)
  speed                 metres per second along its heading, negative backing up
  phase                 gait clock (cycles of the current clip)
  rider                 -1 free, else the rider's id
  owner                 -1 free, else the owner's id
  state                 free, mounting, ridden, dismounting
  since                 when the state began (seconds)
  kind                  riding horse, warhorse, phantom steed, otherworldly steed, dragon
  until                 when a summoned mount leaves; 0 for owned
  hp, max_hp            from its stat block
  flies, wing           a flying mount: its WingState (aethyr/action/flight.coax)
                        holds its height, vertical speed, bank and flight phase
MountTune
  walk, gallop          top speeds per gait (m/s)
  accel, brake          (m/s²)
  turn_walk, turn_gallop  turn rates (rad/s)
  radius, half_length   the body's collision capsule
  reach                 metres from the rider to mount (2.5)
  seat                  baked saddle points per frame
```

A zone keeps one `Mount` per horse it places, through the game layer. The server keeps the same struct for a
multiplayer realm.

## Phases

1. **Now (Everglade Experimental).** One white horse near spawn. E mounts and
   dismounts within 2.5 metres, a phone button does the same, WASD steers,
   Shift gallops, the horse collides with walls, bows fire from the saddle,
   the horse stays where you leave it, and monsters attack the rider.
2. **Owned horses.** Stables, buying, the whistle, feed and care, death and
   recovery, stat blocks in the SRD engine (the horse's HP and AC, falling
   off).
3. **Room server.** The horse as server state, mount requests, ownership and
   correction. Everglade II places its first horse.
4. **Summoned mounts.** Find Steed, Phantom Steed and the fade.
5. **Flying.** The red dragon in Everglade Experimental lands first (see
   "Flying mounts and the red dragon"). Then the Exotic Saddle, flying Find
   Steed, and Dragon's Call.
6. **Mounted combat in full.** Two-handed stowing, the Warhorse's charge,
   Opportunity Attacks on the horse and monsters that target mounts.

## Tests

Every phase lands its checks as `#test` blocks in the module that owns the
behavior.

- **Engine (`aethyr/action/mount.coax`).** You can mount only within reach. E
  toggles mount and dismount. A dismounted horse stays where you left it. The
  gallop is faster than running on foot by a large margin. The horse's speed
  ramps with its acceleration, and its turn rate falls with speed. A wall in
  the horse's path stops it, and a step that would put its body into a box is
  refused. The dismount picks a clear side or refuses. A flying mount takes
  off on the jump, flies faster than the horse's gallop, banks into turns,
  can't be left in the air, lands, and stays where you got off. Low, a wall
  stops it; high, it flies over the wall.
- **Riding pose (`aethyr/action/ride-pose.coax`).** The thighs spread to
  either side of the saddle, the knees bend, and the feet hang below the
  seat.
- **Game and zone (`everglade/mounts.coax`, Experimental).** The horse is placed clear of the spawn's safe
  area and of every collider. A bow shot from horseback leaves from the
  rider's hand and resolves through the SRD attack. A monster's hit while you
  ride lands on you. Mounting asks with your bearing: unconscious or grappled,
  you can't mount, and only a yes plays the mount's sound and dust. In
  Experimental you mount the dragon in reach, take off, fly faster than the
  horse gallops, land, dismount, and it stays put.
- **Room server (phase 3).** Two simultaneous mount requests produce one
  rider. A non-owner's request on an owned horse is refused. A client's
  impossible horse position is corrected.
- **Shots.** Headless shots of the horse near spawn, mounted, galloping and
  drawing the bow on horseback.
