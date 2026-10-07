"""Convert a licensed character into the glTF a private pack compiles from.

Run headless (`verse-private add` runs it for you):
    Blender -b --factory-startup --python scripts/blender/private_character.py -- \
        IN OUT_DIR [--height M] [--near N] [--far N] [--edge PX] [--up AXIS] [--turn DEG]
        [--pose standing|seated]

IN is a `.glb`, `.gltf`, `.fbx`, or `.blend`. OUT_DIR receives
`guest.gltf` and `guest.bin` (the near level), `guest_far.gltf` and
`guest_far.bin` (the far level), `guest.png` and `guest_far.png`,
`preview_front.png`, `preview_side.png`, `preview_face.png`, and
`report.json`. The output is licensed content: write it outside the
repository (docs/verse/private-assets.md).

Steps:

- Imports the source, drops cameras, lights, and empties, applies any
  armature in its rest pose and drops it, joins every mesh, and welds the
  vertices an exporter split along UV seams.
- Stands the body up: `--up` names the source's up axis in Blender's frame
  (`x`, `y`, `z`, `-x`, `-y`, or `-z`); `auto` takes the longest axis,
  positive end up. `--turn` turns it about the vertical, degrees, so its
  front faces -Y in Blender (+Z, glTF's front). It scales the body to
  `--height` meters with its feet at the origin.
- Makes two levels: `--near` triangles with an `--edge`-pixel texture and
  `--far` triangles with half that. Each is decimated from the full mesh,
  unwrapped afresh, and has the full mesh's base color baked into its own
  image. Normal, roughness, and metallic maps are dropped: the pack's
  materials sample base color only.
- Rigs both levels with one five-joint spine (root, hips, spine, chest,
  head), weights each vertex by its height with a smooth blend between
  joints, and keys `idle`: a four-second breath, a weight shift, and a slow
  head turn. The legs stay planted. There is no walk.
- With `--pose seated`, it first bends the standing body into a sitting
  one (`seat`): the legs fold forward at the hips and down at the knees, so
  the thighs lie level and the shins hang, and the hands, which hang beside
  the thighs, come to rest on the lap. The rig's joints follow the seated
  body, and `idle` is an eight-second seated idle: two breaths, the head
  bowed a little toward the desk, and once a cycle a glance up and a little
  aside toward whoever arrives. The report gives `seat_m`, the height of the
  seat under the body, which a chair must match.
"""

import json
import math
import os
import sys

import bpy
from mathutils import Matrix, Vector

sys.dont_write_bytecode = True  # Leave the checkout's __pycache__ alone.
sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

TAU = 2 * math.pi
# Joint pivots as fractions of the body's height, and the height each
# joint's weight takes over from the one below, with its blend half width.
JOINTS = [("root", None, 0.0), ("hips", "root", 0.50), ("spine", "hips", 0.60),
          ("chest", "spine", 0.70), ("head", "chest", 0.85)]
BLEND = 0.03
# The seated pose's bends: the hip and knee pivots as fractions of the
# standing height, and each bend's blend half width, also a fraction.
HIP, KNEE, BEND = 0.49, 0.285, 0.035


def options():
    a = kit.args()
    if len(a) < 2:
        sys.exit("usage: private_character.py IN OUT_DIR [--height M] [--near N] [--far N] "
                 "[--edge PX] [--up AXIS] [--turn DEG] [--pose standing|seated]")
    o = {"in": a[0], "out": a[1], "height": 1.62, "near": 20000, "far": 5000, "edge": 1024,
         "up": "auto", "turn": 0.0, "pose": "standing"}
    rest = a[2:]
    while rest:
        key, value = rest[0], rest[1] if len(rest) > 1 else None
        if not key.startswith("--") or value is None or key[2:] not in o:
            sys.exit(f"unknown or incomplete option: {key}")
        k = key[2:]
        o[k] = value if k in ("up", "pose") else float(value)
        rest = rest[2:]
    o["near"], o["far"], o["edge"] = int(o["near"]), int(o["far"]), int(o["edge"])
    if o["pose"] not in ("standing", "seated"):
        sys.exit("--pose is standing or seated")
    return o


