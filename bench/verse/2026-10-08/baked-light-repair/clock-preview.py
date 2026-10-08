"""Encode an explicitly labeled lossy preview from saved clock screenshots."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("capture_dir", type=Path)
parser.add_argument("evidence_dir", type=Path)
args = parser.parse_args()
report = json.loads((args.capture_dir / "capture.json").read_text())
rows = [row for row in report["timelapse"] if row["file"] is not None]
assert rows and len(report["timelapse"]) == 10441
inputs = []
concat = args.capture_dir.parent / (args.capture_dir.name + "-clock-preview-inputs.ffconcat")
lines = ["ffconcat version 1.0"]
for row in rows:
    path = (args.capture_dir / row["file"]).resolve()
    escaped = str(path).replace("'", "'\\''")
    lines.extend(["file '" + escaped + "'", "duration 0.1666666667"])
    inputs.append({"frame": row["frame"], "requested_hour": row["requested_hour"], "file": row["file"], "path": str(path), "bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
lines.append("file '" + str((args.capture_dir / rows[-1]["file"]).resolve()).replace("'", "'\\''") + "'")
concat.write_text("\n".join(lines) + "\n")
output = args.evidence_dir / "clock-preview.mp4"
command = ["/opt/homebrew/bin/ffmpeg", "-hide_banner", "-loglevel", "warning", "-y", "-f", "concat", "-safe", "0", "-i", str(concat), "-vf", "scale=960:540", "-c:v", "libx264", "-threads", "2", "-preset", "medium", "-crf", "20", "-pix_fmt", "yuv420p", "-fps_mode", "vfr", "-an", "-movflags", "+faststart", "-metadata", "comment=Lossy preview of selected captures from the 24-hour clock timeline; all 10441 metadata records are retained separately.", str(output)]
result = subprocess.run(command, check=True, capture_output=True, text=True)
metadata = {"schema": "openagents.verse.clock-preview.v1", "format": "H.264 MP4", "resolution": [960, 540], "lossy": True, "crf": 20, "source_clock_records": 10441, "selected_clock_images": len(rows), "selected_images_per_second": 6, "last_image_repeated_for_concat_duration": True, "method": "Saved clock images in numeric timeline order, encoded at reduced resolution; no intermediate simulation frames are invented.", "command": command, "concat_file": str(concat), "concat_sha256": hashlib.sha256(concat.read_bytes()).hexdigest(), "inputs": inputs, "output": output.name, "output_sha256": hashlib.sha256(output.read_bytes()).hexdigest(), "output_bytes": output.stat().st_size, "stderr": result.stderr}
(args.evidence_dir / "clock-preview.json").write_text(json.dumps(metadata, indent=2) + "\n")
print(json.dumps({"output": str(output), "bytes": output.stat().st_size, "selected_images": len(rows)}))
