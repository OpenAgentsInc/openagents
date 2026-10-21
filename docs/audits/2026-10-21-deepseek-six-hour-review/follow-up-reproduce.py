#!/usr/bin/env python3
"""Check follow-up billing and build fixtures without cloud access or a GPU."""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[3]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def billing():
    path = ROOT / "scripts/dev/cloud-budget.py"
    spec = importlib.util.spec_from_file_location("follow_up_budget", path)
    budget = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(budget)
    start = budget.parse_time("2027-01-15T08:00:00Z")
    record = {
        "id": "old-ledger-entry", "instance_id": "111", "vm": "reused",
        "project": "fixture", "zone": "fixture", "start_epoch": start,
        "provisioning": "flex-start", "price_per_hour": 64.44,
        "expires_epoch": start + 7200,
    }
    replacement = {
        "id": "222", "status": "TERMINATED",
        "creationTimestamp": "2027-01-15T08:10:00Z",
        "lastStartTimestamp": "2027-01-15T08:10:00Z",
        "lastStopTimestamp": "2027-01-15T09:00:00Z",
    }
    reply = subprocess.CompletedProcess([], 0, json.dumps(replacement), "")
    with patch.object(budget, "gcloud", return_value=reply):
        seen = budget.describe(record)
    events = [{
        "kind": "insert", "at": start + 300, "begun": start + 1,
        "done": True, "ok": False, "instance_id": "111",
    }]
    settled = budget.settle(record, seen, events)
    return {
        "kind": "executed fixture using the actual describe, settle, and stop_record functions",
        "record_instance": "111", "replacement_instance": "222",
        "describe": seen, "settlement": settled,
        "actual_cost_usd": budget.stop_record(record, settled)["cost"] if settled else None,
        "expected": "Do not borrow instance 222's timestamps; instance 111's insert failed and never started.",
    }


