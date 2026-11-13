# Firm reason: 6 (the Unreal Editor's asset and level API exists only in Python and C++).
# Imports an Aethyr export bundle into the open Unreal project
# (docs/unreal/2026-11-12-aethyr-to-unreal-exporter-spec.md, step 1; GitHub #13400).
#
#   AETHYR_BUNDLE=/abs/splice.aexport AETHYR_REPORT=/abs/report.json \
#     UnrealEditor-Cmd Project.uproject -ExecutePythonScript=/abs/import.py -unattended -nosplash -RenderOffscreen
#
# It rebuilds /Game/Aethyr/<Game>/Generated/ and touches nothing else:
#   world meshes (glTF, Interchange), one unlit vertex-colour material, the level L_<Game> with
#   the meshes, a blocking box for each collision box and a marker for each placement, the
#   manifest asset with the tuning, and the fonts.
# Then it checks its own work and writes the report. "ok": false means a check failed:
#   counts against the manifest; every vertex position and colour and every triangle count
#   against world.glb; the material's compile errors; each reference shot against a render of
#   the same camera; and that no file outside Generated/ changed.
# Needs the AethyrBridge plugin (collision box, marker and manifest classes, and the editor
# checks Python cannot do itself).
import hashlib
import json
import math
import os
import shutil
import struct
import zlib

import unreal

BUNDLE = os.environ["AETHYR_BUNDLE"]
REPORT = os.environ["AETHYR_REPORT"]
SHOT_OUT = os.environ.get("AETHYR_SHOTS", "")
UNITS = 100.0
SILHOUETTE_MIN = 0.98
COLOUR_MAX = 6.0       # mean difference of sRGB bytes over pixels both pictures cover
BOX_MIN_HEIGHT = 3.0   # metres: a box the rules give no height still blocks a walker

failures = []
report = {"ok": False, "failures": failures}


def check(ok, text):
    if not ok:
        failures.append(text)
        unreal.log_error("AETHYR_IMPORT FAIL " + text)
    return ok


def to_unreal(p):
    # Aethyr metres, y up, right-handed -> Unreal centimetres, z up (measured, #13399)
    return unreal.Vector(p[0] * UNITS, p[2] * UNITS, p[1] * UNITS)


def srgb8(c):
    c = min(1.0, max(0.0, c))
    v = c * 12.92 if c <= 0.0031308 else 1.055 * (c ** (1.0 / 2.4)) - 0.055
    return int(round(v * 255.0))


def read_glb(path):
    data = open(path, "rb").read()
    magic, version, total = struct.unpack_from("<4sII", data, 0)
    json_n = struct.unpack_from("<I", data, 12)[0]
    doc = json.loads(data[20:20 + json_n])
    blob = data[20 + json_n + 8:]
    views, accessors = doc["bufferViews"], doc["accessors"]

    def floats(index, n):
        a = accessors[index]
        v = views[a["bufferView"]]
        stride = v.get("byteStride", n * 4)
        base = v["byteOffset"] + a.get("byteOffset", 0)
        return [struct.unpack_from("<%df" % n, blob, base + i * stride) for i in range(a["count"])]

    meshes = {}
    for mesh in doc["meshes"]:
        prim = mesh["primitives"][0]
        ia = accessors[prim["indices"]]
        iv = views[ia["bufferView"]]
        indices = struct.unpack_from("<%dI" % ia["count"], blob, iv["byteOffset"] + ia.get("byteOffset", 0))
        meshes[mesh["name"]] = {
            "positions": floats(prim["attributes"]["POSITION"], 3),
            "colours": floats(prim["attributes"]["COLOR_0"], 4),
            "indices": indices,
        }
    return data, meshes


def read_png(path):
    data = open(path, "rb").read()
    at, raw, w, h = 8, b"", 0, 0
    while at < len(data):
        n, kind = struct.unpack_from(">I4s", data, at)
        body = data[at + 8:at + 8 + n]
        if kind == b"IHDR":
            w, h, depth, colour = struct.unpack_from(">IIBB", body, 0)
            if depth != 8 or colour != 6:
                raise ValueError("reference shot is not 8-bit RGBA")
        elif kind == b"IDAT":
            raw += body
        at += 12 + n
    raw = zlib.decompress(raw)
    out, prev, stride = bytearray(), bytearray(w * 4), w * 4
    for y in range(h):
        f = raw[y * (stride + 1)]
        line = bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - 4] if i >= 4 else 0
            b = prev[i]
            c = prev[i - 4] if i >= 4 else 0
            if f == 1:
                line[i] = (line[i] + a) & 255
            elif f == 2:
                line[i] = (line[i] + b) & 255
            elif f == 3:
                line[i] = (line[i] + ((a + b) >> 1)) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else (b if pb <= pc else c))) & 255
        out += line
        prev = line
    return w, h, bytes(out)


