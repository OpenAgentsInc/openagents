#!/usr/bin/env python3
"""Generate the owned courtyard fixture without external art or libraries."""
import hashlib
import json
from pathlib import Path
import struct
import zlib

OUTPUT = Path(__file__).resolve().parents[2] / "public" / "starter"
OUTPUT.mkdir(parents=True, exist_ok=True)


def write_json(name, value):
    (OUTPUT / name).write_text(json.dumps(value, indent=2) + "\n")


def write_blob(name, data):
    (OUTPUT / name).write_bytes(data)
    return {"url": f"/starter/{name}", "sha256": hashlib.sha256(data).hexdigest(), "byteLength": len(data)}


# Each face has independent normals and UVs. Counterclockwise when seen outside.
faces = [
    ((0, 0, 1), [(-.5, -.5, .5), (.5, -.5, .5), (.5, .5, .5), (-.5, .5, .5)]),
    ((0, 0, -1), [(.5, -.5, -.5), (-.5, -.5, -.5), (-.5, .5, -.5), (.5, .5, -.5)]),
    ((1, 0, 0), [(.5, -.5, .5), (.5, -.5, -.5), (.5, .5, -.5), (.5, .5, .5)]),
    ((-1, 0, 0), [(-.5, -.5, -.5), (-.5, -.5, .5), (-.5, .5, .5), (-.5, .5, -.5)]),
    ((0, 1, 0), [(-.5, .5, .5), (.5, .5, .5), (.5, .5, -.5), (-.5, .5, -.5)]),
    ((0, -1, 0), [(-.5, -.5, -.5), (.5, -.5, -.5), (.5, -.5, .5), (-.5, -.5, .5)]),
]
vertices = []
indices = []
for normal, positions in faces:
    tangent = [positions[1][i] - positions[0][i] for i in range(3)]
    for xyz, uv in zip(positions, [(0, 0), (1, 0), (1, 1), (0, 1)]):
        vertices.extend([*xyz, *normal, *uv, *tangent, 1])
    start = len(vertices) // 12 - 4
    indices.extend(start + index for index in [0, 1, 2, 0, 2, 3])
