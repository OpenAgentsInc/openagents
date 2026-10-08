#!/usr/bin/env python3
"""Compose native crops of the clear destruction originals for visual review."""

import hashlib
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

root = Path(__file__).resolve().parent
evidence = json.loads((root / "verification.json").read_text())
source = evidence["destruction_capture"]["source"]
rectangle = [620, 330, 1340, 1050]
names = ["destruction-pristine.png", "destruction-repaired-noon.png", "destruction-repaired-night.png", "destruction-restored.png"]
font = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 18)
board = Image.new("RGBA", (1440, 1536), (30, 30, 30, 255))
panels = []
for index, name in enumerate(names):
    file = root / "clear-capture" / name
    x, y = (index % 2) * 720, (index // 2) * 768
    ImageDraw.Draw(board).text((x + 12, y + 12), name + " | native crop", font=font, fill="white")
    pixels = Image.open(file).convert("RGBA").crop(rectangle)
    board.paste(pixels, (x, y + 48))
    assert board.crop((x, y + 48, x + 720, y + 768)).tobytes() == pixels.tobytes()
    panels.append({"file": "clear-capture/" + name, "sha256": hashlib.sha256(file.read_bytes()).hexdigest(), "source_xyxy": rectangle, "destination_xy": [x, y + 48]})
output = root / "clear-camera-crops.png"
board.save(output, compress_level=6)
metadata = {"source": source, "file": output.name, "resolution": list(board.size), "panels_rescaled": False, "source_pixels_verified": True, "panels": panels, "sha256": hashlib.sha256(output.read_bytes()).hexdigest(), "bytes": output.stat().st_size, "visual_acceptance": "pending_review", "limitations": "Smoke covers central rubble contacts. The panels do not establish a comprehensive absence of floating shadows or pristine/R pixel identity."}
(root / "clear-camera-crops.json").write_text(json.dumps(metadata, indent=2) + "\n")
print(json.dumps({"source": source, "native_panels": len(panels), "bytes": output.stat().st_size}))
