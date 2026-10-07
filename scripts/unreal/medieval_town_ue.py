"""Exports the Modular Medieval Town pack from inside Unreal Editor.

Unreal runs this file through the Python commandlet:

    UnrealEditor PROJECT -run=pythonscript -script=medieval_town_ue.py \
        -unattended -nullrhi

Don't run it by hand; `scripts/unreal/medieval_town_export.py` builds a
private scratch project, sets the environment below, and runs it. The script
reads assets and writes files only under `MEDIEVAL_EXPORT_OUT`. It never
saves a package, so the project's content stays as it was.

Environment:

- `MEDIEVAL_EXPORT_OUT`: the private output directory (required).
- `MEDIEVAL_EXPORT_ROOT`: the content path to export (default
  `/Game/Medieval_Town`).
- `MEDIEVAL_EXPORT_STEPS`: comma-separated steps from `meshes`, `textures`,
  `materials`, and `maps` (default all four).
- `MEDIEVAL_EXPORT_LIMIT`: export at most this many assets of each kind,
  for a quick check (default no limit).
- `MEDIEVAL_EXPORT_MATCH`: only assets whose path contains this text.

Output, all private and never committed:

- `meshes/<relative path>.glb` and `meshes.json`: geometry (the source
  model) and statistics; `meshes/<relative path>.lod0.glb` too when
  Unreal renders a reduced LOD0.
- `textures/<relative path>.png` and `textures.json`.
- `materials.json`: each material's parent, parameters, and textures.
- `maps/<map>.json`: every placed mesh with its transform.
- `export-log.json`: failures and warnings.
"""

import json
import os
import time

import unreal

OUT = os.environ.get("MEDIEVAL_EXPORT_OUT", "")
ROOT = os.environ.get("MEDIEVAL_EXPORT_ROOT", "/Game/Medieval_Town").rstrip("/")
STEPS = set(
    s.strip()
    for s in os.environ.get("MEDIEVAL_EXPORT_STEPS", "meshes,textures,materials,maps").split(",")
    if s.strip()
)
LIMIT = int(os.environ.get("MEDIEVAL_EXPORT_LIMIT", "0") or 0)
MATCH = os.environ.get("MEDIEVAL_EXPORT_MATCH", "")

LOG = {"failures": [], "warnings": [], "timings": {}}


def log(message):
    unreal.log("MEDIEVAL " + message)


def fail(kind, path, why):
    LOG["failures"].append({"kind": kind, "asset": path, "why": str(why)})
    unreal.log_warning("MEDIEVAL failed %s %s: %s" % (kind, path, why))


def write_json(name, value):
    path = os.path.join(OUT, name)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as handle:
        json.dump(value, handle, indent=1, sort_keys=True)


def relative(object_path):
    """Returns `Meshes/Architecture/SM_Wall` for `/Game/Medieval_Town/Meshes/Architecture/SM_Wall.SM_Wall`."""
    package = object_path.split(".")[0]
    if package.startswith(ROOT + "/"):
        return package[len(ROOT) + 1 :]
    return package.lstrip("/")


def asset_list(class_name):
    registry = unreal.AssetRegistryHelpers.get_asset_registry()
    found = []
    for data in registry.get_assets_by_path(ROOT, recursive=True):
        if str(data.asset_class_path.asset_name) != class_name:
            continue
        path = str(data.package_name) + "." + str(data.asset_name)
        if MATCH and MATCH not in path:
            continue
        found.append(path)
    found.sort()
    if LIMIT:
        found = found[:LIMIT]
    return found


def vec(v):
    return [round(v.x, 4), round(v.y, 4), round(v.z, 4)]


def rotator(r):
    return [round(r.roll, 4), round(r.pitch, 4), round(r.yaw, 4)]


def soft(obj):
    return obj.get_path_name() if obj else None


# --- Meshes -----------------------------------------------------------------


