# Firm reason: 6 (the Unreal Editor's asset import API exists only in Python and C++).
# Imports the axis fixture (metaverse-engine/tools/unreal-axes-fixture.coax) with Unreal's default glTF
# import and prints where each marker cube landed, which way its faces wind, and its colour.
# GitHub #13399. Run headless:
#   AETHYR_AXES_GLB=/abs/axes.glb AETHYR_AXES_OUT=/abs/axes.txt UnrealEditor-Cmd AethyrProbe.uproject -run=pythonscript \
#     -script=/abs/import_axes.py -nullrhi -unattended -stdout
import os
import unreal

path = os.environ["AETHYR_AXES_GLB"]
lines = []


def say(text):
    # unreal.log is below the level -stdout shows, so the lines also go to a file.
    lines.append(text)
    unreal.log(text)


dest = "/Game/AxesProbe"
task = unreal.AssetImportTask()
task.filename = path
task.destination_path = dest
task.automated = True
task.replace_existing = True
task.save = False
unreal.AssetToolsHelpers.get_asset_tools().import_asset_tasks([task])

registry = unreal.AssetRegistryHelpers.get_asset_registry()
assets = registry.get_assets_by_path(dest, recursive=True)
meshes = []
for data in assets:
    asset = data.get_asset()
    say("AXES_PROBE asset %s %s" % (asset.get_class().get_name(), asset.get_path_name()))
    if isinstance(asset, unreal.StaticMesh):
        meshes.append(asset)
say("AXES_PROBE meshes %d" % len(meshes))

def cross(a, b):
    return unreal.Vector(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x)


def winding(mesh):
    # For a convex mesh: how many triangles have cross(p1-p0, p2-p0) pointing away from the
    # centre, and how many toward it. The engine's own cube gives the sign Unreal calls front.
    d = mesh.get_static_mesh_description(0)
    box = mesh.get_bounding_box()
    centre = (box.min + box.max) * 0.5
    out = 0
    inward = 0
    for t in range(d.get_triangle_count()):
        inst = d.get_triangle_vertex_instances(unreal.TriangleID(t))
        p = [d.get_vertex_position(d.get_vertex_instance_vertex(i)) for i in inst]
        g = cross(p[1] - p[0], p[2] - p[0])
        m = (p[0] + p[1] + p[2]) * (1.0 / 3.0) - centre
        if g.x * m.x + g.y * m.y + g.z * m.z > 0:
            out += 1
        else:
            inward += 1
    return out, inward


engine_cube = unreal.load_asset("/Engine/BasicShapes/Cube")
say("AXES_PROBE engine cube triangles outward %d inward %d" % winding(engine_cube))
for mesh in sorted(meshes, key=lambda m: m.get_name()):
    box = mesh.get_bounding_box()
    c = (box.min + box.max) * 0.5
    s = box.max - box.min
    say("AXES_PROBE mesh %s centre %.3f %.3f %.3f size %.3f %.3f %.3f" % (mesh.get_name(), c.x, c.y, c.z, s.x, s.y, s.z))
    say("AXES_PROBE winding %s triangles outward %d inward %d" % ((mesh.get_name(),) + winding(mesh)))
    d = mesh.get_static_mesh_description(0)
    say("AXES_PROBE api %s" % " ".join(n for n in dir(d) if "color" in n.lower() or "normal" in n.lower() or "uv" in n.lower()))
    say("AXES_PROBE lod0 vertices %d triangles %d materials %s" % (d.get_vertex_count(), d.get_triangle_count(), [str(m.material_interface.get_name()) for m in mesh.static_materials]))
say("AXES_PROBE done")
with open(os.environ["AETHYR_AXES_OUT"], "w") as f:
    for line in lines:
        f.write(line + chr(10))
