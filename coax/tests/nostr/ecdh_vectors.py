#!/usr/bin/env python3
"""Public NIP-44 and explicitly adapted Wycheproof raw-x checks."""
import ctypes as C,hashlib,hmac,json,os,pathlib,random,subprocess,tempfile
import secp_field as field,secp_group as group
from scalar_candidate import Scalar
from secp_field_mul import file_sha
from oracle import raw_ecdh,lift_x,point_mul,P,N,NativeOracle,DEFAULT_LIBRARY
ROOT=field.ROOT;SOURCE=ROOT/'coax/lib/crypto/ecdh_owned_adapter.coax'
class Work(C.Structure):_fields_=[('scalar',Scalar),('shared',group.Point),('affine',group.Affine),('staged',C.c_uint8*32)]
class WV(C.Structure):_fields_=[('data',C.POINTER(Work)),('count',C.c_int64)]
def call(lib,key,peer):
 a=(C.c_uint8*32).from_buffer_copy(key);b=(C.c_uint8*32).from_buffer_copy(peer);w=Work();C.memset(C.byref(w),0xa5,C.sizeof(w));status=lib.ecdh_stage_bridge(field.View(a,32),field.View(b,32),WV(C.pointer(w),1));return status,bytes(w.staged)
def main():
 assert C.sizeof(Work)==472;oracle=NativeOracle(DEFAULT_LIBRARY);rows=[];counts={'nip44':0,'wycheproof_valid':0,'wycheproof_invalid_x':0,'wycheproof_excluded':0,'invalid_scalar':0,'invalid_peer':0,'random':0,'seeded_default_hash':0}
 def add(k,p,status,raw):rows.append((k,p,status,raw))
 source_vectors=ROOT/'coax/tests/vectors/upstream/nip44/nip44.vectors.json';corpus=ROOT/'coax/tests/vectors/upstream/wycheproof/ecdh_secp256k1_test.json'
 for v in json.loads(source_vectors.read_text())['v2']['valid']['get_conversation_key']:
  k=bytes.fromhex(v['sec1']);p=bytes.fromhex(v['pub2']);raw=oracle.ecdh(k,p);assert raw==raw_ecdh(k,p);assert hmac.new(b'nip44-v2',raw,hashlib.sha256).hexdigest()==v['conversation_key'];add(k,p,0,raw);counts['nip44']+=1
 prefix='3056301006072a8648ce3d020106052b8104000a03420004'
 for group_record in json.loads(corpus.read_text())['testGroups']:
  for t in group_record['tests']:
   pub=t['public']
   if not pub.startswith(prefix) or len(pub)!=len(prefix)+128:counts['wycheproof_excluded']+=1;continue
   x=int(pub[len(prefix):len(prefix)+64],16);y=int(pub[-64:],16);k=int(t['private'],16);point=lift_x(x) if x<P else None
   if t['result']=='valid' and 1<=k<N and point is not None and (y*y-x*x*x-7)%P==0:
    a=k.to_bytes(32,'big');b=x.to_bytes(32,'big');raw=bytes.fromhex(t['shared']);assert raw==oracle.ecdh(a,b)==raw_ecdh(a,b),t['tcId'];add(a,b,0,raw);counts['wycheproof_valid']+=1
   elif t['result']=='invalid' and 1<=k<N and point is None and x<1<<256:
    add(k.to_bytes(32,'big'),x.to_bytes(32,'big'),2,bytes(32));counts['wycheproof_invalid_x']+=1
   else:counts['wycheproof_excluded']+=1
 peer=rows[0][1]
 for k in [0,N,N+1,(1<<256)-1]:add(k.to_bytes(32,'big'),peer,3,bytes(32));counts['invalid_scalar']+=1
 add((1).to_bytes(32,'big'),bytes([255])*32,2,bytes(32));counts['invalid_peer']+=1
 sk=bytes.fromhex('0ba165eceff93249956b5ff2b97cebcfb06059468ef89de69e384466f8f6c237');pk=bytes.fromhex('1fce669f5acfe74fc21e19df757704260a21fed50cd8478efcfe318f355cfd9c');raw=oracle.ecdh(sk,pk);assert raw!=oracle.ecdh(sk,pk,True);add(sk,pk,0,raw);counts['seeded_default_hash']+=1
 rng=random.Random(20261029)
 for i in range(256):
  k=rng.randrange(1,N);q=rng.randrange(1,N);pub=oracle.point(q.to_bytes(32,'big'));a=k.to_bytes(32,'big');b=pub[1:33];raw=oracle.ecdh(a,b);assert raw==raw_ecdh(a,b);point=lift_x(int.from_bytes(b,'big'));opposite=(point[0],(-point[1])%P);assert point_mul(k,point)[0]==point_mul(k,opposite)[0]==int.from_bytes(raw,'big');add(a,b,0,raw);counts['random']+=1
 deps=json.loads(field.run(field.COMPILER,'deps',SOURCE).stdout);assert not deps['descriptors']
 graph={str(pathlib.Path(p).resolve().relative_to(ROOT)):file_sha(pathlib.Path(p)) for p in deps['sources']}
 report={'source_graph':graph,'oracle_sha256':file_sha(DEFAULT_LIBRARY),'schema':'coax.ecdh-public-oracle.v1','scope':'public fixtures; no source-flow, hidden-copy, target or production approval','counts':counts,'compiler_sha256':file_sha(field.COMPILER),'inputs':{str(p.relative_to(ROOT)):file_sha(p) for p in [SOURCE,ROOT/'coax/lib/crypto/ecdh_fast.coax',pathlib.Path(__file__),source_vectors,corpus,ROOT/'coax/tests/nostr/ecdh_wasm.c',ROOT/'coax/tests/nostr/ecdh_wasm.cjs']},'modes':[]}
 with tempfile.TemporaryDirectory(prefix='coax-ecdh-vectors-') as td:
  d=pathlib.Path(td);fixture=d/'cases.bin';fixture.write_bytes(b''.join(k+p+bytes([status])+raw for k,p,status,raw in rows));report['fixtures_sha256']=file_sha(fixture)
  for mode,flags in [('checked',[]),('release',['--release'])]:
   c=d/(mode+'.c');header=d/'ecdh.h';source_map=d/(mode+'-map.json');field.run(field.COMPILER,'c',SOURCE,'--library','--source-map',source_map,'--header',header,'-o',c,*flags)
   metadata=json.loads(source_map.read_text());assert metadata['sources']==list(metadata['source_hashes']);assert {str(pathlib.Path(p).resolve().relative_to(ROOT)):h for p,h in metadata['source_hashes'].items()}==graph
   libpath=d/(mode+'.dylib');field.run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',libpath);lib=C.CDLL(str(libpath));lib.ecdh_stage_bridge.argtypes=[field.View,field.View,WV];lib.ecdh_stage_bridge.restype=C.c_uint32
   for i,(k,p,status,raw) in enumerate(rows):
    actual,out=call(lib,k,p);assert actual==status,(mode,i,actual,status)
    if status!=2:assert out==raw,(mode,i)
   wasm=d/(mode+'.wasm');field.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),c,ROOT/'coax/tests/nostr/ecdh_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base,-z,stack-size=1048576','-lm','-o',wasm)
   w=json.loads(field.run('node',ROOT/'coax/tests/nostr/ecdh_wasm.cjs',wasm,fixture,timeout=1800).stdout);report['modes'].append({'mode':mode,'native_cases':len(rows),'native_failures':0,'native_artifact_sha256':file_sha(libpath),'wasm_artifact_sha256':file_sha(wasm),'wasm':w})
 for name,h in graph.items():assert file_sha(ROOT/name)==h,name
 assert file_sha(field.COMPILER)==report['compiler_sha256']
 oracle.close();text=json.dumps(report,indent=2)+'\n';path=os.environ.get('COAX_ECDH_REPORT');pathlib.Path(path).write_text(text) if path else print(text,end='')
if __name__=='__main__':main()
