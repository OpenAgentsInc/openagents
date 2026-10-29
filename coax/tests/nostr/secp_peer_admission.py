#!/usr/bin/env python3
"""Adversarial public raw-peer tests against the pre-admission behavior."""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import secp_field as field
import secp_group as group
from scalar_candidate import Scalar, scalar

ROOT=field.ROOT
SOURCE=ROOT/'coax/lib/crypto/secp_mult_const.coax'
BASE='c3484fd00e29ceb3d43b924138577c5bf434911a'
DRIVER=ROOT/'coax/tests/nostr/secp_peer_admission_wasm.cjs'
WASM_ADAPTER=ROOT/'coax/tests/nostr/secp_mult_const_wasm.c'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def run(*args,**kwargs):
 r=subprocess.run([str(x) for x in args],capture_output=True,text=True,**kwargs)
 if r.returncode:raise RuntimeError(f'{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}')
 return r
def cases():
 gx=int('79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798',16)
 gy=int('483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8',16)
 g=group.point((gx,gy)); limbs=[int(x) for coord in (g.x,g.y,g.z) for x in coord.limb]
 rows=[{'name':'generator','scalar':1,'limbs':limbs,'bounded':True},
       {'name':'infinity','scalar':1,'limbs':[1]+[0]*9+[1]+[0]*19,'bounded':True},
       {'name':'malformed bounded','scalar':1,'limbs':[0]*30,'bounded':True},
       {'name':'zero scalar generator','scalar':0,'limbs':limbs,'bounded':True},
       {'name':'magnitude eight all limbs','scalar':8,'limbs':[8*((1<<(22 if i%10==9 else 26))-1) for i in range(30)],'bounded':True}]
 for at in range(30):
  limit=8*((1<<(22 if at%10==9 else 26))-1)
  for value in [limit-1,limit,limit+1,(1<<64)-1]:
   values=limbs[:];values[at]=value
   rows.append({'name':f'coordinate {at//10} limb {at%10} value {value}','scalar':1,'limbs':values,'bounded':value<=limit})
 return rows

def point(row):
 p=group.Point()
 for i,v in enumerate(row['limbs']):getattr(p,('x','y','z')[i//10]).limb[i%10]=v
 return p

def main():
 assert __debug__, 'assertions required'
 compiler=field.COMPILER.resolve();inputs=cases();source_hash=sha(SOURCE);binary_hash=sha(compiler)
 deps=json.loads(run(compiler,'deps',SOURCE).stdout);assert not deps['descriptors']
 source_graph={str(Path(p).resolve().relative_to(ROOT)):sha(p) for p in deps['sources']}
 report={'schema':'coax.raw-peer-admission.v1','scope':'public raw-peer boundary behavior, not target constant time or production approval','baseline_commit':BASE,'compiler':run(compiler,'version').stdout.strip(),'compiler_sha256':binary_hash,'source_sha256':source_hash,'driver_sha256':sha(DRIVER),'test_sha256':sha(__file__),'source_graph':source_graph,'native_compiler':run('cc','--version').stdout.splitlines()[0],'wasm_compiler':run('/opt/homebrew/opt/llvm/bin/clang','--version').stdout.splitlines()[0],'modes':[]}
 with tempfile.TemporaryDirectory(prefix='coax-peer-admit-') as d:
  tmp=Path(d).resolve();old=tmp/'coax/lib/crypto';shutil.copytree(ROOT/'coax/lib/crypto',old)
  (tmp/'coax/tests/nostr').mkdir(parents=True);shutil.copy2(ROOT/'coax/tests/nostr/scalar_fixtures.coax',tmp/'coax/tests/nostr/scalar_fixtures.coax')
  original=run('git','show',BASE+':coax/lib/crypto/secp_mult_const.coax',cwd=ROOT).stdout
  (old/'secp_mult_const.coax').write_text(original);report['baseline_source_sha256']=sha(old/'secp_mult_const.coax')
  fixtures=tmp/'cases.json';fixtures.write_text(json.dumps([{**row,'limbs':[str(v) for v in row['limbs']]} for row in inputs])); report['fixtures_sha256']=sha(fixtures)
  for name,flags in [('checked',[]),('release',['--release'])]:
   libs=[];wasms=[];artifacts=[];header_bytes=None
   for label,src in [('current',SOURCE),('baseline',old/'secp_mult_const.coax')]:
    c=tmp/(name+'-'+label+'.c');header=tmp/'secp_mult_const.h';source_map=tmp/(name+'-'+label+'-map.json')
    c.write_text(run(compiler,'c',src,'--library','--header',header,'--source-map',source_map,*flags).stdout)
    normalized_header=b'\n'.join(header.read_bytes().split(b'\n')[1:])
    if header_bytes is None:header_bytes=normalized_header
    else:assert normalized_header==header_bytes, 'exported header changed beyond source path'
    metadata=json.loads(source_map.read_text());assert metadata['sources']==list(metadata['source_hashes'])
    for source,h in metadata['source_hashes'].items():
     actual=Path(source).resolve();assert sha(actual)==h
     if label=='current':assert source_graph[str(actual.relative_to(ROOT))]==h
     elif actual.name!='secp_mult_const.coax':assert source_graph[str(actual.relative_to(tmp))]==h
    assert len(metadata['source_hashes'])==len(source_graph)

    lib=tmp/(name+'-'+label+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',lib);loaded=C.CDLL(str(lib));loaded.smc_multiply.argtypes=[Scalar,group.Point];loaded.smc_multiply.restype=group.Result;libs.append(loaded)
    wasm=tmp/(name+'-'+label+'.wasm');run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(tmp),c,WASM_ADAPTER,'-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base,-z,stack-size=1048576','-lm','-o',wasm);wasms.append(wasm)
    artifacts.append({'version':label,'c_sha256':sha(c),'library_sha256':sha(lib),'wasm_sha256':sha(wasm),'source_map_sha256':sha(source_map),'header_sha256':sha(header),'normalized_header_sha256':hashlib.sha256(header_bytes).hexdigest()})
   bounded=refused=0
   for row in inputs:
    peer=point(row);original_peer=bytes(peer);got=libs[0].smc_multiply(scalar(row['scalar']),peer);assert bytes(peer)==original_peer,row['name']
    if row['bounded']:
     prior=libs[1].smc_multiply(scalar(row['scalar']),peer);assert got.ok==prior.ok and bytes(got.value)==bytes(prior.value),row['name'];bounded+=1
    else:
     assert not got.ok and [int(v) for f in (got.value.x,got.value.y,got.value.z) for v in f.limb]==[1]+[0]*9+[1]+[0]*19,row['name'];refused+=1
   wasm_report=json.loads(run('node',DRIVER,*wasms,fixtures,timeout=600).stdout);assert wasm_report['bounded']==bounded and wasm_report['refused']==refused
   report['modes'].append({'mode':name,'artifacts':artifacts,'native':{'bounded':bounded,'refused':refused,'failures':0},'wasm':wasm_report})
  assert sha(SOURCE)==source_hash and sha(compiler)==binary_hash
  for path,h in source_graph.items():assert sha(ROOT/path)==h,path
 output=json.dumps(report,indent=2)+'\n'
 if os.environ.get('COAX_PEER_REPORT'):Path(os.environ['COAX_PEER_REPORT']).write_text(output)
 else:print(output,end='')
if __name__=='__main__':main()