def write_png(path, w, h, rgba):
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
    rows = b"".join(b"\x00" + rgba[y * w * 4:(y + 1) * w * 4] for y in range(h))
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows, 6)) + chunk(b"IEND", b""))


def content_outside(content_dir, generated_rel):
    seen = {}
    skip = os.path.join(content_dir, generated_rel)
    for root, dirs, files in os.walk(content_dir):
        if root.startswith(skip):
            continue
        for name in files:
            p = os.path.join(root, name)
            st = os.stat(p)
            seen[os.path.relpath(p, content_dir)] = [st.st_size, int(st.st_mtime)]
    return seen


manifest = json.load(open(os.path.join(BUNDLE, "manifest.json")))
check(manifest.get("aexport") == 1, "manifest version is not 1")
game = manifest["game"]["name"]
GEN = "/Game/Aethyr/%s/Generated" % game
content_dir = unreal.Paths.convert_relative_path_to_full(unreal.Paths.project_content_dir())
generated_rel = os.path.join("Aethyr", game, "Generated")
before = content_outside(content_dir, generated_rel)

glb_path = os.path.join(BUNDLE, manifest["world"]["file"])
glb_bytes, glb_meshes = read_glb(glb_path)
check(hashlib.sha256(glb_bytes).hexdigest() == manifest["world"]["sha256"], "world.glb does not match the manifest's SHA-256")

assets = unreal.EditorAssetLibrary
tools = unreal.AssetToolsHelpers.get_asset_tools()
levels = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem)
actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
checks = unreal.AethyrEditorChecks

# ---- rebuild Generated/ from nothing
levels.load_level("/Engine/Maps/Entry")
if assets.does_directory_exist(GEN):
    assets.delete_directory(GEN)   # reports failure when only the empty folder is left, so look instead
    left = [str(d.package_name) for d in unreal.AssetRegistryHelpers.get_asset_registry().get_assets_by_path(GEN, recursive=True)]
    check(len(left) == 0, "could not delete %d assets in %s" % (len(left), GEN))
    shutil.rmtree(os.path.join(content_dir, generated_rel), ignore_errors=True)   # files the editor left behind

task = unreal.AssetImportTask()
task.filename = glb_path
task.destination_path = GEN
task.automated = True
task.replace_existing = True
task.save = False
tools.import_asset_tasks([task])

registry = unreal.AssetRegistryHelpers.get_asset_registry()
meshes = {}
for data in registry.get_assets_by_path(GEN, recursive=True):
    asset = data.get_asset()
    if isinstance(asset, unreal.StaticMesh):
        meshes[asset.get_name()] = asset
groups = manifest["world"]["groups"]
check(len(meshes) == len(groups), "imported %d meshes, the manifest lists %d" % (len(meshes), len(groups)))

# Unreal's glTF import turns Nanite on, and a Nanite mesh renders a reduced fallback where Nanite
# is not supported (the web build has none). The export is the mesh, triangle for triangle.
mesh_tools = unreal.get_editor_subsystem(unreal.StaticMeshEditorSubsystem)
for mesh in meshes.values():
    nanite = mesh.get_editor_property("nanite_settings")
    if nanite.enabled:
        nanite.enabled = False
        mesh_tools.set_nanite_settings(mesh, nanite, True)

