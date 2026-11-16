# Firm reason: 6 (Blender's FBX importer and glTF exporter exist only in bpy; there is no C ABI)
# Data step of the character rig import (rig-io.coax, GitHub #13651): a skeletal mesh and its clips
# become glTF files the engine's glTF reader (metaverse-engine/anim/gltf-scene.coax,
# gltf-animation.coax) admits. It moves no joint, bakes no pose and retargets nothing; Coax does.
#   Blender -b --factory-startup -P rig-blender.py -- OUT_DIR BODY.fbx|BODY.glb [ROLE=CLIP.fbx|.glb ...]
# What it writes:
#   OUT_DIR/character.gltf  BODY's one armature and the meshes skinned to it, no animation. A stray
#       unskinned mesh (a Tripo export's Icosphere) is dropped and printed. Each mesh's object
#       transform is baked into its vertices, so every node has unit scale (the reader takes no
#       other); an armature with a scale is refused.
#   OUT_DIR/ROLE.gltf  each CLIP's own armature with its own action, as it is in its file. A clip
#       file's armature rests in the clip's first pose (an FBX without a bind pose), so its action
#       is not moved onto the body's armature: glTF's channels are each joint's whole local pose,
#       which the reader puts on the body's joints of the same names.
# Each file is a separate glTF: no materials, every influence, animations sampled every frame at
# 30 a second. A bone's scale, at rest or animated, a float's rounding from one (0.9999999, within
# 1e-4) is written as no scale; any other scale is refused.
import json
import sys
import bpy


def armature_of(objects, path):
    arms = [o for o in objects if o.type == "ARMATURE"]
    if len(arms) != 1:
        raise SystemExit("rig-blender: %d armatures in %s, expected one" % (len(arms), path))
    arm = arms[0]
    if any(abs(s - 1) > 1e-4 for s in arm.matrix_world.to_scale()):
        raise SystemExit("rig-blender: the armature in %s has a scale; apply it in the source first" % path)
    return arm


def load(path):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.scene.render.fps = 30
    if path.lower().endswith(".fbx"):
        bpy.ops.import_scene.fbx(filepath=path, use_anim=True)
    else:
        bpy.ops.import_scene.gltf(filepath=path)
    return list(bpy.data.objects)


def export(out, animations):
    bpy.ops.export_scene.gltf(
        filepath=out,
        export_format="GLTF_SEPARATE",
        export_materials="NONE",
        export_skins=True,
        export_all_influences=True,
        export_morph=True,
        export_animations=animations,
        export_animation_mode="ACTIONS",
        export_force_sampling=True,
        export_frame_step=1,
    )
    with open(out) as f:
        doc = json.load(f)
    for n in doc.get("nodes", []):
        s = n.get("scale")
        if s is None:
            continue
        if any(abs(v - 1) > 1e-4 for v in s):
            raise SystemExit("rig-blender: node %s has scale %s" % (n.get("name"), s))
        del n["scale"]
    # the same for an animated scale: a channel every key of which is within 1e-4 of one is dropped
    import struct
    binary = None
    for a in doc.get("animations", []):
        kept = []
        for ch in a["channels"]:
            if ch["target"]["path"] != "scale":
                kept.append(ch)
                continue
            if binary is None:
                with open(out.rsplit("/", 1)[0] + "/" + doc["buffers"][0]["uri"], "rb") as f:
                    binary = f.read()
            acc = doc["accessors"][a["samplers"][ch["sampler"]]["output"]]
            view = doc["bufferViews"][acc["bufferView"]]
            at = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
            values = struct.unpack_from("<%df" % (acc["count"] * 3), binary, at)
            if any(abs(v - 1) > 1e-4 for v in values):
                raise SystemExit("rig-blender: %s scales node %d" % (a["name"], ch["target"]["node"]))
        a["channels"] = kept
    with open(out, "w") as f:
        json.dump(doc, f)


def main():
    argv = sys.argv[sys.argv.index("--") + 1:]
    out_dir, body, clips = argv[0], argv[1], argv[2:]
    objects = load(body)
    arm = armature_of(objects, body)
    for o in objects:
        if o.type != "MESH":
            continue
        skinned = any(m.type == "ARMATURE" and m.object == arm for m in o.modifiers) and len(o.vertex_groups) > 0
        if not skinned:
            print("rig-blender: dropped unskinned mesh", o.name)
            bpy.data.objects.remove(o, do_unlink=True)
            continue
        world = o.matrix_world.copy()
        o.parent = None
        o.data.transform(world)
        o.matrix_world.identity()
        o.parent = arm
        o.matrix_parent_inverse = arm.matrix_world.inverted()
    for o in list(bpy.data.objects):
        if o.type not in ("ARMATURE", "MESH"):
            bpy.data.objects.remove(o, do_unlink=True)
    if arm.animation_data:
        arm.animation_data.action = None
    export(out_dir + "/character.gltf", False)
    for spec in clips:
        role, path = spec.split("=", 1)
        objects = load(path)
        src = armature_of(objects, path)
        action = src.animation_data.action if src.animation_data else None
        if action is None:
            raise SystemExit("rig-blender: no action in " + path)
        for o in objects:
            if o is not src:
                bpy.data.objects.remove(o, do_unlink=True)
        for a in list(bpy.data.actions):
            if a is not action:
                bpy.data.actions.remove(a)
        action.name = role
        print("rig-blender: clip", role, tuple(action.frame_range))
        export(out_dir + "/" + role + ".gltf", True)


if __name__ == "__main__":
    main()
