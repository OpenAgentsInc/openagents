#!/usr/bin/env python3
"""Public full-u32 fused-field differential fixtures; no secret-use approval."""
import ctypes as C,json,os,pathlib,random,tempfile
import secp_field as base
from secp_field32 import Field32,pack,number
from secp_field_mul import file_sha
ROOT=base.ROOT;SOURCE=ROOT/'coax/tests/nostr/ecdh_fast_adapter.coax'
def main():
 rng=random.Random(20261029);M=1<<256;edges=sorted(set([0,1,2,base.P-1,base.P,base.P+1,M-2,M-1]+[1<<i for i in range(256)]));pairs=[(a,b) for a in edges for b in [0,1,base.P-1,base.P,M-1]]+[(rng.getrandbits(256),rng.getrandbits(256)) for _ in range(100000)]
 rows=[]
 for a,b in pairs:rows.append((0,a,b,(a+b)%base.P));rows.append((1,a,b,(a*b)%base.P))
 for a in edges+[rng.randrange(1,base.P) for _ in range(256)]:rows.append((2,a,0,pow(a%base.P,-1,base.P) if a%base.P else 0))
 deps=json.loads(base.run(base.COMPILER,'deps',SOURCE).stdout);assert not deps['descriptors'];graph={str(pathlib.Path(p).resolve().relative_to(ROOT)):file_sha(pathlib.Path(p)) for p in deps['sources']}
 report={'schema':'coax.ecdh-fused-field-oracle.v1','scope':'public full-width arithmetic fixtures, no target or production approval','source_graph':graph,'compiler_sha256':file_sha(base.COMPILER),'pairs':len(pairs),'inverse_cases':len(rows)-2*len(pairs),'modes':[]}
 with tempfile.TemporaryDirectory(prefix='coax-ecdh-field-') as td:
  d=pathlib.Path(td);fixtures=d/'cases.bin';fixtures.write_bytes(b''.join(bytes([op])+a.to_bytes(32,'little')+b.to_bytes(32,'little')+want.to_bytes(32,'little') for op,a,b,want in rows));report['fixture_sha256']=file_sha(fixtures)
  for label,flags in [('checked',[]),('release',['--release'])]:
   c=d/(label+'.c');h=d/'ecdh_fast_adapter.h';metadata=d/(label+'-map.json');base.run(base.COMPILER,'c',SOURCE,'--library','--source-map',metadata,'--header',h,'-o',c,*flags)
   x=json.loads(metadata.read_text());assert x['sources']==list(x['source_hashes']) and {str(pathlib.Path(p).resolve().relative_to(ROOT)):h for p,h in x['source_hashes'].items()}==graph
   libpath=d/(label+'.dylib');base.run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',libpath);lib=C.CDLL(str(libpath))
   for op in ['add','mul']:
    fn=getattr(lib,'ef_test_'+op);fn.argtypes=[Field32,Field32];fn.restype=Field32
   lib.ef_test_inverse.argtypes=[Field32];lib.ef_test_inverse.restype=Field32
   for i,(op,a,b,want) in enumerate(rows):
    got=number(lib.ef_test_add(pack(a),pack(b)) if op==0 else lib.ef_test_mul(pack(a),pack(b)) if op==1 else lib.ef_test_inverse(pack(a)));assert got==want,(label,i,op)
   wasm=d/(label+'.wasm');base.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),c,ROOT/'coax/tests/nostr/ecdh_fast_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base,-z,stack-size=1048576','-lm','-o',wasm)
   w=json.loads(base.run('node',ROOT/'coax/tests/nostr/ecdh_fast_wasm.cjs',wasm,fixtures,timeout=1800).stdout);report['modes'].append({'mode':label,'native_cases':len(rows),'native_failures':0,'wasm':w,'native_sha256':file_sha(libpath),'wasm_sha256':file_sha(wasm)})
 for p,h in graph.items():assert file_sha(ROOT/p)==h,p
 path=os.environ.get('COAX_ECDH_FAST_REPORT');text=json.dumps(report,indent=2)+'\n';pathlib.Path(path).write_text(text) if path else print(text,end='')
if __name__=='__main__':main()
