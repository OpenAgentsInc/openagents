# 53. Bring in assets

> **Status: partly implemented.** A realm's asset pack (`VPK1`) is read
> whole or refused by Aethyrforge's reader in Coax
> ([aethyr/forge/pack.coax](../forge/pack.coax), `#authority`). The reader
> checks the pack's budget, its layout and a licence on every asset, and
> admits the pack into a realm's assets by content
> ([chapter 16](ch016-budgets-that-refuse.md)). The pack is written again byte
> for byte as `verse/tools/pack_realm.py` writes it. Both committed packs
> (Everglade's and the fixture's) read, write again to the same bytes and are
> admitted. The trace below is worked by hand and run in
> [aethyr/examples/bring-in-assets.coax](../examples/bring-in-assets.coax).
> Not built yet: the import from sources (scene meshes, images, sounds) that
> would retire `pack_realm.py`, unit checks with a recorded rescale, uploads
> as typed states, and the cast and drop packers on the same path. See
> [Still to do](#still-to-do).

An asset arrives from outside the engine: a CC0 kit's mesh, a character with
its clips, a photo of someone's dog, a model a project owner dropped. Before a
realm may use it, the engine must know what it is (a content hash), what it
may be used for (a licence), what it measures (units and bounds) and what it
costs (bytes against a budget). Chapter 16 gave identities and budgets; this
chapter is the door they are checked at. Physics imports (URDF, MJCF,
tetrahedral meshes) are [chapter 35](ch035-bring-in-robots-and-meshes.md)'s.

## What this chapter teaches

- Packing many assets into one streamed file with a table, a catalog and a
  budget.
- A licence on every asset, from its source, refused when it is missing or
  doesn't match its catalog line.
- Reading untrusted bytes whole: every field checked before anything is
  kept, and a refusal that names what it refused.
- A pack's entries admitted into a realm's assets by their content.

## The format

A pack is a header, a table of entries (kind, licence, offset, bytes, name)
and their data on 4-byte boundaries. There is one catalog, one vertex buffer
and one index buffer, then the texture layers, then the sounds. The catalog
is text: it states the pack's budget and size again, declares each licensed
source (`L K NAME|LICENCE|URL`), names every mesh's index range, source and
bounds in metres (`M`), and says which source each layer (`T`) and sound
(`S`) comes from. A realm draws a mesh as `[first, count, instance, 1]`
without parsing geometry.

<!-- source: aethyr/forge/pack.coax format -->
```jai
PACK_MAGIC :: "VPK1";
PACK_HEADER :: 32;
PACK_ENTRY :: 48;
PACK_MAX :: 48;        // entries a pack has at most (verse/tools/pack-files.coax's limit too)
PACK_NAME :: 32;       // a name's field; a name is 1 to 31 bytes
PACK_SOURCES :: 32;    // licensed sources (L lines) a catalog declares at most
PACK_CATALOG :: 1;
PACK_VERTICES :: 2;
PACK_INDICES :: 3;
PACK_LAYER :: 4;
PACK_SOUND :: 5;
PACK_MANY :: 255;      // the licence of the catalog, vertices and indices: every source's

// One entry as the table has it: its bytes are p[at..<at + n] of the pack read.
PackEntry :: struct { kind: int; licence: int; at: int; n: int; name: string; }
PackRead :: struct {
  ok: bool;
  why: string; entry: int = -1;      // the refusal, and the entry it names (-1: the pack as a whole)
  count: int; bytes: int; budget: int;
  entries: [PACK_MAX] PackEntry;
  sources: int;                      // licensed sources the catalog declares
}
// An entry to write: kind, licence, name and bytes.
PackSource :: struct { kind: int; licence: int; name: string; data: string; }
```

The catalog, vertices and indices hold every source's assets, so their
licence is `PACK_MANY` (255). A layer or a sound has exactly one source.
`pack_realm.py` gives a layer that meshes of two sources share the licence
255 and refuses it, so a pack never holds such a layer.

## Reading a pack whole

The reader takes the pack's bytes as untrusted. It checks the header, then
every entry in table order: its kind in the format's order, its reserved
fields zero, its name plain, its bytes starting exactly where the last
entry's padding ended, and the padding zero. Only then does it read the
catalog. Nothing is kept until all of it holds:

<!-- source: aethyr/forge/pack.coax read -->
```jai
// Read a pack whole: its header, its table in the format's order and layout, and its catalog's
// account of every licence. False, with why and the entry, at the first thing that doesn't hold.
pack_read :: (p: string, r: [] PackRead) -> bool {
  if p.count < PACK_HEADER || p[0..<4] != PACK_MAGIC { return pack_why(r, "not a VPK1 pack", -1); }
  pk_count := pack_u32(p, 4);
  if pack_u32(p, 8) != p.count { return pack_why(r, "its stated size isn't its size", -1); }
  if pk_count < 3 || pk_count > PACK_MAX || pack_u32(p, 16) != PACK_HEADER || pack_u32(p, 20) != PACK_HEADER + PACK_ENTRY * pk_count
      || pack_u32(p, 24) != 0 || pack_u32(p, 28) != 0 || p.count < PACK_HEADER + PACK_ENTRY * pk_count {
    return pack_why(r, "a malformed header", -1);
  }
  if p.count > pack_u32(p, 12) { return pack_why(r, "over its budget", -1); }
  got: [1] PackRead;
  got[0].count = pk_count; got[0].bytes = p.count; got[0].budget = pack_u32(p, 12);
  pk_at := PACK_HEADER + PACK_ENTRY * pk_count;   // where the next entry's bytes must start
  for pk_k: 0..<pk_count {
    pk_e := PACK_HEADER + PACK_ENTRY * pk_k;
    pk_kind := cast(int) p[pk_e];
    pk_lic := cast(int) p[pk_e + 1];
    pk_off := pack_u32(p, pk_e + 4);
    pk_n := pack_u32(p, pk_e + 8);
    pk_name := pack_name(p[pk_e + 16..<pk_e + PACK_ENTRY]);
    pk_first := ifx pk_k < 3 then pk_kind == pk_k + 1 else pk_kind == PACK_LAYER || pk_kind == PACK_SOUND;
    if !pk_first || (pk_kind == PACK_LAYER && got[0].entries[pk_k - 1].kind == PACK_SOUND) { return pack_why(r, "an entry out of order", pk_k); }
    if p[pk_e + 2] != 0 || p[pk_e + 3] != 0 || pack_u32(p, pk_e + 12) != 0 || pk_name.count == 0 { return pack_why(r, "a malformed entry", pk_k); }
    if pk_off != pk_at || pk_off + pack_pad(pk_n) > p.count { return pack_why(r, "an entry out of place", pk_k); }
    for pk_z: pk_off + pk_n..<pk_off + pack_pad(pk_n) { if p[pk_z] != 0 { return pack_why(r, "an entry out of place", pk_k); } }
    if (pk_k < 3) != (pk_lic == PACK_MANY) { return pack_why(r, "an asset without one licence", pk_k); }
    got[0].entries[pk_k] = PackEntry.{kind = pk_kind, licence = pk_lic, at = pk_off, n = pk_n, name = pk_name};
    pk_at = pk_off + pack_pad(pk_n);
  }
  if pk_at != p.count { return pack_why(r, "bytes after the last entry", -1); }
  pk_cat := got[0].entries[0];
  if !pack_catalog(p[pk_cat.at..<pk_cat.at + pk_cat.n], got[..]) { r[0] = got[0]; return false; }
  got[0].ok = true;
  r[0] = got[0];
  return true;
}
```

The catalog must account for the pack. Its stated budget and size are the
header's, its sources are numbered 0, 1, ... in order, and every mesh names a
declared source. Each layer and sound has exactly one line, naming the
licence its own table entry carries:

<!-- source: aethyr/forge/pack.coax catalog -->
```jai
// The catalog's account: the pack's budget and size again, its sources in order, every mesh's
// source declared, and each layer and sound the source its own entry says.
pack_catalog :: (t: string, r: [] PackRead) -> bool {
  if t.count == 0 || t[t.count - 1] != '\n' { return pack_why(r, "a catalog that doesn't end its last line", 0); }
  pk_stated := 0; pk_layer := 0; pk_sound := 0; pk_at := 0;
  pk_layers := 0; pk_sounds := 0;
  for pk_k: 3..<r[0].count { if r[0].entries[pk_k].kind == PACK_LAYER { pk_layers += 1; } else { pk_sounds += 1; } }
  for pk_i: 0..<t.count {
    if t[pk_i] != '\n' { continue; }
    pk_line := t[pk_at..<pk_i]; pk_at = pk_i + 1;
    pk_what := pack_field(pk_line, 0);
    if pk_what == "budget" || pk_what == "bytes" {
      pk_v := pack_number(pack_field(pk_line, 1));
      if pk_v != (ifx pk_what == "budget" then r[0].budget else r[0].bytes) { return pack_why(r, "a catalog that misstates the pack", 0); }
      pk_stated += 1;
    }
    if pk_what == "L" {
      if pack_number(pack_field(pk_line, 1)) != r[0].sources || r[0].sources == PACK_SOURCES { return pack_why(r, "a source out of order", 0); }
      r[0].sources += 1;
    }
    if pk_what == "M" || pk_what == "C" {
      pk_m := pack_number(pack_field(pk_line, 4));
      if pk_m < 0 || pk_m >= r[0].sources { return pack_why(r, "a mesh without a declared licence", 0); }
    }
    if pk_what == "T" || pk_what == "S" {
      pk_kind := ifx pk_what == "T" then PACK_LAYER else PACK_SOUND;
      pk_nth := ifx pk_what == "T" then pk_layer else pk_sound;
      pk_e := pack_nth(r, pk_kind, pk_nth);
      if pk_e == -1 { return pack_why(r, "a catalog line for an entry the pack doesn't have", 0); }
      pk_lic := pack_number(pack_field(pk_line, ifx pk_what == "T" then 3 else 2));
      pk_ok := ifx pk_what == "T" then pack_number(pack_field(pk_line, 1)) == pk_layer else pack_field(pk_line, 1) == r[0].entries[pk_e].name;
      if !pk_ok || pk_lic < 0 || pk_lic >= r[0].sources || pk_lic != r[0].entries[pk_e].licence { return pack_why(r, "an asset without a declared licence", pk_e); }
      if pk_what == "T" { pk_layer += 1; } else { pk_sound += 1; }
    }
  }
  if pk_stated != 2 { return pack_why(r, "a catalog that misstates the pack", 0); }
  if pk_layer != pk_layers { return pack_why(r, "an asset without a declared licence", pack_nth(r, PACK_LAYER, pk_layer)); }
  if pk_sound != pk_sounds { return pack_why(r, "an asset without a declared licence", pack_nth(r, PACK_SOUND, pk_sound)); }
  return true;
}
```

A refusal says why and which entry: −1 for the pack as a whole, 0 for the
catalog, or the table index of the layer or sound without a declared
licence. A refused read leaves an empty `PackRead`, so no partly read pack
can be admitted.

## Writing a pack

The writer lays entries out exactly as `pack_realm.py` does. It writes every
byte once, so a buffer's old contents never show through. It doesn't check
licences or the catalog; `pack_read` does, so what it writes is read back
before it is kept:

<!-- source: aethyr/forge/pack.coax write -->
```jai
// The bytes a pack of these entries takes: header, table, and each entry padded to 4 bytes.
pack_size :: (s: [] PackSource) -> int {
  pk_n := PACK_HEADER + PACK_ENTRY * s.count;
  for pk_e: s { pk_n += pack_pad(pk_e.data.count); }
  return pk_n;
}
pack_put32 :: (out: [] u8, at: int, v: int) {
  out[at] = cast,trunc(u8) v; out[at + 1] = cast,trunc(u8) (v >> 8);
  out[at + 2] = cast,trunc(u8) (v >> 16); out[at + 3] = cast,trunc(u8) (v >> 24);
}
// Write a pack as pack_realm.py does: the bytes written, or -1 when out can't hold them, there are
// more than PACK_MAX entries, or a name isn't 1 to 31 bytes. Nothing is checked beyond the layout:
// pack_read is the check, so what this writes is read back before it is kept.
pack_write :: (s: [] PackSource, budget: int, out: [] u8) -> int {
  pk_total := pack_size(s);
  if s.count > PACK_MAX || pk_total > out.count { return -1; }
  for pk_e: s { if pk_e.name.count == 0 || pk_e.name.count >= PACK_NAME { return -1; } }
  // Every byte is written once: the header, each entry of the table, each entry's bytes and padding.
  for pk_c, pk_k: PACK_MAGIC { out[pk_k] = pk_c; }
  pack_put32(out, 4, s.count); pack_put32(out, 8, pk_total); pack_put32(out, 12, budget);
  pack_put32(out, 16, PACK_HEADER); pack_put32(out, 20, PACK_HEADER + PACK_ENTRY * s.count);
  pack_put32(out, 24, 0); pack_put32(out, 28, 0);
  pk_at := PACK_HEADER + PACK_ENTRY * s.count;
  for pk_e, pk_k: s {
    pk_t := PACK_HEADER + PACK_ENTRY * pk_k;
    out[pk_t] = cast,trunc(u8) pk_e.kind; out[pk_t + 1] = cast,trunc(u8) pk_e.licence; out[pk_t + 2] = 0; out[pk_t + 3] = 0;
    pack_put32(out, pk_t + 4, pk_at); pack_put32(out, pk_t + 8, pk_e.data.count); pack_put32(out, pk_t + 12, 0);
    for pk_j: 0..<PACK_NAME { out[pk_t + 16 + pk_j] = ifx pk_j < pk_e.name.count then pk_e.name[pk_j] else 0; }
    for pk_j: 0..<pack_pad(pk_e.data.count) { out[pk_at + pk_j] = ifx pk_j < pk_e.data.count then pk_e.data[pk_j] else 0; }
    pk_at += pack_pad(pk_e.data.count);
  }
  return pk_total;
}
```

## Admitted by content

A pack read whole becomes a realm's assets entry by entry, each known by the
SHA-256 of its bytes, through chapter 16's registry. The registry takes the
whole pack or none of it. Bytes it already holds are shared, not counted
twice:

<!-- source: aethyr/forge/pack.coax admit -->
```jai
// A pack read takes its place in a realm's assets as its entries, each known by its content
// (chapter 16): taken whole, or refused whole and nothing held changes.
pack_admit :: (reg: [] AssetRegistry, p: string, r: [] PackRead) -> int {
  if !r[0].ok { return asset_refuse(reg, ASSET_MALFORMED, ASSET_NONE, 0, reg[0].held); }
  pk_content: [PACK_MAX] string; pk_table: [PACK_MAX] AssetId;
  for pk_k: 0..<r[0].count { pk_content[pk_k] = pack_bytes(p, r, pk_k); pk_table[pk_k] = asset_id(pk_content[pk_k]); }
  return asset_admit(reg, pk_content[0..<r[0].count], pk_table[0..<r[0].count]);
}
```

## Worked by hand

A tiny pack has five entries: a crate's catalog (206 bytes), 6 bytes of
vertices, 12 of indices, one texture layer of 5 bytes from the kit (source 0)
and a door's sound of 3 bytes (source 1). The header and table take
32 + 5 × 48 = 272 bytes, so the catalog starts at 272. Its 206 bytes pad to
208, so the vertices start at 480. The vertices pad from 6 to 8 (indices at
488), the indices take 12 (layer at 500), and the layer pads from 5 to 8
(sound at 508). The sound pads from 3 to 4, so the pack ends at 512. That is
the `bytes 512` its catalog states, under a budget of 600.

The entries' bytes add up to 206 + 6 + 12 + 5 + 3 = 232. A realm whose
assets may hold 232 bytes admits the pack: five assets, 232 bytes. The same
pack again adds a use to each entry and no bytes. A realm with 231 bytes
refuses it and holds nothing.

Three broken packs:

- The same entries with the layer under source 1, while its catalog line
  `T 0 layer3.jpg 0` says source 0. The catalog's account fails at the
  layer, entry 3.
- The same bytes under a budget of 511. The header says the pack is over
  its budget.
- The pack one byte short. Its header's size, 512, isn't its size, 511.

## Bring in a crate

The program is
[aethyr/examples/bring-in-assets.coax](../examples/bring-in-assets.coax):

<!-- source: aethyr/examples/bring-in-assets.coax example -->
```jai
#load "../forge/pack.coax";
#import "Basic";

CATALOG :: "pack tiny\nbudget 600\nbytes 512\nL 0 Kit|CC0 1.0 Universal|https://example.org/\nL 1 Sounds|CC0 1.0 Universal|https://example.org/\nM Crate 0 3 0 -0.500 0.000 -0.500 0.500 1.000 0.500\nT 0 layer3.jpg 0\nS door 1\n";

// The tiny pack's five entries: a crate's catalog, vertices and indices, one texture layer from
// the kit (source 0) and a door's sound (source 1).
sources :: (out: [] PackSource, layer_licence: int) {
  out[0] = PackSource.{kind = PACK_CATALOG, licence = PACK_MANY, name = "catalog", data = CATALOG};
  out[1] = PackSource.{kind = PACK_VERTICES, licence = PACK_MANY, name = "vertices", data = "vvvvvv"};
  out[2] = PackSource.{kind = PACK_INDICES, licence = PACK_MANY, name = "indices", data = "iiiiiiiiiiii"};
  out[3] = PackSource.{kind = PACK_LAYER, licence = layer_licence, name = "layer0", data = "JPEG!"};
  out[4] = PackSource.{kind = PACK_SOUND, licence = 1, name = "door", data = "mp3"};
}
// Why a pack is refused, or "read whole".
verdict :: (p: string) -> string {
  r: [1] PackRead;
  if pack_read(p, r[..]) { return "read whole"; }
  return r[0].why;
}

main :: () {
  s: [5] PackSource; sources(s[..], 0);
  print("the pack takes % bytes\n", pack_size(s[..]));
  b: [600] u8; n := pack_write(s[..], 600, b[..]);
  p := cast(string) b[0..<n];
  r: [1] PackRead;
  print("read: %, % entries, % sources\n", pack_read(p, r[..]), r[0].count, r[0].sources);
  for k: 0..<r[0].count {
    e := r[0].entries[k];
    print("  % kind % licence %: % bytes at %\n", e.name, e.kind, e.licence, e.n, e.at);
  }
  hex: [64] u8; ok := asset_hex(asset_id(pack_bytes(p, r[..], 3)), hex[..]);
  print("layer0 is asset %\n", cast(string) hex[..]);
  // Admitted into a realm's assets: 206 + 6 + 12 + 5 + 3 = 232 bytes.
  reg: [1] AssetRegistry; reg[0] = asset_registry(232);
  print("admitted under 232 bytes: %, % assets, % bytes\n", pack_admit(reg[..], p, r[..]) == ASSET_OK, reg[0].held, reg[0].used);
  print("the same pack again: %, % bytes, layer0 used % times\n", pack_admit(reg[..], p, r[..]) == ASSET_OK, reg[0].used, asset_uses(reg[0], asset_id("JPEG!")));
  small: [1] AssetRegistry; small[0] = asset_registry(231);
  print("under 231 bytes: refused % (asked %), % held\n", pack_admit(small[..], p, r[..]) == ASSET_OVER_BYTES, small[0].first.asked, small[0].held);
  // Broken packs. The layer given the sounds' licence, while its catalog line says the kit's:
  t: [5] PackSource; sources(t[..], 1);
  c: [600] u8; m := pack_write(t[..], 600, c[..]);
  q := cast(string) c[0..<m];
  bad: [1] PackRead; refused := !pack_read(q, bad[..]);
  print("layer0 under source 1: refused %: % (entry %); the read keeps % entries\n", refused, bad[0].why, bad[0].entry, bad[0].count);
  // The same bytes under a 511-byte budget, and one byte cut off the end.
  d: [600] u8; m = pack_write(s[..], 511, d[..]);
  print("a 511-byte budget: %\n", verdict(cast(string) d[0..<m]));
  print("one byte short: %\n", verdict(p[0..<n - 1]));
}
```

```shell
coax/bin/coax run aethyr/examples/bring-in-assets.coax
```

```text
the pack takes 512 bytes
read: true, 5 entries, 2 sources
  catalog kind 1 licence 255: 206 bytes at 272
  vertices kind 2 licence 255: 6 bytes at 480
  indices kind 3 licence 255: 12 bytes at 488
  layer0 kind 4 licence 0: 5 bytes at 500
  door kind 5 licence 1: 3 bytes at 508
layer0 is asset c579f80088119b4b77f6a8b32c542ce2ab6033574361a03c08ab6c32fb5b9487
admitted under 232 bytes: true, 5 assets, 232 bytes
the same pack again: true, 232 bytes, layer0 used 2 times
under 231 bytes: refused true (asked 232), 0 held
layer0 under source 1: refused true: an asset without a declared licence (entry 3); the read keeps 0 entries
a 511-byte budget: over its budget
one byte short: its stated size isn't its size
```

Every number is the hand-worked one. The layer's id is its SHA-256, which
`printf 'JPEG!' | sha256sum` agrees with:

```text
c579f80088119b4b77f6a8b32c542ce2ab6033574361a03c08ab6c32fb5b9487  -
```

## The committed packs

The independent check is `pack_realm.py` itself. It wrote both committed
packs, and the Coax writer, fed what the Coax reader read, must give back
the same bytes. `aethyr/tools/pack-check.coax` reads each pack, writes it
again, compares the bytes and admits the pack under its own budget:

```shell
coax/web/tool.sh aethyr/tools/pack-check.coax apps/openagents-web/realms/fixture/assets/fixture.pack apps/openagents-web/realms/everglade/assets/everglade.pack
```

```text
apps/openagents-web/realms/fixture/assets/fixture.pack: 1267160 bytes of a 1500000-byte budget, 11 entries, 4 sources
  ...
  10 sound chime licence 3, 10100 bytes at 1257060
  sha256 df9c027378c87383f5be40d61c764b9fb615148524388d4b310229e9b0e39ad9
  written again from what was read: the same bytes
  admitted under its budget: true, 11 assets, 1266591 bytes
apps/openagents-web/realms/everglade/assets/everglade.pack: 9039012 bytes of a 9050000-byte budget, 39 entries, 6 sources
  ...
  38 layer layer35 licence 4, 6157 bytes at 9032852
  sha256 5595e2a3839bb38bd2cd839c688bb9a13a64ed23958fda84907e03c0dc90f8d8
  written again from what was read: the same bytes
  admitted under its budget: true, 38 assets, 9035236 bytes
```

Both SHA-256s are the ones `sha256sum` prints for the files. The fixture's
`df9c027378c8` is also the content name its engine manifest uses
([chapter 54](ch054-publish-a-realm.md#worked-by-hand)). The fixture admits
11 assets but holds 1,266,591 bytes: its file's 1,267,160 bytes less the
header, the table and the padding. Everglade's 39 entries are 38 assets,
because `layer18` and `layer19` are the same 1,820 bytes, held once. (Only
each pack's last lines are shown; the tool prints every entry.)

## Checks

- `coax/bin/coax test aethyr/forge/pack.coax --library` (34 checks with
  chapter 16's and SHA-256's): the layout written and read back; the
  writer's refusals and every byte written once; each header field refused
  whole; entries out of order, malformed or out of place, each named; the
  catalog's account of the budget, size, sources, meshes, layers and sounds;
  three and 48 entries, and a table that ends the pack; admission by
  content, whole or not at all.
- `coax/bin/coax mutate aethyr/forge/pack.coax --root-only`: 564 mutants,
  564 killed.
- `coax/bin/coax run aethyr/examples/bring-in-assets.coax` (in the book's
  `--coax` runs, debug and release): the trace above.
- `coax/web/tool.sh aethyr/tools/pack-check.coax PACK...`: the committed
  packs, written again to the same bytes.

## What exists beside it

- [docs/verse/realm-host.md](../../docs/verse/realm-host.md), "Asset packs":
  manifests in [verse/packs/](../../verse/packs/everglade.json), the
  compiler [verse/tools/pack_realm.py](../../verse/tools/pack_realm.py)
  (format in its header), `realms/<slug>/licences.md`, message 605 progress
  while a pack streams and `coaxial_asset_ready` per asset.
- [verse/tools/pack_cast.py](../../verse/tools/pack_cast.py): the Grid's cast
  (`Cast.bin` and texture layers) from aethyr-cpp's assets;
  [pack_grid.py](../../verse/tools/pack_grid.py),
  [pack_haunt.py](../../verse/tools/pack_haunt.py) and
  [pack_sandbox.py](../../verse/tools/pack_sandbox.py) for their realms.
- [coax/web/uploads.js](../../coax/web/uploads.js): action 30 and messages
  52 and 53, used for Hecate's Cemetery's pets (`coax/web/api/villa.c`).
- [apps/openagents-web/droprealm.coax](../../apps/openagents-web/droprealm.coax):
  a project's dropped models packed by `coax/web/api/drops.c` into a `VPK1`
  pack, stood in a ring, scaled into `MIN_SIZE`–`MAX_SIZE` metres.

## Still to do

- **Import from sources.** `import_pack` from a manifest
  (`verse/packs/*.json`): the scene's meshes, the images (with the
  painterly style's texture rules and paint pass), the sounds and the
  characters, giving packs byte-identical to `pack_realm.py`'s for
  `everglade.json` and `fixture.json`, and then the Python retired. Today
  only the last step, laying out entries already made, is Coax.
- **Units.** `aethyr/forge/units.coax`: per-kind size ranges; a centimetre
  model rescaled into range with its factor in the report, and a kit mesh out
  of range refused. `droprealm.coax` scales dropped models today, without a
  report.
- **Uploads.** `aethyr/forge/upload.coax`: admit, upload and scan as typed
  states (sending, scanning, kept, refused, failed). An upload that fails its
  scan never reaches the realm's state.
- **One path for every pack.** The Grid's cast, the drops packer
  (`drops.c`) and `pack-files.coax` reading and writing through
  `pack.coax`.
- **The licence report.** `licences.md` written from the catalog the reader
  checked, not beside it by the packer.

## Dependencies

[Chapter 2](ch002-spaces-and-units.md) (units),
[chapter 14](ch014-figures-and-animation.md) (rigs and clips),
[chapter 16](ch016-budgets-that-refuse.md) (identities and budgets) and
[chapter 51](ch051-a-realm-is-a-document.md) (what a document may name).
