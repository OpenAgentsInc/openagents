# Firm reason: 2 (independent struct/IEEE source-coverage oracle; no production builder imports).
"""Check VWH1 against every original indexed corner and immutable VPK1 payload."""
import hashlib
import pathlib
import struct
import sys

root = pathlib.Path(sys.argv[1])
raw = (root / "world.catalog").read_bytes()
old = (root / "world-reference.catalog").read_bytes()
assert raw[:len(old)] == old
v = (root / "world.vertices").read_bytes()
indices = (root / "world.indices").read_bytes()
rig = 10142528
u32 = lambda data, at: struct.unpack_from("<I", data, at)[0]
nd, vb, ib = struct.unpack_from("<III", raw, rig + 8)
assert len(v) == vb and len(indices) == ib
vertex_word, matrix_word = struct.unpack_from("<II", raw, rig + 44)
h = memoryview(raw)[len(old):]
magic, version, nm, nn, ni, mo, no, io = struct.unpack_from("<8I", h)
assert (magic, version, ni, mo, no, io, len(h)) == (
    0x31485756, 1, nd, 32, 32 + nm * 24, 32 + nm * 24 + nn * 48,
    32 + nm * 24 + nn * 48 + nd * 52)
models = [struct.unpack_from("<6I", h, mo + m * 24) for m in range(nm)]
nodes = [struct.unpack_from("<4I6f2I", h, no + k * 48) for k in range(nn)]
leaves_total = triangles = 0
for first, count, start, length, tree_root, leaves in models:
    assert length == leaves * 2 - 1 and tree_root == start + length - 1
    cursor = first
    for k in range(start, start + leaves):
        f, n, left, right, *tail = nodes[k]
        low, high, height, vertices = tail[:3], tail[3:6], tail[6], tail[7]
        assert f == cursor and n % 3 == 0 and 3 <= n <= 372
        assert left == right == 0xffffffff and height == 0
        ids = struct.unpack_from(f"<{n}I", indices, f * 4)
        assert len(set(ids)) == vertices <= 64
        for index in ids:
            assert 2000000 <= index < 2000000 + (matrix_word - vertex_word) * 4 // 36
            p = struct.unpack_from("<3f", v, vertex_word * 4 + (index - 2000000) * 36)
            assert all(a <= x <= b for x, a, b in zip(p, low, high))
        cursor += n
    assert cursor == first + count
    seen = set()
    pending = [tree_root]
    while pending:
        k = pending.pop()
        assert k not in seen and start <= k < start + length
        seen.add(k)
        f, n, left, right, *tail = nodes[k]
        if left == 0xffffffff:
            continue
        a, b = nodes[left], nodes[right]
        assert start <= left < k and start <= right < k
        assert a[0] == f and b[0] == f + a[1] and a[1] + b[1] == n
        assert tail[6] == 1 + max(a[10], b[10]) and tail[7] == 0
        assert tuple(tail[:3]) == tuple(min(a[j], b[j]) for j in range(4, 7))
        assert tuple(tail[3:6]) == tuple(max(a[j], b[j]) for j in range(7, 10))
        pending.extend((right, left))
    assert len(seen) == length and nodes[tree_root][:2] == (first, count)
    leaves_total += leaves
    triangles += count // 3
for k in range(nd):
    at = io + k * 52
    model = u32(h, at)
    assert model < nm
    assert models[model][:2] == struct.unpack_from("<2I", raw, rig + 64 + 66049 * 4 + k * 32)
    assert h[at + 4:at + 52] == v[matrix_word * 4 + k * 96:matrix_word * 4 + k * 96 + 48]

def entries(path):
    with path.open("rb") as f:
        head = f.read(32)
        rows = f.read(u32(head, 4) * 48)
        result = []
        for at in range(0, len(rows), 48):
            kind, offset, size = struct.unpack_from("<3I", rows, at)
            name = rows[at + 16:at + 48].split(b"\0")[0]
            f.seek(offset)
            digest = hashlib.sha256()
            remaining = size
            while remaining:
                chunk = f.read(min(remaining, 1048576))
                assert chunk
                digest.update(chunk)
                remaining -= len(chunk)
            result.append((kind, name, size, digest.hexdigest()))
        return result

before = entries(root / "valley-world-reference.pack")
after = entries(root / "valley-world.pack")
assert before[1:] == after[1:]
print(f"PASS {nm} models, {leaves_total} leaves, {nn} nodes, {triangles} unique triangles, "
      f"{nd} exact matrix/range bindings; every indexed corner bounded exactly once; "
      f"all {len(after)-1} geometry/texture payloads unchanged")
