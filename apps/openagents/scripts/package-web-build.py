#!/usr/bin/env python3
"""Check a GPUI module's transfer budget and name its assets by content."""
import gzip
import hashlib
import json
import os
import re
import shutil
from pathlib import Path
import subprocess
import sys

BUDGET = 1_500_000

# Modules allowed more than BUDGET, with brotli. The `/gpui` chat draws the
# desktop's own transcript (#12216): the last visit's first frame paints in
# about 0.1 s before the module arrives, and the extra 250 KB costs about
# 0.1 s at 20 Mbit, which beats keeping a second transcript (#12219).
BUDGETS = {"openagents_web_chat_bg.wasm": 1_750_000}


def budget(module):
    """The brotli cap for `module`."""
    return BUDGETS.get(module, BUDGET)


def commit():
    """The build's commit: OPENAGENTS_COMMIT, else git's, else "unknown" (the
    website image's build stage has neither git nor the repository)."""
    named = os.environ.get("OPENAGENTS_COMMIT", "").strip()
    if named:
        return named
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], text=True, stderr=subprocess.DEVNULL).strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def measure(directory, wasm):
    """The module's brotli size, from a fresh compression: a stale .br must
    never pass."""
    subprocess.run(["brotli", "-q", "11", "-f", str(directory / wasm)], check=True)
    return {
        "module": wasm,
        "raw_bytes": (directory / wasm).stat().st_size,
        "brotli_bytes": (directory / (wasm + ".br")).stat().st_size,
    }


def package(directory, name, *later):
    """Package the page's module `name` and the modules its start script
    loads `later` (each under the same budget), naming every asset by
    content."""
    directory = Path(directory)
    wasm = name + "_bg.wasm"
    measured = [measure(directory, module + "_bg.wasm") for module in (name, *later)]
    report = {"commit": commit(), **measured[0], "budget_bytes": budget(measured[0]["module"])}
    if later:
        report["later"] = measured[1:]
    (directory / "size.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report), flush=True)
    for module in measured:
        cap = budget(module["module"])
        if module["brotli_bytes"] > cap:
            raise SystemExit(
                f"{module['module']}: {module['brotli_bytes']} bytes brotli exceeds the {cap} byte budget"
            )

    assets = {}

    def write(original, content):
        path = Path(original)
        digest = hashlib.sha256(content).hexdigest()
        hashed = f"{path.stem}.{digest}{path.suffix}"
        assets[original] = hashed
        target = directory / original
        target.write_bytes(content)
        target.with_name(original + ".gz").write_bytes(gzip.compress(content, mtime=0))
        if not original.endswith("_bg.wasm"):
            subprocess.run(["brotli", "-q", "11", "-f", str(target)], check=True)
        for suffix in ("", ".gz", ".br"):
            shutil.copyfile(directory / (original + suffix), directory / (hashed + suffix))
        return hashed

    source = (directory / "start.js").read_text()
    for module in (name, *later):
        wasm = module + "_bg.wasm"
        hashed_wasm = write(wasm, (directory / wasm).read_bytes())
        glue = module + ".js"
        glue_source = (directory / glue).read_text()
        if wasm not in glue_source:
            raise SystemExit(f"{glue}: wasm-bindgen no longer names {wasm}")
        hashed_glue = write(glue, glue_source.replace(wasm, hashed_wasm).encode())
        if glue not in source or wasm not in source:
            raise SystemExit(f"start.js must name both {glue} and its streaming module")
        source = source.replace(glue, hashed_glue).replace(wasm, hashed_wasm)
    # Read only fonts named by the source loader, not hashed leftovers from
    # an earlier build in this directory.
    fonts = re.findall(r'"(?:\./)?(Noto[^/"]+\.(?:ttf|otf))"', source)
    for filename in sorted(set(fonts)):
        hashed_font = write(filename, (directory / filename).read_bytes())
        source = source.replace(filename, hashed_font)
    write("start.js", source.encode())
    (directory / "assets.json").write_text(json.dumps(assets, indent=2) + "\n")


if __name__ == "__main__":
    if len(sys.argv) < 3:
        raise SystemExit("usage: package-web-build.py OUTDIR MODULE_NAME [LATER_MODULE...]")
    package(*sys.argv[1:])