def gltf_options():
    options = unreal.GLTFExportOptions()
    # Centimeters to meters; glTF is Y-up and the exporter converts axes.
    options.set_editor_property("export_uniform_scale", 0.01)
    # Material baking needs a renderer, which -nullrhi lacks. The raw
    # textures and parameters are exported separately instead.
    options.set_editor_property("bake_material_inputs", unreal.GLTFMaterialBakeMode.DISABLED)
    options.set_editor_property("export_source_model", True)
    options.set_editor_property("export_vertex_colors", True)
    options.set_editor_property("default_level_of_detail", 0)
    options.set_editor_property("export_lights", False)
    options.set_editor_property("export_cameras", False)
    options.set_editor_property("export_lightmaps", False)
    options.set_editor_property("include_copyright_notice", False)
    options.set_editor_property("texture_image_format", unreal.GLTFTextureImageFormat.PNG)
    return options


def glb_document(path):
    """The JSON chunk of the binary glTF at `path`, or None."""
    with open(path, "rb") as handle:
        header = handle.read(20)
        if len(header) < 20 or header[:4] != b"glTF":
            return None
        length = int.from_bytes(header[12:16], "little")
        return json.loads(handle.read(length))


def glb_has_mesh(path):
    """Whether the binary glTF at `path` holds at least one mesh."""
    document = glb_document(path)
    return bool(document and document.get("meshes"))


def glb_triangles(path):
    """Triangles over every indexed primitive of the glTF at `path`."""
    document = glb_document(path) or {}
    total = 0
    for mesh in document.get("meshes", []):
        for primitive in mesh["primitives"]:
            if "indices" in primitive:
                total += document["accessors"][primitive["indices"]]["count"] // 3
    return total


def export_meshes():
    started = time.time()
    options = gltf_options()
    records = []
    paths = asset_list("StaticMesh")
    log("meshes: %d" % len(paths))
    for index, path in enumerate(paths):
        mesh = unreal.load_asset(path)
        if mesh is None:
            fail("mesh", path, "did not load")
            continue
        rel = relative(path)
        record = {"asset": path, "path": rel}
        try:
            lods = mesh.get_num_lods()
            record["lods"] = lods
            record["triangles"] = [mesh.get_num_triangles(i) for i in range(lods)]
            record["vertices"] = [mesh.get_num_vertices(i) for i in range(lods)]
            box = mesh.get_bounding_box()
            # Unreal units: centimeters, Z up.
            record["bounds_cm"] = {"min": vec(box.min), "max": vec(box.max)}
            record["sections"] = [mesh.get_num_sections(i) for i in range(lods)]
            slots = []
            for slot in mesh.get_editor_property("static_materials"):
                slots.append(
                    {
                        "slot": str(slot.get_editor_property("material_slot_name")),
                        "material": soft(slot.get_editor_property("material_interface")),
                    }
                )
            record["material_slots"] = slots
            try:
                record["nanite"] = bool(mesh.get_editor_property("nanite_settings").get_editor_property("enabled"))
            except Exception:
                record["nanite"] = None
            # The subsystem is missing under a commandlet; the older
            # library has the same calls.
            subsystem = unreal.get_editor_subsystem(unreal.StaticMeshEditorSubsystem) or unreal.EditorStaticMeshLibrary
            try:
                record["simple_collision"] = subsystem.get_simple_collision_count(mesh)
                record["collision_complex_as_simple"] = str(subsystem.get_collision_complexity(mesh))
            except Exception as error:
                record["simple_collision"] = None
                LOG["warnings"].append({"asset": path, "why": "collision: %s" % error})
        except Exception as error:
            fail("mesh-stats", path, error)
        target = os.path.join(OUT, "meshes", rel + ".glb")
        os.makedirs(os.path.dirname(target), exist_ok=True)
        try:
            result = unreal.GLTFExporter.export_to_gltf(mesh, target, options, set())
            # Unreal's binding returns the out parameter alone or with the
            # boolean, depending on the version.
            ok, messages = result if isinstance(result, tuple) else (True, result)
            record["glb"] = os.path.relpath(target, OUT) if ok and os.path.exists(target) else None
            errors = [str(m) for m in messages.get_editor_property("errors")]
            warnings = [str(m) for m in messages.get_editor_property("warnings")]
            if errors:
                record["export_errors"] = errors
            if warnings:
                record["export_warnings"] = warnings[:8]
            if record["glb"] and not glb_has_mesh(target):
                # A few source models export as an empty scene; the
                # render data still has the mesh.
                fallback = gltf_options()
                fallback.set_editor_property("export_source_model", False)
                unreal.GLTFExporter.export_to_gltf(mesh, target, fallback, set())
                record["glb_from"] = "render-data"
                if not glb_has_mesh(target):
                    record["glb"] = None
                    errors.append("the exported scene has no mesh")
            if record["glb"]:
                record["glb_triangles"] = glb_triangles(target)
                # Many pieces ship a reduced LOD0 (the mesh's reduction
                # settings) beside a denser source. Keep Unreal's reduced
                # level too: a ready-made middle level.
                reduced = record.get("triangles", [0])[0]
                if record.get("glb_from") is None and reduced and record["glb_triangles"] > reduced:
                    render = os.path.join(OUT, "meshes", rel + ".lod0.glb")
                    fallback = gltf_options()
                    fallback.set_editor_property("export_source_model", False)
                    unreal.GLTFExporter.export_to_gltf(mesh, render, fallback, set())
                    if glb_has_mesh(render):
                        record["render_glb"] = os.path.relpath(render, OUT)
                        record["render_glb_triangles"] = glb_triangles(render)
            if not record["glb"]:
                fail("mesh-glb", path, "; ".join(errors) or "exporter returned false")
        except Exception as error:
            record["glb"] = None
            fail("mesh-glb", path, error)
        records.append(record)
        if index % 25 == 0:
            log("mesh %d/%d %s" % (index + 1, len(paths), rel))
    write_json("meshes.json", records)
    LOG["timings"]["meshes"] = round(time.time() - started, 1)


