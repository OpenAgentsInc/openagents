#!/usr/bin/env python3
"""Check historical identities and final reactive temporal AA evidence."""
from pathlib import Path
import hashlib
import json
import math

ROOT = Path(__file__).resolve().parent


def read(path):
    return json.loads((ROOT / path).read_text())


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def check_attempt(v, commands, prefix, required_source=None, require_medium=True):
    records = {r["name"]: r for r in commands["captures"]}
    if required_source is not None:
        assert v["tested_source_commit"] == required_source
    total = 0
    for label, summary in v["results"].items():
        report = read(prefix + summary["report"])
        record = records[summary["capture_command"]]
        assert record["source"] == v["tested_source_commit"] == summary["source"]
        assert record["binary_sha256"] == v["tested_binary_sha256"] == summary["binary_sha256"]
        assert record["profile"] == summary["profile"] == "release"
        assert record["exit"] == 0
        assert record["quality"] == summary["quality"] == report["effective_quality"]
        assert report["width"] == 1920 and report["height"] == 1080 and report["fps"] == 60
        assert report["temporal_aa_available"] and report["temporal_aa_enabled"]
        frames = summary["simulation_frames"]
        submission = report["submission_timing"]
        assert submission["submitted_frames"] == submission["completed_frames"] == frames
        assert submission["temporal_baseline_submitted_frames"] == submission["temporal_baseline_completed_frames"] == frames
        assert submission["policy"] == "serial_diagnostics"
        pixels = report["pixel_readback"]
        assert pixels["policy"] == "all_frames_temporal_comparison"
        assert pixels["primary_readback_count"] == pixels["temporal_baseline_readback_count"] == frames
        assert pixels["primary_readback_frame_indices"] == list(range(frames))
        assert pixels["temporal_baseline_readback_frame_indices"] == list(range(frames))
        temporal = report["temporal_comparison"]
        assert temporal["warmup_frames"] == 8
        measured = []
        for phase, values in summary["phases"].items():
            raw = temporal["phases"][phase]
            assert values == {key: raw.get(key) for key in values}, f"Summary changed: {label}/{phase}"
            results = raw["frame_results"]
            assert len(results) == raw["frames"] == raw["invalid_gpu_frames"]
            assert raw["valid_gpu_frames"] == 0
            assert raw["gpu_increment_ms"] is None and raw["mean_gpu_increment_ms"] is None
            assert raw["off_first_frames"] + raw["on_first_frames"] == len(results)
            for row in results:
                assert row["gpu_increment_ms"] is None
                assert row["off_gpu_ms"] is None or row["on_gpu_ms"] is None
            if not results:
                assert raw["mean_wall_render_completion_increment_ms"] is None
                assert raw["wall_render_completion_mean_95pct_ci_ms"] is None
                assert raw["wall_mean_upper_95pct_under_1ms"] is None
                continue
            mean = sum(row["wall_render_completion_increment_ms"] for row in results) / len(results)
            assert math.isclose(mean, raw["mean_wall_render_completion_increment_ms"], abs_tol=1e-6)
            interval = raw["wall_render_completion_mean_95pct_ci_ms"]
            assert interval["samples"] == len(results)
            assert raw["wall_mean_upper_95pct_under_1ms"] == (interval["upper"] < 1)
            if label != "medium-short-8s" and (require_medium or label != "medium"):
                assert interval["upper"] < 1
            measured += [row["frame"] for row in results]
        assert measured == list(range(8, frames)), f"Missing/repeated pair: {label}"
        total += len(measured)
    for test in v["native_tests"]:
        log = (ROOT / (prefix + test["proof"])).read_text()
        assert f"test {test['name']} ... ok" in log
        assert "1 passed; 0 failed" in log
    checks = v["checks"]
    assert checks["source"] == v["tested_source_commit"]
    log = (ROOT / (prefix + checks["proof"])).read_text()
    for package in ["physics", "verse_pbr", "verse_zone_everglade", "capture_example"]:
        count = checks[package]["passed"]
        assert f"test result: ok. {count} passed; 0 failed" in log
    assert "Finished `release` profile" in log
    return total


