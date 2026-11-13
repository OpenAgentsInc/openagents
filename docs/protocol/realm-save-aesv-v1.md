# Realm save (AESV) v1

How a realm keeps a player's progress. Profile:
[`realm-save-aesv-v1.json`](../../proto/verse/realm-save-aesv-v1.json).
Codec: `metaverse-engine/state/save.coax`, which every realm page loads. The
OpenAgents Metaverse Engine book's [chapter 81](../../metaverse-engine/book/ch081-save-and-come-back.md)
explains the design.

## The container

| Bytes | Field |
| --- | --- |
| 4 | `AESV` |
| 1 | Container version: 2 is written; 1 and 2 are read |
| 1 | Layer: 2, player progress |
| 2 | Reserved, zero |
| 4 | Schema, unsigned, little-endian: the payload's own version |
| varint, then that many | The world's name |
| varint, then that many | The payload |
| 4 | CRC-32 (IEEE 802.3, as zlib) of every byte before it, little-endian |

The smallest container is 18 bytes. The payload's bytes belong to the realm;
this profile doesn't describe them.

Two readers check a container:

- The page's `save_open` checks everything: the magic, the version, the
  layer, the CRC-32, the world's name, a schema no newer than the build
  knows, and that both lengths are exact.
- The API's `container_ok` (`coax/web/api/player.c`) checks a posted save's
  size (at least 18 bytes), magic, version, layer and CRC-32 before it keeps
  it in `verse.progress`. It doesn't read the world, the schema or the
  lengths. A container it keeps that `save_open` refuses is one the page
  never loads: the page falls back to the previous save.

## The browser's record

The page keeps two saves in IndexedDB as five lines of text:

```
verse-save 1
<the current save's time, ms>
<its container, base64>
<the previous save's time>
<its container, base64>
```

The last two lines are empty when there is no previous save. A new save
makes the old current one the previous one only when it still opens;
otherwise the old previous one stays. A record from before containers is
the save as bare lower-case hex, and reads as schema 0.

## The post

`POST /api/playtest/progress/WORLD/SLOT` takes
`{"bytes":"<container, base64>","at":<ms>,"rev":<revision>}`. A page that
knows no revision leaves `rev` out.

## Check

```sh
coax/web/tool.sh apps/openagents-web/save-profile.coax proto/verse/realm-save-aesv-v1.json
```

The golden containers were made with zlib's CRC-32, not with the codec. The
check packs, opens, reads and picks each through `save.coax`.
`coax/web/api/test_player.py` posts every container in the profile to the
API and requires what the profile's `server` field says: `refuses` is a
400, `keeps` is a 200 with the same bytes back.

A change to the container's layout is a new container version, read beside
versions 1 and 2, with new goldens here.
