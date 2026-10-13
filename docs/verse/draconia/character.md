# Draconia: characters

Every character is built the way Alice is (`docs/verse/female-character.md`):
- a scripted Blender build on the Universal rig (65 joints);
- the face, hair and outfit as separate script steps;
- exported as glTF.

The names here are working names. The published versions (phase D4) use
original surnames and an original school, so nothing below copies canon
names, houses or crests.

## Draconia (the lead)

- **Look:**
  - late teens, slim;
  - very long, straight, pale platinum hair to the waist with a slight wave
    at the ends;
  - green eyes;
  - small gold hoop earrings;
  - a calm, proud face that breaks into a fierce shout.
- **Battle outfit:** a black school coat (long, open), a dark knit sweater,
  a dark skirt, black tights, and black ankle boots with a block heel.
  - The original crest and colours: an invented school; silver and deep
    green are fine, a snake crest is not.
- **The battle version** adds decals: soot on the cheek, a fine cut on the
  cheek, a line of blood at the lip.
- **Wand:** a slim dark wood wand with a pale handle. It's held in the right
  hand, and the tip carries a light socket for spell effects.

### What she needs beyond Alice

| Need | Plan |
| --- | --- |
| Hair that streams in wind (shots 1, 7, 13) | Spring chains on the hair strip joints (about 12 chains of 6 joints), driven by a wind vector with gusts and collision with the head and shoulders. Strand rendering comes later, only if the close-ups need it. |
| An eye that holds an ECU (shot 11) | An eye shader with a layered iris (a green stroma texture, a darker limbal ring), a refractive cornea, a wet highlight, and a fire reflection from the scene. 4K eye textures for this shot only. |
| A shout (shot 10, on a student) and small expressions | Blend shapes: jaw open, mouth wide, brow down, eyes wide, a sob. Driven by keys in the shot file. |
| Skin that holds a close-up | Subsurface scattering on the face, pore normal map, peach fuzz rim light. |
| Coat and skirt motion | Cloth as bone chains (like the hair) for the coat hem and skirt, before any real cloth simulation. |

## Supporting cast (D1–D2)

| Who | Shots | Build |
| --- | --- | --- |
| The crying girl | 2 | An Alice variant: dark brown curly hair, a uniform with a red-and-gold tie, the sob blend shape |
| The boy at the wall | 3, 4 | A new male base on the Universal rig: short dark hair, a sweater vest and tie, the coat |
| The pair running | 8 | Two variants from the bases |
| The red-haired boy | 9 | A male variant, red hair |
| The student shouting | 10 | A face close-up on the male base |
| Duelers | 6, 15 | 6–20 students from the two bases with varied hair, skin and ties. Instanced, with duel and shield loops |
| Hooded figures | 5, 15, later | One tall cloaked figure: a hood over darkness, ragged hem bone chains, no face visible. Later it's the threat in the night sky and the field. |

## Animation set

- **Draconia:** stand in wind, turn the head, a wand raise, a wand cast (a
  short flick, and a long hold for the guardian), walk and run, and a small
  smile.
- **Students:** cower and sob, back-to-wall flinch, duck and run, a run
  pulling someone, a duel cast, a shield hold, and a fall.
- **Source:**
  - The shared Universal animation library where it fits.
  - Anything missing is keyed in Blender by script, the same way Alice's
    custom clips are.
  - Motion capture from video is a later option, not D1.
