"""Refuses a staged release that carries a private pack (docs/verse/metahuman-support.md).

deploy.sh runs it on the staged site before anything is uploaded:

    python3 coax/web/private-guard.py STAGE_DIR

A MetaHuman pack reaches visitors only through the broker (coax/web/api/verse_packs.c), a session
at a time, never as a file. So the release fails on any file under a private/ directory, and on any
VPK1 pack whose catalog names the MetaHuman licence or a MetaHuman body, whatever it is called.
"""
import os, struct, sys


def private_pack(path):
    with open(path, "rb") as f:
        head = f.read(32)
        if len(head) < 32 or head[:4] != b"VPK1":
            return False
        n, = struct.unpack_from("<I", head, 4)
        table, = struct.unpack_from("<I", head, 16)
        f.seek(table)
        rows = f.read(48 * min(n, 4096))
        for k in range(len(rows) // 48):
            kind = rows[k * 48]
            offset, length = struct.unpack_from("<II", rows, k * 48 + 4)
            if kind != 1:   # the catalog (data)
                continue
            f.seek(offset)
            text = f.read(min(length, 1 << 16))
            if b"licence MetaHuman" in text or text.startswith(b"human metahuman"):
                return True
    return False


def main(stage):
    bad = []
    for root, _, files in os.walk(stage):
        for name in files:
            path = os.path.join(root, name)
            rel = os.path.relpath(path, stage)
            if "private" in rel.split(os.sep)[:-1] or private_pack(path):
                bad.append(rel)
    for rel in bad:
        print(f"private-guard: {rel} is a private pack or under private/; MetaHumans ship only through the broker", file=sys.stderr)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
