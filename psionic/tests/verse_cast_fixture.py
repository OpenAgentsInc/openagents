"""Chapter 150's Verse character slice: the skeleton of Verse's Cast.bin
(parents and bind positions) and the first frame of its idle clip, written
as a Coax fixture.

    python3 -I psionic/tests/verse_cast_fixture.py [--write]

Reads apps/openagents-ios/OpenAgents/Resources/Cast.bin (verse/tools/pack_cast.py
writes it; verse/cast.coax reads it), records its SHA-256, and compares the
fixture it would write with the committed one; --write replaces it. Values
are copied as F32 bit patterns, so the fixture is exact. Tooling only.
"""
import hashlib
import pathlib
import struct
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CAST = ROOT / "apps/openagents-ios/OpenAgents/Resources/Cast.bin"
FIXTURE = ROOT / "psionic/fixtures/verse-cast-skeleton.coax"


def words(values):
    rows = []
    for i in range(0, len(values), 8):
        rows.append("    " + ",".join(values[i:i + 8]) + ",")
    return "\n".join(rows)


def main() -> int:
    if len(sys.argv) > 2 or (len(sys.argv) == 2 and sys.argv[1] != "--write"):
        print("usage: python3 -I psionic/tests/verse_cast_fixture.py [--write]", file=sys.stderr)
        return 2
    b = CAST.read_bytes()
    sha = hashlib.sha256(b).hexdigest()
    magic, version, count = struct.unpack_from("<3I", b, 0)
    if magic != 0x3143524D or version != 1 or count != 14:
        raise SystemExit("refused: not a Cast.bin version 1 with 14 buffers")
    table = {}
    for i in range(count):
        at = 16 + i * 32
        name = b[at:at + 16].split(b"\0")[0].decode()
        table[name] = struct.unpack_from("<4I", b, at + 16)
    off, _size, stride, joints = table["skeleton"]
    assert stride == 16 and joints <= 65
    parents, binds = [], []
    for j in range(joints):
        parents.append(str(struct.unpack_from("<i", b, off + j * 16)[0]))
        binds += ["0x%08x" % v for v in struct.unpack_from("<3I", b, off + j * 16 + 4)]
    coff, _, _, _ = table["clips"]
    first, frames = struct.unpack_from("<2I", b, coff)   # clip 0: idle
    poff, _, pstride, _ = table["poses"]
    idle = []
    for j in range(joints):
        idle += ["0x%08x" % v for v in struct.unpack_from("<7I", b, poff + (first * joints + j) * pstride)]
    text = f"""// Chapter 150's Verse character slice, written by psionic/tests/verse_cast_fixture.py
// from apps/openagents-ios/OpenAgents/Resources/Cast.bin (sha256 {sha}):
// the character's {joints} joints (parent, then bind position x, y, z as F32 bit
// patterns) and frame 0 of its idle clip ({frames} frames; per joint the
// skinning rotation x, y, z, w and translation x, y, z as F32 bit patterns).
MV_CAST_JOINTS :: {joints};
MV_CAST_PARENT :: [{joints}]int.[
{words(parents)}
];
MV_CAST_BIND_BITS :: [{joints * 3}]u32.[
{words(binds)}
];
MV_CAST_IDLE_BITS :: [{joints * 7}]u32.[
{words(idle)}
];
"""
    if len(sys.argv) == 2:
        FIXTURE.write_text(text)
        print(f"wrote {FIXTURE.relative_to(ROOT)}")
        return 0
    if not FIXTURE.exists() or FIXTURE.read_text() != text:
        print(f"{FIXTURE.relative_to(ROOT)} differs from Cast.bin (sha256 {sha})", file=sys.stderr)
        return 1
    print(f"{FIXTURE.relative_to(ROOT)} matches Cast.bin (sha256 {sha})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
