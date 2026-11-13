# Aethyr to Unreal

The Unreal side of the exporter. The design is
[docs/unreal/2026-11-12-aethyr-to-unreal-exporter-spec.md](../../../docs/unreal/2026-11-12-aethyr-to-unreal-exporter-spec.md).

## Export, check, import

Run from the repository root.

```sh
# 1. Export Splice. The folder must not exist. Same source, same bytes.
coax/web/tool.sh splice/splice-export.coax /tmp/splice.aexport

# 2. Read the bundle back with Aethyr's own glTF reader.
coax/web/tool.sh aethyr/tools/unreal-export-check.coax /tmp/splice.aexport

# 3. Import into an Unreal project (Mac). Exit 0 means every check passed.
coax/web/tool.sh aethyr/tools/unreal-import.coax /tmp/splice.aexport \
  ~/UnrealProjects/SpliceExport/AethyrHost.uproject ~/UnrealEngine-5.9/src
```

Step 3 makes the project when it is named `AethyrHost.uproject` and does not
exist. It copies the plugin in, builds the editor modules, and runs
`import.py` in a headless editor. A first run on a new project compiles
engine shaders for several minutes; later runs take about a minute.

## What is here

| Path | What it is |
|---|---|
| `aethyr_host.h` | The host interface with real pointers, for the hosted rules (step 2) |
| `AethyrBridge/` | The Unreal plugin: manifest asset, collision box, marker, and the editor checks Python cannot do |
| `project/` | A bare host project that receives an export |
| `import.py` | The importer and its checks |
| `probe/` | Step 0: the frame-hash probe and the axis fixture |

## What the importer builds

Everything goes under `/Game/Aethyr/<Game>/Generated/`, which is deleted and
rebuilt each time. Nothing else in the project is touched; the importer
compares every other content file before and after.

- `world/StaticMeshes/`: one static mesh per world group, Nanite off.
- `Materials/M_AeVertexColour`: unlit, both faces, the vertex colour. This
  is the stand-in look until the bridge's cel materials exist.
- `L_<Game>`: the meshes, one `AethyrCollisionBox` per collision box, one
  `AethyrMarker` per placement.
- `DA_<Game>Manifest`: the bundle's hash and every tuning number.
- `Fonts/`: the two font faces.

## What the importer checks

The report is `Saved/aethyr-import-report.json` in the project.

- The bundle's hash, and mesh, triangle, box, marker, font and tuning counts
  against the manifest.
- Every rendered vertex position and colour, and every mesh's bounds,
  against `world.glb`.
- The material's compile errors, read from Unreal, not from a log.
- Each reference shot against a render of the same camera: at least 98
  percent of pixels agree on covered or empty, and the mean colour
  difference over covered pixels is at most 6 of 255.
- That a second import gives the same report.

## Things to know

- Unreal's glTF import turns Nanite on. A Nanite mesh renders a reduced
  fallback where Nanite is unsupported, so the importer turns it off.
- Unreal stores glTF vertex colours as linear bytes. The material reads them
  as they are.
- Building a project's editor target the first time relinks one engine
  library (NetCore) and re-signs the shared `UnrealEditor.app`. Another
  editor from the same engine keeps running through it.
- Saving the level logs an "Unable to Check Out From Revision Control"
  message in a headless run. The level is saved.
- The reference shots are drawn in software from the same vertex stream the
  page draws (`aethyr/export/raster.coax`). They check position and colour,
  not the anime shading.
