# Firm reason: 6 (Unreal Editor's source-mesh SDK has no C ABI).
# Read-only data adapter. Coax owns selection, geometry conversion and runtime budgets.
import json
import re
from pathlib import Path
import unreal

source = Path(unreal.Paths.project_saved_dir()) / 'world-source'
target = Path(unreal.Paths.project_saved_dir()) / 'world-detail'
target.mkdir(parents=True, exist_ok=True)
world = json.loads((source / 'world.json').read_text())
command = unreal.SystemLibrary.get_command_line()
start = re.search(r'-detailstart=(\d+)', command)
end = re.search(r'-detailend=(\d+)', command)
start = int(start[1]) if start else 0
end = int(end[1]) if end else start + 16
rows = []
export = re.search(r'-detailexport=(\d+)', command)
for index in ([int(export[1])] if export else range(start, min(end, len(world['meshes'])))):
    path = world['meshes'][index]['path']
    mesh = unreal.load_asset(path)
    high = unreal.GeometryScriptMeshReadLOD(lod_type=unreal.GeometryScriptLODType.HI_RES_SOURCE_MODEL)
    available = unreal.GeometryScript_AssetUtils.check_static_mesh_has_available_lod(mesh, high)
    row = dict(index=index, path=path, fallback_vertices=mesh.get_num_vertices(0),
               fallback_triangles=mesh.get_num_triangles(0),
               nanite_vertices=mesh.get_num_nanite_vertices(), nanite_triangles=mesh.get_num_nanite_triangles(),
               hires_available=bool(available[0] if isinstance(available, tuple) else available))
    rows.append(row)
    (target / (('mesh-%03d-source.json' % index) if export else ('inventory-%03d.json' % start))).write_text(json.dumps(rows, indent=2))
    unreal.log('VALLEY_DETAIL_INVENTORY ' + json.dumps(row))
    if export:
        options = unreal.GLTFExportOptions()
        options.export_source_model = True
        options.bake_material_inputs = unreal.GLTFMaterialBakeMode.DISABLED
        options.export_vertex_colors = True
        options.default_level_of_detail = 0
        result = unreal.GLTFExporter.export_to_gltf(mesh, str(target / ('mesh%03d.gltf' % index)), options, set())
        if result.errors:
            raise RuntimeError(str(result.errors))
        unreal.log('VALLEY_DETAIL_SOURCE_EXPORTED %d' % index)
