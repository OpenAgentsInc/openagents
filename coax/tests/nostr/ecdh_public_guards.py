#!/usr/bin/env python3
"""Adversarial public-peer fixtures; no guard or production attestation."""
import ctypes as C,hashlib,json,os,pathlib,random,tempfile
import secp_field as field
from ecdh_vectors import Work,WV
from oracle import P,lift_x,G
from secp_field_mul import file_sha
ROOT=field.ROOT;SOURCE=ROOT/'coax/lib/crypto/ecdh_owned_adapter.coax'
def main():
 rng=random.Random(462026);values={0,1,2,P-1,P,P+1,(1<<256)-1,G[0]}
 values.update(1<<i for i in range(256));values.update(((1<<256)-1)^(1<<i) for i in range(256));values.update(rng.getrandbits(256) for _ in range(256))
 rows=[];counts={'valid':0,'invalid_lift':0,'noncanonical':0}
 for x in sorted(values):
  peer=x.to_bytes(32,'big');valid=lift_x(x) is not None;status=0 if valid else 2
  counts['valid' if valid else 'noncanonical' if x>=P else 'invalid_lift']+=1;rows.append(((1).to_bytes(32,'big'),peer,status,peer if valid else bytes(32)))
 deps=json.loads(field.run(field.COMPILER,'deps',SOURCE).stdout);assert not deps['descriptors'];graph={str(pathlib.Path(p).resolve().relative_to(ROOT)):file_sha(pathlib.Path(p)) for p in deps['sources']}
 report={'schema':'coax.ecdh-public-guard-fixtures.v1','scope':'public fixture comparison only; no proof of guard safety, target behavior or production approval','source_graph':graph,'compiler_sha256':file_sha(field.COMPILER),'seed':462026,'counts':counts,'inputs':{str(p.relative_to(ROOT)):file_sha(p) for p in [pathlib.Path(__file__),ROOT/'coax/tests/nostr/ecdh_public_guards_wasm.cjs',ROOT/'coax/tests/nostr/ecdh_public_guards_wasm.c',ROOT/'tools/coax-nostr/fuzz/oracle.py']},'modes':[]}
 with tempfile.TemporaryDirectory(prefix='coax-ecdh-public-guards-') as td:
  d=pathlib.Path(td);fixture=d/'cases.bin';fixture.write_bytes(b''.join(k+p+bytes([status])+raw for k,p,status,raw in rows));report['fixture_sha256']=file_sha(fixture)
  for mode,flags in [('checked',[]),('release',['--release'])]:
   c=d/(mode+'.c');h=d/'ecdh.h';source_map=d/(mode+'-map.json');field.run(field.COMPILER,'c',SOURCE,'--library','--source-map',source_map,'--header',h,'-o',c,*flags)
   meta=json.loads(source_map.read_text());assert meta['sources']==list(meta['source_hashes']);assert {str(pathlib.Path(p).resolve().relative_to(ROOT)):x for p,x in meta['source_hashes'].items()}==graph
   libpath=d/(mode+'.dylib');field.run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',libpath);lib=C.CDLL(str(libpath));lib.ecdh_stage_bridge.argtypes=[field.View,field.View,WV];lib.ecdh_stage_bridge.restype=C.c_uint32
   for i,(k,p,status,want) in enumerate(rows):
    a=(C.c_uint8*32).from_buffer_copy(k);b=(C.c_uint8*32).from_buffer_copy(p);w=Work();C.memset(C.byref(w),0xa5,C.sizeof(w));actual=lib.ecdh_stage_bridge(field.View(a,32),field.View(b,32),WV(C.pointer(w),1));assert actual==status,(mode,i,actual,status)
    if status==2:assert C.string_at(C.byref(w),C.sizeof(w))==bytes([0xa5])*C.sizeof(w),(mode,i,'refusal work')
    else:assert bytes(w.staged)==want,(mode,i,'raw x')
   wasm=d/(mode+'.wasm');field.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),c,ROOT/'coax/tests/nostr/ecdh_public_guards_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base,-z,stack-size=1048576','-lm','-o',wasm)
   result=json.loads(field.run('node',ROOT/'coax/tests/nostr/ecdh_public_guards_wasm.cjs',wasm,fixture,timeout=1800).stdout);report['modes'].append({'mode':mode,'native_cases':len(rows),'native_failures':0,'native_sha256':file_sha(libpath),'wasm_sha256':file_sha(wasm),'wasm':result})
 for name,h in graph.items():assert file_sha(ROOT/name)==h,name
 assert file_sha(field.COMPILER)==report['compiler_sha256'];text=json.dumps(report,indent=2)+'\n';path=os.environ.get('COAX_ECDH_PUBLIC_GUARDS_REPORT');pathlib.Path(path).write_text(text) if path else print(text,end='')
if __name__=='__main__':main()