# --- Textures ---------------------------------------------------------------


def export_textures():
    started = time.time()
    records = []
    paths = asset_list("Texture2D")
    log("textures: %d" % len(paths))
    for index, path in enumerate(paths):
        texture = unreal.load_asset(path)
        if texture is None:
            fail("texture", path, "did not load")
            continue
        rel = relative(path)
        record = {"asset": path, "path": rel}
        try:
            record["width"] = texture.blueprint_get_size_x()
            record["height"] = texture.blueprint_get_size_y()
            record["srgb"] = bool(texture.get_editor_property("srgb"))
            record["compression"] = str(texture.get_editor_property("compression_settings"))
            record["lod_group"] = str(texture.get_editor_property("lod_group"))
        except Exception as error:
            LOG["warnings"].append({"asset": path, "why": "texture stats: %s" % error})
        # The PNG exporter asserts on a floating-point source, so no
        # exporter is named: Unreal picks one that supports the texture
        # for the extension, and an HDR source falls back to EXR.
        record["png"] = None
        for extension in ("png", "exr"):
            target = os.path.join(OUT, "textures", rel + "." + extension)
            os.makedirs(os.path.dirname(target), exist_ok=True)
            task = unreal.AssetExportTask()
            task.set_editor_property("object", texture)
            task.set_editor_property("filename", target)
            task.set_editor_property("automated", True)
            task.set_editor_property("prompt", False)
            task.set_editor_property("replace_identical", True)
            try:
                ok = unreal.Exporter.run_asset_export_task(task)
            except Exception as error:
                ok = False
                LOG["warnings"].append({"asset": path, "why": "%s: %s" % (extension, error)})
            if ok and os.path.exists(target):
                record[extension] = os.path.relpath(target, OUT)
                break
        if not record.get("png") and not record.get("exr"):
            fail("texture-png", path, "no exporter wrote it")
        records.append(record)
        if index % 50 == 0:
            log("texture %d/%d %s" % (index + 1, len(paths), rel))
    write_json("textures.json", records)
    LOG["timings"]["textures"] = round(time.time() - started, 1)


