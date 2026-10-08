#!/usr/bin/env python3
"""Validate and stage a temporal AA attempt without promoting acceptance."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import shutil

ROOT = Path(__file__).resolve().parent
CASES = {
    "high-dev": ("high", 960, None),
    "high-release": ("high", 960, None),
    "high-paired": ("high", 960, [439, 484]),
    "medium-paired": ("medium", 960, [439, 484]),
    "pan-paired": ("high", 480, [120, 135]),
    "orbit-paired": ("high", 480, [120, 135]),
}


def read(path):
    return json.loads(Path(path).read_text())


def digest(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def identity(path):
    path = Path(path)
    return {"sha256": digest(path), "bytes": path.stat().st_size}


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def validate(plan):
    require(re.fullmatch(r"[0-9a-f]{40}", plan["source"] or ""), "Set an exact source commit")
    require(set(plan["cases"]) == set(CASES), "Bind all six cases explicitly")
    require(plan["assets"], "Record the exact asset inputs")
    for asset in plan["assets"].values():
        require(identity(asset["path"]) == {k: asset[k] for k in ["sha256", "bytes"]}, "Asset identity changed")
    for name, source in plan.get("proofs", {}).items():
        require(not Path(name).is_absolute() and ".." not in Path(name).parts and Path(source).is_file(), "Use an existing proof and a relative destination filename")
    jobs = {}
    reports = {}
    for label, (quality, frames, sequence) in CASES.items():
        binding = plan["cases"][label]
        records = read(binding["manifest"])
        if isinstance(records, dict):
            records = [records]
        matching = [row for row in records if row.get("name") == binding["job"]]
        require(len(matching) == 1, f"Missing or repeated job: {label}")
        job = matching[0]
        require(job["exit"] == 0 and job["source"] == plan["source"], f"Incomplete/wrong-source job: {label}")
        require(job["binary_sha256"] == binding["binary_sha256"], f"Binary identity changed: {label}")
        require(re.fullmatch(r"[0-9a-f]{64}", job["binary_sha256"]), f"Invalid binary digest: {label}")
        require(job["profile"] == binding["profile"], f"Profile changed: {label}")
        require(job["profile"] == "release" if label != "high-dev" else job["profile"] in ["dev", "debug"], f"Unexpected profile: {label}")
        require(job["quality"] == quality, f"Quality changed: {label}")
        require(job["start_unix"] < job["end_unix"], f"Invalid job chronology: {label}")
        for resource in ["quiet", "gpu"]:
            receipt = read(binding[f"{resource}_receipt"])
            require(receipt["resource"] == resource and receipt["exit"] == 0 and receipt["held_whole_run"], f"Incomplete {resource} lease: {label}")
            require(receipt["acquired_at_ms"] <= job["start_unix"] * 1000, f"Lease starts after job: {label}/{resource}")
            require(receipt["released_at_ms"] >= job["end_unix"] * 1000, f"Lease ends before job: {label}/{resource}")
        report = read(Path(binding["prefix"]) / "capture.json")
        require(report["width"] == 1920 and report["height"] == 1080 and report["fps"] == 60, f"Capture dimensions changed: {label}")
        require(report["effective_quality"] == quality and report["temporal_aa_enabled"], f"Renderer configuration changed: {label}")
        require(report["submission_timing"]["submitted_frames"] == report["submission_timing"]["completed_frames"] == frames, f"Incomplete submissions: {label}")
        require(sum(phase["frames"] for phase in report["phases"].values()) == frames, f"Missing phase frames: {label}")
        require(report.get("temporal_texture_diagnostics", {}).get("enabled") is not True, f"Diagnostic snapshots cannot establish timing: {label}")
        argv = job["command"]
        require(str(Path(argv[1]).resolve()) == str(Path(binding["prefix"]).resolve()), f"Report/command mismatch: {label}")
        require("--live" in argv and "--settle-light" in argv, f"Wrong capture mode: {label}")
        require(float(argv[argv.index("--seconds") + 1]) == frames / 60, f"Wrong duration: {label}")
        if sequence is None:
            require("--compare-temporal-aa" not in argv, f"Unpaired benchmark became paired: {label}")
            require("--capture-rebuild" in argv, f"Missing destruction/reset capture: {label}")
            rebuild = report["rebuild_capture"]
            require(rebuild is not None and rebuild["pristine_frame"] == 0 and rebuild["restoration_frame"] == frames, f"Missing reset receipt: {label}")
            require(rebuild["pristine_view"] == rebuild["restored_view"], f"Reset camera changed: {label}")
            for key in ["pristine_image", "restored_image"]:
                require((Path(binding["prefix"]) / rebuild[key]).is_file(), f"Missing reset image: {label}/{key}")
        else:
            require("--compare-temporal-aa" in argv and report["sequence_frames"] == sequence, f"Wrong paired sequence: {label}")
            timing = report["submission_timing"]
            pixels = report["pixel_readback"]
            require(timing["policy"] == "serial_diagnostics", f"Paired completion policy changed: {label}")
            require(timing["temporal_baseline_submitted_frames"] == timing["temporal_baseline_completed_frames"] == frames, f"Incomplete baseline: {label}")
            require(pixels["policy"] == "all_frames_temporal_comparison", f"Paired readback policy changed: {label}")
            require(pixels["primary_readback_frame_indices"] == pixels["temporal_baseline_readback_frame_indices"] == list(range(frames)), f"Missing paired readback: {label}")
            comparison = report["temporal_comparison"]
            ordered = []
            for phase in comparison["phases"].values():
                rows = phase["frame_results"]
                require(len(rows) == phase["frames"], f"Missing paired rows: {label}")
                ordered.extend(row["frame"] for row in rows)
                if rows:
                    mean = sum(row["wall_render_completion_increment_ms"] for row in rows) / len(rows)
                    require(math.isclose(mean, phase["mean_wall_render_completion_increment_ms"], abs_tol=1e-6), f"Paired mean changed: {label}")
                    upper = phase["wall_render_completion_mean_95pct_ci_ms"]["upper"]
                    require(phase["wall_mean_upper_95pct_under_1ms"] == (upper < 1), f"Paired bound flag changed: {label}")
            require(ordered == list(range(comparison["warmup_frames"], frames)), f"Missing/reordered pair: {label}")
            for frame in range(sequence[0], sequence[1] + 1):
                for variant in ["off", "on"]:
                    require((Path(binding["prefix"]) / "frames" / f"{frame:04}-taa-{variant}.png").is_file(), f"Missing sequence image: {label}/{frame}/{variant}")
        if label in ["pan-paired", "orbit-paired"]:
            require(report["static_houses"] and report["camera_path"] == label.split("-")[0], f"Camera path changed: {label}")
        else:
            require(not report["static_houses"], f"Missing destruction simulation: {label}")
        kit = job["environment"].get("VERSE_KIT_PACK")
        require(kit == plan["assets"]["kit"]["path"], f"Kit command identity changed: {label}")
        jobs[label], reports[label] = job, report
    return jobs, reports


def archive_current():
    current = read(ROOT / "verification.json")
    destination = ROOT / "legacy" / f"reactive-{current['tested_source_commit'][:12]}-top-level"
    require(not destination.exists(), f"Archive already exists: {destination}")
    files = {}
    for path in sorted(ROOT.rglob("*")):
        relative = path.relative_to(ROOT)
        if not path.is_file() or relative.parts[0] in ["legacy", "supplementary", "staged", "__pycache__"]:
            continue
        if path.name in ["curate.py", "curation-plan.template.json", "artifacts.json"]:
            continue
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        files[str(relative)] = identity(path)
    write(destination / "archive.json", {
        "schema": "openagents.verse.temporal-archive.v1",
        "source": current["tested_source_commit"],
        "binary_sha256": current["tested_binary_sha256"],
        "status_at_archive": current["status"],
        "files": files,
        "shared_history": "Other legacy and supplementary attempts remain at the evidence root; original relative inventories are unchanged.",
    })
    return str(destination.relative_to(ROOT))


def check_archive(destination):
    destination = Path(destination)
    archive = read(destination / "archive.json")
    original = read(destination / "verification.json")
    require(archive["source"] == original["tested_source_commit"] and archive["binary_sha256"] == original["tested_binary_sha256"], "Archived source identity changed")
    require(archive["status_at_archive"] == original["status"], "Archived verdict changed")
    for name, expected in archive["files"].items():
        require(identity(destination / name) == expected, f"Archived artifact changed: {name}")
    print(f"ARCHIVE INTEGRITY PASS: unchanged {archive['source']} attempt")


def stage(plan, jobs, reports, images):
    destination = ROOT / "staged" / plan["source"][:12]
    require(not destination.exists(), f"Staged attempt already exists: {destination}")
    source_images = {}
    for label, binding in plan["cases"].items():
        prefix = Path(binding["prefix"])
        target = destination / "reports" / f"{label}.json"
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(prefix / "capture.json", target)
        for key in ["manifest", "quiet_receipt", "gpu_receipt"]:
            target = destination / "proof" / f"{label}-{key}.json"
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(binding[key], target)
        for source in sorted(prefix.rglob("*.png")):
            relative = source.relative_to(prefix)
            source_images[f"{label}/{relative}"] = {"path": str(source.resolve()), **identity(source), "storage": "durable_scratch"}
    for name, source in plan.get("proofs", {}).items():
        require(not Path(name).is_absolute() and ".." not in Path(name).parts, "Use a relative proof filename")
        target = destination / "proof" / name
        require(not target.exists(), f"Proof destination collides with a case receipt: {name}")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
    if images:
        curate_images(destination, plan, source_images)
    write(destination / "source-images.json", {"schema": "openagents.verse.image-inventory.v1", "images": source_images})
    write(destination / "commands.json", {"source": plan["source"], "jobs": jobs, "bindings": plan["cases"], "assets": plan["assets"]})
    write(destination / "verification.json", {
        "schema": "openagents.verse.temporal-staged.v1", "status": "review_pending", "source": plan["source"],
        "acceptance": {"visual": None, "timing": None, "promotion_authorized": False},
        "reports": {label: {"file": f"reports/{label}.json", "sha256": digest(destination / "reports" / f"{label}.json")} for label in reports},
        "note": "Raw reports retain every sample and outlier. Independent case leases finished before curation. No top-level acceptance is changed.",
    })
    write(destination / "artifacts.json", {str(path.relative_to(destination)): identity(path) for path in sorted(destination.rglob("*")) if path.is_file() and path.name != "artifacts.json"})
    check_staged(destination)
    return str(destination.relative_to(ROOT))


def curate_images(destination, plan, inventory):
    from PIL import Image, ImageDraw
    for label, binding in plan["cases"].items():
        prefix = Path(binding["prefix"])
        for name in ["establishing", "impact", "aftermath", "pristine", "restored"]:
            suffixes = [""] if CASES[label][2] is None else ["-taa-off", "-taa-on"]
            for suffix in suffixes:
                source = prefix / f"{name}{suffix}.png"
                if not source.exists():
                    continue
                target = destination / "selected" / f"{label}-{source.name}"
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, target)
                inventory[f"{label}/{source.name}"].update(storage="git", file=str(target.relative_to(destination)))
        sequence = CASES[label][2]
        if sequence is None:
            continue
        crop = plan["crops"][label]
        width, height = crop[2] - crop[0], crop[3] - crop[1]
        require(width > 0 and height > 0 and 0 <= crop[0] < crop[2] <= 1920 and 0 <= crop[1] < crop[3] <= 1080, f"Invalid crop: {label}")
        frames = list(range(467, 479)) if sequence[0] == 439 else list(range(120, 136))
        animation = []
        for frame in frames:
            pair = Image.new("RGB", (width * 2, height + 24))
            ImageDraw.Draw(pair).text((4, 4), f"{label} frame {frame}: off left, on right; native crop {crop}", fill="white")
            for column, variant in enumerate(["off", "on"]):
                source = prefix / "frames" / f"{frame:04}-taa-{variant}.png"
                with Image.open(source) as original:
                    require(original.size == (1920, 1080), f"Image dimensions changed: {source}")
                    native = original.convert("RGB").crop(crop)
                pair.paste(native, (column * width, 24))
                target = destination / "audit" / f"{label}-{frame}-{variant}.png"
                target.parent.mkdir(parents=True, exist_ok=True)
                native.save(target)
            animation.append(pair)
        animation[0].save(destination / "audit" / f"{label}-sequence.gif", save_all=True, append_images=animation[1:], duration=100, loop=0, optimize=False, disposal=2)
        for frame in ([469, 471, 472] if sequence[0] == 439 else [120, 128, 135]):
            sheet = Image.new("RGB", (width * 2, height))
            for column, variant in enumerate(["off", "on"]):
                with Image.open(destination / "audit" / f"{label}-{frame}-{variant}.png") as crop_image:
                    sheet.paste(crop_image, (column * width, 0))
            sheet.save(destination / "audit" / f"{label}-{frame}-pair.png")
    write(destination / "audit" / "crop-method.json", {"crops": plan["crops"], "pixels": "Native PNG crops; off left, on right. GIFs are palette-quantized inspection aids slowed to 10 fps, not measurement inputs or pixel proofs."})


def check_staged(destination):
    destination = Path(destination)
    value = read(destination / "verification.json")
    require(value["status"] == "review_pending" and value["acceptance"] == {"visual": None, "timing": None, "promotion_authorized": False}, "Staging must not claim acceptance")
    for name, expected in read(destination / "artifacts.json").items():
        require(identity(destination / name) == expected, f"Staged artifact changed: {name}")
    for item in read(destination / "source-images.json")["images"].values():
        source = destination / item["file"] if item["storage"] == "git" else Path(item["path"])
        if source.exists():
            require(identity(source) == {k: item[k] for k in ["sha256", "bytes"]}, f"Source image changed: {source}")
    print(f"STAGED INTEGRITY PASS: {value['source']}; timing and visual review pending")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plan", nargs="?", type=Path)
    parser.add_argument("--stage", action="store_true", help="Copy raw reports and provenance into staged; does not promote")
    parser.add_argument("--images", action="store_true", help="Create selected originals, native crops, and short paired GIFs after all leases finish")
    parser.add_argument("--archive-current", action="store_true", help="Copy the current top-level attempt unchanged into legacy")
    parser.add_argument("--check-staged", type=Path)
    parser.add_argument("--check-archive", type=Path)
    args = parser.parse_args()
    if args.check_staged:
        check_staged(args.check_staged)
        return
    if args.check_archive:
        check_archive(args.check_archive)
        return
    require(args.plan is not None, "Pass a completed curation plan")
    require(not args.images or args.stage, "Image curation requires --stage")
    require(not args.archive_current or args.stage, "Archiving requires a validated fresh staged attempt")
    plan = read(args.plan)
    jobs, reports = validate(plan)
    print(f"INPUT INTEGRITY PASS: six completed jobs at {plan['source']}; no acceptance claim")
    if args.stage:
        print("STAGED:", stage(plan, jobs, reports, args.images))
        if args.archive_current:
            print("ARCHIVED:", archive_current())


if __name__ == "__main__":
    main()
