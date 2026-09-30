#!/usr/bin/env python3
"""Run the offline native chat matrix and retain its measurements."""
import argparse
import json
import math
import pathlib
import subprocess


def percentile(values, fraction):
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=pathlib.Path)
    parser.add_argument("output", type=pathlib.Path)
    parser.add_argument("--summarize", action="store_true", help="Read existing runs.")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    results = []
    for minimum in (False, True):
        for scale in (1, 2):
            for backdrop in (False, True):
                name = f"{'minimum' if minimum else 'default'}-{scale}x-{'verse' if backdrop else 'plain'}"
                directory = args.output / name
                if not args.summarize:
                    command = [str(args.binary.resolve()), "--chat-benchmark", str(directory.resolve()),
                               "--benchmark-scale", str(scale)]
                    if minimum:
                        command.append("--benchmark-minimum")
                    if not backdrop:
                        command.append("--no-backdrop")
                    directory.mkdir(parents=True, exist_ok=True)
                    print(f"Running {name}", flush=True)
                    with (directory / "run.log").open("w") as log:
                        subprocess.run(command, stdout=log, stderr=subprocess.STDOUT,
                                       timeout=90, check=True)
                report = json.loads((directory / "native.json").read_text())
                phases = {}
                for phase in ("scroll", "streaming", "sidebar"):
                    samples = [sample for sample in report["samples"] if sample["phase"] == phase]
                    if len(samples) < 100:
                        raise RuntimeError(f"Incomplete {name}/{phase}: {len(samples)} samples")
                    values = [(sample["step_us"] + sample["frame"]["total_us"]) / 1000
                              for sample in samples]
                    phases[phase] = {"samples": len(samples), "p50_ms": percentile(values, .5),
                                     "p99_ms": percentile(values, .99), "max_ms": max(values)}
                resources = {phase["phase"]: phase for phase in report["phases"]}
                results.append({"case": name, "platform": report["platform"],
                                "points": report["points"], "scale": scale, "backdrop": backdrop,
                                "phases": phases, "resources": resources,
                                "scroll_pass": phases["scroll"]["p99_ms"] < 8.3})
                print(f"{name}: scroll p99={phases['scroll']['p99_ms']:.3f} ms", flush=True)
    (args.output / "summary.json").write_text(json.dumps(results, indent=2) + "\n")
    if not all(result["scroll_pass"] for result in results):
        raise SystemExit("Scrolling exceeds 8.3 ms; retain evidence and document a plan.")


if __name__ == "__main__":
    main()
