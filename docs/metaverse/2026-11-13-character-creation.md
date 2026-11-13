# Character creation: many body shapes, one wardrobe

Status: the first deliverable is built (2026-11-13, GitHub #13462) and
this page describes it as built. See it at `/worlds/crew`. It covers the
player character for Splice and any other realm drawn with the cel kit. Everglade's Character Studio
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
named values of the record; sliders edit it. The record is sixteen whole
numbers (`metaverse-engine/kit/crew-body.coax`): the height in centimetres,
a belly and a stoop from 0 to 100, and every other number a percentage of
the usual build at that height. The ranges are in
[the document's page](../protocol/crew-character-v1.md).

- Height.
- Shoulder width, chest, waist, belly, hips.
- Arm girth, leg girth, arm length, leg length.
- Neck length and girth, head size.
- Hand size, foot size.
- Stoop: upright to stooped.

The six bodies in the table above are the first presets. A check asserts
that every preset and every corner of the slider ranges (all 65,536)
produces a body whose parts still join: no gap at a shoulder, hip, knee or
neck, and feet on the ground (`cb_joined`). Two numbers are held to the body
they are on so that this is true everywhere: a neck is never wider than the
head on it, and shoulders are never narrower than the chest between them.

The record makes a form: the joints, and a loft for the torso and for each
arm and leg. A loft is a smooth skin over a row of rings
(`metaverse-engine/render/cel-loft.coax`); the cel kit had tubes, boxes and
ellipsoids, and a torso or a trouser leg needs more than a tube.

The body places a fixed set of named joints (the same names as the engine's
figure rig in `metaverse-engine/anim/figure.coax`), so a created character
can take the engine's gaits and emotes. Bodies differ in bone lengths, never
in bone count or names. The body and the rig can't be loaded into one unit
today (each has its own short names for the same arithmetic), so
`coax/web/tool.sh metaverse-engine/tools/crew-rig-names.coax` holds the two
tables of joints together. Nothing drives the joints yet: a created
character stands in one pose.

### A garment is fitted, not modelled

A garment is a function of the body record. It reads the body's
measurements and draws itself round them with a stated ease (fitted, loose,
baggy). No garment has a mesh of its own, so there is nothing to refit when
a body changes, and a new body shape needs no new clothes.

- **Slots:** head, face, top, outer, hands, belt, legs, feet, and three
  carried things (pack, satchel, tool).
- **First wardrobe:** the pieces of the six outfits above, each in its
  slot (`metaverse-engine/kit/crew-wardrobe.coax`). Six presets name the
  full outfits; a player can mix pieces. The jacket tied round the waist is
  a belt, since that is where it is worn. The clear bubble helmet is a seat
  ring, two hoops and ear pieces: the cel pass draws nothing see-through.
- **The look lives in one place:** the cloth tones, the stitch line, the
  buckle metal and the boot tread are one palette and one set of helper
  shapes. A stitch line is geometry or a shader line the cel renderer
  already supports, never a texture a body shape would stretch.
- **Rules between slots** are data (`CW_RULES`): a helmet hides the hair's
  bun and a jacket's hood, a hood presses the hair flat, an outer coat hides
  a tied jacket, overalls replace the belt. A hidden piece stays in the
  document; the creation screen says it is hidden.

Every fitted piece is the body's own rings with an ease added, so it has
the body's ring stations and its sides. A check dresses every body preset
in every outfit, and the two bodies furthest apart in every single piece,
and asserts that no garment is thinner than the body under it at any ring
(`cw_covers`: the torso, each limb, and the whole body's hull for a skirt,
a coat or a hood), and that a dressed character stays inside 34,000
vertices (30,000 before the modelled face and hair, #13490).

### Head, face, hair and skin

Enough for a player to look like their own person, in the sheet's manner:

- Skin: a row of tones.
- Face: a few shapes for jaw, eyes, brows and nose; optional scar, beard or
  moustache.
- Hair: eight styles (messy bun, crop, mohawk with braid, long tied,
  curls, shaved, topknot, long loose), in a row of eight colours.

They are `metaverse-engine/kit/crew-head.coax`. Every face is drawn in
one style, taken from the owner's face sheet (not in the repository): a
comic look with ink lines and cel shading. The choices vary features inside
that style:

- **The head is modelled.** An oval with a V-shaped jaw down to a pointed
  chin, cheekbones with a plane under each, eye sockets, a brow ridge, a
  nose ridge with wings, and full lips. The cel pass shades the planes and
  inks the creases. The jaw choice moves the jaw: soft (the base), square
  (wide corners) or narrow.
- **The features are drawn on the surface.** Large almond eyes with a dark
  lash line and a wing, a lower liner, a lid crease, light irises and a
  highlight; angled brows; a small nose drawn with a tip line and
  nostrils; a defined upper lip with a bow.
- **Rough choices follow the owner's second sheet.** A square jaw, heavy
  brows, a broad nose, a moustache or a beard make the whole face rougher:
  a heavier brow ridge and brows, narrower eyes, creases between the brows
  and beside the nose, and darker irises.
- **Hair is locks.** Tapered locks swept from a part, loose strands at the
  face, and for the bun a high knot of loops and flyaways. Small silver
  hoops hang at the ears when nothing covers them.

The head, face and hair together stay inside 14,000 vertices.

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
beside the scene, as the editor's panes do, and never over the character:
the canvas is the part of the window the panes leave free.

The pane is a module of its own (`apps/openagents-web/crew-pane.coax`),
drawn by the browser beside the page's canvas. Two shared pieces were
missing and were added where the next page finds them: a slider in the kit
(`k_slider`, `components/kit/dock.coax`, in the mono skin) and a page's own
pane in the host (`coax/web/host.js`: a page names its pane's module, tells
it what it holds, and takes its actions as messages).

### The character is a document

A character is one small, versioned document, `crew.character.v1`: a name,
the body record, the face and hair choices, and one garment per slot, as
five lines of text ([its page](../protocol/crew-character-v1.md); the one
codec is `metaverse-engine/kit/crew-doc.coax`). USE THIS CHARACTER keeps it
in the browser's storage and the page opens with it next time. An agent
writes one through the page's address (`?doc=`) or a message, and a game
loads it with the same codec. A document with a value out of range, an
unknown key, a key twice or a part missing is refused with a reason, never
clamped silently; the page shows the reason and keeps the character it had.

## First deliverable

A standalone scene at `/worlds/crew` (unlisted), built like the review
scenes for Splice's cast of five: Anime Lab's shading, with a studio's plain
grey paper for a sky and a floor that shows only the figures' shadows
(`apps/openagents-web/gpu/crew.coax`).

| Address | What it shows |
| --- | --- |
| `/worlds/crew` | The creation screen: a turntable on your character and the five steps |
| `/worlds/crew?view=row` | Six example characters beside a height scale and a 1.8 m figure, one per body preset, each in a different outfit |
| `/worlds/crew?view=fit&outfit=deckhand` | One outfit on all six bodies, to prove the fitting |
| `/worlds/crew?view=outfits&body=fighter` | The six outfits on one body, as the outfit sheet lays them out |

`&pane=0` shows the scene alone, `&turn=DEGREES` holds the turntable,
`&close=1` frames the turntable on the head, and on the turntable `&body=`
and `&outfit=` dress the character in a preset.

Pictures of each view, taken headless and compared by eye against the two
sheets:

- [the row](shots/crew-row.png)
- [the deckhand on all six bodies](shots/crew-fit-deckhand.png) and
  [the shroud on all six](shots/crew-fit-shroud.png)
- [the six outfits on the fighter](shots/crew-outfits-fighter.png) and
  [on the elder](shots/crew-outfits-elder.png)
- [the turntable at four angles](shots/crew-turntable.png)
- [four characters close up](shots/crew-close.png)
- [the creation screen, the Body step](shots/crew-create-body.png) and
  [the Outfit step](shots/crew-create-outfit.png)

The modules live in the Metaverse Engine (`metaverse-engine/`), not in
Splice, so any realm can use them. The page is its own realm page with its
own modules (`crew`, budget 188,000 bytes; `crew-pane`, 132,000); it adds
nothing to Splice's module.

How close the pictures are to the sheets, weakest first:

- **Hair** is tapered locks: thinner and tidier than the face sheet's
  messy bun and ink strands, and it shades grey rather than ink black.
- **Faces** have the sheet's shape, eyes, brows, nose and lips. The eyes
  are drawn on the surface, not modelled lids; the nose shows mostly
  through its lines; the skin is lighter than the sheet's tan once lit.
- **The runner's jacket** reads as a cropped jacket with full sleeves and a
  ribbed hem, but boxy, and its hood lies out of sight on the back.
- **The clear helmet** is hoops, not glass.
- **The deckhand's top** has mesh cap sleeves and long cuffs; the sheet's
  mesh yoke over the chest is left out.
- **Cloth doesn't fold.** Baggy trousers balloon and gather at the boot,
  but nothing creases at a knee or an elbow.
- **Flight suit, vac suit, shroud, fitter, the boots and the gloves** read
  at a glance: collar and open throat, chest and cargo pockets, belts and
  buckles, piping, knee pads, the long open coat, the bib and its straps,
  laces, treads and stitch lines.
- **The six bodies** are clearly six builds at the sheet's heights. They
  all stand in one pose; the sheet's people each have their own.

## Not in this pass

- Using the created character in Splice's game or in rooms.
- Cloth simulation. Baggy cloth is shaped, not simulated.
- More than one wardrobe look, or colour choices for clothes.
- Quaternius or Blender assets. The cel kit's shapes are the source, so
  bodies stay parametric. If a later pass wants sculpted heads or hands, it
  imports them through the pack importer.
- Poses and animation. The joints are placed and named; nothing moves them
  yet.

## Open questions

- Whether clothes get a colour choice later, or the black wardrobe is the
  game's uniform.
- How much face detail the cel style carries at play distance.