def load(path):
    ext = os.path.splitext(path)[1].lower()
    if ext == ".blend":
        bpy.ops.wm.open_mainfile(filepath=path)
    else:
        kit.reset()
        if ext in (".glb", ".gltf"):
            bpy.ops.import_scene.gltf(filepath=path)
        elif ext == ".fbx":
            bpy.ops.import_scene.fbx(filepath=path)
        else:
            sys.exit(f"unsupported source: {path}")
    # Any rig is applied in its rest pose; this pipeline adds its own.
    for arm in [o for o in bpy.data.objects if o.type == "ARMATURE"]:
        arm.data.pose_position = "REST"
    bpy.context.view_layer.update()
    dg = bpy.context.evaluated_depsgraph_get()
    for o in kit.meshes():
        if any(m.type == "ARMATURE" for m in o.modifiers) or o.data.shape_keys:
            e = o.evaluated_get(dg)
            mesh = bpy.data.meshes.new_from_object(e, preserve_all_data_layers=True, depsgraph=dg)
            world = o.matrix_world.copy()
            o.modifiers.clear()
            o.shape_key_clear() if o.data.shape_keys else None
            o.data = mesh
            o.parent = None
            o.matrix_world = world
    for o in list(bpy.data.objects):
        if o.type != "MESH" or o.hide_render:
            bpy.data.objects.remove(o, do_unlink=True)
    for o in kit.meshes():
        o.parent = None
    body = kit.join("guest_source")
    # Exported meshes split their vertices along every UV seam; weld them,
    # so the surface decimates and unwraps as one piece. UVs live on face
    # corners, so the source texture still maps exactly.
    select(body)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.remove_doubles(threshold=1e-5)
    bpy.ops.object.mode_set(mode="OBJECT")
    return body


def stand(body, up, turn, height):
    pts = [v.co for v in body.data.vertices]
    lo = Vector([min(p[i] for p in pts) for i in range(3)])
    hi = Vector([max(p[i] for p in pts) for i in range(3)])
    size = hi - lo
    if up == "auto":
        axis = max(range(3), key=lambda i: size[i])
        up = "xyz"[axis]
    sign = -1.0 if up.startswith("-") else 1.0
    axis = "xyz".index(up[-1])
    # Rotations that take the source's up axis to +Z.
    to_z = {
        (2, 1.0): Matrix.Identity(4),
        (2, -1.0): Matrix.Rotation(math.pi, 4, "X"),
        (1, 1.0): Matrix.Rotation(math.pi / 2, 4, "X"),
        (1, -1.0): Matrix.Rotation(-math.pi / 2, 4, "X"),
        (0, 1.0): Matrix.Rotation(-math.pi / 2, 4, "Y"),
        (0, -1.0): Matrix.Rotation(math.pi / 2, 4, "Y"),
    }[(axis, sign)]
    body.data.transform(Matrix.Rotation(math.radians(turn), 4, "Z") @ to_z)
    dims = kit.ground(body)
    scale = height / dims.z
    body.data.transform(Matrix.Scale(scale, 4))
    return up, scale


def decimate(obj, target):
    now = kit.triangles([obj])
    if now > target:
        mod = obj.modifiers.new("Decimate", "DECIMATE")
        mod.ratio = target / now
        mod.use_collapse_triangulate = True
        select(obj)
        bpy.ops.object.modifier_apply(modifier=mod.name)
    return kit.triangles([obj])


def select(*objs):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[-1]


def level(high, name, target, edge):
    """A copy of `high` decimated to `target` triangles, unwrapped afresh,
    with `high`'s base color baked into one `edge`-pixel image. Collapsing
    a dense AI mesh smears its own UV seams; a bake from the full mesh
    doesn't."""
    low = high.copy()
    low.data = high.data.copy()
    low.name = low.data.name = name
    bpy.context.scene.collection.objects.link(low)
    triangles = decimate(low, target)
    uvs = low.data.uv_layers
    baked = uvs.new(name="baked")
    uvs.active = baked
    select(low)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    # A wide angle limit keeps islands few and large on a decimated mesh,
    # and concave packing fills the square, so mipmaps don't bleed the
    # background into thin islands.
    bpy.ops.uv.smart_project(angle_limit=math.radians(89), island_margin=0.003, area_weight=1.0)
    bpy.ops.uv.pack_islands(rotate=True, margin=0.003, shape_method="CONCAVE")
    bpy.ops.object.mode_set(mode="OBJECT")
    image = bpy.data.images.new(name, edge, edge, alpha=False)
    image.file_format = "PNG"
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.8
    tex = mat.node_tree.nodes.new("ShaderNodeTexImage")
    tex.image = image
    mat.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    mat.node_tree.nodes.active = tex
    low.data.materials.clear()
    low.data.materials.append(mat)
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 4
    select(high, low)
    bpy.ops.object.bake(type="DIFFUSE", pass_filter={"COLOR"}, use_selected_to_active=True,
                        cage_extrusion=0.02, max_ray_distance=0.06, margin=8)
    image.pack()
    for layer in [u for u in uvs if u.name != "baked"]:
        uvs.remove(layer)
    return low, triangles


