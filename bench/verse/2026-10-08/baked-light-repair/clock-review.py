#!/usr/bin/env python3
"""Retain original clock images and compose labeled crops without rescaling."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil

from PIL import Image, ImageDraw, ImageFont

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("capture_dir", type=Path)
parser.add_argument("evidence_dir", type=Path)
args = parser.parse_args()
report = json.loads((args.capture_dir / "capture.json").read_text())
assert report["timelapse_status"] == "complete"
assert len(report["timelapse"]) == 10441
output = args.evidence_dir / "clock"
output.mkdir(exist_ok=True)
font = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 19)
identity = {"source": report["run"]["source_commit"], "binary_sha256": report["run"]["binary"]["sha256"], "report": "historical-continuation-full/capture.json", "clock_status": "complete", "containing_run_exit": 1, "containing_run_destruction_status": report["destruction_status"], "destruction_acceptance": False, "source_resolution": [1920, 1080], "panels_rescaled": False, "crop_xyxy": [320, 180, 1120, 780], "originals": {}, "boards": {}, "boundary_pixel_gaps": [8.0, 15.5, 17.5]}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


for row in report["captures"]:
    name = row["file"]
    original = args.capture_dir / name
    destination = output / name
    shutil.copyfile(original, destination)
    assert Image.open(destination).size == (1920, 1080)
    identity["originals"][name] = {"metadata": row, "path": str(original.resolve()), "sha256": digest(original), "bytes": original.stat().st_size, "retained_file": "clock/" + name}

phases = ["dawn.png", "noon.png", "dusk.png", "night.png"]
board = Image.new("RGBA", (3840, 2256), (30, 30, 30, 255))
panels = []
for index, name in enumerate(phases):
    x, y = (index % 2) * 1920, (index // 2) * 1128
    ImageDraw.Draw(board).text((x + 12, y + 12), name + " | requested hour " + str(identity["originals"][name]["metadata"]["requested_hour"]) + " | native 1920x1080", font=font, fill="white")
    board.paste(Image.open(output / name).convert("RGBA"), (x, y + 48))
    panels.append({"file": "clock/" + name, "source_xyxy": [0, 0, 1920, 1080], "destination_xy": [x, y + 48]})
board.save(output / "phase-contact-sheet.png", compress_level=6)
identity["boards"]["clock/phase-contact-sheet.png"] = {"resolution": list(board.size), "panels": panels}

for label, hour in [("08", 8.0), ("12", 12.0), ("1530", 15.5), ("1730", 17.5)]:
    names = ["sun-" + label + "-before.png", "noon.png" if hour == 12 else None, "sun-" + label + "-after.png"]
    board = Image.new("RGBA", (2400, 696), (30, 30, 30, 255))
    panels = []
    for index, name in enumerate(names):
        x = index * 800
        draw = ImageDraw.Draw(board)
        if name is None:
            draw.text((x + 12, 12), "At " + str(hour) + ": no exact-boundary pixels", font=font, fill="white")
            draw.text((x + 36, 330), "No exact-boundary image retained", font=font, fill="white")
            panels.append({"file": None, "requested_hour": hour, "destination_xy": [x, 96], "status": "missing_exact_boundary_pixels"})
            continue
        metadata = identity["originals"][name]["metadata"]
        draw.text((x + 12, 12), name, font=font, fill="white")
        draw.text((x + 12, 42), "Requested hour " + str(metadata["requested_hour"]), font=font, fill="white")
        if hour == 12 and index == 1:
            draw.text((x + 12, 69), "Named warmed noon capture", font=font, fill="white")
        pixels = Image.open(output / name).convert("RGBA").crop(identity["crop_xyxy"])
        board.paste(pixels, (x, 96))
        panels.append({"file": "clock/" + name, "requested_hour": metadata["requested_hour"], "source_xyxy": identity["crop_xyxy"], "destination_xy": [x, 96]})
    file = "clock/sun-" + label + "-boundary-crops.png"
    board.save(args.evidence_dir / file, compress_level=6)
    identity["boards"][file] = {"resolution": list(board.size), "panels": panels}

for name, item in identity["boards"].items():
    file = args.evidence_dir / name
    item.update({"sha256": digest(file), "bytes": file.stat().st_size})
(args.evidence_dir / "clock-review.json").write_text(json.dumps(identity, indent=2) + "\n")
(args.evidence_dir / "clock-records.jsonl").write_text("".join(json.dumps(row, separators=(",", ":")) + "\n" for row in report["timelapse"]))
print(json.dumps({"clock_records": 10441, "retained_original_pngs": len(identity["originals"]), "boards": len(identity["boards"]), "missing_exact_boundary_pixels": identity["boundary_pixel_gaps"]}))
