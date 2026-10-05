"""Shared helpers for the generated-model scripts under scripts/blender/.

Each model script adds this directory to `sys.path` and imports `kit`. The
helpers build low-poly solids in Blender's +Z-up frame; the glTF exporter
turns that into +Y up. A finished model has its origin at the center of its
base, so it stands on the ground where a zone places it.
"""

import json
import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
VILLAGE = os.path.join(REPO, "assets", "verse", "everglade", "village")


def args():
    """Return the arguments after Blender's `--`."""
    argv = sys.argv
    return argv[argv.index("--") + 1 :] if "--" in argv else []


def out_path(name):
    """Where a script writes model `name`: its argument when that names a
    `.glb`, else `<argument or assets/verse/generated>/<name>.glb`."""
    a = args()
    if a and a[0].endswith(".glb"):
        return a[0]
    folder = a[0] if a else os.path.join(REPO, "assets", "verse", "generated")
    return os.path.join(folder, name + ".glb")


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def mat(name, rgb, rough=0.8, metal=0.0, emit=None, strength=1.0):
    """A flat stylized material; `emit` makes it glow in that color."""
    m = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*rgb, 1.0)
    bsdf.inputs["Roughness"].default_value = rough
    bsdf.inputs["Metallic"].default_value = metal
    if emit:
        bsdf.inputs["Emission Color"].default_value = (*emit, 1.0)
        bsdf.inputs["Emission Strength"].default_value = strength
    m.diffuse_color = (*rgb, 1.0)
    return m


def tex_mat(name, image, size=256, tint=(1.0, 1.0, 1.0)):
    """A material sampling a village kit texture, downscaled and packed.

    The kit's images are 512 or 1024 pixels; a generated prop needs far less,
    so the image is scaled to `size` and packed into the glTF as PNG.
    """
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nodes = m.node_tree.nodes
    bsdf = nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.9
    img = bpy.data.images.load(os.path.join(VILLAGE, image))
    img.scale(size, size)
    img.pack()
    img.name = name + "_BaseColor"
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = img
    m.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    if tint != (1.0, 1.0, 1.0):
        # glTF carries a base color factor beside the texture.
        mix = nodes.new("ShaderNodeMix")
        mix.data_type = "RGBA"
        mix.blend_type = "MULTIPLY"
        mix.inputs["Factor"].default_value = 1.0
        m.node_tree.links.new(tex.outputs["Color"], mix.inputs["A"])
        mix.inputs["B"].default_value = (*tint, 1.0)
        m.node_tree.links.new(mix.outputs["Result"], bsdf.inputs["Base Color"])
    return m


def _place(obj, name, material, loc, rot):
    obj.name = name
    obj.location = loc
    obj.rotation_euler = rot
    if material is not None:
        obj.data.materials.append(material)
    return obj


def box(name, size, loc=(0, 0, 0), material=None, rot=(0, 0, 0), bevel=0.0):
    bpy.ops.mesh.primitive_cube_add(size=1)
    o = bpy.context.object
    o.scale = size
    bpy.ops.object.transform_apply(scale=True)
    if bevel:
        mod = o.modifiers.new("Bevel", "BEVEL")
        mod.width = bevel
        mod.segments = 1
        mod.limit_method = "ANGLE"
    return _place(o, name, material, loc, rot)


def cyl(name, r, depth, loc=(0, 0, 0), material=None, verts=12, r2=None, rot=(0, 0, 0), cap=True):
    """A cylinder (or a cone frustum when `r2` is set) centered on `loc`."""
    bpy.ops.mesh.primitive_cone_add(
        vertices=verts,
        radius1=r,
        radius2=r if r2 is None else r2,
        depth=depth,
        end_fill_type="NGON" if cap else "NOTHING",
    )
    return _place(bpy.context.object, name, material, loc, rot)


def ball(name, r, loc=(0, 0, 0), material=None, segs=10, rings=6, scale=(1, 1, 1), rot=(0, 0, 0)):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=segs, ring_count=rings, radius=r)
    o = bpy.context.object
    o.scale = scale
    bpy.ops.object.transform_apply(scale=True)
    return _place(o, name, material, loc, rot)


def ring(name, major, minor, loc=(0, 0, 0), material=None, segs=16, minor_segs=4, rot=(0, 0, 0)):
    bpy.ops.mesh.primitive_torus_add(
        major_radius=major, minor_radius=minor, major_segments=segs, minor_segments=minor_segs
    )
    return _place(bpy.context.object, name, material, loc, rot)


