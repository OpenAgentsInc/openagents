#!/usr/bin/env python3
"""Check retained baked-light evidence without running Rust or a renderer."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parent
BAKE_SOURCE = "9967c94cef19031ff91eac6d8cd222ab7c3024eb"
SCENE = "f55a1e76fca2e2b96af777e296549aa9b51776cadbcfff6eaf1c77a8ad83b2ab"
LAYERS = "fc5414a1bfef9e730f3d7d779e4447f12cc86d4e571042eec42518abb30ef7c2"


def read(name):
    return json.loads((ROOT / name).read_text())


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def near(actual, expected, tolerance=0.00001):
    assert math.isfinite(actual) and abs(actual - expected) <= tolerance


def png(path, expected_dimensions=(1920, 1080)):
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path.name
    offset = 8
    dimensions = None
    ended = False
    compressed = bytearray()
    while offset < len(data):
        length = struct.unpack(">I", data[offset:offset + 4])[0]
        kind = data[offset + 4:offset + 8]
        body = data[offset + 8:offset + 8 + length]
        crc = struct.unpack(">I", data[offset + 8 + length:offset + 12 + length])[0]
        assert zlib.crc32(kind + body) & 0xffffffff == crc, path.name
        if kind == b"IHDR":
            assert length == 13, path.name
            dimensions = struct.unpack(">II", body[:8])
            assert body[8:10] == bytes([8, 6]), path.name
            assert body[10:] == bytes([0, 0, 0]), path.name
        elif kind == b"IDAT":
            compressed.extend(body)
        elif kind == b"IEND":
            assert offset + 12 + length == len(data), path.name
            ended = True
            break
        offset += 12 + length
    assert ended and dimensions == expected_dimensions, path.name
    pixels = zlib.decompress(compressed)
    width, height = dimensions
    assert len(pixels) == height * (1 + width * 4), path.name
    assert all(pixels[row * (1 + width * 4)] <= 4 for row in range(height)), path.name


def completed_clock(args, evidence, emit=True):
    current = read("clock-review.json")
    report = read(current["report"])
    manifest = read(evidence["historical_continuation_full"]["manifest"])
    receipt = read(evidence["historical_continuation_full"]["gpu_lease"])
    assert manifest["exit"] == receipt["exit"] == current["containing_run_exit"] == 1
    assert report["run"]["source_commit"] == current["source"] == manifest["source"]
    assert report["run"]["binary"]["sha256"] == current["binary_sha256"] == manifest["binary_sha256"]
    assert report["timelapse_status"] == current["clock_status"] == "complete"
    assert report["destruction_status"] == "failed" and report["destruction"] is None
    assert current["destruction_acceptance"] is False and current["panels_rescaled"] is False
    assert receipt["held_whole_run"] and receipt["acquired_at_ms"] <= manifest["start_unix"] * 1000
    assert receipt["released_at_ms"] >= manifest["end_unix"] * 1000
    timeline = report["timelapse"]
    assert [row["frame"] for row in timeline] == list(range(10441))
    assert [json.loads(line) for line in (ROOT / "clock-records.jsonl").read_text().splitlines()] == timeline
    for row in timeline:
        near(row["requested_hour"], 24 * row["frame"] / 10440)
        assert len(row["sun"]) == 4 and all(math.isfinite(value) for value in row["sun"])
        assert math.isfinite(row["sky_lux"])
    captures = {row["file"]: row for row in report["captures"]}
    assert set(current["originals"]) == set(captures) and len(captures) == 12
    for name, item in current["originals"].items():
        assert item["metadata"] == captures[name]
        file = ROOT / item["retained_file"]
        assert digest(file) == item["sha256"] and file.stat().st_size == item["bytes"]
        png(file)
    assert current["boundary_pixel_gaps"] == [8.0, 15.5, 17.5]
    for hour in current["boundary_pixel_gaps"]:
        assert not any(row["file"] is not None and abs(row["requested_hour"] - hour) < 1e-9 for row in timeline)
        assert not any(abs(row["requested_hour"] - hour) < 1e-9 for row in captures.values())
    for name, item in current["boards"].items():
        file = ROOT / name
        assert digest(file) == item["sha256"] and file.stat().st_size == item["bytes"]
        png(file, tuple(item["resolution"]))
        for panel in item["panels"]:
            if panel["file"] is None:
                assert panel["status"] == "missing_exact_boundary_pixels"
                assert panel["requested_hour"] in current["boundary_pixel_gaps"]
            else:
                assert panel["file"] in {row["retained_file"] for row in current["originals"].values()}
    preview = read("clock-preview.json")
    saved = [row for row in timeline if row["file"] is not None]
    assert len(saved) == preview["selected_clock_images"] == 50
    assert preview["lossy"] and preview["resolution"] == [960, 540]
    assert preview["source_clock_records"] == 10441
    assert [(row["frame"], row["requested_hour"], row["file"]) for row in preview["inputs"]] == [(row["frame"], row["requested_hour"], row["file"]) for row in saved]
    screenshots = read("historical-continuation-screenshot-inventory.json")["screenshots"]
    for item in preview["inputs"]:
        assert item["sha256"] == screenshots[item["file"]]["sha256"]
        assert item["bytes"] == screenshots[item["file"]]["bytes"]
    assert digest(ROOT / preview["output"]) == preview["output_sha256"]
    assert (ROOT / preview["output"]).stat().st_size == preview["output_bytes"]
    assert digest(ROOT / "clock-preview-inputs.ffconcat") == preview["concat_sha256"]
    verify_inventory(args, evidence)
    result = {"status": "completed_clock_artifacts_verified", "clock_records": 10441, "original_clock_pngs": 12, "preview_saved_frames": 50, "preview_lossy": True, "missing_exact_boundary_pixels": current["boundary_pixel_gaps"], "containing_run_exit": 1, "repair_acceptance": False, "visual_quality": evidence["completed_clock"]["visual_quality"]}
    if emit:
        print(json.dumps(result))
    return result


def healthy(repair):
    assert repair["enabled"] and repair["error"] is None
    assert repair["error_count"] == repair["rejected_chunk_vertices"] == 0


def complete(before, current):
    healthy(current)
    assert current["active"]
    assert (current["generation"] > before["generation"]
            or current.get("clock_revision", 0) > before.get("clock_revision", 0))
    assert current["current_targets"] > 0 and current["current_complete"]
    assert current["current_backlog"] == current["current_skipped"] == 0
    assert current["current_processed"] == current["current_applied_vertices"] == current["current_targets"]
    assert current["last_completed_generation"] == current["generation"]
    if "clock_revision" in current:
        assert current["last_completed_clock_revision"] == current["clock_revision"]
        assert current["last_completed_clock_minute"] == current["clock_minute"]
        assert not current["current_mixed_clock"]
    for key in ["completed_generations", "applied_batches", "delivered_vertices", "applied_static_vertices", "applied_chunk_vertices"]:
        assert current[key] > before[key], key


def diagnostics(value, location="destruction"):
    if isinstance(value, dict):
        if {"enabled", "generation", "geometry_epoch", "current_targets"} <= value.keys():
            return [{"location": location, "repair": value}]
        return [row for key, child in value.items() for row in diagnostics(child, location + "." + key)]
    if isinstance(value, list):
        return [row for index, child in enumerate(value) for row in diagnostics(child, location + "[" + str(index) + "]")]
    return []


def lease_covers(manifest, filename, placement=False):
    receipt = read(filename)
    if placement:
        assert receipt["exit"] == 0
        if "leases" in receipt:
            receipt = receipt["leases"][0]
    assert receipt["exit"] == 0 and receipt["held_whole_run"]
    assert receipt["acquired_at_ms"] <= manifest["start_unix"] * 1000
    assert receipt["released_at_ms"] >= manifest["end_unix"] * 1000


def verify_inventory(args, evidence):
    if args.raw_root:
        for local, original in evidence["raw_copy_map"].items():
            assert digest(ROOT / local) == digest(args.raw_root / original), local
        inventories = [*ROOT.glob("*screenshot-inventory.json")]
        for inventory in inventories:
            for screenshot in json.loads(inventory.read_text())["screenshots"].values():
                path = Path(screenshot["path"])
                assert path.stat().st_size == screenshot["bytes"] and digest(path) == screenshot["sha256"]
    lines = (ROOT / "SHA256SUMS").read_text().splitlines()
    assert all(len(line) > 66 and line[64:66] == "  " for line in lines)
    inventory = {line[66:]: line[:64] for line in lines}
    assert len(inventory) == len(lines)
    files = {str(path.relative_to(ROOT)): path for path in ROOT.rglob("*") if path.is_file() and path.name != "SHA256SUMS"}
    assert set(inventory) == set(files)
    for name, path in files.items():
        assert digest(path) == inventory[name], name
    if "artifacts" in evidence:
        assert set(evidence["artifacts"]) == set(files) - {"verification.json"}
        for name, identity in evidence["artifacts"].items():
            assert identity["sha256"] == inventory[name]
            assert identity["bytes"] == files[name].stat().st_size
    assert not any(path.suffix in [".vlay", ".vtp"] for path in files.values())


def recorded_failures(args, evidence):
    for folder, expected_frames, expected_processed in [("historical-full-capture", 3600, 203297), ("historical-wall-capture", 10221, 491660)]:
        manifest = read(folder + "/command-manifest.json")
        repair = read(folder + "/repair-verification.json")
        preflight = read(folder + "/preflight.json")
        assert manifest["exit"] == 1 and repair["verified"] is False
        assert preflight["verified"] and preflight["scene"] == SCENE
        assert repair["hold"]["frames"] == expected_frames
        assert repair["repair"]["current_processed"] == expected_processed
        assert repair["repair"]["current_targets"] == 618035
        assert not repair["repair"]["current_complete"]
        assert repair["repair"]["completed_generations"] == 0
        healthy(repair["repair"])
        for progress in repair["hold"]["progress"]:
            healthy(progress["repair"])
        for filename, placement in [("quiet-lease.json", True), ("gpu-lease.json", False)]:
            receipt = read(folder + "/" + filename)
            if placement:
                receipt = receipt["leases"][0]
            assert receipt["exit"] == 1 and receipt["held_whole_run"]
            assert receipt["acquired_at_ms"] <= manifest["start_unix"] * 1000
            assert receipt["released_at_ms"] >= manifest["end_unix"] * 1000
    assert not (ROOT / "historical-full-capture/capture.json").exists()
    report = read("historical-wall-capture/capture.json")
    assert report["destruction_status"] == "failed" and report["destruction"] is None
    assert len(report["timelapse"]) == 10441
    assert [row["frame"] for row in report["timelapse"]] == list(range(10441))
    assert len(report["samples"]) == 128
    assert [row["pair"] for row in report["samples"]] == list(range(128))
    assert all(row["off_first"] == (row["pair"] % 2 == 0) for row in report["samples"])
    assert report["measurement"]["filtered_samples"] == 0
    increments = [row["frame_completion_increment_ms"] for row in report["samples"]]
    mean = sum(increments) / 128
    batches = [sum(increments[start:start + 16]) / 16 for start in range(0, 128, 16)]
    variance = sum((value - mean) ** 2 for value in batches) / 7
    half = 2.364624 * math.sqrt(variance / 8)
    near(report["measurement"]["frame_completion_mean_increment_ms"], mean)
    near(report["measurement"]["approximate_95pct_block_interval_ms"][0], mean - half)
    near(report["measurement"]["approximate_95pct_block_interval_ms"][1], mean + half)
    failure_count = 2
    if "historical_continuation_full" in evidence:
        item = evidence["historical_continuation_full"]
        manifest = read(item["manifest"])
        report = read(item["report"])
        receipt = read(item["gpu_lease"])
        assert manifest["exit"] == receipt["exit"] == 1
        assert manifest["source"] == report["run"]["source_commit"] == item["source"]
        assert manifest["binary_sha256"] == report["run"]["binary"]["sha256"]
        assert report["destruction_status"] == "failed" and report["destruction"] is None
        assert report["destruction_error"] == "Everglade has no offensive spells"
        assert report["timelapse_status"] == "complete"
        assert [row["frame"] for row in report["timelapse"]] == list(range(10441))
        assert report["measurement_status"] == "skipped" and report["samples"] == []
        assert not (ROOT / "historical-continuation-full/repair-verification.json").exists()
        assert receipt["held_whole_run"]
        assert receipt["acquired_at_ms"] <= manifest["start_unix"] * 1000
        assert receipt["released_at_ms"] >= manifest["end_unix"] * 1000
        failure_count += 1
    verify_inventory(args, evidence)
    print(json.dumps({"status": "recorded_failure_artifacts_verified", "historical_repair_acceptance": False, "clear_capture": "diagnostic_complete" if "destruction_capture" in evidence else "pending", "clock_records": 10441, "paired_samples": 128, "failure_count": failure_count}))


def completed_blend(args, evidence, emit=True):
    current = evidence["continuation"]
    report = read(current["blend_report"])
    manifest = read(current["blend_manifest"])
    assert manifest["source"] == current["source"] and manifest["exit"] == 0
    assert report["run"]["source_commit"] == manifest["source"]
    assert report["run"]["command"] == manifest["command"]
    assert report["run"]["binary"]["sha256"] == manifest["binary_sha256"] == current["binary_sha256"]
    assert report["measurement_status"] == "complete"
    assert report["destruction_status"] == report["timelapse_status"] == "skipped"
    assert report["scene"] == SCENE and report["inputs"]["layers"]["sha256"] == LAYERS
    assert report["inputs_hashed_before_simulation"] and manifest["environment"]["VERSE_KIT_UNPINNED"] == "1"
    lease_covers(manifest, current["blend_quiet_lease"], placement=True)
    lease_covers(manifest, current["blend_gpu_lease"])
    samples, measure = report["samples"], report["measurement"]
    assert [row["pair"] for row in samples] == list(range(128))
    assert measure["pairs"] == 128 and measure["filtered_samples"] == 0
    assert measure["off_first"] == measure["on_first"] == 64
    assert all(row["off_first"] == (row["pair"] % 2 == 0) for row in samples)
    increments = [row["frame_completion_increment_ms"] for row in samples]
    for row in samples:
        near(row["frame_completion_increment_ms"], row["on_encode_ms"] + row["on_completion_ms"] - row["off_encode_ms"] - row["off_completion_ms"], .0001)
    mean = sum(increments) / 128
    batches = [sum(increments[start:start + 16]) / 16 for start in range(0, 128, 16)]
    half = 2.364624 * math.sqrt(sum((value - mean) ** 2 for value in batches) / 7 / 8)
    near(measure["frame_completion_mean_increment_ms"], mean)
    near(measure["approximate_95pct_block_interval_ms"][0], mean - half)
    near(measure["approximate_95pct_block_interval_ms"][1], mean + half)
    assert [json.loads(line) for line in (ROOT / "paired-samples.jsonl").read_text().splitlines()] == samples
    checks = evidence["checks"]["current"]
    native = read(checks["native_manifest"])
    assert len(native) == 6 and all(row["source"] == current["source"] and row["exit"] == 0 for row in native)
    for row in native:
        lease_covers(row, checks["native_gpu_lease"])
    preflight = read(checks["preflight"])
    assert preflight["verified"] and preflight["scene"] == SCENE
    for item in checks["logs_and_leases"]:
        assert read(item["lease"])["exit"] == 0
    if "published_pin_validation" in evidence:
        pin = evidence["published_pin_validation"]
        pin_manifest = read(pin["manifest"])
        pin_preflight = read(pin["preflight"])
        assert pin_manifest["exit"] == 0 and pin_preflight["verified"]
        assert pin_manifest["source"] == pin_preflight["run"]["source_commit"] == pin["source"]
        assert pin_manifest["binary_sha256"] == pin_preflight["run"]["binary"]["sha256"]
        assert "VERSE_KIT_UNPINNED" not in pin_manifest["environment"]
        assert "VERSE_KIT_UNPINNED" in pin_manifest["removed_environment"]
        assert pin_preflight["inputs"] == read(pin["inputs"])
        assert pin_preflight["inputs"]["layers"]["sha256"] == LAYERS
        assert pin_preflight["scene"] == SCENE and pin_preflight["captured_before_simulation"]
    verify_inventory(args, evidence)
    result = {"status": "completed_blend_artifacts_verified", "paired_samples": 128,
                      "native_tests": 6, "frame_completion_mean_increment_ms": mean,
                      "approximate_95pct_block_interval_ms": [mean - half, mean + half],
                      "published_pin_preflight": "verified" if "published_pin_validation" in evidence else "pending",
                      "repair_acceptance": False, "full_capture": current["full_capture_status"],
                      "visual_quality": "not_evaluated_by_blend_measurement", "negligible_cost": current["blend_acceptance"]}
    if emit:
        print(json.dumps(result))
    return result


def completed_destruction(args, evidence, emit=True):
    item = evidence["destruction_capture"]
    manifest = read(item["manifest"])
    report = read(item["report"])
    preflight = read(item["preflight"])
    inputs = read(item["inputs"])
    assert manifest["exit"] == 0
    assert report["run"]["source_commit"] == manifest["source"] == item["source"]
    assert report["run"]["command"] == manifest["command"]
    assert report["run"]["binary"]["sha256"] == manifest["binary_sha256"] == item["binary_sha256"]
    assert set(manifest["features"]) == set(item["features"]) == {"capture", "dev-destruction"}
    assert "--destruction-only" in manifest["command"]
    assert "VERSE_KIT_UNPINNED" not in manifest["environment"]
    assert "VERSE_KIT_UNPINNED" in manifest["removed_environment"]
    assert report["destruction_status"] == "complete"
    assert report["timelapse_status"] == report["measurement_status"] == "skipped"
    assert report["timelapse"] == report["samples"] == [] and report["measurement"] is None
    assert report["inputs"] == preflight["inputs"] == inputs
    assert preflight["verified"] and preflight["captured_before_simulation"]
    assert report["inputs_hashed_before_simulation"]
    assert report["scene"] == preflight["scene"] == SCENE
    assert report["vertices"] == preflight["vertices"] == 4326184
    assert inputs["layers"]["sha256"] == LAYERS and inputs["layers"]["bytes"] == 51684139
    assert report["resolution"] == [1920, 1080] and report["quality"].lower() == "high"
    assert report["temporal_aa"] is False
    assert item["timing_claim"] is False and item["quiet_lease"] is None
    lease_covers(manifest, item["gpu_lease"])
    build = evidence["checks"]["published_capture_build"]
    assert build["source"] == item["source"] and build["binary_sha256"] == item["binary_sha256"]
    assert read(build["test_lease"])["exit"] == read(build["build_lease"])["exit"] == 0
    cpu = evidence["checks"]["published_cpu"]
    assert read(cpu["command_ledger"])["checks"] == cpu["checks"]
    for check in cpu["checks"]:
        assert read(check["build_lease"])["exit"] == 0
        log = (ROOT / check["log"]).read_text()
        for result in check["results"].values():
            assert ("test result: ok. " + str(result["passed"]) + " passed; 0 failed; " + str(result["ignored"]) + " ignored") in log
    native_light = evidence["checks"]["published_native_light"]
    native = read(native_light["manifest"])
    assert len(native) == native_light["manifest_tests"] == 8
    assert all(row["source"] == native_light["source"] and row["exit"] == 0 for row in native)
    assert native_light["production_module_identity"]["comparison_source"] == item["source"]
    assert native_light["production_module_identity"]["changed_files"] == []
    for row in native:
        lease_covers(row, native_light["gpu_lease"])
    assert len(native_light["logs"]) == native_light["retained_light_tests"] == 2
    for test, log_name in native_light["logs"].items():
        assert any(row["test"] == test for row in native)
        log = (ROOT / log_name).read_text()
        assert "test " + test + " ... ok" in log
        assert "test result: ok. 1 passed; 0 failed" in log

    destruction = report["destruction"]
    repair = read(item["repair_verification"])
    assert repair == destruction["selective_repair"] and repair["verified"]
    assert [json.loads(line) for line in (ROOT / "repair-diagnostics.jsonl").read_text().splitlines()] == diagnostics(destruction)
    assert destruction["building_is_kit"] and destruction["frames"] == 960
    assert destruction["relit_pieces_max"] > 0 and destruction["hidden_placements_max"] > 0
    assert destruction["restore_fallback_reset"]
    for key in ["camera_eye", "camera_aim", "player_ground_origin"]:
        assert destruction[key] == item[key]
        assert len(destruction[key]) == 3 and all(math.isfinite(value) for value in destruction[key])
    for key in ["before", "after_swarm", "noon", "night", "restored_before_poll", "restored_after_poll"]:
        healthy(repair[key])
    complete(repair["before"], repair["noon"])
    complete(repair["noon"], repair["night"])
    noon, night = repair["noon"], repair["night"]
    assert noon["completed_geometry_generations"] > repair["before"]["completed_geometry_generations"]
    assert night["geometry_epoch"] == noon["geometry_epoch"]
    assert night["completed_clock_generations"] > noon["completed_clock_generations"]
    assert night["clock_minute"] != noon["clock_minute"] and night["sun_direction"] != noon["sun_direction"]
    assert night["sun_lux"] < noon["sun_lux"] and night["sky_lux"] < noon["sky_lux"]
    for key in ["restored_before_poll", "restored_after_poll"]:
        assert repair[key]["current_targets"] == repair[key]["current_backlog"] == 0
    assert not repair["restored_before_poll"]["active"]
    assert repair["restore_warmup_simulation_dt"] == 0.0
    for key in ["noon_hold", "night_hold"]:
        hold = repair[key]
        assert hold["simulation_dt"] == 0.0
        assert hold["wall_seconds_limit"] == item["diagnostic_hold_seconds"] == 600
        assert hold["wall_seconds"] <= hold["wall_seconds_limit"] + 1
        for row in hold["progress"]:
            healthy(row["repair"])
    assert [row["frame"] for row in destruction["captures"]] == [360, 480, 600, 900]
    for row in destruction["captures"]:
        healthy(row["repair"])
    before_restore = read(item["repair_before_restore"])
    assert before_restore["noon"] == repair["noon"] and before_restore["night"] == repair["night"]
    assert before_restore["noon_hold"] == repair["noon_hold"] and before_restore["night_hold"] == repair["night_hold"]
    expected = {row["file"] for row in destruction["captures"]}
    expected.update([destruction["pristine"], destruction["restored"], repair["noon_image"], repair["night_image"]])
    inventory = read(item["screenshot_inventory"])["screenshots"]
    assert set(inventory) == set(item["selected_original_pngs"]) == expected
    for name, identity in inventory.items():
        file = ROOT / "clear-capture" / name
        assert digest(file) == identity["sha256"] and file.stat().st_size == identity["bytes"]
        png(file)
    crops = read("clear-camera-crops.json")
    assert crops["source"] == item["source"] and crops["source_pixels_verified"]
    assert crops["panels_rescaled"] is False and len(crops["panels"]) == 4
    crop_file = ROOT / crops["file"]
    assert digest(crop_file) == crops["sha256"] and crop_file.stat().st_size == crops["bytes"]
    png(crop_file, tuple(crops["resolution"]))
    for panel in crops["panels"]:
        assert digest(ROOT / panel["file"]) == panel["sha256"]
    if "portable_fallback" in evidence["checks"]:
        portable = evidence["checks"]["portable_fallback"]
        assert read(portable["lease"])["exit"] == read(portable["wasm_lease"])["exit"] == 0
        assert "test result: ok. 319 passed; 0 failed; 2 ignored" in (ROOT / portable["log"]).read_text()
    if "portable_guard" in evidence["checks"]:
        guard = evidence["checks"]["portable_guard"]
        assert read(guard["lease"])["exit"] == 0
        assert "wasm32-unknown-unknown" in guard["command"]
        example = evidence["checks"]["receiver_example"]
        assert read(example["lease"])["exit"] == 0
        assert "test result: ok. 4 passed; 0 failed" in (ROOT / example["log"]).read_text()
    if args.input_files:
        for identity in inputs.values():
            file = Path(identity["path"])
            assert digest(file) == identity["sha256"] and file.stat().st_size == identity["bytes"]
    verify_inventory(args, evidence)
    result = {"status": "completed_destruction_artifacts_verified", "source": item["source"], "original_pngs": len(expected), "repair_records": len(diagnostics(destruction)), "noon_hold_seconds": repair["noon_hold"]["wall_seconds"], "night_hold_seconds": repair["night_hold"]["wall_seconds"], "diagnostic_hold_limit_seconds": 600, "default_hold_limit_seconds": 180, "completion_within_default_bound": "not_claimed", "gpu_time_claim": False, "visual_quality": item["visual_quality"]}
    if emit:
        print(json.dumps(result))
    return result


def receiver_inspection(args, evidence, emit=True):
    item = evidence["receiver_inspection"]
    manifest = read(item["manifest"])
    report = read(item["report"])
    inputs = read(item["inputs"])
    preflight = read(item["preflight"])
    assert manifest["exit"] == 0
    assert manifest["source"] == report["run"]["source_commit"] == item["source"]
    assert manifest["command"] == report["run"]["command"]
    assert "--receiver-inspection-only" in manifest["command"]
    assert manifest["binary_sha256"] == report["run"]["binary"]["sha256"] == item["binary_sha256"]
    assert set(manifest["features"]) == {"capture", "dev-destruction"}
    assert "VERSE_KIT_UNPINNED" not in manifest["environment"]
    assert report["destruction_status"] == "complete"
    assert report["measurement_status"] == report["timelapse_status"] == "skipped"
    assert report["measurement"] is None and report["samples"] == report["timelapse"] == []
    assert report["scene"] == preflight["scene"] == SCENE
    assert report["inputs"] == inputs == preflight["inputs"]
    assert preflight["verified"] and preflight["captured_before_simulation"]
    assert inputs["layers"]["sha256"] == LAYERS and inputs["layers"]["bytes"] == 51684139
    assert report["quality"].lower() == "high" and report["resolution"] == [1920, 1080]
    assert report["temporal_aa"] is False
    scope = report["receiver_inspection_scope"]
    assert scope["supplementary"] and not scope["full_verification"]
    assert scope["pin_enforced"] and scope["layer_pin"] == LAYERS
    assert scope["selective_holds_status"] == scope["performance_status"] == "skipped"
    assert scope["filtered_channels"] == ["sprites", "ribbons", "glow"]
    lease_covers(manifest, item["gpu_lease"])
    assert item["quiet_lease"] is None and item["timing_claim"] is False

    destruction = report["destruction"]
    repair = destruction["selective_repair"]
    assert destruction["mode"] == "receiver-inspection-only" and destruction["frames"] == 960
    assert destruction["building_is_kit"] and destruction["restore_fallback_reset"]
    assert repair == read(item["inspection"])
    assert repair["verified"] is False
    assert repair["selective_holds_status"] == "skipped"
    assert repair["noon_hold"] is repair["night_hold"] is None
    assert repair["pin_enforced"] and repair["layer_pin"] == LAYERS
    assert "pending" in repair["selective_completion_status"]
    ordinary_pair = repair["inspection"]
    assert ordinary_pair["frame"] == 900 and ordinary_pair["same_simulation_state"]
    assert ordinary_pair["simulation_ticks_between_pair"] == 0
    assert ordinary_pair["lighting_stage_identical"]
    assert ordinary_pair["filtered_channels"] == ["sprites", "ribbons", "glow"]
    assert ordinary_pair["retained_lit_vertices"] >= 0 and ordinary_pair["retained_rigid_instances"] > 0
    for key in ["before", "after_swarm", "restored_before_poll", "restored_after_poll"]:
        healthy(repair[key])
    healthy(ordinary_pair["repair"])
    assert repair["restore_warmup_simulation_dt"] == 0.0 and repair["restore_fallback_reset"]
    assert not repair["restored_before_poll"]["active"]
    for key in ["restored_before_poll", "restored_after_poll"]:
        assert repair[key]["current_targets"] == repair[key]["current_backlog"] == 0
    assert [row["frame"] for row in destruction["captures"]] == [360, 480, 600, 900]
    example = evidence["checks"]["receiver_example"]
    build = evidence["checks"]["receiver_example_build"]
    assert example["source"] == build["source"] == item["source"]
    assert read(example["lease"])["exit"] == read(build["lease"])["exit"] == 0
    prior = read(evidence["destruction_capture"]["report"])["destruction"]
    for key in ["camera_eye", "camera_aim", "player_ground_origin", "building_center", "building_is_kit", "frames", "fps"]:
        assert destruction[key] == prior[key], key
    checkpoint = read(item["before_restore"])
    for key in ["before", "after_swarm", "inspection", "selective_holds_status", "selective_completion_status", "verified"]:
        assert checkpoint[key] == repair[key]
    assert [json.loads(line) for line in (ROOT / item["diagnostics"]).read_text().splitlines()] == diagnostics(destruction)
    screenshots = read(item["screenshot_inventory"])["screenshots"]
    expected = {row["file"] for row in destruction["captures"]}
    expected.update([destruction["pristine"], destruction["restored"], repair["pristine_receiver_image"], ordinary_pair["file"], repair["restored_receiver_image"]])
    assert set(screenshots) == expected and len(expected) == 9
    for name, identity in screenshots.items():
        file = ROOT / "receiver-inspection" / name
        assert digest(file) == identity["sha256"] and file.stat().st_size == identity["bytes"]
        png(file)
    crops = read("receiver-camera-crops.json")
    assert crops["source"] == item["source"] and crops["source_pixels_verified"]
    assert crops["panels_rescaled"] is False and len(crops["panels"]) == 4
    assert crops["selective_convergence_claim"] is crops["performance_claim"] is False
    assert crops["same_state_pair_frame"] == 900 and crops["simulation_ticks_between_pair"] == 0
    crop_file = ROOT / crops["file"]
    assert digest(crop_file) == crops["sha256"] and crop_file.stat().st_size == crops["bytes"]
    png(crop_file, tuple(crops["resolution"]))
    for panel in crops["panels"]:
        assert digest(ROOT / panel["file"]) == panel["sha256"]
    verify_inventory(args, evidence)
    result = {"status": "receiver_inspection_artifacts_verified", "source": item["source"], "original_pngs": len(expected), "frames": 960, "filtered_channels": scope["filtered_channels"], "same_state_pair_frame": 900, "selective_convergence": "not_evaluated", "performance": "not_measured", "visual_quality": item["visual_quality"]}
    if emit:
        print(json.dumps(result))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--raw-root", type=Path, help="Compare retained copies with the capture scratch directory.")
    parser.add_argument("--input-files", action="store_true", help="Hash the licensed local inputs at their recorded paths without copying them.")
    parser.add_argument("--recorded-failures", action="store_true", help="Verify every retained failed run.")
    parser.add_argument("--completed-blend", action="store_true", help="Verify completed blend and continuation functional checks.")
    parser.add_argument("--completed-clock", action="store_true", help="Verify completed clock records and selected pixels from the retained failed full run.")
    parser.add_argument("--completed-destruction", action="store_true", help="Verify the completed clear destruction component, when present.")
    parser.add_argument("--receiver-inspection", action="store_true", help="Verify the supplementary particle-free inspection without asserting convergence.")
    args = parser.parse_args()
    evidence = read("verification.json")
    if args.recorded_failures:
        recorded_failures(args, evidence)
        return
    if args.completed_blend:
        completed_blend(args, evidence)
        return
    if args.completed_clock:
        completed_clock(args, evidence)
        return
    if args.completed_destruction:
        completed_destruction(args, evidence)
        return
    if args.receiver_inspection:
        receiver_inspection(args, evidence)
        return
    if "destruction_capture" in evidence:
        clock = completed_clock(args, evidence, emit=False)
        blend = completed_blend(args, evidence, emit=False)
        destruction = completed_destruction(args, evidence, emit=False)
        assert evidence["capture"]["reports_merged"] is False
        clock_report = read(evidence["completed_clock"]["report"])
        blend_report = read(evidence["continuation"]["blend_report"])
        destruction_report = read(evidence["destruction_capture"]["report"])
        for report in [clock_report, blend_report, destruction_report]:
            assert report["scene"] == SCENE and report["inputs"]["layers"]["sha256"] == LAYERS
        bake = read("matched-bake-receipt.json")
        bake_command = read("matched-bake-command.json")
        assert bake_command["source"] == bake["commit"] == BAKE_SOURCE
        assert bake_command["exit"] == 0 and not bake["dirty"]
        assert bake["scene"]["digest"] == SCENE and bake["scene"]["vertices"] == 4326184
        assert bake["layers"]["sha256"] == LAYERS and bake["layers"]["bytes"] == 51684139
        assert all(report["bake_key"] == bake["bake_key"] for report in [clock_report, blend_report, destruction_report])
        lease_covers(bake_command, "matched-bake-build-lease.json")
        old = read("published-preflight-failure.json")
        assert not old["verified"] and old["captured_before_simulation"]
        assert old["actual_scene"] == SCENE and old["expected_scene"] != SCENE
        assert old["actual_vertices"] == old["expected_vertices"] == 4326184
        supplementary = receiver_inspection(args, evidence, emit=False) if "receiver_inspection" in evidence else None
        print(json.dumps({"status": "components_verified", "clock": clock, "blend": blend, "destruction": destruction, "receiver_inspection": supplementary, "reports_merged": False, "visual_quality": evidence["acceptance"]["visual_quality"], "negligible_cost": evidence["acceptance"]["negligible_cost"]}))
        return
    raise AssertionError("Clear destruction evidence is unavailable; use --completed-clock, --completed-blend, or --recorded-failures to verify retained components.")


if __name__ == "__main__":
    main()
