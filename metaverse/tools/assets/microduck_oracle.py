# Firm reason: 2 (independent Python/struct/XML oracle; never shipped or run upstream code).
"""Check a converted RBA1 against independently read pinned source data.

python3 metaverse/tools/assets/microduck_oracle.py SOURCE.glb components.json
    robot_allcollisions.xml microduck.rba microduck.pack
"""
import hashlib
import json
import math
from pathlib import Path
import struct
import sys
import xml.etree.ElementTree as XML


def check(glb_path, components_path, xml_path, record_path, pack_path):
    glb, metadata, topology, record, pack = (Path(p).read_bytes() for p in
        (glb_path, components_path, xml_path, record_path, pack_path))
    assert glb[:4] == b"glTF"
    size, tag = struct.unpack_from("<II", glb, 12)
    assert tag == 0x4E4F534A
    scene = json.loads(glb[20:20 + size])
    source_binary = glb[28 + size:]
    components = json.loads(metadata)
    magic, length, nj, np, nv, ni, nn = struct.unpack_from("<4s6I", record)
    assert (magic, length, nj, np, nn) == (b"RBA1", len(record), 14, 70, 70)
    assert record[28:124] == b"".join(hashlib.sha256(x).digest() for x in (glb, metadata, topology))
    assert length == 124 + 168 * nj + 368 * np
    joints = {j["id"]: i for i, j in enumerate(components["joints"])}
    bodies = {}

    def visit(body, owner=-1):
        own = body.find("joint")
        if own is not None:
            index = joints[own.attrib["name"]]
            row = 124 + index * 168
            name = record[row:row + 128].split(b"\0")[0].decode()
            parent, *values = struct.unpack_from("<i9f", record, row + 128)
            declared = components["joints"][index]
            assert name == own.attrib["name"] and parent == owner
            assert all(math.isclose(a, b, abs_tol=2e-7) for a, b in zip(values[:6], declared["axis"] + declared["pivot"]))
            limits = list(map(float, own.attrib["range"].split()))
            assert all(math.isclose(a, b, abs_tol=2e-7) for a, b in zip(values[6:], limits + [declared["stand_radians"]]))
            owner = index
        bodies[body.attrib["name"]] = owner
        for child in body.findall("body"):
            visit(child, owner)

    visit(XML.fromstring(topology).find("worldbody/body"))
    entries = struct.unpack_from("<I", pack, 4)[0]
    table = struct.unpack_from("<I", pack, 16)[0]
    positions = next(struct.unpack_from("<II", pack, table + k * 48 + 4)
                     for k in range(entries) if pack[table + k * 48] == 2)[0]
    first_vertex = first_index = 0
    for i, (node, part) in enumerate(zip(scene["nodes"], components["parts"])):
        row = 124 + nj * 168 + i * 368
        name = record[row:row + 128].split(b"\0")[0].decode()
        label = record[row + 128:row + 256].split(b"\0")[0].decode()
        owner, node_id, parent, fv, count, fi, indices = struct.unpack_from("<7i", record, row + 256)
        assert name == node["name"] == part["id"]
        assert label == part["label_en"].replace("·", "/")
        assert (owner, node_id, parent, fv, fi) == (bodies[part["source_body"]], i, -1, first_vertex, first_index)
        mesh = scene["meshes"][node["mesh"]]
        assert count == sum(scene["accessors"][p["attributes"]["POSITION"]]["count"] for p in mesh["primitives"])
        assert indices == sum(scene["accessors"][p["indices"]]["count"] for p in mesh["primitives"])
        bounds = struct.unpack_from("<6f", record, row + 284)
        explosion = struct.unpack_from("<3f", record, row + 308)
        matrix = struct.unpack_from("<12f", record, row + 320)
        assert all(math.isclose(a, b, abs_tol=2e-7) for a, b in zip(explosion, part["explode"]))
        x, y, z, w = node.get("rotation", [0, 0, 0, 1])
        scale = node.get("scale", [1, 1, 1])
        translation = node.get("translation", [0, 0, 0])
        expected = [1-2*(y*y+z*z), 2*(x*y-z*w), 2*(x*z+y*w), translation[0],
                    2*(x*y+z*w), 1-2*(x*x+z*z), 2*(y*z-x*w), translation[1],
                    2*(x*z-y*w), 2*(y*z+x*w), 1-2*(x*x+y*y), translation[2]]
        for k in range(12):
            if k % 4 < 3:
                expected[k] *= scale[k % 4]
        assert all(math.isclose(a, b, abs_tol=2e-6) for a, b in zip(matrix, expected)), name
        # Real converted positions, all vertices, independently accumulated bounds.
        low, high = [math.inf] * 3, [-math.inf] * 3
        for j in range(count):
            point = struct.unpack_from("<3f", pack, positions + (fv + j) * 48)
            for k in range(3):
                low[k] = min(low[k], point[k]); high[k] = max(high[k], point[k])
        assert tuple(low + high) == bounds, name
        first_vertex += count; first_index += indices
    assert (first_vertex, first_index) == (nv, ni) == (861777, 2390406)
    print(f"Microduck oracle: {nj} XML joints, {np} source parts, exact ranges, transforms and {nv} vertex bounds match")


if __name__ == "__main__":
    if len(sys.argv) != 6:
        raise SystemExit(__doc__)
    check(*sys.argv[1:])
