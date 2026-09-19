#!/usr/bin/env python3
"""Exports a trained checkpoint to a `.fmadapter` package and checks it.

    python3 export.py --run runs/lev-v1 --out runs/lev-v1/lev.fmadapter

Export is the toolkit's. The checking afterwards is ours, and it is the part
worth having: a package with correct metadata and correct tensor values is
still rejected by the runtime if the Core ML blob-storage layout is wrong, so
this verifies the package with the Rust reader and then asks the device to
load it before anyone tries to serve with it.
"""

import argparse
import json
import pathlib
import subprocess
import sys

import toolkit

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parents[1]


def verify(package: pathlib.Path):
    """Loads the package through the bridge, which is Apple's own reader."""
    helper = REPO / "swift/lev-bridge/.build/release/lev-bridge"
    request = json.dumps(
        {"id": "1", "op": "adapter_load", "adapterPath": str(package.resolve())}
    )
    result = subprocess.run(
        [str(helper)], input=request + "\n", capture_output=True, text=True, check=True
    )
    return json.loads(result.stdout.strip().splitlines()[0])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", default="runs/lev-v1")
    parser.add_argument(
        "--checkpoint",
        help="the trained checkpoint; defaults to adapter-final.pt inside --run",
    )
    parser.add_argument("--draft-checkpoint", help="only if a draft model was trained")
    parser.add_argument("--out")
    parser.add_argument("--name", default="lev")
    parser.add_argument("--toolkit")
    parser.add_argument("--author", default="openagents")
    parser.add_argument(
        "--description",
        default="Support-desk judgments for the Lev System One door, trained on support-v2.",
    )
    args = parser.parse_args()

    root = toolkit.find(args.toolkit)
    run = pathlib.Path(args.run)
    out = pathlib.Path(args.out) if args.out else run / f"{args.name}.fmadapter"

    # Apple's exporter takes the checkpoint file, not the directory holding
    # it. Passing the directory is the easy mistake and it fails late.
    checkpoint = (
        pathlib.Path(args.checkpoint) if args.checkpoint else run / "adapter-final.pt"
    )
    if not checkpoint.exists():
        print(
            f"no checkpoint at {checkpoint}. Training writes adapter-final.pt into\n"
            f"--checkpoint-dir; pass --checkpoint if yours is elsewhere.",
            file=sys.stderr,
        )
        return 2

    command = [
        sys.executable,
        "-m",
        "export.export_fmadapter",
        "--adapter-name",
        args.name,
        "--checkpoint",
        str(checkpoint.resolve()),
        "--output-dir",
        str(out.parent.resolve()) + "/",
    ]
    command += ["--author", args.author, "--description", args.description]
    if args.draft_checkpoint:
        command += ["--draft-checkpoint", str(pathlib.Path(args.draft_checkpoint).resolve())]
    print(" ".join(command), file=sys.stderr)
    result = subprocess.run(command, cwd=root, check=False)
    if result.returncode != 0:
        return result.returncode

    if not out.exists():
        print(f"the exporter did not produce {out}", file=sys.stderr)
        return 2

    # Our reader first: it names which rule failed.
    ours = subprocess.run(
        ["cargo", "run", "--quiet", "-p", "lev", "--bin", "lev-adapter-check", "--", str(out)],
        cwd=REPO,
        check=False,
    )
    if ours.returncode != 0:
        print("the package did not pass this repository's checks", file=sys.stderr)
        return ours.returncode

    # Then Apple's, which is the one that decides.
    response = verify(out)
    if not response.get("ok"):
        print(json.dumps(response, indent=1), file=sys.stderr)
        return 3
    print(f"{out} loads on this device; metadata {response.get('adapterMetadata')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
