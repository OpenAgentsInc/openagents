#!/usr/bin/env python3
"""The actual Coax details fold against the TS oracle, not event admission."""
import json, os, subprocess, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
if not __debug__: raise SystemExit('run without optimized Python')
def run(*args, data=None):
 p=subprocess.run([str(x) for x in args],input=data,capture_output=True)
 if p.returncode: raise RuntimeError(f'{args[0]} failed {p.returncode}: {p.stderr[-2000:].decode(errors="replace")}')
 return p.stdout
cases=json.loads(run('bun',ROOT/'coax/web/project/details-fixture.ts'))
with tempfile.TemporaryDirectory(prefix='coax-project-details-') as tmp:
 d=Path(tmp); runner=d/'run.mjs'
 runner.write_text("import{WASI}from'node:wasi';import{readFileSync}from'node:fs';const w=new WASI({version:'preview1',args:process.argv.slice(2),env:{},preopens:{}});const{instance}=await WebAssembly.instantiate(readFileSync(process.argv[2]),{wasi_snapshot_preview1:w.wasiImport});w.start(instance);\n")
 for mode,flags in [('checked',[]),('release',['--release'])]:
  src=d/(mode+'.c'); header=d/'project_details_adapter.h'
  src.write_bytes(run(ROOT/'coax/bin/coax','c',ROOT/'coax/tests/nostr/project_details_adapter.coax','--library','--header',header,*flags))
  common=['-std=c11','-O1','-I'+str(d),src,ROOT/'coax/tests/nostr/project_details.c','-lm']
  exe=d/(mode+'-san'); run('cc',*common,'-fsanitize=address,undefined','-fno-sanitize-recover=all','-o',exe)
  clang=os.environ.get('COAXIAL_CLANG','/opt/homebrew/opt/llvm/bin/clang'); sysroot=os.environ.get('COAXIAL_SYSROOT','/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'); resources=os.environ.get('COAXIAL_RESOURCES','/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes')
  wasm=d/(mode+'.wasm'); run(clang,'--target=wasm32-wasip1','--sysroot='+sysroot,'-resource-dir='+resources,*common,'-Wl,-z,stack-size=1048576,--max-memory=67108864','-o',wasm)
  for target in ([exe],['node','--no-warnings',runner,wasm]):
   for case in cases:
    actual=run(*target,data=case['input'].encode()).decode()
    assert actual==case['expected'],f'{mode} {target[0]} input {case["input"]!r}: actual {actual!r}, expected {case["expected"]!r}'
  for case in cases: assert case['oracle']==case['expected'],f'TS body input {case["input"]!r}: {case["oracle"]!r} != {case["expected"]!r}'
  print(mode,'details matched normalized oracle on native sanitizers and Wasm',flush=True)