def smoothstep(t):
    t = min(max(t, 0.0), 1.0)
    return t * t * (3.0 - 2.0 * t)


def seat(body, height):
    """Bends the standing `body`, feet at the origin and front toward -Y,
    into a sitting one: the shins swing back at the knees, then the legs
    swing forward at the hips, so the thighs lie level toward the front and
    the shins hang. Each vertex turns about a joint's pivot by an angle
    that eases in across the joint, so the bends stay smooth. The torso
    stays over the origin, lowered so the feet are on the ground. Returns
    how far it was lowered and the seat's height under it, m."""
    verts = body.data.vertices
    original = [v.co.copy() for v in verts]

    def middle(z):
        ys = [p.y for p in original if abs(p.z - z) < 0.01 * height]
        return (min(ys) + max(ys)) / 2 if ys else 0.0

    b = BEND * height
    for joint, angle in ((KNEE, math.pi / 2), (HIP, -math.pi / 2)):
        z0 = joint * height
        y0 = middle(z0)
        for v, p in zip(verts, original):
            a = angle * smoothstep((z0 + b - p.z) / (2 * b))
            if a == 0.0:
                continue
            c, s = math.cos(a), math.sin(a)
            y, z = v.co.y - y0, v.co.z - z0
            v.co.y = y0 + y * c - z * s
            v.co.z = z0 + y * s + z * c
    low = min(v.co.z for v in verts)
    for v in verts:
        v.co.z -= low
    body.data.update()
    # The seat: the lowest point of the buttocks, behind the hip's pivot,
    # among the vertices that stood near the hip.
    hip = HIP * height
    y_hip = middle(hip)
    under = [v.co.z for v, p in zip(verts, original) if abs(p.z - hip) < 0.08 * height and p.y > y_hip]
    return low, min(under) if under else hip + low


def weigh(body, pivots, height):
    """Weights each vertex of `body` by its height: fully to the joint whose
    segment holds it, blended with the neighbor within BLEND of a pivot.
    `pivots` are the joints' heights, m, on a body `height` m tall standing."""
    body.vertex_groups.clear()
    groups = [body.vertex_groups.new(name=name) for name, _, _ in JOINTS]
    blend = BLEND * height
    for v in body.data.vertices:
        t = v.co.z
        k = max(i for i, f in enumerate(pivots) if t >= f or i == 0)
        weights = {k: 1.0}
        if k + 1 < len(pivots) and t > pivots[k + 1] - blend:
            w = (t - (pivots[k + 1] - blend)) / (2 * blend)
            weights = {k: 1.0 - w, k + 1: w}
        elif k > 0 and t < pivots[k] + blend:
            w = (t - (pivots[k] - blend)) / (2 * blend)
            weights = {k - 1: 1.0 - w, k: w}
        for j, w in weights.items():
            if w > 1e-3:
                groups[j].add([v.index], w, "REPLACE")


def pivots(height, drop):
    """Each joint's height, m: as the body stands, or, seated, lowered by
    `drop` above the root, so the hips' joint is at the seat."""
    return [0.0 if f == 0.0 else f * height - drop for _, _, f in JOINTS]


def rig(body, heights, height, seated):
    bones = [(name, parent, (0.0, 0.0, z), (0.0, 0.0, z + 0.05))
             for (name, parent, _), z in zip(JOINTS, heights)]
    arm = kit.armature("guest_rig", bones)
    weigh(body, heights, height)
    kit.skin(arm, body)

    if seated:
        def bump(t, at, width):
            """1 at `at`, easing to 0 `width` either side, round the cycle."""
            d = min(abs(t - at), 1.0 - abs(t - at))
            return smoothstep(1.0 - d / width)

        def sitting(t):
            # Two breaths a cycle, and the head bowed a little toward the
            # desk, lifting and turning a little aside once a cycle. No
            # weight shift: she sits.
            s, c = math.sin(2 * TAU * t), math.cos(2 * TAU * t)
            look = bump(t, 0.65, 0.18)
            return {
                "spine": (0.005 * c, 0.0, 0.0),
                "chest": (-0.016 * s, 0.0, -0.004 * s),
                "head": (0.12 * (1.0 - look) - 0.03 * look, 0.16 * look + 0.02 * math.sin(TAU * t), 0.0),
            }

        # Twenty-four keys eight frames apart: eight seconds at 24 fps.
        kit.action(arm, "idle", 24, sitting, cyclic=True, step=8)
        return arm

    def idle(t):
        s, c = math.sin(TAU * t), math.cos(TAU * t)
        return {
            # Bones point up +Z: a rotation about local Y turns about the
            # vertical, about X pitches, about Z leans side to side.
            "hips": (0.0, 0.0, 0.022 * s),
            "spine": (0.006 * c, 0.0, -0.014 * s),
            "chest": (-0.016 * s, 0.0, -0.006 * s),
            "head": (0.02 * c, 0.09 * s, 0.01 * s),
        }

    kit.action(arm, "idle", 24, idle, cyclic=True, step=4)
    return arm


