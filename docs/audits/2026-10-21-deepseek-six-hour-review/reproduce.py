#!/usr/bin/env python3
"""Reproduce the audit's billing and startup findings without cloud access."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[3]


def load_budget_tests():
    path = ROOT / "scripts/dev/test_cloud_budget.py"
    spec = importlib.util.spec_from_file_location("audit_cloud_budget_tests", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def billing_probes():
    tests = load_budget_tests()
    results = {}
    case = tests.CloudBudgetTests()
    case.setUp()
    try:
        made = case.create("audit-flex-sync", "--flex-start", hours="2", spot=False)
        assert made.returncode == 0, made.stderr
        ended = case.run_guard(
            "sync", "--budget", "test", at=tests.T0 + 7200,
            FAKE_DESCRIBE="TERMINATED",
            FAKE_STARTED="2027-01-15T09:00:00Z",
            FAKE_OPS=json.dumps([{
                "operationType": "delete", "endTime": "2027-01-15T10:00:00Z",
            }]),
        )
        assert ended.returncode == 0, ended.stderr
        results["flex_sync_after_a_real_start"] = {
            "requested_at": "2027-01-15T08:00:00Z",
            "started_at": "2027-01-15T09:00:00Z",
            "ended_at": "2027-01-15T10:00:00Z",
            "expected_compute_usd": 64.44,
            "actual_ledger_usd": case.report(tests.T0 + 7200)["spent"],
            "stop_record": case.records()[-1],
        }
    finally:
        case.doCleanups()

    case = tests.CloudBudgetTests()
    case.setUp()
    try:
        made = case.create("audit-flex-race", "--flex-start", hours="2", spot=False)
        assert made.returncode == 0, made.stderr
        fake = case.root / "gcloud"
        transition = case.root / "started-during-delete"
        source = fake.read_text().replace(
            "args = sys.argv[1:]",
            "args = sys.argv[1:]\n"
            "if args[:3] == ['compute', 'instances', 'delete']:\n"
            f"    open({str(transition)!r}, 'w').write('RUNNING before delete completed')",
        )
        fake.write_text(source)
        ended = case.run_guard(
            "stop", "audit-flex-race", "--budget", "test",
            at=tests.T0 + 1800, FAKE_DESCRIBE="PENDING",
        )
        assert ended.returncode == 0, ended.stderr
        results["pending_to_running_before_delete"] = {
            "kind": "fault injection; not an observed Google Cloud transition",
            "transition_injected": transition.exists(),
            "actual_ledger_usd": case.report(tests.T0 + 1800)["spent"],
            "reason": case.records()[-1]["reason"],
            "expected": "Do not assert never-started from a pre-delete snapshot.",
        }
    finally:
        case.doCleanups()
    return results


def startup_probes(scratch: Path):
    with tempfile.TemporaryDirectory(dir=scratch, prefix="v41-startup-audit-") as temp:
        root = Path(temp)
        bucket = root / "bucket"
        metadata = root / "metadata"
        worker = "bin/abc/sm_120/psionic-v41-stage"
        worker_dir = str(Path(worker).parent)

        def write(path, value, executable=False):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(value)
            if executable:
                path.chmod(0o755)

        manifest = '{"schema":"audit-stage-manifest"}'
        digest = hashlib.sha256(manifest.encode()).hexdigest()
        original_worker = '#!/bin/sh\necho original-worker\n'
        write(bucket / worker, original_worker, True)
        original_sha = hashlib.sha256(original_worker.encode()).hexdigest()
        write(bucket / worker_dir / "BUILD.json", json.dumps({"sha256": original_sha}))
        write(bucket / worker_dir / "lib/libcudart.so.13", "audit library")
        write(bucket / worker_dir / f"manifests/{digest}.json", manifest)
        write(bucket / "w/model.safetensors.index.json", json.dumps({
            "weight_map": {"layers.0.attn.wkv.weight": "a.safetensors"},
        }))
        for name in ["config.json", "tokenizer.json", "tokenizer_config.json", "a.safetensors"]:
            write(bucket / "w" / name, name)
        for key, value in {
            "oa-weights": "gs://audit/w/", "oa-layers": "0-10",
            "oa-embeddings": "1", "oa-head": "0",
            "oa-worker": "gs://audit/" + worker, "oa-manifest": digest,
        }.items():
            write(metadata / key, value)
        write(root / "smi", "#!/bin/sh\necho 12.0\n", True)
        write(root / "gcloud", f'''#!/usr/bin/env python3
import glob, pathlib, shutil, sys
args = sys.argv[1:]
assert args[:2] == ['storage', 'cp'], args
args = args[2:]
dest = pathlib.Path(args[-1])
sources = sys.stdin.read().splitlines() if args[0] == '-I' else args[:-1]
for source in sources:
    matches = glob.glob(str(pathlib.Path({str(bucket)!r}) / source.removeprefix('gs://audit/')))
    assert matches, source
    for match in matches:
        shutil.copy(match, dest)
''', True)
        env = dict(os.environ, OA_STAGE_METADATA=str(metadata),
                   OA_STAGE_ROOT=str(root / "stage"), OA_STAGE_OPT=str(root / "opt"),
                   OA_GCLOUD=str(root / "gcloud"), OA_NVIDIA_SMI=str(root / "smi"))

        def boot():
            return subprocess.run(
                ["sh", str(ROOT / "scripts/pylon/v41-stage-startup.sh")],
                env=env, capture_output=True, text=True,
            )

        first = boot()
        assert first.returncode == 0, (root / "stage/startup.log").read_text()
        write(bucket / worker_dir / f"manifests/{digest}.json", "corrupted manifest")
        second = boot()
        run = subprocess.run([str(root / "opt/stage-run")],
                             capture_output=True, text=True, timeout=5)
        readiness = {
            "first_boot_returncode": first.returncode,
            "second_boot_returncode": second.returncode,
            "ready_marker_after_failed_boot": (root / "stage/ready").exists(),
            "stage_run_after_failed_boot": run.stdout.strip(),
        }

        write(bucket / worker_dir / f"manifests/{digest}.json", manifest)
        replacement = '#!/bin/sh\necho replacement-worker\n'
        write(bucket / worker, replacement, True)
        third = boot()
        run = subprocess.run([str(root / "opt/stage-run")],
                             capture_output=True, text=True, timeout=5)
        identity = {
            "boot_returncode_with_replaced_worker": third.returncode,
            "build_record_worker_sha256": original_sha,
            "installed_worker_sha256": hashlib.sha256((root / "opt/stage-worker").read_bytes()).hexdigest(),
            "stage_run": run.stdout.strip(),
            "scope": "Shell worker substitutes test artifact pin enforcement, not CUDA execution.",
        }
        return {"stale_ready_marker": readiness, "unverified_worker_identity": identity}


def rng_probe(scratch: Path):
    path = ROOT / "crates/psionic/crates/psionic-v41-stage/src/cuda/synthetic.rs"
    source = path.read_text()
    code = "struct Rng(u64);" + source.split("struct Rng(u64);", 1)[1].split("fn scale_exponent", 1)[0]
    code += '''
fn main() {
    let mut rng = Rng(11);
    let (mut lo, mut hi, mut sum) = (f32::INFINITY, f32::NEG_INFINITY, 0f64);
    let mut outside = 0;
    for _ in 0..4096 {
        let x = rng.unit();
        lo = lo.min(x); hi = hi.max(x); sum += f64::from(x);
        outside += usize::from(!(-1.0..1.0).contains(&x));
    }
    println!("{{\\"min\\":{lo},\\"max\\":{hi},\\"mean\\":{},\\"outside_declared_range\\":{outside}}}", sum/4096.0);
}
'''
    with tempfile.TemporaryDirectory(dir=scratch, prefix="v41-rng-audit-") as temp:
        program = Path(temp) / "probe.rs"
        binary = Path(temp) / "probe"
        program.write_text(code)
        subprocess.run(["rustc", "--edition=2021", str(program), "-o", str(binary)],
                       cwd=ROOT, check=True, capture_output=True, text=True)
        result = json.loads(subprocess.check_output([str(binary)], text=True))
    result.update({"samples": 4096, "seed": 11,
                   "method": "Compile the unchanged Rng implementation extracted from synthetic.rs.",
                   "compiler": subprocess.check_output(["rustc", "--version"], cwd=ROOT, text=True).strip()})
    return result


def main():
    global ROOT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, default=ROOT,
                        help="checkout containing the audited source, such as commit 4dab114ee5")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    ROOT = args.source_root.resolve()
    retained = json.loads(Path(__file__).with_name("results.json").read_text())
    changed = [name for name, want in retained["source_sha256"].items()
               if not (ROOT / name).is_file()
               or hashlib.sha256((ROOT / name).read_bytes()).hexdigest() != want]
    if changed:
        parser.error("this reproduces historical findings, and the audited source differs: "
                     + ", ".join(changed)
                     + ". Use --source-root with a checkout of 4dab114ee5; "
                     "the follow-up-reproduce.py script checks the later fixes.")
    scratch = Path(subprocess.check_output(["openagents", "scratch"], text=True).strip())
    baseline = subprocess.run(
        ["python3", "-m", "unittest", "scripts/dev/test_cloud_budget.py"],
        cwd=ROOT, capture_output=True, text=True,
    )
    cuda_build = ROOT / "crates/psionic/crates/psionic-backend-cuda/build.rs"
    tracked = re.findall(r"rerun-if-env-changed=([A-Z_]+)", cuda_build.read_text())
    reviewed_files = [
        "scripts/dev/cloud-budget.py", "scripts/pylon/v41-stage-startup.sh",
        "scripts/pylon/v41-stage-build.sh",
        "crates/psionic/crates/psionic-backend-cuda/build.rs",
        "crates/psionic/crates/psionic-v41-stage/src/cuda.rs",
        "crates/psionic/crates/psionic-v41-stage/src/cuda/synthetic.rs",
        "crates/psionic/crates/psionic-v41-stage/src/serve.rs",
        "crates/psionic/crates/psionic-v41-stage/src/lib.rs",
        "crates/psionic/crates/psionic-models/src/stage_state.rs",
    ]
    result = {
        "schema": "openagents.deepseek_v41.six_hour_audit.v1",
        "reviewed_head": "30ba21cb1efab4097872a7b49e438e1d947924e7",
        "tested_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "window_utc": ["2026-10-21T07:03:36Z", "2026-10-21T13:03:36Z"],
        "source_sha256": {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
                          for name in reviewed_files},
        "baseline_guard_tests": {"returncode": baseline.returncode,
                                 "output": (baseline.stdout + baseline.stderr).strip()},
        "billing": billing_probes(),
        "startup": startup_probes(scratch),
        "synthetic_rng": rng_probe(scratch),
        "cuda_arch_invalidation": {
            "kind": "Source inspection; no CUDA build performed.",
            "tracked_environment": tracked,
            "untracked_architecture_environment": [name for name in ["CUDAARCHS", "PSI_CUDA_ARCH"]
                                                  if name not in tracked],
        },
        "cuda_prefill_wire_lower_bound": {
            "tokens": 3277, "streams": 4, "hidden": 5120, "bytes_per_f32": 4,
            "residual_only_bytes": 3277 * 4 * 5120 * 4,
            "codec_max_section_bytes": 256 << 20,
            "worker_default_max_context": 8192,
            "kind": "Dimension calculation from source; no GPU or large allocation.",
        },
        "cloud_access": False,
    }
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
