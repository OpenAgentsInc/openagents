#!/usr/bin/env python3
"""Compose native receiver crops without changing the retained originals."""

import hashlib
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

root = Path(__file__).resolve().parent
evidence = json.loads((root / "verification.json").read_text())
source = evidence["receiver_inspection"]["source"]
rectangle = [620, 330, 1340, 1050]
names = ["destruction-pristine-receivers.png", "destruction-0900.png", "destruction-0900-receivers.png", "destruction-restored-receivers.png"]
labels = ["Pristine receiver copy", "Frame 900 ordinary", "Frame 900 receiver copy: same simulation state", "R restored receiver copy"]
font = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 18)
board = Image.new("RGBA", (1440, 1536), (30, 30, 30, 255))
panels = []
for index, (name, label) in enumerate(zip(names, labels)):
    file = root / "receiver-inspection" / name
    x, y = (index % 2) * 720, (index // 2) * 768
    ImageDraw.Draw(board).text((x + 12, y + 12), label, font=font, fill="white")
    pixels = Image.open(file).convert("RGBA").crop(rectangle)
    board.paste(pixels, (x, y + 48))
    assert board.crop((x, y + 48, x + 720, y + 768)).tobytes() == pixels.tobytes()
    panels.append({"file": "receiver-inspection/" + name, "sha256": hashlib.sha256(file.read_bytes()).hexdigest(), "source_xyxy": rectangle, "destination_xy": [x, y + 48]})
output = root / "receiver-camera-crops.png"
board.save(output, compress_level=6)
metadata = {"source": source, "file": output.name, "resolution": list(board.size), "panels_rescaled": False, "source_pixels_verified": True, "panels": panels, "sha256": hashlib.sha256(output.read_bytes()).hexdigest(), "bytes": output.stat().st_size, "same_state_pair_frame": 900, "simulation_ticks_between_pair": 0, "selective_convergence_claim": False, "performance_claim": False, "scope": "Sprites, ribbons, and glow are removed only from copied meshes. Ordinary and receiver-copy frame-900 images share one simulation state. Pristine and R are separate states. Vegetation still occludes some contacts."}
(root / "receiver-camera-crops.json").write_text(json.dumps(metadata, indent=2) + "\n")
print(json.dumps({"source": source, "native_panels": len(panels), "bytes": output.stat().st_size}))