# --- Materials --------------------------------------------------------------


def material_record(material):
    lib = unreal.MaterialEditingLibrary
    record = {"asset": material.get_path_name(), "path": relative(material.get_path_name())}
    base = material
    chain = []
    while isinstance(base, unreal.MaterialInstance):
        parent = base.get_editor_property("parent")
        if parent is None:
            break
        chain.append(parent.get_path_name())
        base = parent
    record["parents"] = chain
    record["class"] = material.get_class().get_name()
    if isinstance(base, unreal.Material):
        for name in ("blend_mode", "shading_model", "two_sided"):
            try:
                record[name] = str(base.get_editor_property(name))
            except Exception:
                pass
    scalars, vectors, textures, switches = {}, {}, {}, {}
    if isinstance(material, unreal.MaterialInstance):
        for name in lib.get_scalar_parameter_names(material):
            scalars[str(name)] = lib.get_material_instance_scalar_parameter_value(material, name)
        for name in lib.get_vector_parameter_names(material):
            c = lib.get_material_instance_vector_parameter_value(material, name)
            vectors[str(name)] = [round(c.r, 5), round(c.g, 5), round(c.b, 5), round(c.a, 5)]
        for name in lib.get_texture_parameter_names(material):
            textures[str(name)] = soft(lib.get_material_instance_texture_parameter_value(material, name))
        for name in lib.get_static_switch_parameter_names(material):
            switches[str(name)] = bool(lib.get_material_instance_static_switch_parameter_value(material, name))
        overridden = {
            "scalar": [str(p.parameter_info.name) for p in material.get_editor_property("scalar_parameter_values")],
            "vector": [str(p.parameter_info.name) for p in material.get_editor_property("vector_parameter_values")],
            "texture": [str(p.parameter_info.name) for p in material.get_editor_property("texture_parameter_values")],
        }
        record["overridden"] = overridden
    else:
        for name in lib.get_scalar_parameter_names(material):
            scalars[str(name)] = lib.get_material_default_scalar_parameter_value(material, name)
        for name in lib.get_vector_parameter_names(material):
            c = lib.get_material_default_vector_parameter_value(material, name)
            vectors[str(name)] = [round(c.r, 5), round(c.g, 5), round(c.b, 5), round(c.a, 5)]
        for name in lib.get_texture_parameter_names(material):
            textures[str(name)] = soft(lib.get_material_default_texture_parameter_value(material, name))
        for name in lib.get_static_switch_parameter_names(material):
            switches[str(name)] = bool(lib.get_material_default_static_switch_parameter_value(material, name))
    record["scalars"] = scalars
    record["vectors"] = vectors
    record["textures"] = textures
    record["switches"] = switches
    try:
        record["used_textures"] = sorted(t.get_path_name() for t in lib.get_used_textures(material))
    except Exception:
        record["used_textures"] = []
    return record


def export_materials():
    started = time.time()
    records = []
    paths = asset_list("Material") + asset_list("MaterialInstanceConstant")
    log("materials: %d" % len(paths))
    for path in paths:
        material = unreal.load_asset(path)
        if material is None:
            fail("material", path, "did not load")
            continue
        try:
            records.append(material_record(material))
        except Exception as error:
            fail("material", path, error)
    write_json("materials.json", records)
    LOG["timings"]["materials"] = round(time.time() - started, 1)


# --- Maps -------------------------------------------------------------------


