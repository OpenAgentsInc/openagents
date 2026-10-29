#!/usr/bin/env python3
"""Conditional source-bound proof controls and independent public arithmetic witnesses."""
import argparse
import ctypes as C
import hashlib
import json
import os
import platform
from pathlib import Path
import random
import signal
import shutil
import subprocess
import sys
import tempfile

ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'coax/tests'))
from quiet_crash import run as run_quiet_crash
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
ADAPTER=ROOT/'coax/tests/nostr/field_guard_adapter.coax'
SANITIZER=ROOT/'coax/tests/nostr/field_guard_sanitized.c'
PROOF=ROOT/'tools/coax-nostr/field-guards/prove.py'
CRASH_LAUNCHER=ROOT/'coax/tests/quiet_crash.py'
P=(1<<256)-(1<<32)-977
class U32(C.Structure):_fields_=[('data',C.POINTER(C.c_uint32)),('count',C.c_int64)]
class U64(C.Structure):_fields_=[('data',C.POINTER(C.c_uint64)),('count',C.c_int64)]
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def run(*args,**kwargs):return subprocess.run([str(x) for x in args],check=True,capture_output=True,text=True,**kwargs)
def library(path):
 lib=C.CDLL(str(path));lib.field_guard_normalize.argtypes=[U64,U64];lib.field_guard_normalize.restype=C.c_bool
 for name in ['field_guard_add32','field_guard_mul32']:
  f=getattr(lib,name);f.argtypes=[U32,U32,U32];f.restype=C.c_bool
 return lib
def words(n):return [(n>>(32*i))&0xffffffff for i in range(8)]
def packed(values):return sum(v<<(32*i) for i,v in enumerate(values))
def invalid(path):
 lib=library(path);a=(C.c_uint32*8)(*([0xffffffff]*8));out=(C.c_uint32*8)()
 lib.field_guard_add32(U32(a,8),U32(a,8),U32(out,8))
 raise SystemExit(0)
def invalid_normalize(path):
 lib=library(path);a=(C.c_uint64*10)(*([0xffffffffffffffff]*10));out=(C.c_uint64*10)()
 lib.field_guard_normalize(U64(a,10),U64(out,10))
 raise SystemExit(0)