write_blob("vertices.bin", struct.pack("<" + "f" * len(vertices), *vertices))
write_blob("indices.bin", struct.pack("<" + "I" * len(indices), *indices))
write_blob("colors.bin", bytes([255] * (len(vertices) // 12 * 4)))


def material(name, color):
    return {"id": name, "name": name, "baseColorFactor": [*color, 1], "emissiveFactor": [0, 0, 0],
            "roughness": .8, "metallic": 0, "normalScale": 1, "uvScale": [1, 1], "alphaMode": "OPAQUE",
            "alphaCutoff": .5, "doubleSided": False, "packedChannels": "metallic-r-roughness-g-occlusion-b"}


materials = [material("courtyard-stone", [.44, .49, .43]), material("warm-plaster", [.65, .48, .32]),
             material("brass-key", [1, .62, .05]), material("green-exit", [.12, .7, .3]),
             material("wood", [.3, .16, .07])]
# Owned colorful mural: deterministic pixel blocks supply a deferred texture fixture.
def png_chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)


pixels = []
seed = 0xA37F0123
for y in range(128):
    row = []
    for x in range(128):
        seed = (1664525 * seed + 1013904223) & 0xffffffff
        row.append(bytes([(seed >> 24) & 255, (seed >> 16) & 255, (seed >> 8) & 255]))
    pixels.append(row)
raw = bytearray()
for y in range(512):
    raw.append(0)
    for x in range(512):
        raw.extend(pixels[y // 4][x // 4])
png = (b'\x89PNG\r\n\x1a\n' + png_chunk(b'IHDR', struct.pack('>IIBBBBB', 512, 512, 8, 2, 0, 0, 0))
       + png_chunk(b'IDAT', zlib.compress(bytes(raw), 6)) + png_chunk(b'IEND', b''))
write_blob('mural.png', png)
materials.append({**material('owned-mural', [1, 1, 1]), 'baseColorUrl': '/starter/mural.png'})
instances = []


def box(name, center, size, mat=0):
    x, y, z = center
    sx, sy, sz = size
    instances.append({"mesh": 0, "model": [sx, 0, 0, 0, 0, sy, 0, 0, 0, 0, sz, 0, x, y, z, 1],
                      "materialOverrides": [mat], "actor": name, "component": name, "sourceClass": "owned-starter-box"})


box("far-wing-mural", [5, 1, -5], [.2, 2, 2], 5)
box("courtyard-floor", [0, -.15, 0], [12, .3, 14])
box("west-wall", [-6, 1.5, 0], [.25, 3, 14], 1)
box("east-wall", [6, 1.5, 0], [.25, 3, 14], 1)
box("south-wall", [0, 1.5, 7], [12, 3, .25], 1)
box("north-left-wall", [-3.5, 1.5, -7], [5, 3, .25], 1)
box("north-right-wall", [4.5, 1.5, -7], [3, 3, .25], 1)
box("north-door-lintel", [1, 2.65, -7], [4, .7, .25], 1)
# Furniture gives the controller obstacles and the scene depth without hiding the objective.
box("bench-seat", [-4.5, .5, -2], [1.8, .2, .6], 4)
box("bench-left-leg", [-5.1, .2, -2], [.18, .4, .5], 4)
box("bench-right-leg", [-3.9, .2, -2], [.18, .4, .5], 4)
box("key-plinth", [-2, .3, 1], [.8, .6, .8], 0)
box("exit-left-post", [1, 1, -3], [.18, 2, .3], 3)
box("exit-right-post", [3, 1, -3], [.18, 2, .3], 3)
box("exit-lintel", [2, 2, -3], [2.2, .2, .3], 3)
scene = {"schema": "aethyrforge.private-scene.v1", "scenario": "day", "vertices": "/starter/vertices.bin",
         "indices": "/starter/indices.bin", "vertexColors": "/starter/colors.bin", "vertexStride": 48,
         "vertexCount": len(vertices) // 12, "indexCount": len(indices), "materials": materials,
         "meshes": [{"id": "owned-cube", "name": "Owned unit cube", "primitives": [{"firstIndex": 0,
                     "indexCount": 36, "baseVertex": 0, "material": 0, "materialSlot": 0}],
                     "bounds": {"min": [-.5, -.5, -.5], "max": [.5, .5, .5]}}], "instances": instances,
         "batches": [{"mesh": 0, "materialOverrides": instance["materialOverrides"], "firstInstance": i, "instanceCount": 1}
                     for i, instance in enumerate(instances)],
         "bounds": {"min": [-6.125, -.3, -7.125], "max": [6.125, 3, 7.125]},
         "spawn": {"position": [0, 1.62, 4], "target": [0, 1.62, -1], "source": "owned-starter"},
         "source": {"map": "owned-courtyard", "placements": len(instances), "actorCensus": {"owned-box": len(instances)}},
         "diagnostics": {"warnings": [], "skippedPlacements": 0}, "catalog": "/starter/catalog.json",
         "characterURL": "/starter/character.json"}
# Distinct material bindings reuse the same cube bytes for authored game markers.
for name, mat in [("owned-key", 2), ("owned-exit", 3)]:
    scene["meshes"].append({"id": name, "name": name, "primitives": [{"firstIndex": 0, "indexCount": 36,
                            "baseVertex": 0, "material": mat, "materialSlot": 0}],
                            "bounds": {"min": [-.5, -.5, -.5], "max": [.5, .5, .5]}})
write_json("scene.json", scene)
write_json("scene-night.json", {**scene, "scenario": "night"})
colliders = []
for instance in instances:
    name = instance["actor"]
    if "wall" not in name and not name.startswith("bench-") and name not in ["key-plinth", "north-door-lintel"]:
        continue
    m = instance["model"]
    center, size = m[12:15], [m[0], m[5], m[10]]
    colliders.append({"id": name, "min": [center[k] - size[k] / 2 for k in range(3)],
                      "max": [center[k] + size[k] / 2 for k in range(3)]})
write_json("colliders.json", colliders)
assets = []
for name, aliases, dimensions in [("key", ["brass key"], [.45, .45, .45]), ("exit", ["green exit"], [1.8, 2, .25]),
                                  ("barrel", ["crate"], [.8, .9, .8]), ("bench", ["seat"], [1.8, .8, .6])]:
    assets.append({"id": name, "name": name.title(), "aliases": aliases, "mesh": 1 if name == "key" else 2 if name == "exit" else 0, "dimensions": dimensions,
                   "bounds": {"min": [-.5, -.5, -.5], "max": [.5, .5, .5]}, "kind": "prop",
                   "materialProfile": "gltf-pbr", "wallParts": []})
write_json("catalog.json", {"schema": "aethyrforge.private-catalog.v1", "assets": assets,
                            "source": "owned-procedural-fixture", "limitations": ["Box proxies; no purchased assets or texture maps."]})
write_json("game.json", {"schema": "aethyrforge.starter-game.v1", "id": "town-key-and-exit", "title": "The courtyard key",
                         "spawn": [0, 0, 4], "key": {"id": "starter-key", "position": [-2, 0, 1], "radius": 1.8},
                         "exit": {"id": "starter-exit", "position": [2, 0, -3], "radius": 1.8},
                         "requiredCapabilities": ["character.controller", "camera.first-third-person", "interaction.proximity",
                                                  "objective.key-exit", "game.restart", "environment.sky"]})

# Small owned block avatar: one joint, flat material, and deterministic stride/bob clips.
char_vertices = []
char_indices = []
for center, size in [([0, .8, 0], [.45, .8, .3]), ([0, 1.4, 0], [.35, .35, .35]),
                     ([-.13, .2, 0], [.18, .4, .22]), ([.13, .2, 0], [.18, .4, .22]),
                     ([-.33, .8, 0], [.17, .7, .22]), ([.33, .8, 0], [.17, .7, .22])]:
    base = len(char_vertices) // 16
    for i in range(0, len(vertices), 12):
        v = vertices[i:i + 12]
        xyz = [v[k] * size[k] + center[k] for k in range(3)]
        char_vertices.extend([*xyz, *v[3:8], 0, 0, 0, 0, 1, 0, 0, 0])
    char_indices.extend(base + index for index in indices)
cv = write_blob("character-vertices.bin", struct.pack("<" + "f" * len(char_vertices), *char_vertices))
ci = write_blob("character-indices.bin", struct.pack("<" + "I" * len(char_indices), *char_indices))
identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
clips = []
for name in ["idle", "walk", "run", "jump", "backpedal", "strafe_left", "strafe_right"]:
    bob = .025 if name == "idle" else .06 if name != "jump" else .1
    clips.append({"name": name, "duration": 1, "distance": 0 if name in ["idle", "jump"] else 1.3,
                  "tracks": [{"joint": 0, "translation": [{"time": 0, "value": [0, 0, 0]},
                                                           {"time": .5, "value": [0, bob, 0]},
                                                           {"time": 1, "value": [0, 0, 0]}], "rotation": [], "scale": []}]})
write_json("character.json", {"schema": "aethyrforge.character.v1", "name": "Owned courtyard explorer",
                              "geometry": {"vertices": cv, "indices": ci, "vertexCount": len(char_vertices) // 16,
                                           "indexCount": len(char_indices)},
                              "joints": [{"parent": -1, "translation": [0, 0, 0], "rotation": [0, 0, 0, 1],
                                          "scale": [1, 1, 1], "inverseBind": identity}], "clips": clips,
                              "materials": [{"name": "Explorer blue", "color": [.15, .4, .8, 1], "alphaCutoff": 0,
                                             "doubleSided": False}], "draws": [{"material": 0, "firstIndex": 0,
                                                                               "indexCount": len(char_indices)}]})
