# Firm reason: 2 (independent oracle: decodes metaverse-engine/render/exr.coax's files from the OpenEXR layout with Python's zlib).
"""Decode the EXR files that metaverse-engine/tools/exr-sample.coax writes, without loading Coax code.

    python3 metaverse-engine/tests/exr_oracle.py            # builds the samples in a temporary directory
    python3 metaverse-engine/tests/exr_oracle.py DIR        # checks DIR/none.exr, zips.exr and zip.exr
    python3 metaverse-engine/tests/exr_oracle.py --frame FRAME.rgba16f FILE.exr   # a film-frames --exr file

It parses the header, the channel list and the offset table, inflates each ZIP chunk, undoes
OpenEXR's predictor and byte split, and compares every channel's value, bit for bit, with the
sample image computed here from its own formula.
"""
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

W, H = 37, 21
NAMES = ["R", "G", "B", "A", "depth.Z"]


def sample(c, x, y):
    p = y * W + x
    if p == 0:
        return 65504.0
    if p == 1:
        return -(2.0 ** -16)
    if p == 2:
        return 2.0 ** -24
    return ((x * 7 + y * 13 + c * 5) % 61) / 16.0 - 1.5


def read_text(b, at):
    end = b.index(b"\0", at)
    return b[at:end].decode(), end + 1


def decode(path):
    b = Path(path).read_bytes()
    magic, version = struct.unpack_from("<ii", b, 0)
    assert magic == 20000630 and version == 2, "not a single-part scanline EXR"
    at = 8
    attrs = {}
    while b[at] != 0:
        name, at = read_text(b, at)
        kind, at = read_text(b, at)
        (size,) = struct.unpack_from("<i", b, at)
        at += 4
        attrs[name] = (kind, b[at:at + size])
        at += size
    at += 1
    kind, raw = attrs["channels"]
    assert kind == "chlist"
    channels = []
    i = 0
    while raw[i] != 0:
        name, i = read_text(raw, i)
        ptype, _linear, xs, ys = struct.unpack_from("<iB3xii", raw, i)
        assert xs == 1 and ys == 1
        channels.append((name, ptype))
        i += 16
    assert [n for n, _ in channels] == sorted(n for n, _ in channels), "channel list out of order"
    compression = attrs["compression"][1][0]
    x0, y0, x1, y1 = struct.unpack("<iiii", attrs["dataWindow"][1])
    w, h = x1 - x0 + 1, y1 - y0 + 1
    lines = {0: 1, 2: 1, 3: 16}[compression]
    chunks = (h + lines - 1) // lines
    offsets = struct.unpack_from("<%dQ" % chunks, b, at)
    size_of = {1: 2, 2: 4}
    px = sum(size_of[t] for _, t in channels)
    out = {n: [None] * (w * h) for n, _ in channels}
    for k, off in enumerate(offsets):
        y, size = struct.unpack_from("<ii", b, off)
        assert y == k * lines
        count = min(lines, h - y)
        n = count * w * px
        data = b[off + 8:off + 8 + size]
        if size < n:
            t = bytearray(zlib.decompress(data))
            assert len(t) == n
            for j in range(1, n):
                t[j] = (t[j - 1] + t[j] - 128) & 255
            half = (n + 1) // 2
            data = bytes(t[j // 2] if j % 2 == 0 else t[half + j // 2] for j in range(n))
        assert len(data) == n
        at2 = 0
        for line in range(count):
            for name, t in channels:
                fmt = "<%d%s" % (w, "e" if t == 1 else "f")
                vals = struct.unpack_from(fmt, data, at2)
                at2 += w * size_of[t]
                for x in range(w):
                    out[name][(y + line) * w + x] = vals[x]
    return w, h, compression, channels, out


def check(path, compression):
    w, h, comp, channels, out = decode(path)
    assert (w, h, comp) == (W, H, compression), (w, h, comp)
    types = dict(channels)
    assert sorted(types) == sorted(NAMES)
    assert types["depth.Z"] == 2 and all(types[n] == 1 for n in "RGBA")
    for c, name in enumerate(NAMES):
        for p in range(W * H):
            want = sample(c, p % W, p // W)
            got = out[name][p]
            assert struct.pack("<f", got) == struct.pack("<f", want), (path, name, p, got, want)
    return len(channels) * W * H


def check_frame(raw_path, exr_path):
    w, h, comp, channels, out = decode(exr_path)
    raw = Path(raw_path).read_bytes()
    targets = len(raw) // (w * h * 8)
    assert targets * w * h * 8 == len(raw) and len(channels) == 4 * targets
    vals = struct.unpack("<%de" % (len(raw) // 2), raw)
    for t in range(targets):
        prefix = "" if t == targets - 1 else "target%d." % t
        for c, letter in enumerate("RGBA"):
            got = out[prefix + letter]
            for p in range(w * h):
                want = vals[t * w * h * 4 + p * 4 + c]
                assert struct.pack("<e", got[p]) == struct.pack("<e", want), (prefix + letter, p)
    print("exr oracle: %dx%d, %d targets, %d values equal to the frame's halves" % (w, h, targets, targets * 4 * w * h))


def main():
    if len(sys.argv) == 4 and sys.argv[1] == "--frame":
        check_frame(sys.argv[2], sys.argv[3])
        return
    if len(sys.argv) > 1:
        root = Path(sys.argv[1])
    else:
        root = Path(tempfile.mkdtemp(prefix="exr-oracle-"))
        repo = Path(__file__).resolve().parents[2]
        subprocess.run(["coax/web/tool.sh", "metaverse-engine/tools/exr-sample.coax", str(root)], cwd=repo, check=True)
    total = 0
    for name, comp in (("none.exr", 0), ("zips.exr", 2), ("zip.exr", 3)):
        total += check(root / name, comp)
    print("exr oracle: 3 files, %d values decoded and equal" % total)


if __name__ == "__main__":
    main()