def lathe(name, profile, loc=(0, 0, 0), material=None, segs=16, rot=(0, 0, 0)):
    """Revolve a list of (radius, height) points around Z.

    The profile runs from the bottom center outward and up, ending on the
    axis, so the solid is closed. Faces point outward.
    """
    bm = bmesh.new()
    rings = []
    for r, z in profile:
        if r < 1e-6:
            rings.append([bm.verts.new((0, 0, z))])
        else:
            rings.append(
                [
                    bm.verts.new((r * math.cos(2 * math.pi * i / segs), r * math.sin(2 * math.pi * i / segs), z))
                    for i in range(segs)
                ]
            )
    for a, b in zip(rings, rings[1:]):
        for i in range(segs):
            j = (i + 1) % segs
            if len(a) == 1 and len(b) == 1:
                continue
            if len(a) == 1:
                bm.faces.new((a[0], b[i], b[j]))
            elif len(b) == 1:
                bm.faces.new((a[i], a[j], b[0]))
            else:
                bm.faces.new((a[i], a[j], b[j], b[i]))
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    return _place(o, name, material, loc, rot)


def patch(name, r, az, el, segs=(12, 6), loc=(0, 0, 0), material=None, scale=(1, 1, 1)):
    """A patch of a sphere between azimuths `az` and elevations `el`.

    Angles are in degrees; azimuth 0 is +X and runs counterclockwise. The
    patch is a single sheet with normals outward; solidify it for thickness.
    """
    bm = bmesh.new()
    na, ne = segs
    grid = []
    for j in range(ne + 1):
        e = math.radians(el[0] + (el[1] - el[0]) * j / ne)
        row = []
        for i in range(na + 1):
            a = math.radians(az[0] + (az[1] - az[0]) * i / na)
            p = (r * math.cos(e) * math.cos(a) * scale[0], r * math.cos(e) * math.sin(a) * scale[1], r * math.sin(e) * scale[2])
            row.append(bm.verts.new(p))
        grid.append(row)
    for j in range(ne):
        for i in range(na):
            bm.faces.new((grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]))
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    return _place(o, name, material, loc, (0, 0, 0))


def solidify(obj, thickness):
    mod = obj.modifiers.new("Solidify", "SOLIDIFY")
    mod.thickness = thickness
    mod.offset = 0
    return obj


def box_uv(obj, tile=1.0):
    """Project world-space UVs onto each face by its dominant axis.

    `tile` is the world size, in meters, one texture repeat covers, so kit
    textures keep one scale across a model's parts.
    """
    apply_modifiers(obj)
    me = obj.data
    if not me.uv_layers:
        me.uv_layers.new(name="UVMap")
    uv = me.uv_layers.active.data
    mw = obj.matrix_world
    for poly in me.polygons:
        n = (mw.to_3x3() @ poly.normal)
        ax = max(range(3), key=lambda i: abs(n[i]))
        for li in poly.loop_indices:
            p = mw @ me.vertices[me.loops[li].vertex_index].co
            u, v = [(p.y, p.z), (p.x, p.z), (p.x, p.y)][ax]
            uv[li].uv = (u / tile, v / tile)


def cyl_uv(obj, tile=1.0, center=(0.0, 0.0)):
    """Wrap UVs around a vertical axis: u by arc length, v by height."""
    apply_modifiers(obj)
    me = obj.data
    if not me.uv_layers:
        me.uv_layers.new(name="UVMap")
    uv = me.uv_layers.active.data
    mw = obj.matrix_world
    for poly in me.polygons:
        n = mw.to_3x3() @ poly.normal
        pts = [mw @ me.vertices[me.loops[li].vertex_index].co for li in poly.loop_indices]
        cx = sum(p.x for p in pts) / len(pts) - center[0]
        cy = sum(p.y for p in pts) / len(pts) - center[1]
        base = math.atan2(cy, cx)
        for li, p in zip(poly.loop_indices, pts):
            if abs(n.z) > 0.7:
                uv[li].uv = (p.x / tile, p.y / tile)
                continue
            a = math.atan2(p.y - center[1], p.x - center[0])
            # Keep a face's corners on one side of the seam.
            while a - base > math.pi:
                a -= 2 * math.pi
            while base - a > math.pi:
                a += 2 * math.pi
            r = math.hypot(p.x - center[0], p.y - center[1])
            uv[li].uv = (a * max(r, 0.2) / tile, p.z / tile)


def apply_modifiers(obj):
    if not obj.modifiers:
        return
    bpy.context.view_layer.objects.active = obj
    for m in list(obj.modifiers):
        if m.type == "ARMATURE":
            continue
        bpy.ops.object.modifier_apply(modifier=m.name)


def meshes():
    return [o for o in bpy.data.objects if o.type == "MESH"]


def bake(obj):
    """Apply an object's modifiers and transform into its mesh."""
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    apply_modifiers(obj)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)


def fix_normals(obj):
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(obj.data)
    bm.free()


def join(name, objs=None):
    """Bake and join meshes into one object named `name`."""
    objs = objs or meshes()
    for o in objs:
        bake(o)
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.join()
    o = bpy.context.object
    o.name = name
    o.data.name = name
    return o