def export(arm, mesh, out):
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    mesh.select_set(True)
    bpy.ops.export_scene.gltf(
        filepath=out, export_format="GLTF_SEPARATE", use_selection=True, export_yup=True,
        export_apply=False, export_animations=True, export_animation_mode="ACTIONS",
        export_image_format="AUTO", export_materials="EXPORT")


def preview(objs, out):
    """Front and side views of `objs` side by side."""
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x, scene.render.resolution_y = 480, 640
    scene.world = bpy.data.worlds.new("w")
    scene.world.color = (0.55, 0.6, 0.65)
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.rotation_euler = (0.7, 0.2, -0.4)
    scene.collection.objects.link(sun)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    scene.collection.objects.link(cam)
    scene.camera = cam
    h = max(v.co.z for v in objs[0].data.vertices)
    paths = []
    for name, direction, center, reach in (
            ("front", Vector((0, -1, 0)), Vector((0, 0, h / 2)), 2.2),
            ("side", Vector((1, 0, 0)), Vector((0, 0, h / 2)), 2.2),
            ("face", Vector((0.3, -1, 0)).normalized(), Vector((0, 0, h * 0.9)), 0.45)):
        cam.location = center + direction * h * reach
        cam.rotation_euler = (center - cam.location).to_track_quat("-Z", "Y").to_euler()
        path = out.replace(".png", f"_{name}.png")
        scene.render.filepath = path
        bpy.ops.render.render(write_still=True)
        paths.append(path)
    return paths


def main():
    o = options()
    os.makedirs(o["out"], exist_ok=True)
    body = load(o["in"])
    source_triangles = kit.triangles([body])
    up, scale = stand(body, o["up"], o["turn"], o["height"])
    seated = o["pose"] == "seated"
    drop, seat_m = seat(body, o["height"]) if seated else (0.0, 0.0)
    near_body, near = level(body, "guest", o["near"], o["edge"])
    far_body, far = level(body, "guest_far", o["far"], o["edge"] // 2)
    bpy.data.objects.remove(body, do_unlink=True)
    body = near_body
    heights = pivots(o["height"], drop)
    arm = rig(body, heights, o["height"], seated)
    # The far level takes the same weights by the same rule, on the same rig.
    weigh(far_body, heights, o["height"])
    kit.skin(arm, far_body)
    # Each level exports alone with the shared rig.
    far_body.hide_set(True)
    export(arm, body, os.path.join(o["out"], "guest.gltf"))
    far_body.hide_set(False)
    body.hide_set(True)
    export(arm, far_body, os.path.join(o["out"], "guest_far.gltf"))
    body.hide_set(False)
    far_body.hide_render = True
    arm.hide_render = True
    previews = preview([body], os.path.join(o["out"], "preview.png"))
    report = {
        "blender": bpy.app.version_string,
        "source": os.path.basename(o["in"]),
        "source_triangles": source_triangles,
        "up": up,
        "turn_degrees": o["turn"],
        "scale": round(scale, 6),
        "height_m": o["height"],
        "near_triangles": near,
        "far_triangles": far,
        "texture_edge": o["edge"],
        "textures": {"guest": o["edge"], "guest_far": o["edge"] // 2},
        "rig": "generated spine: " + ", ".join(n for n, _, _ in JOINTS),
        "clips": ["idle"],
        "pose": o["pose"],
        "seat_m": round(seat_m, 3),
        "previews": [os.path.basename(p) for p in previews],
    }
    with open(os.path.join(o["out"], "report.json"), "w") as f:
        json.dump(report, f, indent=2, sort_keys=True)
    print("PRIVATE_CHARACTER " + json.dumps(report, sort_keys=True))


main()
