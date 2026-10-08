"""Retain a completed destruction-only run without merging clock or blend rows."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("scratch", type=Path)
parser.add_argument("evidence", type=Path)
parser.add_argument("--prefix", default="10907-published-clear")
parser.add_argument("--gpu-receipt", default="10907-published-clear-gpu.json")
args = parser.parse_args()
capture = args.scratch / args.prefix
manifest = json.loads((args.scratch / (args.prefix + "-manifest.json")).read_text())
report = json.loads((capture / "capture.json").read_text())
gpu = json.loads((args.scratch / args.gpu_receipt).read_text())
assert manifest["exit"] == gpu["exit"] == 0
assert report["destruction_status"] == "complete" and report["destruction"] is not None
assert report["timelapse_status"] == report["measurement_status"] == "skipped"
assert report["timelapse"] == report["samples"] == [] and report["measurement"] is None
assert report["run"]["source_commit"] == manifest["source"]
assert report["run"]["command"] == manifest["command"]
assert report["run"]["binary"]["sha256"] == manifest["binary_sha256"]
assert "VERSE_KIT_UNPINNED" not in manifest["environment"]
assert set(manifest["features"]) == {"capture", "dev-destruction"}
assert gpu["held_whole_run"] and gpu["acquired_at_ms"] <= manifest["start_unix"] * 1000
assert gpu["released_at_ms"] >= manifest["end_unix"] * 1000


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def diagnostics(value, location="destruction"):
    if isinstance(value, dict):
        if {"enabled", "generation", "geometry_epoch", "current_targets"} <= value.keys():
            return [{"location": location, "repair": value}]
        return [row for key, child in value.items() for row in diagnostics(child, location + "." + key)]
    if isinstance(value, list):
        return [row for index, child in enumerate(value) for row in diagnostics(child, location + "[" + str(index) + "]")]
    return []


evidence = json.loads((args.evidence / "verification.json").read_text())
output = args.evidence / "clear-capture"
output.mkdir(exist_ok=True)
copies = {"command-manifest.json": args.prefix + "-manifest.json", "capture.log": args.prefix + ".log", "gpu-lease.json": args.gpu_receipt, "runner.py": args.prefix + ".py"}
for name in ["capture.json", "inputs.json", "preflight.json", "repair-verification.json", "repair-before-restore.json"]:
    copies[name] = args.prefix + "/" + name
for name, original in copies.items():
    shutil.copyfile(args.scratch / original, output / name)
    evidence["raw_copy_map"]["clear-capture/" + name] = original
destruction = report["destruction"]
repair = destruction["selective_repair"]
assert repair == json.loads((capture / "repair-verification.json").read_text())
assert repair["verified"]
screenshots = {}
for file in sorted(capture.glob("*.png")):
    screenshots[file.name] = {"path": str(file.resolve()), "bytes": file.stat().st_size, "sha256": digest(file)}
    shutil.copyfile(file, output / file.name)
    evidence["raw_copy_map"]["clear-capture/" + file.name] = args.prefix + "/" + file.name
(args.evidence / "clear-screenshot-inventory.json").write_text(json.dumps({"source": manifest["source"], "binary_sha256": manifest["binary_sha256"], "screenshots": screenshots}, indent=2) + "\n")
rows = diagnostics(destruction)
(args.evidence / "repair-diagnostics.jsonl").write_text("".join(json.dumps(row, separators=(",", ":")) + "\n" for row in rows))
evidence["destruction_capture"] = {"source": manifest["source"], "binary_sha256": manifest["binary_sha256"], "manifest": "clear-capture/command-manifest.json", "report": "clear-capture/capture.json", "inputs": "clear-capture/inputs.json", "preflight": "clear-capture/preflight.json", "repair_verification": "clear-capture/repair-verification.json", "repair_before_restore": "clear-capture/repair-before-restore.json", "gpu_lease": "clear-capture/gpu-lease.json", "quiet_lease": None, "timing_claim": False, "features": manifest["features"], "mode": "destruction-only", "clock_status": "skipped", "measurement_status": "skipped", "camera_eye": destruction["camera_eye"], "camera_aim": destruction["camera_aim"], "player_ground_origin": destruction["player_ground_origin"], "diagnostic_hold_seconds": 600, "default_hold_seconds": 180, "noon_hold_seconds": repair["noon_hold"]["wall_seconds"], "night_hold_seconds": repair["night_hold"]["wall_seconds"], "diagnostic_completion": "verified", "completion_within_default_bound": "not_claimed", "selected_original_pngs": sorted(screenshots), "screenshot_inventory": "clear-screenshot-inventory.json", "repair_diagnostic_records": len(rows), "visual_quality": "pending_review"}
evidence["tested_source_commit"] = manifest["source"]
evidence["acceptance"]["repair_diagnostics"] = "verified_diagnostic_600_second_limit"
evidence["acceptance"]["published_pin"] = "verified_for_selected_local_files"
evidence["acceptance"]["visual_quality"] = "pending_review"
evidence["capture"] = {"components": {"clock": "completed_clock", "blend": "continuation", "destruction": "destruction_capture"}, "reports_merged": False, "source_identity_scope": "Each raw report and command manifest retains its own source and binary identity."}
(args.evidence / "verification.json").write_text(json.dumps(evidence, indent=2) + "\n")
print(json.dumps({"source": manifest["source"], "original_pngs": len(screenshots), "repair_records": len(rows), "noon_hold_seconds": repair["noon_hold"]["wall_seconds"], "night_hold_seconds": repair["night_hold"]["wall_seconds"], "visual_acceptance": "pending_review"}))
