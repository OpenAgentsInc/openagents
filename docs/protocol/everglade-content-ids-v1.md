# Everglade content numbers v1

GitHub #13286. Profile: `proto/verse/everglade-content-ids-v1.json`. Codec:
`everglade/model/content-ids.coax`. Check:

```sh
coax/web/tool.sh coax/web/room/content-profile.coax proto/verse/everglade-content-ids-v1.json
coax/web/tool.sh wire-check    # rule 7: no number is gone or changed since origin/main
```

## What it is

Every piece of content that a wire frame or a save names has one row: a kind,
a number and a key. The wire and the saves carry the number. The key says what
the number means and is how code finds it.

| Kind | Key | Where the number goes |
| --- | --- | --- |
| `ability` | `ability/CLASS/SLUG`, or `ability/CLASS/spell/SPELL` | `SLOT` in the `C` and `e cast` frames (`everglade/shared/class-kit.coax`, `xk_wire`) |
| `spell` | `spell/ID`, the SRD source's ID | `SPELL` in a rules event, and in the cast frames' catalog |
| `creature` | `monster/ID`, the stat block's ID | A stat block: a spawn's template, a Wild Shape or Shapechange form |
| `item` | `item/ID` | The item ledger's `kind`. No rows yet: #13291 adds the first |
| `spawn` | `everglade2/GROUP/NAME` | `M` in the `h2` and `h3` creature frames |

Typed numbers are in `aethyr/core/ids.coax`: `AbilityId`, `SpellId`, `TemplateId`,
`ItemKindId` and `SpawnId`.

## The rules

1. A number is never changed and never used again. `wire-check` compares the
   profile with `origin/main`'s copy and fails when a row is gone, or has
   another number or key.
2. A row is only ever added. Content that leaves the game keeps its row.
3. Order is presentation. A class's bar is a list of abilities in the order it
   is shown; `xk_wire` looks each one up by key. Reordering a bar, the spell
   table or the spawn list changes no wire byte and no save. The checks
   `class kits: reordering a class's bar changes no wire slot` and `e2
   monsters: every spawn's number, key and stat block are the content
   registry's` hold this.
4. The numbers frozen on 2026-11-12 are the ones the wire already carried: an
   ability's was `128 + class * 24 + place in its catalog`, a spell's its
   place in the rules' table, a spawn's its place in the monster list. No byte
   changed when the registry was added.

## Add content

1. Add the row at the end of its kind in `CONTENT`
   (`everglade/model/content-ids.coax`) with the next unused number, and the
   same row in the profile's `ids`.
2. Run the check. It prints the new hash. Set `CONTENT_HASH`, the profile's
   `hash` and `WIRE_CONTENT_IDS` (`everglade/shared/everglade-wire.coax`) to it.
3. Raise `WIRE_CONTENT` only when an older page and a newer room must not
   meet, for example when a row's meaning changes. A row that is only added
   needs no new pin, because an older build never sends it.

## The content pin

The `P` frame's `content` pin (`WIRE_CONTENT`) is sealed with the registry's
hash, `WIRE_CONTENT_IDS`. A check fails when the hash recorded beside the pin
is not the registry's, so the registry can't change without the pin's owner
seeing it. The hash is two modular sums over every row's `KIND ID KEY` line,
in one number below 2^53.

## What still carries an unregistered number

- `h2`'s `KIND` is Everglade II's monster kind (0 a wolf, 1 a goblin), an enum
  of the combat profile; the spawn's row names the stat block.
- Rules events carry `spell`, `by` and `to` as plain integers (`RsEvent`);
  typing them is #13319's.
