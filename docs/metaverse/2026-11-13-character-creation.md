# Character creation: many body shapes, one wardrobe

Status: spec, 2026-11-13. It covers the player character for Splice and any
other realm drawn with the cel kit. Everglade's Character Studio
(`character-creator.md`) is a separate thing: it dresses the Quaternius
humans for Everglade II and doesn't change.

## What we want

A player makes their own character in two moves:

1. **Pick and tune a body.** Bodies differ as much as a real crew does: a
   towering bruiser, a short stocky mechanic, a tall lean scout, a huge
   heavyset elder, an athletic fighter, a small slight kid.
2. **Dress it.** Every garment comes from one wardrobe with one look: black
   work clothes with white stitch lines. Every garment fits every body.

The result is a 3D character in the cel style Splice's cast of five already
uses (`metaverse/splice/splice-cast.coax`): flat fills, clean ink contours, a
silhouette you can read from far away.

## The two reference sheets

The owner supplied two sheets. They aren't in the repository; build from the
descriptions here and the owner's local copies.

- **Body shapes (the yellow crew sheet).** Six people in yellow work suits
  beside a 1.8 m scale figure. Use it for proportions only, not for clothes
  or colour:

  | Body | Height | Build |
  |---|---|---|
  | Bruiser | about 2.2 m | Very wide shoulders, thick arms and neck, big hands, narrow hips |
  | Mechanic | about 1.5 m | Short and stocky, round belly, short legs, thick forearms |
  | Scout | about 1.9 m | Tall, lean, long legs, narrow shoulders |
  | Elder | about 2.1 m | Very heavy, wide belly and hips, sloped shoulders, short neck |
  | Fighter | about 1.75 m | Athletic, defined waist, medium shoulders |
  | Kid | about 1.6 m | Small, slight, thin limbs, larger head for the body |

- **Clothing (the black outfit sheet).** One person in six outfits. Match it
  as closely as the cel kit allows:

  | Outfit | Pieces |
  |---|---|
  | Flight suit | Zipped jumpsuit with a collar, chest pockets and rolled sleeves; wide belt; cargo pockets; knee patches |
  | Deckhand | Cropped top with mesh sleeves; baggy cargo trousers; a jacket tied round the waist |
  | Vac suit | Fitted suit with piping along the limbs; clear bubble helmet; ear pieces; a pack on one hip; hard knee and elbow caps |
  | Runner | Cropped hooded jacket with zips; fitted leggings with piping; knee pads; thigh strap |
  | Shroud | Hood and face mask; layered scarf; long open coat; chest strap; shin guards |
  | Fitter | Bib overalls over a long-sleeved shirt; cross strap; satchel; a wrench in the belt; goggles on the head |

  Common to all six: black cloth in two or three near-black tones, thin
  white stitch lines along seams, pockets, belts and soles, silver buckles,
  fingerless gloves, and chunky lace-up boots with a thick tread.

## Design

### A body is a small record

A body is a set of numbers, each clamped to a stated range. Presets are
named values of the record; sliders edit it.

- Height.
- Shoulder width, chest depth, waist, belly, hips.
- Arm thickness, leg thickness, arm length, leg length.
- Neck length and thickness, head size.
- Hand size, foot size.
- Posture: upright to stooped.

The six bodies in the table above are the first presets. A check asserts
that every preset and every corner of the slider ranges produces a body
whose parts still join: no gap at a shoulder, hip, knee or neck, and feet on
the ground.

The body places a fixed set of named joints (the same names as the engine's
figure rig in `metaverse-engine/anim/figure.coax`), so a created character
can take the engine's gaits and emotes. Bodies differ in bone lengths, never
in bone count or names.

### A garment is fitted, not modelled

A garment is a function of the body record. It reads the body's
measurements and draws itself round them with a stated ease (fitted, loose,
baggy). No garment has a mesh of its own, so there is nothing to refit when
a body changes, and a new body shape needs no new clothes.

- **Slots:** head, face, top, outer, hands, belt, legs, feet, and up to
  three carried things (pack, satchel, tool).
- **First wardrobe:** the pieces of the six outfits above, each in its
  slot. Six presets name the full outfits; a player can mix pieces.
- **The look lives in one place:** the cloth tones, the stitch line, the
  buckle metal and the boot tread are one palette and one set of helper
  shapes. A stitch line is geometry or a shader line the cel renderer
  already supports, never a texture a body shape would stretch.
- **Rules between slots** are data: a helmet hides the hair and hood, an
  outer coat hides a tied jacket, overalls replace the belt.

A check dresses every body preset in every outfit and asserts that no
garment is thinner than the body under it at any measured ring (so skin
never pokes through), and that the vertex count stays inside a stated
budget.

### Head, face, hair and skin

Enough for a player to look like their own person, in the sheet's manner:

- Skin: a row of tones.
- Face: a few shapes for jaw, eyes, brows and nose; optional scar, beard or
  moustache.
- Hair: six to eight styles to start (messy bun, crop, mohawk with braid,
  long tied, curls, shaved), in any hair colour.

### The process a player sees

One screen, a turntable with the character lit in the middle, and five
steps down the side. A player can jump between steps; nothing is lost.

1. **Body:** six presets, then the sliders.
2. **Face and hair.**
3. **Outfit:** six presets, then one row per slot.
4. **Carry:** pack, satchel, tool.
5. **Name:** then USE THIS CHARACTER.

RANDOM makes a whole valid character. Every control is a `components/kit`
control on the mono skin, the same as `/chat`. The steps sit in a pane
beside the scene, as the editor's panes do, and never over the character.

### The character is a document

A character is one small, versioned document: the body record, the face and
hair choices, and one garment per slot. The page saves it, an agent can
write it through the page's link, and the game loads it. A document with a
value out of range is refused with a reason, never clamped silently.

## First deliverable

A standalone scene, built like the review scenes for Splice's cast of five:

- Six example characters in a row on a plain ground with a height scale,
  one per body preset, each in a different outfit from the wardrobe.
- A second view that shows one outfit on all six bodies, to prove the
  fitting.
- A turntable on a chosen character, and the body sliders and outfit rows
  working on it.
- Pictures of each view committed under `docs/metaverse/shots/`, taken
  headless, and compared by eye against the two sheets.

The modules live in the Metaverse Engine (`metaverse-engine/`), not in
Splice, so any realm can use them. The page is its own realm page; it adds
nothing to Splice's module, which is near its size budget.

## Not in this pass

- Using the created character in Splice's game or in rooms.
- Cloth simulation. Baggy cloth is shaped, not simulated.
- More than one wardrobe look, or colour choices for clothes.
- Quaternius or Blender assets. The cel kit's shapes are the source, so
  bodies stay parametric. If a later pass wants sculpted heads or hands, it
  imports them through the pack importer.

## Open questions

- Whether clothes get a colour choice later, or the black wardrobe is the
  game's uniform.
- How much face detail the cel style carries at play distance.