def startup(scratch):
    with tempfile.TemporaryDirectory(dir=scratch, prefix="v41-follow-up-") as temp:
        root = Path(temp)

        def write(path, content, executable=False):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
            if executable:
                path.chmod(0o755)

        manifest = '{"schema":"audit-stage-manifest"}'
        manifest_sha = sha(manifest.encode())
        folder = root / "bucket/bin/abc/sm_120"
        worker = "#!/bin/sh\necho fixture-worker\n"
        write(folder / "psionic-v41-stage", worker, True)
        write(folder / f"manifests/{manifest_sha}.json", manifest)
        write(root / "bucket/w/model.safetensors.index.json", json.dumps({
            "weight_map": {"layers.0.attn.wkv.weight": "a.safetensors"},
        }))
        for name in ("config.json", "tokenizer.json", "tokenizer_config.json", "a.safetensors"):
            write(root / "bucket/w" / name, "{}")
        for name, value in {
            "oa-weights": "gs://fixture/w/", "oa-layers": "0-10",
            "oa-embeddings": "0", "oa-head": "0",
            "oa-worker": "gs://fixture/bin/abc/sm_120/psionic-v41-stage",
            "oa-manifest": manifest_sha,
        }.items():
            write(root / "metadata" / name, value)
        write(root / "boot", "fixture-boot\n")
        write(root / "nvidia-smi", "#!/bin/sh\necho 12.0\n", True)
        write(root / "gcloud", """#!/usr/bin/env python3
import glob, os, pathlib, shutil, sys
args = sys.argv[1:]
assert args[:2] == ['storage', 'cp'], args
sources, dest = args[2:-1], pathlib.Path(args[-1])
if sources == ['-I']:
    sources = sys.stdin.read().splitlines()
for source in sources:
    matches = glob.glob(str(pathlib.Path(os.environ['FIXTURE_BUCKET']) / source.removeprefix('gs://fixture/')))
    if not matches:
        sys.exit(1)
    for match in matches:
        shutil.copyfile(match, dest / pathlib.Path(match).name if dest.is_dir() else dest)
""", True)
        env = dict(os.environ, OA_STAGE_METADATA=str(root / "metadata"),
                   OA_STAGE_ROOT=str(root / "stage"), OA_STAGE_OPT=str(root / "opt"),
                   OA_BOOT_ID=str(root / "boot"), OA_GCLOUD=str(root / "gcloud"),
                   OA_NVIDIA_SMI=str(root / "nvidia-smi"), FIXTURE_BUCKET=str(root / "bucket"))
        builds = []
        changed_after_install = None
        for version in ("A", "B"):
            library = f"fixture CUDA library {version}"
            write(folder / "lib/libcudart.so.13", library)
            build = json.dumps({
                "schema": "openagents.psionic_v41_stage.build.v2", "arch": "sm_120",
                "profile": "cuda-device-sm_120", "sha256": sha(worker.encode()),
                "libraries": {"libcudart.so.13": sha(library.encode())},
                "manifests": {manifest_sha: "fixture.json"},
            })
            write(folder / "BUILD.json", build)
            if version == "A":
                write(root / "metadata/oa-build", sha(build.encode()))
            else:
                (root / "metadata/oa-build").unlink()
            boot = subprocess.run(["sh", str(ROOT / "scripts/pylon/v41-stage-startup.sh")],
                                  env=env, capture_output=True, text=True)
            assert boot.returncode == 0, (root / "stage/startup.log").read_text()
            run = subprocess.run([str(root / "opt/stage-run")], env=env,
                                 capture_output=True, text=True)
            assert run.returncode == 0, run.stderr
            log = (root / "stage/startup.log").read_text()
            builds.append({"build_json_sha256": sha(build.encode()),
                           "worker_sha256": sha(worker.encode()),
                           "library_sha256": sha(library.encode()),
                           "startup_returncode": boot.returncode, "stage_run": run.stdout.strip(),
                           "labelled_development": f"development launch: the worker build {sha(build.encode())}" in log})
            if version == "A":
                installed = root / "opt/builds" / sha(build.encode())
                changed = json.loads(build)
                changed_library = "self-consistent replacement after installation"
                changed["libraries"]["libcudart.so.13"] = sha(changed_library.encode())
                changed_build = json.dumps(changed)
                write(installed / "lib/libcudart.so.13", changed_library)
                write(installed / "BUILD.json", changed_build)
                run = subprocess.run([str(root / "opt/stage-run")], env=env,
                                     capture_output=True, text=True)
                changed_after_install = {
                    "pinned_build_json_sha256": sha(build.encode()),
                    "installed_build_json_sha256": sha(changed_build.encode()),
                    "stage_run_returncode": run.returncode,
                    "stage_run": run.stdout.strip(),
                    "scope": "Local library and its BUILD.json changed together; this does not demonstrate changing already loaded GPU code.",
                }
        # #12410: the replaced library and BUILD.json are refused, and only
        # the unpinned bundle is labelled a development launch. The group
        # and qualification identity checks are Rust tests (#12410).
        checks = {
            "changed_after_pinned_install_refused": changed_after_install["stage_run_returncode"] != 0,
            "pinned_bundle_not_labelled_development": not builds[0]["labelled_development"],
            "unpinned_bundle_labelled_development": builds[1]["labelled_development"],
        }
        return {
            "kind": "executed startup shell with a pinned v2 bundle A and an unpinned bundle B, and a shell worker; no CUDA execution",
            "checks_12410": checks,
            "passed": all(checks.values()),
            "builds": builds,
            "changed_after_pinned_install": changed_after_install,
            "interpretation": "Optional pins permit both bundles. The source-level group and qualification identity includes the worker hash, not the differing library or BUILD.json hashes.",
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    scratch = subprocess.check_output(["openagents", "scratch"], text=True).strip()
    files = ["scripts/dev/cloud-budget.py", "scripts/pylon/v41-stage-startup.sh",
             "crates/pylon/src/stage_group.rs", "crates/pylon/src/cloud.rs",
             "crates/psionic/crates/psionic-v41-stage/src/lib.rs",
             "crates/psionic/crates/psionic-v41-stage/src/serve.rs",
             "crates/psionic/crates/psionic-v41-stage/src/qualify.rs"]
    result = {
        "schema": "openagents.deepseek_v41.audit_follow_up.v1",
        "tested_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_sha256": {name: sha((ROOT / name).read_bytes()) for name in files},
        "billing_reused_name": billing(), "unpinned_bundles": startup(scratch),
        "cloud_access": False, "gpu_execution": False,
    }
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