def check():
    from curate import check_archive, check_staged
    for archived in sorted((ROOT / "legacy").glob("reactive-*-top-level/archive.json")):
        check_archive(archived.parent)
    for staged in sorted((ROOT / "staged").glob("*/verification.json")):
        check_staged(staged.parent)
    manifest = read("artifacts.json")
    for name, identity in manifest.items():
        path = ROOT / name
        assert path.stat().st_size == identity["bytes"], f"Size changed: {name}"
        assert digest(path) == identity["sha256"], f"Hash changed: {name}"
    supplemental = "supplementary/neighbor-dependency-diagnostic/"
    attempt = read(supplemental + "verification.json")
    assert attempt["status"] == "visual_failed"
    assert attempt["timing_acceptance_available"] is False
    assert attempt["supersedes_top_level_acceptance"] is False
    command = read(supplemental + attempt["command_manifest"])
    assert command["source"] == attempt["source"] and command["exit"] == 0
    assert command["binary_sha256"] == attempt["binary_sha256"]
    receipt = read(supplemental + attempt["gpu_receipt"])
    assert receipt["resource"] == "gpu" and receipt["exit"] == 0
    assert receipt["held_whole_run"] and receipt["released_at_ms"] >= command["end_unix"] * 1000
    native = read(supplemental + attempt["native"]["manifest"])
    assert len(native) == 8 and all(row["exit"] == 0 for row in native)
    assert {row["source"] for row in native} == {attempt["native"]["source"]}
    assert {row["binary_sha256"] for row in native} == {attempt["native"]["binary_sha256"]}
    for index, row in enumerate(native):
        log = (ROOT / supplemental / "proof" / f"10936-dependency-native-{index}.log").read_text()
        assert f"test {row['test']} ... ok" in log and "1 passed; 0 failed" in log
    supplemental_sources = read(supplemental + "source-images.json")["images"]
    for identity in supplemental_sources.values():
        path = Path(identity["path"])
        if path.exists():
            assert path.stat().st_size == identity["bytes"] and digest(path) == identity["sha256"]
    print(f"SUPPLEMENTARY VISUAL FAIL RETAINED: eight native passes, {len(supplemental_sources)} full-image identities, no timing gate")
    live_prefix = "supplementary/live-marker-history-51e1/"
    live = read(live_prefix + "verification.json")
    assert live["status"] == "visual_failed" and live["timing_acceptance_available"] is False
    assert live["supersedes_top_level_acceptance"] is False
    command = read(live_prefix + live["command_manifest"])
    assert command["exit"] == 0 and command["source"] == live["source"]
    assert command["binary_sha256"] == live["binary_sha256"]
    receipt = read(live_prefix + live["gpu_receipt"])
    assert receipt["exit"] == 0 and receipt["resource"] == "gpu" and receipt["held_whole_run"]
    assert receipt["released_at_ms"] >= command["end_unix"] * 1000
    diagnostics = read(live_prefix + "capture.json")["temporal_texture_diagnostics"]
    assert diagnostics["enabled"] and diagnostics["timing_acceptance_available"] is False
    records = diagnostics["frames"]
    assert [row["frame"] for row in records] == list(range(464, 477))
    assert all(row["marker_written_this_frame"] and row["reactive_ranges_valid"] for row in records)
    assert all(row["dynamic_lit_vertices"] == 2880 and len(row["reactive_ranges"]) == 3 for row in records)
    assert all(row["marker_covered_pixels"] > 0 and row["nonretainable_history_pixels"] >= row["marker_covered_pixels"] for row in records)
    assert all(a["current_jittered_world_to_clip"] == b["previous_jittered_world_to_clip"] for a, b in zip(records, records[1:]))
    observations = read(live_prefix + "audit/observations.json")
    assert all(row["marked_pixels_with_retainable_history"] == 0 for row in observations["marker_history"])
    live_sources = read(live_prefix + live["source_images"])["images"]
    assert len(live_sources) == 65
    for identity in live_sources.values():
        path = ROOT / live_prefix / identity["file"] if identity["storage"] == "git" else Path(identity["path"])
        if path.exists():
            assert path.stat().st_size == identity["bytes"] and digest(path) == identity["sha256"]
    print("LIVE DIAGNOSTIC FAIL RETAINED: 13 actual marker/history/HDR records, 65 original image identities, exact camera chronology, no timing gate")
    inventory = read("image-inventory.json")
    archived_present = 0
    for image in inventory["images"]:
        assert image["storage"] in ["git", "durable_scratch"]
        path = ROOT / image["file"] if image["storage"] == "git" else Path(image["path"])
        if image["storage"] == "durable_scratch" and not path.exists():
            continue
        assert path.stat().st_size == image["bytes"], f"Image size changed: {path}"
        assert digest(path) == image["sha256"], f"Image hash changed: {path}"
        archived_present += image["storage"] == "durable_scratch"
    print(f"IMAGE INVENTORY: {len(inventory['images'])} identities; {archived_present} durable scratch images available and verified")
    sources = read("audit/source-frame-hashes.json")["frames"]
    assert len(sources) == 248
    available_sources = 0
    for identity in sources.values():
        assert identity["pixels"] == [1920, 1080] and identity["bytes"] > 0
        path = Path(identity["source"])
        if path.exists():
            assert path.stat().st_size == identity["bytes"] and digest(path) == identity["sha256"]
            available_sources += 1
    print(f"SOURCE INVENTORY: {len(sources)} exact frames; {available_sources} scratch originals available and verified")
    v = read("verification.json")
    old_prefix = "legacy/pre-reactive-c559/"
    old = read(old_prefix + "verification.json")
    historical_samples = check_attempt(old, read(old_prefix + "commands.json"), old_prefix)
    assert old["results"]["medium-short-8s"]["phases"]["swarm"]["wall_mean_upper_95pct_under_1ms"] is False
    assert old["acceptance"]["meteor_fast_motion_ghost_duration"].startswith("unestablished")
    history = v["historical_attempts"]["pre_reactive_c559"]
    assert history["source"] == old["tested_source_commit"]
    assert history["binary_sha256"] == old["tested_binary_sha256"]
    assert history["fast_head_acceptance"] is False
    caster = read("legacy/caster-dae/verification.json")
    assert all(phase["wall_mean_upper_95pct_under_1ms"] is False for phase in caster["results"]["high"].values())
    assert caster["source"] != old["tested_source_commit"] and caster["unpinned_override"]
    if v["status"] == "awaiting_final_reactive_capture":
        assert v["tested_source_commit"] is None and v["tested_binary_sha256"] is None
        assert v["results"] == {} and v["native_tests"] == []
        assert v["acceptance"]["wall_mean_upper_95pct_under_1ms"] is None
        assert v["acceptance"]["meteor_fast_motion_ghost_duration"]["status"] == "pending"
        print(f"HISTORICAL PASS: {len(manifest)} artifact hashes and {historical_samples} paired samples")
        raise SystemExit("PENDING: final reactive captures/native proofs; no final fast-head or budget pass")
    samples = check_attempt(v, read("commands.json"), "", v["candidate_source_commit"], require_medium=False)
    assert set(v["results"]) >= {"high", "medium", "pan", "orbit"}
    assert v["acceptance"]["high_wall_mean_upper_95pct_under_1ms"] is True
    assert v["acceptance"]["pan_and_orbit_wall_mean_upper_95pct_under_1ms"] is True
    medium_passes = all(p["wall_mean_upper_95pct_under_1ms"] for p in v["results"]["medium"]["phases"].values() if p["frames"])
    assert v["acceptance"]["medium_full_wall_mean_upper_95pct_under_1ms"] == medium_passes
    batch = read(v["capture_batch"]["manifest"])
    assert len(batch) == 6 and all(r["exit"] == 0 for r in batch)
    for resource, path in [("quiet", v["capture_batch"]["quiet_receipt"]), ("gpu", v["capture_batch"]["gpu_receipt"])]:
        lease = read(path)
        assert lease["resource"] == resource and lease["exit"] == 0 and lease["held_whole_run"]
        assert lease["released_at_ms"] >= max(r["end_unix"] * 1000 for r in batch)
    native_records = read("proof/native-manifest.json")
    assert len(native_records) == 4 and all(r["exit"] == 0 for r in native_records)
    assert {r["source"] for r in native_records} == {v["native_provenance"]["native_source"]}
    assert {r["binary_sha256"] for r in native_records} == {v["native_provenance"]["native_binary_sha256"]}
    failed_native = read(v["native_provenance"]["parser_attempt0"])
    assert failed_native[-1]["exit"] == 101
    assert "redefinition of `ndc`" in (ROOT / v["native_provenance"]["parser_failure"]).read_text()
    head = v["acceptance"]["meteor_fast_motion_ghost_duration"]
    assert head["historical_evidence_satisfies_gate"] is False
    audit = read(head["visual_review"])
    assert audit["source"] == v["tested_source_commit"]
    assert audit["reactive_meteor_head"]["status"] == head["status"]
    native = [test for test in v["native_tests"] if test.get("covers_reactive_head")]
    assert native and all(test["result"] == "passed" and test["samples"] == [1, 4] for test in native)
    assert any(test.get("covers_occlusion") for test in native)
    print(f"INTEGRITY PASS: {len(manifest)} artifact hashes, {samples} reactive and {historical_samples} pre-reactive paired samples, High wall gates, disclosed Medium result, native/check/lease proofs, and retained failures")
    if head["status"] != "verified_reviewed_sequence":
        print("VISUAL UNRESOLVED: detached TAA-only meteor contours remain; no fast-head PASS")
        raise SystemExit(2)
    print("VISUAL PASS: limited reviewed sequence plus recorded native coverage")


if __name__ == "__main__":
    check()
