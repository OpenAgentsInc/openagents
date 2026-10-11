# The Ice Cream Man

Status: design 2026-10-11; MVP being built as a browser game first, the way
[Grow Little Bunny](grow-little-bunny.md) was. A Roblox version follows the
Verse one, so controls, camera and feel copy Roblox from the start. Numbers
marked *tunable* are starting values for playtests, not measured results.

The Ice Cream Man is a silly "horror" escape game. You wake up in an
Airbnb that keeps going: hallways and rooms repeat like the Backrooms, but
everything is ice cream. Somewhere in the rental is the Ice Cream Man, a
walking ice cream cone with a huge bald forehead, who hunts you and fires
sprinkles. You fight back with a hot fudge blaster and a fart launcher,
find the way out, and escape. It is meant to be funny, not scary: the
"horror" is a weird-looking man and, sometimes, a giant booger.

It is the second community game, built on the same engine contract as Grow
Little Bunny.

## Contents

- [Pillars](#pillars)
- [Core loop](#core-loop)
- [The Airbnb](#the-airbnb)
- [The Ice Cream Man](#the-ice-cream-man-1)
- [The Giant Booger](#the-giant-booger)
- [Weapons](#weapons)
- [Health, hits and losing](#health-hits-and-losing)
- [Escaping](#escaping)
- [Controls (Roblox-style)](#controls-roblox-style)
- [Camera](#camera)
- [Art direction](#art-direction)
- [Audio](#audio)
- [Multiplayer](#multiplayer)
- [MVP scope](#mvp-scope)
- [After the MVP](#after-the-mvp)
- [Roblox version](#roblox-version)

## Pillars

1. **Funny, not frightening.** Chases are tense, but every hit is a joke:
   sprinkles, fudge, fart clouds, a booger. No gore, no jump scares that
   punish; kid-safe.
2. **Roblox feel.** A blocky third-person avatar, WASD and mouse camera,
   a tool bar at the bottom, jump on Space. Anyone who has played Roblox
   knows how to play in five seconds.
3. **Liminal ice cream.** Rooms that feel familiar and slightly wrong:
   carpet the colour of strawberry, wallpaper of waffle cone, buzzing
   freezer lights, a hallway that is longer than it should be.
4. **One short run.** A run lasts 5–10 minutes *tunable*: find the way out
   or get caught.

## Core loop

1. Spawn in the Airbnb's bedroom with a check-in note ("Check-out is at
   11. Don't let him in.").
2. Explore rooms to find the **three check-out keys** *tunable*.
3. The Ice Cream Man roams, hears you, and chases on sight. Shoot him with
   hot fudge to slow and stun him, or blast him away with the fart
   launcher, then break line of sight and hide.
4. With all keys, reach the front door and escape. Your time and how often
   you were hit go on the run card.

## The Airbnb

- A house that loops: a set of hand-made room modules (bedroom, kitchen,
  living room, bathroom, laundry, hallway, stairwell, "the freezer room")
  joined by doors. Each run shuffles the layout from a seed, Backrooms
  style: some hallways repeat, some doors open onto the same room again.
- 12–20 rooms per run *tunable*, one floor in the MVP.
- Ice-cream theming: walls in pastel flavours (strawberry, mint, vanilla,
  chocolate), waffle-cone trim, melting drips on ceilings, sprinkles on the
  carpet, freezer-hum lighting with a slight flicker.
- Interactables: doors (some locked until a key), closets and under-bed
  spots to hide, fridges that hold pickups, the front door (exit).
- Keys spawn in fixed slots per room module, three chosen by the seed.

## The Ice Cream Man

- **Look:** a walking ice cream cone. Waffle-cone body and legs, a scoop
  of vanilla on top as his head, a huge shiny bald forehead taking up most
  of the scoop, tiny eyes, a wide grin, little sprinkles stuck to his
  cheeks. He bobs when he walks and leaves drips.
- **Behaviour (state machine):** *wander* rooms → *investigate* noises
  (running, firing, doors) → *chase* on sight → *search* the last seen spot
  → back to *wander*. Speed slightly below a sprinting player and above a
  walking one *tunable*, so sprinting matters.
- **Attack:** fires a spray of **sprinkles** at range (each volley a few
  pellets, a hit costs health) and grabs at close range (a bigger hit and a
  knockback).
- **Weakness:** hot fudge slows him and, after enough fudge, he freezes
  solid for a few seconds (melted fudge hardens on him). The fart launcher
  knocks him back and confuses him (he loses track of you).
- He can't open closets you are hiding in unless he saw you enter.

## The Giant Booger

A rare second threat for variety (MVP: optional, behind a flag). A huge
green booger blob that oozes slowly along hallways. It doesn't chase; it
blocks routes and is sticky (touching it slows you a lot and costs a
little health). Fart blasts push it back down a hallway.

## Weapons

Tools in the hot bar (keys 1–3), Roblox style. Each has limited ammo,
refilled from pickups in fridges and cupboards.

| Tool | What it does | Ammo *tunable* |
|---|---|---|
| **Hot Fudge Blaster** | Fires a stream of hot fudge. Slows the Ice Cream Man per hit; enough hits freeze him for 3 s. | 30 squirts |
| **Fart Launcher** | Lobs a fart cloud: knockback in a radius, and he loses track of you. Funny sound. | 3 clouds |
| **Flashlight** (MVP optional) | Lights dark rooms; shining it on him makes him shield his forehead for a moment. | battery |

## Health, hits and losing

- 100 health *tunable*. Sprinkle pellet: −5. Grab: −30 and knockback.
  Booger touch: −2 per second and slowed.
- Health slowly regenerates when you haven't been hit for 10 s.
- At 0 health you're "scooped": a short funny cut (he plops you into a
  cone), then the run card and Try again.

## Escaping

Collect all keys, reach the front door, hold E for 2 s to unlock while he
may be closing in. Escape plays a short exit scene (you burst out into
daylight, he waves sadly from the window) and the run card: time, keys,
hits taken, fudge accuracy.

## Controls (Roblox-style)

| Action | Keyboard and mouse | Touch | Gamepad (later) |
|---|---|---|---|
| Move | WASD / arrows | left thumbstick | left stick |
| Camera | right-drag to orbit, wheel to zoom; Shift Lock toggle (Ctrl) for over-the-shoulder aim | drag right side | right stick |
| Jump | Space | jump button | A |
| Sprint | hold Shift | sprint toggle | L3 |
| Use tool | left click | fire button | RT |
| Pick tool | 1 / 2 / 3, or click the hot bar | tap hot bar | LB / RB |
| Interact (doors, hide, pick up) | E | interact button | X |
| Menu | Esc | menu button | Start |

A small first-run tutorial teaches these in four cards (move, look, shoot,
find the keys), skippable and re-openable, like Grow Little Bunny's.

## Camera

Roblox default: third person behind the avatar, orbit with right-drag,
zoom with the wheel (first person at closest zoom). Shift Lock locks the
camera over the right shoulder with a crosshair for aiming tools. The
camera pulls in when a wall is behind the avatar.

## Art direction

Low-poly and blocky like Roblox: a block avatar, chunky furniture, flat
pastel materials with a slight gloss. The Ice Cream Man is the one detailed
model, so he reads as weird. Lighting is the horror: dim freezer-blue
corridors, flickering fluorescent tubes, warm lamps in safe rooms.

## Audio

Freezer hum and fluorescent buzz in halls; an ice-cream-truck jingle,
slightly out of tune, that gets louder as he gets closer (the chase cue);
squelchy fudge shots; comic fart sounds; a "bloop" for sprinkles.

## Multiplayer

MVP is single player. Next, 1–4 players in one Airbnb over NIP-MV presence
(the same path as Grow Little Bunny's meadow), with the Ice Cream Man's
state run by the host and mirrored to the others.

## MVP scope

Built first as a browser game in this repository, the way Grow Little Bunny
is (`crates/bunny-web` and `crates/bunny-rules` are the models): a pure
rules crate (`crates/icecream-rules`: layout generation from a seed, the
Ice Cream Man's state machine, combat, health, keys, win and lose) with
unit tests, and a wasm web crate (`crates/icecream-web`) that draws it with
the same engine stack as Grow Little Bunny and is served at
`/games/the-ice-cream-man`, linked from the Blue Rush Studios homepage.

The MVP has:

- a seeded one-floor Airbnb of 12+ rooms from at least 6 room modules;
- the blocky avatar with Roblox controls and camera (keyboard, mouse,
  touch);
- the Ice Cream Man with wander, investigate, chase and search, sprinkle
  volleys and a grab;
- the hot fudge blaster and the fart launcher, ammo pickups;
- health, hiding in closets, three keys, the front door escape, the
  scooped loss, the run card;
- the first-run tutorial and a How to play page on the Blue Rush site;
- placeholder low-poly art and sounds that read clearly.

Not in the MVP: the Giant Booger (flag), multiplayer, the flashlight,
gamepad, a second floor, cosmetics.

## After the MVP

The Giant Booger, 1–4 player co-op, more room modules and floors, daily
seeds and leaderboards, unlockable avatar hats (a waffle cone, a cherry),
harder "Double Scoop" nights with two Ice Cream Men.

## Roblox version

After the Verse MVP is fun, port it to Roblox (Luau): the same rooms as
Roblox models, the Ice Cream Man as a scripted NPC with PathfindingService,
tools as Roblox Tools in the backpack (the hot bar is Roblox's own), and
the same tuning numbers. The rules crate's numbers and the room list are
the shared source for both.
