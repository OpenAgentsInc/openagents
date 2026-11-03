"""Writes room-catalog.c: Everglade's pack catalog (apps/openagents-web/realms/everglade/assets/everglade.pack,
its kind-1 entry; verse/tools/pack_realm.py) as C bytes, for the room server's world (room-world.c), whose town
breaks with the same pieces' bounds the page's does.
    python3 coax/web/room/room-catalog.py PACK OUT.c"""
import struct, sys
pack, out = sys.argv[1], sys.argv[2]
b = open(pack, "rb").read()
assert b[:4] == b"VPK1", "not a pack"
n, table = struct.unpack_from("<I", b, 4)[0], struct.unpack_from("<I", b, 16)[0]
cat = None
for i in range(n):
    kind, _, _, off, size = struct.unpack_from("<BBHII", b, table + 48 * i)
    if kind == 1: cat = b[off:off + size]; break
assert cat is not None, "no catalog"
with open(out, "w") as f:
    f.write("/* written by coax/web/room/room-catalog.py from the Everglade pack: its catalog */\n")
    f.write("const int room_catalog_n = %d;\nconst unsigned char room_catalog[%d] = {\n" % (len(cat), len(cat) + 1))
    for i in range(0, len(cat), 24): f.write("  " + ",".join(str(c) for c in cat[i:i + 24]) + ",\n")
    f.write("  0};\n")
