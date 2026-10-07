"""Converts one Modular Medieval Town piece into a private pack build.

The proof of the export path in docs/verse/everglade-medieval-refactor.md:
a kit piece, exported by `scripts/unreal/medieval_town_export.py`, becomes
the six files `compile::private` reads (`guest.gltf`, `guest.bin`,
`guest.png`, and the `guest_far` level), so it loads in Everglade through
the existing private-asset path. Today that path draws characters only, so
the piece is rigged as a still figure: one root joint that every vertex
follows and an `idle` clip that holds still. A kit kind of private pack,
with static models, is phase P2 of the plan.

Run headless:

    Blender -b --factory-startup --python scripts/blender/medieval_piece.py -- \
        EXPORT_DIR MESH OUT_DIR [--near N] [--far N] [--edge PX] [--lift M] [--turn DEG]

MESH is a path in the export, such as `Meshes/Props/SM_fountain_01`.
`--lift` raises the piece, meters, such as a roof onto the top of a wall;
`--turn` turns it about the vertical, degrees. The piece keeps the kit's
pivot, so pieces placed at the same spot meet as they do in the kit.

Each level is the piece (decimated to `--near` or `--far` triangles when it
has more), unwrapped afresh, with its tiled base colors baked into one
image, `--edge` pixels for the near level and half that for the far. The
output is licensed content; write it outside the repository.
"""

import json
import math
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402
import medieval_kit  # noqa: E402
import private_character as character  # noqa: E402


def options():
    a = kit.args()
    if len(a) < 3:
        sys.exit(__doc__)
    o = {"export": a[0], "mesh": a[1], "out": a[2], "near": 20000, "far": 4000, "edge": 1024,
         "lift": 0.0, "turn": 0.0}
    rest = a[3:]
    while rest:
        key = rest[0][2:]
        if key not in o or len(rest) < 2:
            sys.exit(f"unknown or incomplete option: {rest[0]}")
        o[key] = float(rest[1])
        rest = rest[2:]
    o["near"], o["far"], o["edge"] = int(o["near"]), int(o["far"]), int(o["edge"])
    return o


def still_rig(meshes):
    """One root joint at the pivot, every vertex bound to it, and an idle
    clip that holds still: what the private path's forms need."""
    arm = kit.armature("piece_rig", [("root", None, (0.0, 0.0, 0.0), (0.0, 0.0, 0.25))])
    for mesh in meshes:
        kit.bind(mesh, "root")
        kit.skin(arm, mesh)
    kit.action(arm, "idle", 2, lambda t: {"root": (0.0, 0.0, 0.0)}, cyclic=True, step=24)
    return arm


def main():
    o = options()
    os.makedirs(o["out"], exist_ok=True)
    export = medieval_kit.Export(o["export"])
    objs = export.load(o["mesh"])
    source_triangles = kit.triangles(objs)
    piece = kit.join("piece_source", objs)
    # Exports split vertices along UV seams; weld them so the piece
    # decimates and unwraps as one surface.
    character.select(piece)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.remove_doubles(threshold=1e-5)
    bpy.ops.object.mode_set(mode="OBJECT")
    if o["turn"] or o["lift"]:
        piece.rotation_euler.z = math.radians(o["turn"])
        piece.location.z = o["lift"]
        character.select(piece)
        bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    lo, hi = medieval_kit.bounds([piece])
    near, near_triangles = character.level(piece, "guest", o["near"], o["edge"])
    far, far_triangles = character.level(piece, "guest_far", o["far"], o["edge"] // 2)
    bpy.data.objects.remove(piece, do_unlink=True)
    arm = still_rig([near, far])
    far.hide_set(True)
    character.export(arm, near, os.path.join(o["out"], "guest.gltf"))
    far.hide_set(False)
    near.hide_set(True)
    character.export(arm, far, os.path.join(o["out"], "guest_far.gltf"))
    near.hide_set(False)
    far.hide_render = True
    arm.hide_render = True
    medieval_kit.thumbnail([near], os.path.join(o["out"], "preview.png"), 512)
    report = {
        "blender": bpy.app.version_string,
        "mesh": o["mesh"],
        "source_triangles": source_triangles,
        "near_triangles": near_triangles,
        "far_triangles": far_triangles,
        "texture_edge": o["edge"],
        "lift_m": o["lift"],
        "turn_degrees": o["turn"],
        "bounds_m": {"min": list(lo), "max": list(hi)},
        "height_m": hi.z - lo.z,
        "rig": "still: root",
        "clips": ["idle"],
    }
    with open(os.path.join(o["out"], "report.json"), "w") as f:
        json.dump(report, f, indent=2, sort_keys=True)
    print("MEDIEVAL_PIECE " + json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