def main():
 ap=argparse.ArgumentParser();ap.add_argument('--invalid-add',type=Path);ap.add_argument('--invalid-normalize',type=Path);ap.add_argument('--report',type=Path);ap.add_argument('--retain',type=Path);args=ap.parse_args()
 if not __debug__:raise SystemExit('witness runner requires Python assertions')
 if args.invalid_add:invalid(args.invalid_add);return
 if args.invalid_normalize:invalid_normalize(args.invalid_normalize);return
 if args.retain:args.retain.mkdir(parents=True,exist_ok=False)
 helper=COMPILER.parent/'provenance-check';record=COMPILER.parent/'provenance.record';provenance=ROOT/'coax/provenance.sh'
 run(provenance,'verify',ROOT,COMPILER,helper,record)
 results={'schema':'coax.field-guard-witnesses.v1','scope':'conditional math and native arithmetic witnesses; not a production-secret approval','compiler':run(COMPILER,'--version').stdout.strip(),'compiler_sha256':sha(COMPILER),'compiler_provenance_helper_sha256':sha(helper),'compiler_provenance_record_sha256':sha(record),'native_compiler':run('cc','--version').stdout.splitlines()[0],'host':platform.platform(),'adapter_sha256':sha(ADAPTER),'sanitizer_sha256':sha(SANITIZER),'proof_sha256':sha(PROOF),'witness_sha256':sha(__file__),'crash_launcher_sha256':sha(CRASH_LAUNCHER),'seed':4405,'modes':{}}
 with tempfile.TemporaryDirectory(prefix='coax-field-guards-') as tmp:
  tmp=Path(tmp);proof=tmp/'proof.json';run(sys.executable,PROOF,'--report',proof);result=json.loads(proof.read_text());results['proof']=result
  if args.retain:shutil.copyfile(proof,args.retain/'proof.json')
  altered=tmp/'altered';
  for path in result['source_sha256']:
   target=altered/path;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(ROOT/path,target)
  field=altered/'coax/lib/crypto/secp_field.coax';field.write_text(field.read_text()+'\n// changed source control\n')
  changed=subprocess.run([sys.executable,str(PROOF),'--root',str(altered)],capture_output=True,text=True)
  assert changed.returncode!=0 and 'source pin mismatch' in changed.stderr;results['changed_source_refused']=True
  for mode,flags in [('checked',[]),('release',['--release'])]:
   c=tmp/(mode+'.c');header=tmp/'field_guard_adapter.h';artifact=tmp/(mode+'.dylib');source_map=tmp/(mode+'-map.json');sanitized=tmp/(mode+'-sanitized')
   assert sha(COMPILER)==results['compiler_sha256']
   run(COMPILER,'c',ADAPTER,'--library','--header',header,'--source-map',source_map,'-o',c,*flags)
   metadata=json.loads(source_map.read_text());sources=metadata['source_hashes'];recorded={};assert metadata['sources']==list(sources)
   for name,expected in sources.items():
    source=Path(name).resolve();assert sha(source)==expected;relative=str(source.relative_to(ROOT.resolve()))
    if relative in result['source_sha256']:assert expected==result['source_sha256'][relative]
    if source==ADAPTER.resolve():assert expected==results['adapter_sha256']
    recorded[relative]=expected
   assert str(ADAPTER.relative_to(ROOT)) in recorded and 'coax/lib/crypto/secp_field.coax' in recorded and 'coax/lib/crypto/secp_field32.coax' in recorded
   run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',artifact);lib=library(artifact);rng=random.Random(4405)
   run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(tmp),c,SANITIZER,'-lm','-o',sanitized);run(sanitized,timeout=30)
   limits=[8*((1<<(22 if i==9 else 26))-1) for i in range(10)]
   normal=[[0]*10,limits[:]]+[[limits[j] if i==j else 0 for i in range(10)] for j in range(10)]
   normal += [[rng.randrange(n+1) for n in limits] for _ in range(512)]
   for raw in normal:
    a=(C.c_uint64*10)(*raw);out=(C.c_uint64*10)();assert lib.field_guard_normalize(U64(a,10),U64(out,10))
    result_value=sum(int(out[i])<<(26*i) for i in range(10));want=sum(raw[i]<<(26*i) for i in range(10))%P
    assert result_value==want and result_value<P and all(out[i]<(1<<(22 if i==9 else 26)) for i in range(10))
   edge=[0,1,2,P-2,P-1,1<<128];pairs=[(x,y) for x in edge for y in edge]+[(rng.randrange(P),rng.randrange(P)) for _ in range(512)]
   for x,y in pairs:
    a=(C.c_uint32*8)(*words(x));b=(C.c_uint32*8)(*words(y))
    for name,want in [('field_guard_add32',(x+y)%P),('field_guard_mul32',(x*y)%P)]:
     out=(C.c_uint32*8)();assert getattr(lib,name)(U32(a,8),U32(b,8),U32(out,8));assert packed(out)==want
   bad=run_quiet_crash([sys.executable,str(Path(__file__).resolve()),'--invalid-add',str(artifact)],capture_output=True,text=True,timeout=20)
   bad_normal=run_quiet_crash([sys.executable,str(Path(__file__).resolve()),'--invalid-normalize',str(artifact)],capture_output=True,text=True,timeout=20)
   assert bad.returncode==-signal.SIGABRT and 'secp_field32.coax:70: assertion failed' in bad.stderr
   if mode=='checked':assert bad_normal.returncode==-signal.SIGABRT and "secp_field.coax:223: integer overflow" in bad_normal.stderr
   for name,expected in sources.items():assert sha(name)==expected
   assert sha(COMPILER)==results['compiler_sha256'] and sha(PROOF)==results['proof_sha256'] and sha(__file__)==results['witness_sha256'] and sha(SANITIZER)==results['sanitizer_sha256'] and sha(CRASH_LAUNCHER)==results['crash_launcher_sha256']
   for name,expected in result['source_sha256'].items():assert sha(ROOT/name)==expected
   run(provenance,'verify',ROOT,COMPILER,helper,record);assert sha(helper)==results['compiler_provenance_helper_sha256'] and sha(record)==results['compiler_provenance_record_sha256']
   results['modes'][mode]={'c_sha256':sha(c),'artifact_sha256':sha(artifact),'sanitized_artifact_sha256':sha(sanitized),'address_sanitizer':True,'undefined_sanitizer':True,'source_map_sha256':sha(source_map),'source_sha256':recorded,'normalization_cases':len(normal),'field_pairs':len(pairs),'invalid_add_exit':bad.returncode,'invalid_add_diagnostic':'secp_field32.coax:70: assertion failed','invalid_normalize_exit':bad_normal.returncode}
   if args.retain:
    target=args.retain/mode;target.mkdir();
    for source in [c,header,artifact,source_map,sanitized]:shutil.copy2(source,target/source.name)
 out=json.dumps(results,indent=2)+'\n'
 if args.report:args.report.write_text(out)
 else:print(out,end='')
if __name__=='__main__':main()