# ---- every vertex and triangle against world.glb
checks.finish_compiling()
triangles = 0
vertex_bad = 0
samples = []
srgb_votes = 0
linear_votes = 0
mesh_rows = []
for g in groups:
    name = g["name"]
    mesh = meshes.get(name)
    if not check(mesh is not None, "mesh %s is missing" % name):
        continue
    read = checks.read_mesh(mesh)   # None when the mesh has no vertex colours
    if not check(read is not None, "mesh %s has no vertex colours" % name):
        continue
    positions, colours, indices = read
    source = glb_meshes[name]
    check(len(indices) == g["triangles"] * 3, "mesh %s has %d triangles, the manifest says %d" % (name, len(indices) // 3, g["triangles"]))
    triangles += len(indices) // 3
    want = {}
    for p, c in zip(source["positions"], source["colours"]):
        key = (round(p[0] * UNITS * 10), round(p[2] * UNITS * 10), round(p[1] * UNITS * 10))   # millimetres
        want.setdefault(key, []).append(((srgb8(c[0]), srgb8(c[1]), srgb8(c[2])), (int(round(c[0] * 255)), int(round(c[1] * 255)), int(round(c[2] * 255)))))
    seen = set()
    lo = [1e30, 1e30, 1e30]
    hi = [-1e30, -1e30, -1e30]
    for p, c in zip(positions, colours):
        key = (round(p.x * 10), round(p.y * 10), round(p.z * 10))
        for k, v in enumerate((p.x, p.y, p.z)):
            lo[k] = min(lo[k], v)
            hi[k] = max(hi[k], v)
        options = want.get(key)
        if options is None:
            # a position on a millimetre boundary rounds either way: look one millimetre around
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    for dz in (-1, 0, 1):
                        near = (key[0] + dx, key[1] + dy, key[2] + dz)
                        if options is None and near in want:
                            options, key = want[near], near
        if options is None:
            vertex_bad += 1
            if len(samples) < 8:
                samples.append("%s: no source vertex at Unreal (%.2f, %.2f, %.2f)" % (name, p.x, p.y, p.z))
            continue
        got = (c.r, c.g, c.b)
        as_srgb = any(all(abs(got[i] - o[0][i]) <= 1 for i in range(3)) for o in options)
        as_linear = any(all(abs(got[i] - o[1][i]) <= 1 for i in range(3)) for o in options)
        srgb_votes += 1 if as_srgb else 0
        linear_votes += 1 if as_linear else 0
        if not (as_srgb or as_linear):
            vertex_bad += 1
            if len(samples) < 8:
                samples.append("%s: colour %s at Unreal (%.2f, %.2f, %.2f), source sRGB/linear %s" % (name, got, p.x, p.y, p.z, options[:2]))
        seen.add(key)
    lost = set(want) - seen
    vertex_bad += len(lost)
    for key in sorted(lost)[:2]:
        if len(samples) < 8:
            samples.append("%s: source vertex at Unreal mm %s is not in the mesh" % (name, key))
    for k, (a, b) in enumerate(((g["min"][0], g["max"][0]), (g["min"][2], g["max"][2]), (g["min"][1], g["max"][1]))):
        check(abs(lo[k] - a * UNITS) < 0.05 and abs(hi[k] - b * UNITS) < 0.05, "mesh %s bounds differ on Unreal axis %d" % (name, k))
    mesh_rows.append([name, len(positions), len(indices) // 3])
check(triangles == manifest["world"]["triangles"], "imported %d triangles, the manifest says %d" % (triangles, manifest["world"]["triangles"]))
check(vertex_bad == 0, "%d vertices differ from world.glb in position or colour" % vertex_bad)
colours_are_srgb = srgb_votes >= linear_votes
report["vertex_samples"] = samples
report["vertex_colours"] = {"stored_as": "sRGB bytes" if colours_are_srgb else "linear bytes", "srgb_matches": srgb_votes, "linear_matches": linear_votes}

# ---- the stand-in look: unlit, both faces, the vertex colour as it was in Aethyr
mel = unreal.MaterialEditingLibrary
material = tools.create_asset("M_AeVertexColour", GEN + "/Materials", unreal.Material, unreal.MaterialFactoryNew())
material.set_editor_property("shading_model", unreal.MaterialShadingModel.MSM_UNLIT)
material.set_editor_property("two_sided", True)
vertex_colour = mel.create_material_expression(material, unreal.MaterialExpressionVertexColor, -700, 0)
if colours_are_srgb:
    decode = mel.create_material_expression(material, unreal.MaterialExpressionCustom, -350, 0)
    decode.set_editor_property("code", "return lerp(c / 12.92, pow((c + 0.055) / 1.055, 2.4), step(0.04045, c));")
    decode.set_editor_property("output_type", unreal.CustomMaterialOutputType.CMOT_FLOAT3)
    entry = unreal.CustomInput()
    entry.set_editor_property("input_name", "c")
    decode.set_editor_property("inputs", [entry])
    mel.connect_material_expressions(vertex_colour, "", decode, "c")
    mel.connect_material_property(decode, "", unreal.MaterialProperty.MP_EMISSIVE_COLOR)
else:
    mel.connect_material_property(vertex_colour, "", unreal.MaterialProperty.MP_EMISSIVE_COLOR)
mel.recompile_material(material)
for mesh in meshes.values():
    mesh.set_material(0, material)
checks.finish_compiling()
errors = list(checks.material_errors(material))
check(len(errors) == 0, "M_AeVertexColour did not compile: " + "; ".join(str(e) for e in errors)[:400])
for name, mesh in meshes.items():
    check(mesh.get_material(0) == material, "mesh %s does not use M_AeVertexColour" % name)

# ---- the level
level_path = "%s/L_%s" % (GEN, game)
check(levels.new_level(level_path), "could not make the level " + level_path)
for g in groups:
    mesh = meshes.get(g["name"])
    if mesh:
        a = actors.spawn_actor_from_object(mesh, unreal.Vector(0, 0, 0))
        a.set_actor_label(g["name"])
        a.set_folder_path("World")
boxes = manifest["collision"]["boxes"]
for i, b in enumerate(boxes):
    height = max(b["height"], BOX_MIN_HEIGHT)
    centre = ((b["x0"] + b["x1"]) * 0.5, height * 0.5, (b["z0"] + b["z1"]) * 0.5)
    a = actors.spawn_actor_from_class(unreal.AethyrCollisionBox, to_unreal(centre))
    a.set_actor_label("box_%02d" % i)
    a.set_folder_path("Collision")
    a.set_editor_property("rules_height", b["height"])
    a.box.set_box_extent(unreal.Vector((b["x1"] - b["x0"]) * 0.5 * UNITS, (b["z1"] - b["z0"]) * 0.5 * UNITS, height * 0.5 * UNITS))
placements = manifest["placements"]
for p in placements:
    a = actors.spawn_actor_from_class(unreal.AethyrMarker, to_unreal(p["at"]))
    a.set_actor_label(p["name"])
    a.set_folder_path("Markers")
    a.set_editor_property("kind", p["kind"])
    a.set_editor_property("marker_name", p["name"])

# ---- the manifest asset and the fonts
factory = unreal.DataAssetFactory()
factory.set_editor_property("data_asset_class", unreal.AethyrManifest)
keep = tools.create_asset("DA_%sManifest" % game, GEN, unreal.AethyrManifest, factory)
if check(keep is not None, "could not make the manifest asset"):
    keep.set_editor_property("game", manifest["game"]["id"])
    keep.set_editor_property("world_sha256", manifest["world"]["sha256"])
    keep.set_editor_property("meshes", len(groups))
    keep.set_editor_property("triangles", manifest["world"]["triangles"])
    keep.set_editor_property("tuning", {k: float(v) for k, v in manifest["tuning"].items()})
font_tasks = []
for rel in manifest.get("fonts", []):
    t = unreal.AssetImportTask()
    t.filename = os.path.join(BUNDLE, rel)
    t.destination_path = GEN + "/Fonts"
    t.automated = True
    t.replace_existing = True
    t.save = False
    font_tasks.append(t)
if font_tasks:
    tools.import_asset_tasks(font_tasks)

assets.save_directory(GEN, only_if_is_dirty=False, recursive=True)
check(levels.save_current_level(), "could not save the level")

# ---- what is there now, against the manifest
world = unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem).get_editor_world()
rows = []
for a in actors.get_all_level_actors():
    loc = a.get_actor_location()
    rows.append([a.get_class().get_name(), a.get_actor_label(), round(loc.x, 2), round(loc.y, 2), round(loc.z, 2)])
rows.sort()
count = lambda cls: sum(1 for r in rows if r[0] == cls)
check(count("StaticMeshActor") == len(groups), "the level has %d mesh actors, the manifest lists %d meshes" % (count("StaticMeshActor"), len(groups)))
check(count("AethyrCollisionBox") == len(boxes), "the level has %d collision boxes, the manifest lists %d" % (count("AethyrCollisionBox"), len(boxes)))
check(count("AethyrMarker") == len(placements), "the level has %d markers, the manifest lists %d" % (count("AethyrMarker"), len(placements)))
asset_rows = sorted([str(d.asset_class_path.asset_name), str(d.package_name)] for d in registry.get_assets_by_path(GEN, recursive=True))
fonts = [r for r in asset_rows if r[0] in ("Font", "FontFace")]
check(len([r for r in fonts if r[0] == "FontFace"]) == len(manifest.get("fonts", [])), "imported %d font faces, the manifest lists %d" % (len([r for r in fonts if r[0] == "FontFace"]), len(manifest.get("fonts", []))))
keep_loaded = assets.load_asset("%s/DA_%sManifest" % (GEN, game))
check(keep_loaded is not None and len(keep_loaded.get_editor_property("tuning")) == len(manifest["tuning"]), "the manifest asset does not hold every tuning number")
report["counts"] = {"meshes": len(meshes), "triangles": triangles, "collision_boxes": count("AethyrCollisionBox"), "markers": count("AethyrMarker"), "font_faces": len([r for r in fonts if r[0] == "FontFace"]), "tuning": len(manifest["tuning"])}
state = json.dumps({"actors": rows, "assets": asset_rows, "meshes": mesh_rows}, sort_keys=True)
report["state_sha256"] = hashlib.sha256(state.encode()).hexdigest()

# ---- the reference shots: the same cameras, rendered here
shots = manifest["shots"]
w, h = shots["width"], shots["height"]
target = unreal.RenderingLibrary.create_render_target2d(world, w, h, unreal.TextureRenderTargetFormat.RTF_RGBA16F)
camera = actors.spawn_actor_from_class(unreal.SceneCapture2D, unreal.Vector(0, 0, 0))
capture = camera.capture_component2d
capture.set_editor_property("texture_target", target)
capture.set_editor_property("capture_source", unreal.SceneCaptureSource.SCS_SCENE_COLOR_HDR)
capture.set_editor_property("capture_every_frame", False)
capture.set_editor_property("capture_on_movement", False)
flags = []
for flag in ("AntiAliasing", "TemporalAA", "Fog", "Atmosphere", "Bloom", "EyeAdaptation", "MotionBlur", "AmbientOcclusion", "Lighting", "SkyLighting", "DynamicShadows"):
    setting = unreal.EngineShowFlagsSetting()
    setting.set_editor_property("show_flag_name", flag)
    setting.set_editor_property("enabled", False)
    flags.append(setting)
capture.set_editor_property("show_flag_settings", flags)
shot_rows = []
for s in shots["list"]:
    eye, at = to_unreal(s["eye"]), to_unreal(s["at"])
    camera.set_actor_location_and_rotation(eye, unreal.MathLibrary.find_look_at_rotation(eye, at), False, False)
    capture.set_editor_property("fov_angle", math.degrees(2.0 * math.atan(math.tan(s["fov"] * 0.5) * w / h)))
    checks.finish_compiling()
    capture.capture_scene()
    capture.capture_scene()
    pixels = unreal.RenderingLibrary.read_render_target_raw(world, target, False)
    rw, rh, ref = read_png(os.path.join(BUNDLE, s["file"]))
    if not check(rw == w and rh == h and len(pixels) == w * h, "shot %s: sizes differ" % s["name"]):
        continue
    agree = 0
    both = 0
    diff = 0
    ours = bytearray(w * h * 4)
    for i, p in enumerate(pixels):
        covered = p.r > 0.002 or p.g > 0.002 or p.b > 0.002
        r, g, b = srgb8(p.r), srgb8(p.g), srgb8(p.b)
        ours[i * 4:i * 4 + 4] = bytes((r, g, b, 255 if covered else 0))
        ref_covered = ref[i * 4 + 3] != 0
        agree += 1 if covered == ref_covered else 0
        if covered and ref_covered:
            both += 1
            diff += abs(r - ref[i * 4]) + abs(g - ref[i * 4 + 1]) + abs(b - ref[i * 4 + 2])
    silhouette = agree / float(w * h)
    colour = diff / float(max(1, both) * 3)
    shot_rows.append({"name": s["name"], "silhouette_agreement": round(silhouette, 4), "mean_colour_difference": round(colour, 2), "pixels_both_cover": both})
    check(silhouette >= SILHOUETTE_MIN, "shot %s: silhouettes agree on %.2f%% of pixels, need %.0f%%" % (s["name"], silhouette * 100, SILHOUETTE_MIN * 100))
    check(colour <= COLOUR_MAX, "shot %s: mean colour difference %.2f, at most %.1f allowed" % (s["name"], colour, COLOUR_MAX))
    if SHOT_OUT:
        os.makedirs(SHOT_OUT, exist_ok=True)
        write_png(os.path.join(SHOT_OUT, "unreal-" + os.path.basename(s["file"])), w, h, bytes(ours))
report["shots"] = shot_rows
actors.destroy_actor(camera)   # the level was saved before the camera was added

after = content_outside(content_dir, generated_rel)
changed = sorted(k for k in set(before) | set(after) if before.get(k) != after.get(k))
check(len(changed) == 0, "files outside Generated/ changed: " + ", ".join(changed[:8]))
report["outside_generated_changed"] = changed

report["ok"] = len(failures) == 0
with open(REPORT, "w") as f:
    json.dump(report, f, indent=1, sort_keys=True)
    f.write("\n")
unreal.log("AETHYR_IMPORT " + ("ok" if report["ok"] else "FAILED"))
