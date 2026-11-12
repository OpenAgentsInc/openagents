# 7. Character Create in the world

> **Status: partly implemented.** The studio and the run out of it are
> [`everglade/client/create.coax`](../client/create.coax),
> loaded into `everglade/client/client.coax`. The wardrobe's choices and record are in
> `charactercreator.coax`. Other agents are editing both files now, so this
> chapter names functions and has no source listings. See
> [`docs/verse/character-creator.md`](../../docs/verse/character-creator.md).

This chapter answers the player's question: "Where do I go after I make my
hero?"

In many games, you make a character on one screen, then wait for the world to
load. In Everglade, Character Create's studio stands in the glade where the
world begins. When you finish, your hero runs out of the glade and down the
path, and the keys become yours. Nothing loads and nothing fades.

## What this chapter teaches

- How to run the studio and the world as one page, one Wasm world, one
  renderer and one clock.
- How the studio keeps your character, with its SRD class and level.
- How the hand-over from the studio to the follow camera works.

## What exists today

- **The studio.** A studio camera on your character, the wardrobe down the
  left and a class pane beside it. You pick one of the twelve SRD 5.2.1
  classes on the CLASS row (`E2C_CLASS_NAMES`) and a level from 1 to 6 on the
  LEVEL row (`E2C_LEVELS`). The class pane shows the class's Hit Die, primary
  ability, saving throws, training and level 1 features.
- **The record.** USE IN EVERGLADE keeps your character as one record with its
  class and level (`e2c_request`, `e2c_action`, message `E2C_SAVE`). The host
  says that it kept the record (message `E2C_KEPT`).
- **The run.** `e2c_walk` starts the run out of the glade, from where the
  studio drew you. The camera swings round behind you over 3 s (`E2C_SWING`),
  you run on your own for up to 20 s (`E2C_RUN`) and the camera eases into the
  follow camera over 1.5 s (`E2C_EASE`). Then the keys are yours. Any key or
  click skips to the ease (`e2c_skip`).
- **The arrival.** Through the portal, the page holds the portal's light until
  the glade has drawn, then opens the glade over 2.9 s, brings the camera in to
  the studio and fades in the wardrobe (`e2c_arrive_step`).

## The method

Make the studio a mode of the world, not a separate page:

1. Boot Everglade II itself, in the glade, in Character Create's mode
   (`e2c_creating`). The world's renderer, light and clock are already
   running.
2. Route your keys and pointer to the wardrobe while you're in the studio
   (`e2c_pointer`, `e2c_key`, `e2c_wheel`).
3. When you finish, keep the record first. Only start the run after the host
   says that the record is kept. If the keep is refused, stay in the studio.
4. Run your character out on the real controller, with the camera following,
   then hand over the keys.
5. Don't change the page. On everglade.world the address stays `/play`. On
   other hosts, the page writes `/worlds/everglade2` into the address bar in
   place (`E2C_ADDRESS`, `history.replaceState`), with nothing loaded.

## A worked example: the hand-over timeline

You press USE IN EVERGLADE at t = 0:

```text
t = 0.0 s     the record is sent; the host keeps it (E2C_KEPT)
t = 0.0 s     the run starts: the controls fade over 0.8 s (E2C_FADE)
t = 0.0–3.0   the camera swings round behind you (E2C_SWING)
t = 3.0–23.0  you run out of the glade and down the path on your own (E2C_RUN)
t = 23.0–24.5 the camera eases into the follow camera (E2C_EASE)
t = 24.5 s    the keys are yours
```

If you press a key at t = 5 s, `e2c_skip` moves the run's clock to 23 s, and
the keys are yours 1.5 s later. At a jog of 4.25 m/s, the full 20 s run can
cover up to 85 m of the path.

## The checks

In `everglade/client/create.coax`:

- `#test "create: USE IN EVERGLADE keeps the record with its class, then runs
  and says Everglade II's address"`.
- `#test "create: the record kept is the one Everglade II restores on its own
  page, class and all"`.
- `#test "create: the LEVEL row steps 1 to 6 round, and the record keeps it for
  Everglade II (#13046)"`.
- `#test "create: a refused keep stays in the studio, and Everglade II's own page
  is not the studio"`.
- `#test "create: through the portal the page holds the portal's light until the
  glade draws, then opens, comes in, pops and shows the wardrobe"`.
- `#test "create: the wardrobe fits a phone and a desktop, on the glass"`.
- `#test "/play is Character Create without a kept character, the world with
  one"`.

The capture in
[`docs/verification/character-create-to-everglade-2026-11-07/`](../../docs/verification/character-create-to-everglade-2026-11-07/)
shows the studio and the walk out.

```sh
coax/bin/coax test --library everglade/client/client.coax
```

## Mistakes

- **A run before the keep.** If the run started before the record was kept, a
  failed keep would leave you in the world with a character that the next
  visit can't restore. The run waits for `E2C_KEPT`, and the refused-keep test
  checks that a refusal leaves you in the studio.
- **A studio on the world's own page.** Opening `/worlds/everglade2` with a
  kept character must not show the studio again. The same test asserts that
  Everglade II's own page is not the studio.

## What comes next

[Chapter 8](ch008-if-you-can-see-it-you-can-walk-to-it.md) makes every tree and
rock you see stop you where you see it.
