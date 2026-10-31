# 53. Bring in assets

> **Status: unimplemented — design draft.** Realms already stream kit art as
> one asset pack per realm (`VPK1`, compiled by `verse/tools/pack_realm.py`),
> take visitors' uploads through an admit, upload and scan protocol, and turn
> a project's dropped models into a first realm. This chapter plans one
> Aethyrforge import path in Coax for all three. Nothing here runs yet.

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
- A licence on every asset, propagated from its source kit, and refused when
  missing or shared ambiguously.
- Unit normalization at import: a model made in centimetres is caught, not
  shown a hundred times too large.
- Untrusted uploads: admitted, stored only where the server opened, scanned
  before any realm reads them.

## The method

A pack is a header, a table of entries (kind, licence, offset, bytes, name)
and their data on 4-byte boundaries: one catalog, one vertex buffer, one
index buffer, the texture layers, the sounds. The catalog lines name every
mesh's index range, licence and bounds in metres, so a realm draws a mesh as
`[first, count, instance, 1]` without parsing geometry.

Import is a function from sources to a pack plus a report:

```jai
import_pack :: (manifest: *PackManifest, out: *Bytes, report: *ImportReport) -> ImportStatus;
// refused: over budget, an asset with no licence, a missing mesh,
// a texture layer used by meshes of two licences
```

Units are checked by bounds: a model whose longest side is outside a declared
range for its kind is either refused (a kit) or rescaled with the factor
recorded (a dropped model). The pack's identity is the SHA-256 of its bytes;
the same manifest and sources give the same bytes.

Uploads are never trusted bytes in memory: the page asks to admit a file
(name, size), puts it to the upload it was given, and asks for a scan; the
realm sees only kept files and their scan results.

## What exists today

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

The packers are Python, each with its own format reader; the repository rule
is that tools are Coax unless there is a stated reason.

## What it builds

- `aethyr/forge/pack.coax`: the `VPK1` reader and writer, the catalog, and
  `import_pack`, replacing `pack_realm.py` byte for byte.
- `aethyr/forge/units.coax`: per-kind size ranges and the recorded rescale.
- `aethyr/forge/upload.coax`: the realm side of admit, upload and scan, as
  typed states (sending, scanning, kept, refused, failed).

## Acceptance checks and oracles

- The Coax packer writes byte-identical packs to `pack_realm.py` for
  `verse/packs/everglade.json` and `fixture.json`; then the Python is
  retired.
- Over-budget, unlicensed, missing-mesh and mixed-licence layer cases are each
  refused with the asset named.
- A centimetre model is rescaled into range with its factor in the report;
  a kit mesh out of range is refused.
- Pack bytes are deterministic: two runs, same SHA-256.
- An upload that fails its scan never reaches the realm's state.

## Dependencies

[Chapter 2](ch002-spaces-and-units.md) (units),
[chapter 14](ch014-figures-and-animation.md) (rigs and clips),
[chapter 16](ch016-budgets-that-refuse.md) (identities and budgets) and
[chapter 51](ch051-a-realm-is-a-document.md) (what a document may name).
