# crew.character.v1

A crew member as one small text document: a name, a body, a face and hair,
and an outfit. The creation page at `/worlds/crew` writes it and keeps it,
an agent writes one through the page's address, and a game reads it.

- Codec: `metaverse-engine/kit/crew-doc.coax` (`cd_write`, `cd_read`). Every
  producer and consumer loads it.
- Profile and pinned bytes: `proto/aethyr/crew-character-v1.json`.
- Check: `coax/web/tool.sh metaverse-engine/tools/crew-character-profile.coax proto/aethyr/crew-character-v1.json`.
- Spec: [character creation](../metaverse/2026-11-13-character-creation.md).

## The document

Five lines of text, each ended by a line feed, at most 640 bytes:

```
crew.character.v1
name Ash
body height=175 shoulders=98 chest=96 waist=82 belly=0 hips=104 arms=96 legs=106 arm_length=100 leg_length=102 neck_length=100 neck=96 head=100 hands=100 feet=100 stoop=0
look skin=2 hair=bun hair_colour=0 jaw=0 eyes=0 brows=0 nose=0 mark=none
wear head=none face=none top=flight outer=none hands=gloves belt=wide legs=flight feet=strap-boots pack=none satchel=none tool=none
```

The first line is the version. The other four parts may come in any order,
and so may the words of each. Every key must be there exactly once.

### name

1 to 24 printable ASCII characters, with no space at either end.

### body

Sixteen whole numbers. `height` is centimetres. `belly` and `stoop` run from
0 to 100. Every other number is a percentage of the usual build at that
height.

| Key | Range | Key | Range |
| --- | --- | --- | --- |
| `height` | 145 to 225 | `arm_length` | 88 to 112 |
| `shoulders` | 80 to 150 | `leg_length` | 85 to 112 |
| `chest` | 80 to 150 | `neck_length` | 50 to 130 |
| `waist` | 75 to 160 | `neck` | 80 to 170 |
| `belly` | 0 to 100 | `head` | 88 to 125 |
| `hips` | 80 to 150 | `hands` | 85 to 140 |
| `arms` | 70 to 170 | `feet` | 85 to 135 |
| `legs` | 70 to 160 | `stoop` | 0 to 100 |

### look

| Key | Values |
| --- | --- |
| `skin` | 0 to 7, light to dark |
| `hair` | `bun`, `crop`, `mohawk`, `tied`, `curls`, `shaved`, `topknot`, `long` |
| `hair_colour` | 0 to 7: black, dark brown, brown, auburn, blond, grey, white, blue |
| `jaw` | 0 soft, 1 square, 2 narrow |
| `eyes` | 0 wide, 1 narrow, 2 round |
| `brows` | 0 straight, 1 arched, 2 heavy |
| `nose` | 0 small, 1 long, 2 broad |
| `mark` | `none`, `scar`, `moustache`, `beard`, `full` |

### wear

One piece a slot.

| Slot | Pieces |
| --- | --- |
| `head` | `none`, `goggles`, `helmet`, `hood` |
| `face` | `none`, `mask` |
| `top` | `flight`, `crop`, `vac`, `fitted`, `tunic`, `shirt` |
| `outer` | `none`, `jacket`, `coat` |
| `hands` | `none`, `gloves`, `long-gloves` |
| `belt` | `none`, `wide`, `utility`, `slung`, `double`, `tied-jacket` |
| `legs` | `flight`, `cargo`, `vac`, `leggings`, `wraps`, `overalls` |
| `feet` | `lace-boots`, `strap-boots` |
| `pack` | `none`, `hip-pack` |
| `satchel` | `none`, `satchel` |
| `tool` | `none`, `wrench` |

A document may name a piece that another piece hides (a belt under bib
overalls). The piece stays in the document and is not drawn; the rules are
`CW_RULES` in `metaverse-engine/kit/crew-wardrobe.coax`.

## Refusals

A reader never clamps or repairs. `cd_read` answers `ok`, or a reason in
words, for example:

- `the body's height is 300, outside 145 to 225`
- `the body's stoop is there twice`
- `the body's stoop is missing`
- `the look's hair is not one of its choices`
- `the outfit's top has no such piece`
- `the first line is not crew.character.v1`

The creation page shows the reason on the scene and keeps the character it
had.

## How the page takes one

- **Its address.** `/worlds/crew?doc=BASE64URL`, the document's bytes in
  base64url with no padding.
- **A message.** `coaxial.send(701, TEXT)` on the page, the document as text.
- **Its pane.** The creation steps beside the scene send the document with
  the same message each time the player changes something.

`coaxial.send(702, "view one")` picks what the scene shows: `view row`,
`view fit`, `view outfits` or `view one`, with `outfit KEY`, `body KEY`,
`turn DEGREES` and `spin on` or `spin off` in the same message.
`coaxial.send(703, "keep")` keeps the turntable's character in the
browser's storage (namespace `crew`, version 1, key `character`). The page
loads it the next time it opens, unless the address names a character.
