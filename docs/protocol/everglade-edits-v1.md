# Everglade city edits v1 (`eg-edit 1`)

What an admin adds to Everglade's designed city: placed things, hidden
buildings and people taken away. The list is text. Profile:
[`everglade-edits-v1.json`](../../proto/verse/everglade-edits-v1.json).
Codec: `verse/everglade/edits.coax`.

The list is the string `doc.edits` of the realm's published world. An admin's
page sends it with `POST /api/playtest/world/everglade` as
`{"doc":{"everglade":1,"edits":"..."},"summary":"..."}`, and every page
reads it from `GET /api/playtest/world/everglade`. The API stores the
document unread.

```
eg-edit 1
t KIND X Z YAW SIZE [GROUP [LIFT TINT]]
b BUILDING
p SPOT
```

| Line | Fields |
| --- | --- |
| `t` | A placed thing. `KIND` is its number in the catalogue (0 to 33). `X` and `Z` are centimetres, `YAW` degrees, `SIZE` percent (25 to 400). `GROUP` is the row it was placed in; `LIFT` is centimetres up on another thing's top (0 to 1000); `TINT` is a colour (0 none, 1 purple to 7 orange) |
| `b` | The plan's building `BUILDING` (0 to 15) is hidden |
| `p` | The person at the plan's spot `SPOT` (0 to 27) is taken away |

All numbers are whole, with an optional minus and at most nine digits. Each
line ends with a line feed. The writer leaves `GROUP` out when the thing
has no row, lift or colour, and leaves `LIFT` and `TINT` out when both are
0. It writes the things first, then the `b` lines, then the `p` lines.

The reader takes a list whole or not at all: a wrong head, an unknown line
or a number that isn't whole refuses the list, and the list held stays as
it was. Within a good list it corrects what it can. It drops a thing of an
unknown kind and a thing that can't stand where it says (in a wall, a
building, the woods, the river or past the edge), clamps the size and the
lift, and ignores a building or a person the plan doesn't have. The profile
holds such lists with the text they are written back as.

The room doesn't parse the list. It only notes that one came
(`coax/web/room/room-world.c` `world_edits`).

Check the profile with:

```sh
coax/web/tool.sh verse/everglade/edits-profile.coax proto/verse/everglade-edits-v1.json
```

A change to a line's fields is `eg-edit 2`, read beside this one.