def transform_record(transform):
    rotation = transform.rotation
    return {
        "location_cm": vec(transform.translation),
        "rotation_quat": [round(rotation.x, 6), round(rotation.y, 6), round(rotation.z, 6), round(rotation.w, 6)],
        "rotation_rpy_deg": rotator(rotation.rotator()),
        "scale": vec(transform.scale3d),
    }


def load_world(path):
    """Loads a map as the editor world without the editor's map-check
    window. `EditorLoadingAndSavingUtils.load_map` opens the map-check
    message log after loading, which asserts under a commandlet when the
    check has a warning, so the editor's own `MAP LOAD` command runs
    instead."""
    package = path.split(".")[0]
    filename = unreal.SystemLibrary.convert_to_absolute_path(
        unreal.Paths.project_content_dir() + package[len("/Game/") :] + ".umap"
    )
    unreal.SystemLibrary.execute_console_command(
        None, 'MAP LOAD FILE="%s" TEMPLATE=0 SHOWPROGRESS=0 FEATURELEVEL=4' % filename
    )
    world = unreal.get_editor_subsystem(unreal.UnrealEditorSubsystem).get_editor_world()
    if world is None or world.get_path_name().split(".")[0] != package:
        return None
    return world


def export_map(path):
    world = load_world(path)
    if world is None:
        fail("map", path, "did not load")
        return
    try:
        descs = unreal.WorldPartitionBlueprintLibrary.get_actor_descs()
        if descs:
            unreal.WorldPartitionBlueprintLibrary.load_actors([d.guid for d in descs])
            log("map %s: loaded %d partitioned actors" % (path, len(descs)))
    except Exception:
        pass
    actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem).get_all_level_actors()
    placed, classes = [], {}
    actor_records = []
    for actor in actors:
        cls = actor.get_class().get_name()
        classes[cls] = classes.get(cls, 0) + 1
        if cls.startswith("BP_") or cls == "GroupActor":
            record = {"actor": actor.get_actor_label(), "class": cls}
            record.update(transform_record(actor.get_actor_transform()))
            actor_records.append(record)
        components = actor.get_components_by_class(unreal.StaticMeshComponent)
        for component in components:
            mesh = component.get_editor_property("static_mesh")
            if mesh is None:
                continue
            overrides = [soft(m) for m in component.get_editor_property("override_materials")]
            entry = {
                "actor": actor.get_actor_label(),
                "class": cls,
                "folder": str(actor.get_folder_path()),
                "component": component.get_name(),
                "mesh": mesh.get_path_name(),
                "visible": bool(component.is_visible()),
            }
            if any(overrides):
                entry["override_materials"] = overrides
            if isinstance(component, unreal.InstancedStaticMeshComponent):
                count = component.get_instance_count()
                entry["instances"] = [
                    transform_record(component.get_instance_transform(i, True)) for i in range(count)
                ]
            else:
                entry.update(transform_record(component.get_world_transform()))
            placed.append(entry)
    name = relative(path).replace("/", "__")
    write_json(
        os.path.join("maps", name + ".json"),
        {"map": path, "actor_classes": classes, "actors": actor_records, "placed": placed},
    )
    log("map %s: %d actors, %d mesh placements" % (path, len(actors), len(placed)))


def export_maps():
    started = time.time()
    for path in asset_list("World"):
        try:
            export_map(path)
        except Exception as error:
            fail("map", path, error)
    LOG["timings"]["maps"] = round(time.time() - started, 1)


def main():
    if not OUT:
        raise SystemExit("MEDIEVAL_EXPORT_OUT is not set")
    os.makedirs(OUT, exist_ok=True)
    log("root %s out %s steps %s" % (ROOT, OUT, sorted(STEPS)))
    if "meshes" in STEPS:
        export_meshes()
    if "textures" in STEPS:
        export_textures()
    if "materials" in STEPS:
        export_materials()
    if "maps" in STEPS:
        export_maps()
    write_json("export-log.json", LOG)
    log("done: %d failures" % len(LOG["failures"]))


main()