def ground(obj, recenter=True):
    """Move geometry so the base center sits at the origin."""
    pts = [obj.matrix_world @ v.co for v in obj.data.vertices]
    lo = Vector([min(p[i] for p in pts) for i in range(3)])
    hi = Vector([max(p[i] for p in pts) for i in range(3)])
    shift = Vector(((lo.x + hi.x) / 2 if recenter else 0, (lo.y + hi.y) / 2 if recenter else 0, lo.z))
    obj.data.transform(Matrix.Translation(-shift))
    return hi - lo


def triangles(objs=None):
    dg = bpy.context.evaluated_depsgraph_get()
    total = 0
    for o in objs or meshes():
        e = o.evaluated_get(dg)
        me = e.to_mesh()
        me.calc_loop_triangles()
        total += len(me.loop_triangles)
        e.to_mesh_clear()
    return total


def flat(objs=None):
    for o in objs or meshes():
        for p in o.data.polygons:
            p.use_smooth = False


def export(out, animations=False, extra=None):
    """Write binary glTF and print one `MODEL` JSON summary line."""
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    bpy.ops.object.select_all(action="SELECT")
    kwargs = dict(filepath=out, export_format="GLB", export_apply=True, export_yup=True)
    if animations:
        kwargs.update(export_animations=True, export_animation_mode="ACTIONS")
    else:
        kwargs.update(export_animations=False)
    bpy.ops.export_scene.gltf(**kwargs)
    dims = [0, 0, 0]
    dg = bpy.context.evaluated_depsgraph_get()
    pts = []
    for o in meshes():
        e = o.evaluated_get(dg)
        me = e.to_mesh()
        pts += [e.matrix_world @ v.co for v in me.vertices]
        e.to_mesh_clear()
    if pts:
        dims = [round(max(p[i] for p in pts) - min(p[i] for p in pts), 3) for i in range(3)]
    info = {
        "out": os.path.relpath(out, REPO) if out.startswith(REPO) else out,
        "triangles": triangles(),
        "size_xyz_m": dims,
        "bytes": os.path.getsize(out),
        "actions": sorted(a.name for a in bpy.data.actions),
        "blender": bpy.app.version_string,
    }
    if extra:
        info.update(extra)
    print("MODEL", json.dumps(info))
    return info


# --- Rigs -------------------------------------------------------------------


def armature(name, bones):
    """Build an armature from `(bone, parent, head, tail)` tuples."""
    data = bpy.data.armatures.new(name)
    arm = bpy.data.objects.new(name, data)
    bpy.context.scene.collection.objects.link(arm)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="EDIT")
    for bone, parent, head, tail in bones:
        eb = data.edit_bones.new(bone)
        eb.head = head
        eb.tail = tail
        if parent:
            eb.parent = data.edit_bones[parent]
    bpy.ops.object.mode_set(mode="OBJECT")
    for pb in arm.pose.bones:
        pb.rotation_mode = "XYZ"
    return arm


def bind(obj, bone):
    """Weight every vertex of `obj` fully to `bone` (rigid skinning)."""
    vg = obj.vertex_groups.new(name=bone)
    vg.add(range(len(obj.data.vertices)), 1.0, "REPLACE")
    return obj


def skin(arm, mesh):
    mesh.parent = arm
    mod = mesh.modifiers.new("Armature", "ARMATURE")
    mod.object = arm


def action(arm, name, frames, keys, cyclic=True, step=4):
    """Key an action on `arm` from a function of phase.

    `keys(t)` returns `{bone: (rx, ry, rz)}` Euler angles in radians, or
    `{bone: {"rot": (...), "loc": (...)}}`, for phase `t` in [0, 1); keys go
    at `frames` keys `step` frames apart, and a cyclic action repeats its first
    key at the end.
    """
    act = bpy.data.actions.new(name)
    act.use_fake_user = True
    arm.animation_data_create()
    arm.animation_data.action = act
    steps = frames if cyclic else frames - 1
    for k in range(frames + (1 if cyclic else 0)):
        t = (k % frames) / frames if cyclic else k / steps
        frame = 1 + k * step
        for bone, v in keys(t).items():
            pb = arm.pose.bones[bone]
            if isinstance(v, dict):
                rot, loc = v.get("rot", (0, 0, 0)), v.get("loc")
            else:
                rot, loc = v, None
            pb.rotation_euler = rot
            pb.keyframe_insert("rotation_euler", frame=frame)
            if loc is not None:
                pb.location = loc
                pb.keyframe_insert("location", frame=frame)
    track = arm.animation_data.nla_tracks.new()
    track.name = name
    track.strips.new(name, 1, act)
    arm.animation_data.action = None
    for pb in arm.pose.bones:
        pb.rotation_euler = (0, 0, 0)
        pb.location = (0, 0, 0)
    return act


def fcurves(act):
    """Every F-curve of an action, across Blender's layered actions."""
    if hasattr(act, "layers") and act.layers:
        out = []
        for layer in act.layers:
            for strip in layer.strips:
                for bag in strip.channelbags:
                    out.extend(bag.fcurves)
        return out
    return list(getattr(act, "fcurves", []))
