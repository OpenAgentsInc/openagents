# Player appearance v1

A player's chosen look, class and level, as one line of ASCII text. The
schema ID is `aethyr.player.appearance.v1`. Profile:
[`player-appearance-v1.json`](../../proto/aethyr/player-appearance-v1.json).
Codec: `aethyr/character-appearance.coax` (`ca_save`, `ca_restore`), which
Character Create, Everglade's join and the room's hello all call.

The page host keeps the text on the player's device under the schema ID:
`localStorage` on the web (`coax/web/host.js`) and the page host's store on
iOS (`aethyr/host/apple/PageHost.m`). The hosts pass it unread; only the
codec parses it.

| Bytes | Field | Values |
| --- | --- | --- |
| 0 to 2 | version | `CC2`, `CC3` or `CC4` |
| 3 | `female` | 0 or 1 |
| 4 | `outfit` | 0 to 4 |
| 5 | `hood` | 0 or 1; 1 exactly when part 26 or 39 is worn |
| 6 | `hair` | 0 to 3; a man's is 0 to 2 |
| 7 | `beard` | 0 or 1 |
| 8 | `dye` | 0 to 8 |
| 9 | `skin` | 0 to 5 |
| 10 | `hair_color` | 0 to 5 |
| 11 | `variant` | 0 to 2 |
| 12 | `pose` | 0 to 3 |
| 13 to 14 | `piece` | 00 to 39 |
| 15 to 54 | `worn` | a 0 or 1 for each part 0 to 39 |
| 55 | `eye_color` | 0 to 5 |
| 56 | `klass` | `CC3` and `CC4`: `a` (Barbarian) to `l` (Wizard) |
| 57 | `level` | `CC4`: 2 to 6 |

Versions:

- `CC2` is 56 bytes: a character with no class.
- `CC3` is 57 bytes: `CC2` and the class. Its level is 1.
- `CC4` is 58 bytes: `CC3` and a level above 1.
- `CC1` is 54 bytes (39 parts, no eye colour). The reader takes it; nothing
  writes it.

The writer uses the smallest version that holds the record, so an older
record's bytes never change. The reader takes a record whole or not at all,
and a refused record leaves the kept character as it was. It corrects two
things: a worn part or piece that can't be chosen is dropped (the piece
becomes 18), and the retired outfit 4 reads as outfit 2.

The profile holds golden records for each version, records the reader
corrects with the bytes they are written back as, and malformed records.
Check it with:

```sh
coax/web/tool.sh aethyr/tools/appearance-profile.coax proto/aethyr/player-appearance-v1.json
```

A change to the layout is a new version (`CC5`) that the reader takes beside
the old ones, with new goldens here.
