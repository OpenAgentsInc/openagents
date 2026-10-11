#!/usr/bin/env python3
"""Launch only the task-owned diagnostic with the existing reference guard."""
import argparse
import json
import os
from pathlib import Path
import sys

ROOT = Path("repo")
TOOLS = ROOT / "crates/psionic/fixtures/clef/tools"
PYTHON = Path("openagents-scratch/codex-01a128ee-daef-7590-baae-4e932a581a2c/clef-image-reference/bin/python")
SCRIPT = Path(__file__).with_name("cpu_head_reference.py")
sys.path.insert(0, str(TOOLS))
import guarded_reference
from cpu_head_reference import require, sha


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    resources = set(os.environ.get("OPENAGENTS_LEASES", "").split(","))
    require({"memory", "gpu"} <= resources and "quiet" not in resources, "requires memory and GPU leases; quiet is forbidden")
    require(not args.output_dir.exists(), "output exists; preserve earlier evidence")
    os.environ.update(CLEF_CPU_HEAD_REFERENCE_GUARDED="1", RAYON_NUM_THREADS="2", PYTHONDONTWRITEBYTECODE="1",
                      CUDA_VISIBLE_DEVICES="", PYTORCH_ENABLE_MPS_FALLBACK="0")
    command = [str(PYTHON), str(SCRIPT), "--output-dir", str((args.output_dir / "results").resolve())]
    identity = {"script_sha256": sha(SCRIPT), "wrapper_sha256": sha(Path(__file__)),
                "guard_sha256": sha(TOOLS / "guarded_reference.py"),
                "memory_helper_sha256": sha(TOOLS / "mac_quiet_bench.py"),
                "conditions": "Shared machine, diagnostic only; existing goldens remain unchanged.",
                "owned_group_cap_gib": 72, "start_available_gib": 84, "available_floor_gib": 12,
                "mps_allocator_cap_gib": 60, "deadline_seconds": 900,
                "peak_assessment": "Retains the existing full BF16 reference cap; no assumption that the CPU-head change reduces peak memory."}
    result = guarded_reference.run(argparse.Namespace(output=args.output_dir, command=command,
        max_rss_gib=72.0, minimum_start_available_gib=84.0, minimum_available_gib=12.0, deadline_seconds=900.0))
    if args.output_dir.exists():
        (args.output_dir / "execution-identity.json").write_text(json.dumps(identity, indent=2) + "\n")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
