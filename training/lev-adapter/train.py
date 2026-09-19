#!/usr/bin/env python3
"""Trains a LoRA adapter on the converted suite, through Apple's toolkit.

    python3 convert.py --out data/
    python3 train.py --data data/ --out runs/lev-v1

This is a driver, not a trainer. The training loop, the base model assets, and
the LoRA implementation are the toolkit's; this script locates it, checks it
against the device, passes the converted data in, and records what it ran so
the result is reproducible.

Everything about the recipe that is ours is here rather than in a notebook:
rank, epochs, learning rate, and the seed.
"""

import argparse
import json
import pathlib
import subprocess
import sys
import time

import toolkit

# Flags match Apple's documented `examples.train_adapter` CLI exactly. The
# toolkit owns rank and seed; they are not exposed there, so they are not
# passed.
DEFAULTS = {
    # Apple documents 5. 98 training records is a small corpus and more
    # epochs on less data overfits, so start one below and let the evaluation
    # split decide.
    "epochs": 4,
    # Apple documents 1e-3. Kev's research log found the single largest
    # quality effect it measured was that too high a rate erodes the base
    # knowledge the task depends on, so this starts an order lower and moves
    # only on evidence.
    "learning_rate": 1e-4,
    "batch_size": 4,
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data", default="data", help="directory holding train.jsonl and valid.jsonl")
    parser.add_argument("--out", default="runs/lev-v1")
    parser.add_argument("--toolkit", help="toolkit root, or set LEV_TOOLKIT_ROOT")
    parser.add_argument("--epochs", type=int, default=DEFAULTS["epochs"])
    parser.add_argument("--learning-rate", type=float, default=DEFAULTS["learning_rate"])
    parser.add_argument("--batch-size", type=int, default=DEFAULTS["batch_size"])
    args = parser.parse_args()

    root = toolkit.find(args.toolkit)
    data = pathlib.Path(args.data)
    train = data / "train.jsonl"
    valid = data / "valid.jsonl"
    for path in (train, valid):
        if not path.exists():
            print(f"missing {path}; run convert.py first", file=sys.stderr)
            return 2

    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    try:
        prefix = toolkit.device_signature_prefix()
    except Exception as error:  # noqa: BLE001
        print(f"warning: could not read the device signature: {error}", file=sys.stderr)
        prefix = None

    command = [
        sys.executable,
        "-m",
        "examples.train_adapter",
        "--train-data",
        str(train.resolve()),
        "--eval-data",
        str(valid.resolve()),
        "--checkpoint-dir",
        str(out.resolve()) + "/",
        "--epochs",
        str(args.epochs),
        "--learning-rate",
        str(args.learning_rate),
        "--batch-size",
        str(args.batch_size),
    ]

    record = {
        "toolkit": str(root),
        "device_signature_prefix": prefix,
        "command": command,
        "recipe": {
            "epochs": args.epochs,
            "learning_rate": args.learning_rate,
            "batch_size": args.batch_size,
        },
        "data": {
            "train": str(train.resolve()),
            "valid": str(valid.resolve()),
            "conversion": json.loads((data / "conversion.json").read_text())
            if (data / "conversion.json").exists()
            else None,
        },
        "started": time.strftime("%Y-%m-%dT%H:%M:%S"),
    }
    (out / "run.json").write_text(json.dumps(record, indent=1) + "\n")
    print(json.dumps(record, indent=1), file=sys.stderr)

    # The toolkit's example entry point is versioned. This matches the
    # published 26.0.0 CLI; if it moves, say so rather than failing with an
    # import error the operator has to decode.
    result = subprocess.run(command, cwd=root, check=False)
    if result.returncode != 0:
        print(
            f"\ntraining exited {result.returncode}. If the entry point moved, look for the\n"
            f"training example under {root}/examples and pass its module path.",
            file=sys.stderr,
        )
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
