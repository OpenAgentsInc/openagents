#!/usr/bin/env python3
"""Lays the Modular Medieval Town thumbnails out as contact sheets.

Reads `catalog.json` from the export and the thumbnails that
`scripts/blender/medieval_kit.py thumbs` rendered, and writes
`contact-<category>.png` for each category and `contact-all.png`, each
thumbnail labeled with its mesh name, LOD0 triangles, and size in meters.
The sheets show licensed content: keep them outside the repository.

Usage: medieval_town_contact_sheet.py EXPORT_DIR THUMBS_DIR OUT_DIR [--size PX]

Needs Pillow.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

LABEL = 34
COLUMNS = 10


def font(size):
    for path in ("/System/Library/Fonts/Menlo.ttc", "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"):
        try:
            return ImageFont.truetype(path, size)
        except OSError:
            continue
    return ImageFont.load_default()


def sheet(title, entries, thumbs, cell):
    rows = (len(entries) + COLUMNS - 1) // COLUMNS
    header = 40
    image = Image.new("RGB", (COLUMNS * cell, header + rows * (cell + LABEL)), (24, 24, 26))
    draw = ImageDraw.Draw(image)
    big, small = font(22), font(11)
    draw.text((10, 8), f"{title}: {len(entries)} meshes", fill=(235, 200, 120), font=big)
    for i, e in enumerate(entries):
        x = (i % COLUMNS) * cell
        y = header + (i // COLUMNS) * (cell + LABEL)
        path = thumbs / f"{e['name']}.png"
        if path.exists():
            thumb = Image.open(path).convert("RGB").resize((cell, cell))
            image.paste(thumb, (x, y))
        else:
            draw.rectangle((x + 4, y + 4, x + cell - 4, y + cell - 4), outline=(160, 60, 60))
            draw.text((x + 10, y + cell // 2), "no thumbnail", fill=(200, 90, 90), font=small)
        name = e["name"].replace("SM_MERGED_StaticMeshActor_UAID_", "MERGED_")
        tri = e["triangles"][0] if e["triangles"] else 0
        size = "x".join(f"{v:.1f}" for v in e["size_m"])
        draw.text((x + 4, y + cell + 2), name[: cell // 7], fill=(225, 225, 225), font=small)
        draw.text((x + 4, y + cell + 17), f"{tri} tris  {size} m", fill=(170, 170, 170), font=small)
    return image


def main():
    args = sys.argv[1:]
    if len(args) < 3:
        sys.exit(__doc__)
    export, thumbs, out = (Path(a).expanduser() for a in args[:3])
    cell = int(args[args.index("--size") + 1]) if "--size" in args else 192
    catalog = json.loads((export / "catalog.json").read_text())
    out.mkdir(parents=True, exist_ok=True)
    groups = defaultdict(list)
    for e in catalog["meshes"]:
        groups[e["category"].replace("/", "-")].append(e)
    written = []
    for name, entries in sorted(groups.items()):
        entries.sort(key=lambda e: e["name"])
        path = out / f"contact-{name}.png"
        sheet(name, entries, thumbs, cell).save(path)
        written.append(path)
    everything = [e for _, v in sorted(groups.items()) for e in v]
    all_path = out / "contact-all.png"
    sheet("Modular Medieval Town", everything, thumbs, 128).save(all_path)
    written.append(all_path)
    print("\n".join(str(p) for p in written))


if __name__ == "__main__":
    main()
