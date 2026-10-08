"""Retain supplementary receiver pixels without asserting selective convergence."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("scratch", type=Path)
parser.add_argument("evidence", type=Path)
parser.add_argument("prefix")
parser.add_argument("gpu_receipt")
parser.add_argument("runner")
args = parser.parse_args()
capture = args.scratch / args.prefix
manifest = json.loads((args.scratch / (args.prefix + "-manifest.json")).read_text())
report = json.loads((capture / "capture.json").read_text())
receipt = json.loads((args.scratch / args.gpu_receipt).read_text())
assert manifest["exit"] == receipt["exit"] == 0
assert manifest["source"] == report["run"]["source_commit"]
assert manifest["command"] == report["run"]["command"]
assert "--receiver-inspection-only" in manifest["command"]
assert manifest["binary_sha256"] == report["run"]["binary"]["sha256"]
assert report["destruction_status"] == "complete"
assert report["receiver_inspection_scope"]["supplementary"]
assert not report["receiver_inspection_scope"]["full_verification"]
assert report["destruction"]["selective_repair"]["verified"] is False
assert receipt["held_whole_run"]
assert receipt["acquired_at_ms"] <= manifest["start_unix"] * 1000
assert receipt["released_at_ms"] >= manifest["end_unix"] * 1000


def digest(file):
    return hashlib.sha256(file.read_bytes()).hexdigest()


def diagnostics(value, location="destruction"):
    if isinstance(value, dict):
        if {"enabled", "generation", "geometry_epoch", "current_targets"} <= value.keys():
            return [{"location": location, "repair": value}]
        return [row for key, child in value.items() for row in diagnostics(child, location + "." + key)]
    if isinstance(value, list):
        return [row for index, child in enumerate(value) for row in diagnostics(child, location + "[" + str(index) + "]")]
    return []


evidence = json.loads((args.evidence / "verification.json").read_text())
output = args.evidence / "receiver-inspection"
output.mkdir(exist_ok=True)
copies = {"command-manifest.json": args.prefix + "-manifest.json", "capture.log": args.prefix + ".log", "gpu-lease.json": args.gpu_receipt, "runner.py": args.runner}
for name in ["capture.json", "inputs.json", "preflight.json", "receiver-inspection.json", "receiver-inspection-before-restore.json"]:
    copies[name] = args.prefix + "/" + name
for name, original in copies.items():
    shutil.copyfile(args.scratch / original, output / name)
    evidence["raw_copy_map"]["receiver-inspection/" + name] = original
screenshots = {}
for file in sorted(capture.glob("*.png")):
    screenshots[file.name] = {"path": str(file.resolve()), "bytes": file.stat().st_size, "sha256": digest(file)}
    shutil.copyfile(file, output / file.name)
    evidence["raw_copy_map"]["receiver-inspection/" + file.name] = args.prefix + "/" + file.name
destruction = report["destruction"]
rows = diagnostics(destruction)
(args.evidence / "receiver-inspection-diagnostics.jsonl").write_text("".join(json.dumps(row, separators=(",", ":")) + "\n" for row in rows))
(args.evidence / "receiver-screenshot-inventory.json").write_text(json.dumps({"source": manifest["source"], "binary_sha256": manifest["binary_sha256"], "screenshots": screenshots}, indent=2) + "\n")
evidence["receiver_inspection"] = {"source": manifest["source"], "binary_sha256": manifest["binary_sha256"], "manifest": "receiver-inspection/command-manifest.json", "report": "receiver-inspection/capture.json", "inputs": "receiver-inspection/inputs.json", "preflight": "receiver-inspection/preflight.json", "inspection": "receiver-inspection/receiver-inspection.json", "before_restore": "receiver-inspection/receiver-inspection-before-restore.json", "gpu_lease": "receiver-inspection/gpu-lease.json", "quiet_lease": None, "timing_claim": False, "mode": "receiver-inspection-only", "selective_completion_status": "not_evaluated", "selective_holds_status": "skipped", "performance_status": "skipped", "frames": 960, "camera_eye": destruction["camera_eye"], "camera_aim": destruction["camera_aim"], "player_ground_origin": destruction["player_ground_origin"], "targets": destruction["targets"], "seeds": destruction["seeds"], "diagnostics": "receiver-inspection-diagnostics.jsonl", "diagnostic_records": len(rows), "screenshot_inventory": "receiver-screenshot-inventory.json", "selected_original_pngs": sorted(screenshots), "visual_quality": "pending_review"}
evidence.pop("receiver_inspection_pending", None)
evidence["capture"]["components"]["receiver_inspection"] = "receiver_inspection"
(args.evidence / "verification.json").write_text(json.dumps(evidence, indent=2) + "\n")
print(json.dumps({"source": manifest["source"], "original_pngs": len(screenshots), "diagnostic_records": len(rows), "selective_convergence": "not_evaluated", "performance": "not_measured"}))
