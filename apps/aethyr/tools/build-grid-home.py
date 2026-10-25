#!/usr/bin/env python3
"""Bundle the Aethyr af-home renderer for the native Grid-only home."""
import argparse,base64,hashlib,json,os,shutil,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
p=argparse.ArgumentParser();p.add_argument("--wasm",required=True,type=Path);a=p.parse_args()
out=ROOT/"apps/openagents/apps/ios/OpenAgents/Resources/AethyrHome.bundle";out.mkdir(parents=True,exist_ok=True)
esbuild=os.environ.get("ESBUILD",str(ROOT/"apps/aethyr/node_modules/.bin/esbuild"))
subprocess.run([esbuild,str(ROOT/"apps/aethyr/apps/grid-home/main.ts"),"--bundle","--minify","--format=iife","--target=safari26",f"--define:__AETHYR_HOME_WASM__={json.dumps(base64.b64encode(a.wasm.read_bytes()).decode())}",f"--outfile={out/'home.js'}"],check=True)
shutil.copyfile(ROOT/"apps/aethyr/apps/grid-home/home.html",out/"home.html")
shutil.copyfile(ROOT/"apps/openagents/apps/ios/OpenAgents/Resources/AethyrGrid.bundle/Michroma-Regular.ttf",out/"Michroma-Regular.ttf")
files={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(out.iterdir()) if p.name!="manifest.json"}
sources=["crates/aethyr/home/src/lib.rs","crates/aethyr/home/src/field.wgsl","apps/aethyr/apps/grid-home/main.ts","apps/aethyr/apps/grid-home/home.html","apps/aethyr/apps/render-lab/gpu-imports.ts","apps/aethyr/apps/render-lab/gpu-batch.ts"]
(out/"manifest.json").write_text(json.dumps({"format":"openagents.aethyr-home.bundle.v1","renderer":"af-home / af-gpu","files":files,"sources":{p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sources}},indent=2)+"\n")
