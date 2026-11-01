#!/usr/bin/env python3
"""Compile checked collision and the published walker separately, then compare their native contacts."""
import argparse
import os
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
LEGACY_COMMIT = "3c3b4c3a39189e6b61e92307e2501e7a26c007bc"


def run(command, *, stdout=subprocess.PIPE):
    result = subprocess.run(command, cwd=ROOT, stdout=stdout, stderr=subprocess.PIPE, timeout=180)
    if result.returncode:
        raise RuntimeError(f"{' '.join(map(str, command))}\n{result.stderr.decode(errors='replace')[-4000:]}")
    return result.stdout


def frozen_legacy(work):
    """Read the pinned reference closure; never follow the adopted live player."""
    mirror = (work / "published").resolve()
    seen = set()

    def materialize(relative):
        path = (mirror / relative).resolve()
        relative = str(path.relative_to(mirror))
        if relative in seen:
            return
        seen.add(relative)
        data = run(["git", "show", f"{LEGACY_COMMIT}:{relative}"])
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        for name in re.findall(rb'^\s*#load\s+"([^"]+)"', data, re.MULTILINE):
            dependency = str((path.parent / name.decode()).resolve().relative_to(mirror))
            materialize(dependency)

    materialize("verse/player.coax")
    adapter = mirror / "aethyr/tests/collide-legacy.coax"
    adapter.parent.mkdir(parents=True, exist_ok=True)
    adapter.write_bytes((ROOT / "aethyr/tests/collide-legacy.coax").read_bytes())
    return adapter


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--coax", default=str(ROOT / "coax/bin/coax"))
    parser.add_argument("--sanitize", action="store_true")
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="aethyr-collide-") as tmp:
        work = Path(tmp)
        sources = []
        for name in ("native", "legacy"):
            code = work / f"{name}.c"
            source = ROOT / f"aethyr/tests/collide-{name}.coax" if name == "native" else frozen_legacy(work)
            command = [args.coax, "c", str(source), "--library"]
            if args.release:
                command.append("--release")
            with code.open("wb") as output:
                run(command, stdout=output)
            sources.append(str(code))
        binary = work / "contacts"
        command = [os.environ.get("CC", "cc"), "-std=c11", "-O2" if args.release else "-O0",
                   "-fno-fast-math", "-ffp-contract=off"]
        if args.sanitize:
            command.extend(["-fsanitize=address,undefined", "-fno-sanitize-recover=all", "-fno-omit-frame-pointer"])
        command.extend([*sources, str(ROOT / "aethyr/tests/collide-native.c"), "-lm", "-o", str(binary)])
        run(command)
        print(run([str(binary)]).decode(), end="")


if __name__ == "__main__":
    main()
