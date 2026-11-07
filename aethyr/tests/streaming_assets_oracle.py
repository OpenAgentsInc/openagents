# Firm reason: 2 (independent oracle for authored corridor geometry, PCM, PNG and cost-manifest bytes).
"""Recompute fixture content and declared costs without loading Coax code.

Reads a generated scene directory only. This checks file/layout arithmetic;
it does not measure platform allocations or qualify a streaming host.
"""
import hashlib
import io
import json
import math
from pathlib import Path
import shutil
import struct
import sys
import tempfile
import wave
import zlib


def digest(data):
    return hashlib.sha256(data).hexdigest()


def words(identity):
    return struct.pack("<8I", *struct.unpack(">8I", bytes.fromhex(identity)))


def png(data):
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    at, compressed, size = 8, bytearray(), None
    while at < len(data):
        n = struct.unpack_from(">I", data, at)[0]
        kind, payload = data[at + 4:at + 8], data[at + 8:at + 8 + n]
        assert len(payload) == n
        assert zlib.crc32(kind + payload) == struct.unpack_from(">I", data, at + 8 + n)[0]
        if kind == b"IHDR":
            width, height, depth, color, comp, filt, interlace = struct.unpack(">IIBBBBB", payload)
            assert (depth, color, comp, filt, interlace) == (8, 6, 0, 0, 0)
            size = width, height
        elif kind == b"IDAT":
            compressed.extend(payload)
        elif kind == b"IEND":
            assert n == 0 and at + 12 == len(data)
        else:
            raise AssertionError(f"unexpected fixture PNG chunk {kind}")
        at += n + 12
    assert at == len(data) and size
    width, height = size
    raw = zlib.decompress(compressed)
    stride = width * 4
    assert len(raw) == height * (stride + 1)
    pixels = bytearray()
    previous = bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        kind = raw[start]
        assert 0 <= kind <= 4
        line = bytearray(raw[start + 1:start + 1 + stride])
        for x in range(stride):
            left = line[x - 4] if x >= 4 else 0
            above, diagonal = previous[x], previous[x - 4] if x >= 4 else 0
            p = left + above - diagonal
            distances = (abs(p - left), abs(p - above), abs(p - diagonal))
            paeth = (left, above, diagonal)[distances.index(min(distances))]
            predictor = (0, left, above, (left + above) // 2, paeth)[kind]
            line[x] = (line[x] + predictor) & 255
        pixels.extend(line)
        previous = line
    return width, height, bytes(pixels), len(compressed) + len(raw)


def pack(data):
    magic, count, size, budget, table, payload, zero0, zero1 = struct.unpack_from("<4s7I", data)
    assert magic == b"VPK1" and count in (4, 5)
    assert size == len(data) <= budget == 16384
    assert table == 32 and payload == 32 + 48 * count and zero0 == zero1 == 0
    at, result = payload, []
    for i in range(count):
        kind, license_id, z0, z1, offset, n, z2, name = struct.unpack_from("<4B3I32s", data, 32 + i * 48)
        assert kind == i + 1 and license_id == (255 if i < 3 else 0)
        assert z0 == z1 == z2 == 0 and offset == at
        assert b"\0" in name and not any(name[name.index(0):])
        end = offset + n
        at = (end + 3) // 4 * 4
        assert at <= len(data) and not any(data[end:at])
        result.append((kind, offset, data[offset:end]))
    assert at == len(data)
    catalog = result[0][2].decode().splitlines()
    assert f"budget {budget}" in catalog and f"bytes {size}" in catalog
    mesh = next(line.split() for line in catalog if line.startswith("M "))
    assert int(mesh[2]) == 0 and int(mesh[3]) == len(result[2][2]) // 4
    vertices = list(struct.iter_unpack("<8f16s", result[1][2]))
    indices = list(struct.iter_unpack("<I", result[2][2]))
    assert all(i[0] < len(vertices) for i in indices)
    lower, upper = list(map(float, mesh[5:8])), list(map(float, mesh[8:11]))
    for vertex in vertices:
        assert all(math.isfinite(x) and abs(x) < 1e6 for x in vertex[:8])
        assert all(lower[k] <= vertex[k] <= upper[k] for k in range(3))
    return result


def inspect(root, report=True):
    scene_bytes = (root / "scene.json").read_bytes()
    scene = json.loads(scene_bytes)
    assert scene["schema"] == "aethyr.streaming-scene.v1"
    assert scene["cells"] == 12 and scene["cell_metres"] == 64
    assert (root / (digest(scene_bytes) + ".json")).read_bytes() == scene_bytes
    sources, containers, packs, info = [], {}, {}, []
    for i, row in enumerate(scene["resources"]):
        assert row["row"] == i
        container = row["container_sha256"]
        raw = i >= 4 and (i - 4) % 5 in (1, 2)
        assert row["raw"] == raw
        file = root / (container + (".raw" if raw else ".vpk"))
        data = containers.setdefault(container, file.read_bytes())
        assert digest(data) == container
        start, n = row["offset"], row["bytes"]
        assert 0 <= start <= len(data) - n and n > 0
        if raw:
            assert start == 0 and n == len(data)
        else:
            entries = packs.setdefault(container, pack(data))
            assert any(start == e[1] and n == len(e[2]) for e in entries) or (
                i == 1 and start == entries[2][1] and n == 24)
        source = data[start:start + n]
        assert digest(source) == row["sha256"]
        sources.append(source)
    assert len(sources) == 64 and len(packs) == 13
    assert struct.unpack("<6I", sources[1]) == tuple(range(6))
    assert struct.unpack("<36I", sources[2]) == tuple(range(36))
    corners = (0, 2, 1, 1, 2, 3)
    for cell in range(12):
        base = 4 + cell * 5
        w, h, pixels, work = png(sources[base])
        assert (w, h) == (1, 1)
        assert pixels == bytes((96 + (cell + 1) * 7, 136 + (cell + 1) * 3, 176 - (cell + 1) * 4, 255))
        assert struct.unpack("<6f", sources[base + 1]) == (0, -1, cell * 64, 64, 0, (cell + 1) * 64)
        assert struct.unpack("<q3d4q", sources[base + 2]) == (cell + 1, 8, 0, cell * 64 + 32, 0, 0, 0, 0)
        with wave.open(io.BytesIO(sources[base + 3])) as sound:
            assert (sound.getnchannels(), sound.getsampwidth(), sound.getframerate(), sound.getnframes()) == (1, 2, 48000, 512)
            samples = struct.unpack("<512h", sound.readframes(512))
        expected = tuple(((i % 128 - 32) if i % 128 <= 64 else (96 - i % 128)) * (32 + cell) for i in range(512))
        assert samples == expected
        for vertex, c in zip(struct.iter_unpack("<8f16s", sources[base + 4]), corners):
            assert vertex[:8] == (c % 2 * 64, 0, cell * 64 + c // 2 * 64, 0, 1, 0, c % 2, c // 2)
            assert vertex[8] == b"\xff\xff\xff\0" + bytes(12)
    for manifest in scene["manifests"]:
        data = (root / (manifest["sha256"] + ".rcm")).read_bytes()
        assert digest(data) == manifest["sha256"]
        assert struct.unpack_from("<4s3I", data) == (b"RCM1", 1, 64, 440) and len(data) == 28208
        model = b"RCM1/positions4-1/indices-1/query-box-1/query-capsule-1/htx-1/apw-1/cell-body-1/vpk-static48-2"
        assert data[16:48] == words(digest(model))
        retained, scratch = 0, 0
        for i, source in enumerate(sources):
            at = 48 + i * 440
            version, fmt, category, backend, zero, side = struct.unpack_from("<I4BI", data, at)
            slot = (i - 4) % 5
            expected_format = (5, 2, 2, 8)[i] if i < 4 else (5, 3, 7, 6, 8)[slot]
            expected_category = (1, 0, 0, 0)[i] if i < 4 else (3, 2, 5, 4, 0)[slot]
            assert (fmt, category, backend, zero) == (expected_format, expected_category, manifest["backend"], 0)
            assert version == (2 if fmt == 8 else 1) and side == (manifest["tier"] + 1 if fmt == 5 else 0)
            assert data[at + 12:at + 44] == words(digest(source))
            decoded = gpu = decode_work = upload_work = actors = voices = width = height = levels = 0
            alignment = 4
            if fmt == 5:
                w, h, pixels, decode_work = png(source)
                width = height = side
                levels = side.bit_length()
                decoded = gpu = sum(4 * (side >> k) ** 2 for k in range(levels))
                elements = decoded // 4
            elif fmt == 8:
                gpu, elements = len(source), len(source) // 48
            elif fmt == 2:
                gpu, elements = len(source), len(source) // 4
            elif fmt == 3:
                decoded, elements = len(source), len(source) // 24
            elif fmt == 7:
                decoded, elements, alignment, actors = len(source), len(source) // 64, 8, 1
            else:
                assert fmt == 6
                decoded, elements, voices = 2048, 512, 1
            if backend:
                upload_work = gpu
            expected = (len(source), decoded, gpu, 168, decode_work, upload_work, alignment, actors, voices, width, height, levels, elements)
            assert struct.unpack_from("<13q", data, at + 44) == expected, (i, expected)
            if scene["converter"].startswith("aethyr.corridor-assets.v2/"):
                deps = [2] if i == 3 else ([i - 1] if i >= 4 and slot == 2 else ([1] if i >= 4 and slot == 4 else []))
            else:
                assert scene["converter"].startswith("aethyr.corridor-assets.v1/")
                deps = [0, 2] if i == 3 else ([0, 1, i - 4] if i >= 4 and slot == 4 else [])
            assert struct.unpack_from("<I", data, at + 148)[0] == len(deps)
            for k in range(8):
                entry = data[at + 152 + k * 36:at + 188 + k * 36]
                assert entry == (struct.pack("<I", deps[k]) + words(digest(sources[deps[k]])) if k < len(deps) else bytes(36))
            retained += len(source) + decoded + gpu + 168
            scratch += decode_work + upload_work
        assert retained == manifest["whole_world_retained_bytes"] > 32768
        assert scratch == manifest["sum_scratch_bytes"]
        info.append({"backend": backend, "tier": manifest["tier"], "retained": retained, "scratch_sum": scratch})
    assert len(info) == 4 and len({(r["backend"], r["tier"]) for r in info}) == 4
    probe = scene["overcap_probe"]
    data = (root / (probe["sha256"] + ".raw")).read_bytes()
    assert digest(data) == probe["sha256"] and len(data) == probe["bytes"]
    w, h, pixels, work = png(data)
    assert (w, h) == (8, 8) and pixels == bytes((180, 172, 128, 255)) * 64
    assert len(data) + 168 + 2 * (256 + 64 + 16 + 4) > 512
    if report:
        print(json.dumps({"scene_sha256": digest(scene_bytes), "packs": len(packs), "resources": len(sources), "manifests": info}, sort_keys=True))


def fault_checks(root):
    # Repair outer identities after each edit: the oracle must independently
    # catch wrong semantics/costs, rather than only finding a stale filename.
    for fault in ("gpu-cost", "body-placement", "audio-sample"):
        with tempfile.TemporaryDirectory(prefix="aethyr-stream-oracle-") as directory:
            copied = Path(directory) / "scene"
            shutil.copytree(root, copied)
            scene = json.loads((copied / "scene.json").read_bytes())
            if fault == "gpu-cost":
                manifest = scene["manifests"][0]
                data = bytearray((copied / (manifest["sha256"] + ".rcm")).read_bytes())
                at = 48 + 3 * 440 + 44 + 2 * 8
                struct.pack_into("<q", data, at, struct.unpack_from("<q", data, at)[0] + 4)
                manifest["sha256"] = digest(data)
                (copied / (manifest["sha256"] + ".rcm")).write_bytes(data)
            else:
                row = scene["resources"][6 if fault == "body-placement" else 7]
                suffix = ".raw" if row["raw"] else ".vpk"
                old = row["container_sha256"]
                data = bytearray((copied / (old + suffix)).read_bytes())
                if fault == "body-placement":
                    struct.pack_into("<d", data, 24, 33)
                else:
                    data[row["offset"] + 44] ^= 1
                new = digest(data)
                (copied / (new + suffix)).write_bytes(data)
                for same in scene["resources"]:
                    if same["container_sha256"] == old:
                        same["container_sha256"] = new
                        same["sha256"] = digest(data[same["offset"]:same["offset"] + same["bytes"]])
            encoded = json.dumps(scene).encode()
            (copied / "scene.json").write_bytes(encoded)
            (copied / (digest(encoded) + ".json")).write_bytes(encoded)
            try:
                inspect(copied, report=False)
            except AssertionError:
                print(f"refused independently rehashed {fault}")
            else:
                raise AssertionError(f"oracle accepted planted {fault}")


if __name__ == "__main__":
    root = Path(sys.argv[1])
    inspect(root)
    if sys.argv[2:] == ["--fault-checks"]:
        fault_checks(root)
