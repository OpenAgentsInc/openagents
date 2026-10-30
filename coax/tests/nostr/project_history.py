#!/usr/bin/env python3
"""Signed public fixtures: real Coax Fold history agrees with the TS oracle.

This checks log projection only, not cryptographic verification or relay trust.
The test key is the public scalar 1 fixture and is never used outside tests.
"""
import json, os, subprocess, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
if not __debug__: raise SystemExit('run without optimized Python')
def run(*args, **kw):
 p=subprocess.run([str(x) for x in args],text=True,capture_output=True,**kw)
 if p.returncode: raise RuntimeError(f'{args[0]} failed {p.returncode}: {p.stderr[-2000:]}')
 return p.stdout
fixture=json.loads(run('bun',ROOT/'coax/web/project/history-fixture.ts'))
frames=''.join(json.dumps(['EVENT','issues',e],separators=(',',':'))+'\n' for e in fixture['events'])
with tempfile.TemporaryDirectory(prefix='coax-project-history-') as tmp:
 d=Path(tmp);runner=d/'run.mjs'
 runner.write_text("import{WASI}from'node:wasi';import{readFileSync}from'node:fs';const w=new WASI({version:'preview1',args:process.argv.slice(2),env:{},preopens:{}});const{instance}=await WebAssembly.instantiate(readFileSync(process.argv[2]),{wasi_snapshot_preview1:w.wasiImport});w.start(instance);\n")
 for mode,flags in [('checked',[]),('release',['--release'])]:
  source=d/(mode+'.c');header=d/'project_history_adapter.h'
  source.write_text(run(ROOT/'coax/bin/coax','c',ROOT/'coax/tests/nostr/project_history_adapter.coax','--library','--header',header,*flags))
  common=['-std=c11','-O1','-I'+str(d),source,ROOT/'coax/tests/nostr/project_history.c','-lm']
  exe=d/(mode+'-san');run('cc',*common,'-fsanitize=address,undefined','-fno-sanitize-recover=all','-o',exe)
  actual=run(exe,fixture['pk'],input=frames,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'})
  assert actual==fixture['expected'],f'{mode} native log differs:\nactual {actual!r}\nexpected {fixture["expected"]!r}'
  wasm=d/(mode+'.wasm')
  clang=os.environ.get('COAXIAL_CLANG','/opt/homebrew/opt/llvm/bin/clang')
  sysroot=os.environ.get('COAXIAL_SYSROOT','/opt/homebrew/opt/wasi-libc/share/wasi-sysroot')
  resources=os.environ.get('COAXIAL_RESOURCES','/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes')
  run(clang,'--target=wasm32-wasip1','--sysroot='+sysroot,'-resource-dir='+resources,*common,'-Wl,-z,stack-size=1048576,--max-memory=67108864','-o',wasm)
  actual=run('node','--no-warnings',runner,wasm,fixture['pk'],input=frames)
  assert actual==fixture['expected'],f'{mode} Wasm log differs'
  print(mode,'signed history fixture matched the TS oracle on native sanitizers and Wasm',flush=True)
