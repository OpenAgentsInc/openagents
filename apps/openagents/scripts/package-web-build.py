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
from concurrent.futures import ThreadPoolExecutor

BUDGET = 1_500_000

# Modules allowed more than BUDGET, with brotli. The `/gpui` chat draws the
# desktop's own transcript (#12216): the last visit's first frame paints in
# about 0.1 s before the module arrives, and the extra 250 KB costs about
# 0.1 s at 20 Mbit, which beats keeping a second transcript (#12219).
BUDGETS = {
    "openagents_web_chat_bg.wasm": 1_750_000,
    "openagents_web_chat_webgl_bg.wasm": 1_750_000,
}


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


def contributors(module, limit=15):
    """The crates taking the most bytes in `module`'s release build, for an
    over-budget report: the names-carrying wasm in WEB_BUILD_NAMES_DIR (the
    Cargo target's `wasm32-unknown-unknown/release`), read with `twiggy`.
    Empty when either is missing."""
    names = os.environ.get("WEB_BUILD_NAMES_DIR", "")
    source = Path(names) / (module.removesuffix("_bg.wasm") + ".wasm") if names else None
    if source and not source.is_file() and "_webgl_bg.wasm" in module:
        source = Path(names) / (module.removesuffix("_webgl_bg.wasm") + ".wasm")
    if not source or not source.is_file() or not shutil.which("twiggy"):
        return []
    try:
        listing = subprocess.check_output(
            ["twiggy", "top", "-n", "1000000", "-f", "json", str(source)],
            text=True, stderr=subprocess.DEVNULL)
    except (OSError, subprocess.CalledProcessError):
        return []
    sizes = {}
    for item in json.loads(listing):
        name = item["name"]
        if name.startswith(('"function names"', "custom section", "...", "\u03a3")):
            continue
        found = re.match(r"^(?:dyn )?([A-Za-z_][A-Za-z0-9_]*)(?:\[[0-9a-f]+\])?::", name.lstrip("<&* "))
        crate = found.group(1) if found else ("data" if name.startswith("data segment") else "other")
        sizes[crate] = sizes.get(crate, 0) + item["shallow_size"]
    return sorted(sizes.items(), key=lambda pair: -pair[1])[:limit]


def check(directory, module, measured=None):
    """Refuse `module` over its brotli budget, naming its largest crates.
    `measured` is the module's measure() result when the caller has one."""
    if measured is None:
        measured = measure(Path(directory), module)
    cap = budget(module)
    if measured["brotli_bytes"] > cap:
        lines = [f"{module}: {measured['brotli_bytes']} bytes brotli exceeds the {cap} byte budget."]
        top = contributors(module)
        if top:
            lines.append("Largest crates in the release build (raw bytes, before wasm-opt):")
            lines.extend(f"  {size:>9}  {crate}" for crate, size in top)
        lines.append("Move page-like features into their own module (web/page-host) rather than raising the budget.")
        if os.environ.get("OA_STAGING_BUILD") == "1":
            # Staging-only builds skip wasm-opt and compress lightly
            # (scripts/deploy/staging-build-env.sh), so they run over; the
            # production build still enforces the budget.
            print("warning (staging build): " + "\n".join(lines), file=sys.stderr, flush=True)
            return measured
        raise SystemExit("\n".join(lines))
    return measured


def package(directory, name, *later):
    """Package the page's module `name` and the modules its start script
    loads `later` (each under the same budget), naming every asset by
    content."""
    directory = Path(directory)
    wasm = name + "_bg.wasm"
    # brotli -q 11 takes about 12 s a module on one core; a page has dozens
    # (#12417), so measure them side by side.
    modules = [module + "_bg.wasm" for module in (name, *later)]
    workers = int(os.environ.get("WEB_BUILD_JOBS") or os.cpu_count() or 4)
    with ThreadPoolExecutor(max_workers=max(1, workers)) as pool:
        measured = list(pool.map(lambda module: measure(directory, module), modules))
    report = {"commit": commit(), **measured[0], "budget_bytes": budget(measured[0]["module"])}
    if later:
        report["later"] = measured[1:]
    (directory / "size.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report), flush=True)
    for module in measured:
        check(directory, module["module"], module)

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
    if (directory / "renderer.js").exists():
        hashed_renderer = write("renderer.js", (directory / "renderer.js").read_bytes())
        source = source.replace('"./renderer.js"', '"./' + hashed_renderer + '"')
    # Read only fonts named by the source loader, not hashed leftovers from
    # an earlier build in this directory.
    fonts = re.findall(r'"(?:\./)?(Noto[^/"]+\.(?:ttf|otf))"', source)
    for filename in sorted(set(fonts)):
        hashed_font = write(filename, (directory / filename).read_bytes())
        source = source.replace(filename, hashed_font)
    write("start.js", source.encode())
    (directory / "assets.json").write_text(json.dumps(assets, indent=2) + "\n")


if __name__ == "__main__":
    if len(sys.argv) == 4 and sys.argv[1] == "--check":
        # One module, right after it is built (build-web-chat.sh): fail early.
        print(json.dumps(check(sys.argv[2], sys.argv[3])), flush=True)
        raise SystemExit(0)
    if len(sys.argv) < 3:
        raise SystemExit("usage: package-web-build.py OUTDIR MODULE_NAME [LATER_MODULE...]")
    package(*sys.argv[1:])
