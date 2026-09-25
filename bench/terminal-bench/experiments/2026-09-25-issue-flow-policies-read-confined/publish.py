#!/usr/bin/env python3
"""Copy run evidence and archive transcripts without the scratch checkout."""

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import tarfile


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def secrets():
    values = []
    for path in [Path.home() / ".codex/auth.json", Path.home() / ".openagents/jev.json"]:
        if not path.exists():
            continue
        def collect(value):
            if isinstance(value, dict):
                for key, item in value.items():
                    if isinstance(item, str) and len(item) >= 20 and any(word in key.lower() for word in ("token", "key", "secret")):
                        values.append(item.encode())
                    else:
                        collect(item)
            elif isinstance(value, list):
                for item in value:
                    collect(item)
        collect(json.loads(path.read_text()))
    return values


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    records = here / "records"
    journal = [json.loads(line) for line in (args.out / "runs.jsonl").read_text().splitlines()]
    sensitive = secrets()
    stop = args.out / "operator-stop.json"
    quarantined = json.loads(stop.read_text()).get("quarantined_slots", []) if stop.exists() else []
    index = []
    for attempt in journal:
        manifest = Path(attempt["manifest"])
        run = manifest.parent
        slot = run.parent.parent
        driver_files = [name for name in ("launch.json", "attempt.json", "stderr.log", "stdout.json", "cache-cleanup.json") if (slot / name).exists()]
        destination = records / slot.name
        destination.mkdir(exist_ok=True)
        if attempt["slot"] in quarantined:
            # Private history reached this run. Keep the original on the
            # operator's host; publish identities and accounting, not content.
            files = [{"path": str(p.relative_to(run)), "bytes": p.stat().st_size, "sha256": sha(p)}
                     for p in sorted(run.rglob("*"))
                     if p.is_file() and not p.is_symlink() and p.relative_to(run).parts[0] != "repo"]
            record = {"slot": attempt["slot"], "entry": attempt["entry"], "arm": attempt["arm"],
                      "status": "quarantined-local-only", "source": str(run),
                      "reason": "Outside private conversation history appeared in command output.",
                      "files": files}
            (destination / "quarantine.json").write_text(json.dumps(record, indent=2) + "\n")
            shutil.copyfile(slot / "attempt.json", destination / "attempt.json")
            index.append({key: value for key, value in record.items() if key != "files"})
            continue
        candidates = [p for p in run.rglob("*") if p.is_file() and not p.is_symlink() and p.relative_to(run).parts[0] != "repo"]
        candidates.extend(slot / name for name in driver_files)
        for path in candidates:
            contents = path.read_bytes()
            if any(secret in contents for secret in sensitive):
                raise SystemExit(f"Refusing publication: credential found in {path.relative_to(args.out)}")
        for source, name in [(manifest, "manifest.json"), (run / "verification/grade.json", "grade.json"),
                             (run / "artifacts/candidate.diff", "candidate.diff"),
                             (run / "artifacts/reply.md", "reply.md"), (slot / "attempt.json", "attempt.json")]:
            shutil.copyfile(source, destination / name)
        # Do not follow symlinks. Candidate snapshots and frozen score scripts
        # under artifacts remain in the archive. The final checkout can be
        # reconstructed from the manifest's base and candidate.diff.
        archive = destination / "traces.tar.gz"
        with archive.open("wb") as raw, gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as zipped:
            with tarfile.open(fileobj=zipped, mode="w|", dereference=False) as tar:
                for path in sorted(run.iterdir()):
                    if path.name != "repo":
                        tar.add(path, arcname=path.name)
                for name in driver_files:
                    tar.add(slot / name, arcname="driver/" + name)
        members = [{"path": str(p.relative_to(run)), "bytes": p.stat().st_size, "sha256": sha(p)}
                   for p in candidates if p.is_relative_to(run)]
        record = {"slot": attempt["slot"], "entry": attempt["entry"], "arm": attempt["arm"],
                  "archive": str(archive.relative_to(here)), "sha256": sha(archive), "bytes": archive.stat().st_size,
                  "files": members}
        (destination / "evidence.json").write_text(json.dumps(record, indent=2) + "\n")
        index.append({key: value for key, value in record.items() if key != "files"})
    (records / "evidence.json").write_text(json.dumps(index, indent=2) + "\n")
    shutil.copyfile(args.out / "runs.jsonl", records / "runs.jsonl")
    shutil.copyfile(args.out / "state.json", records / "state.json")
    print(json.dumps({"archives": sum("archive" in row for row in index),
                      "quarantined": len(quarantined), "bytes": sum(row.get("bytes", 0) for row in index)}))


if __name__ == "__main__":
    main()
