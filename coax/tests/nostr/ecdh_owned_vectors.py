#!/usr/bin/env python3
"""Owned host against public NIP-44 and adapted Wycheproof raw-x checks."""
import ctypes as C,hashlib,hmac,json,os,pathlib,random,subprocess,tempfile
import secp_field as field,secp_group as group
from scalar_candidate import Scalar
from secp_field_mul import file_sha
from oracle import raw_ecdh,lift_x,point_mul,P,N,NativeOracle,DEFAULT_LIBRARY
ROOT=field.ROOT;SOURCE=ROOT/'coax/lib/crypto/ecdh.coax'
class Work(C.Structure):_fields_=[('scalar',Scalar),('shared',group.Point),('affine',group.Affine),('staged',C.c_uint8*32)]
class WV(C.Structure):_fields_=[('data',C.POINTER(Work)),('count',C.c_int64)]
def call(lib,key,peer):
 a=(C.c_uint8*32).from_buffer_copy(key);b=(C.c_uint8*32).from_buffer_copy(peer);w=Work();C.memset(C.byref(w),0xa5,C.sizeof(w));status=lib.ecdh_stage(field.View(a,32),field.View(b,32),WV(C.pointer(w),1));return status,bytes(w.staged)
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
 paths=[SOURCE,ROOT/'coax/lib/crypto/zeroize.coax',ROOT/'coax/lib/crypto/ecdh_lifecycle.coax',ROOT/'coax/lib/crypto/platform/ecdh_owned.c',ROOT/'coax/lib/crypto/platform/ecdh_owned.h',ROOT/'coax/lib/crypto/platform/zeroize.c',ROOT/'coax/tests/nostr/ecdh_owned_vectors.py',ROOT/'coax/tests/nostr/ecdh_owned_wasm.c',ROOT/'coax/tests/nostr/ecdh_owned_wasm.cjs',source_vectors,corpus,DEFAULT_LIBRARY,field.COMPILER]
 hashes={str(p):file_sha(p) for p in paths};report={'schema':'coax.ecdh-owned-oracle.v1','scope':'public correctness of an experimental owner; no target or production approval','counts':counts,'modes':[]}
 with tempfile.TemporaryDirectory(prefix='coax-ecdh-owned-vectors-') as td:
  d=pathlib.Path(td);fixture=d/'cases.bin';fixture.write_bytes(b''.join(k+p+bytes([status])+raw for k,p,status,raw in rows));report['fixtures_sha256']=file_sha(fixture)
  for mode,flags in [('checked',[]),('release',['--release'])]:
   sources=[]
   for name,src in [('ecdh','ecdh.coax'),('zeroize-policy','zeroize.coax'),('ecdh_lifecycle','ecdh_lifecycle.coax')]:
    c=d/(name+'.c');mapping=d/(name+'-map.json');field.run(field.COMPILER,'c',ROOT/'coax/lib/crypto'/src,'--library','--header',d/(name+'.h'),'--source-map',mapping,'-o',c,*flags);sources.append(c)
    for path,digest in json.loads(mapping.read_text())['source_hashes'].items():
     if file_sha(pathlib.Path(path))!=digest or path in hashes and hashes[path]!=digest:raise AssertionError('source map drift')
     hashes[path]=digest
   sources.extend([ROOT/'coax/lib/crypto/platform/zeroize.c',ROOT/'coax/lib/crypto/platform/ecdh_owned.c']);common=['-std=c11','-O2','-I'+str(d),'-I'+str(ROOT/'coax/lib/crypto/platform')]
   libpath=d/(mode+'.dylib');field.run('cc',*common,'-shared','-fPIC',*sources,'-lm','-o',libpath);lib=C.CDLL(str(libpath))
   lib.coax_ecdh_owned_acquire.restype=C.c_uint64;lib.coax_ecdh_owned_start.argtypes=[C.c_uint64,C.c_void_p,C.c_int64,C.c_void_p,C.c_int64];lib.coax_ecdh_owned_start.restype=C.c_uint32;lib.coax_ecdh_owned_collect.argtypes=[C.c_uint64,C.c_void_p,C.c_int64];lib.coax_ecdh_owned_collect.restype=C.c_uint32;lib.coax_ecdh_owned_close.argtypes=[C.c_uint64];lib.coax_ecdh_owned_close.restype=C.c_uint32
   for i,(k,p,status,raw) in enumerate(rows):
    a=(C.c_uint8*32).from_buffer_copy(k);b=(C.c_uint8*32).from_buffer_copy(p);out=(C.c_uint8*32)(*[0xa5]*32);h=lib.coax_ecdh_owned_acquire();assert h
    got=lib.coax_ecdh_owned_start(h,a,32,b,32)
    if got==5:got=lib.coax_ecdh_owned_collect(h,out,32)
    assert got==status,(mode,i,got,status)
    assert bytes(out)==(raw if status==0 else bytes([0xa5])*32),(mode,i,'output')
    assert bytes(a)==k and bytes(b)==p and lib.coax_ecdh_owned_close(h)==0,(mode,i,'ownership')
   wasm=d/(mode+'.wasm');field.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes',*common,*sources,ROOT/'coax/tests/nostr/ecdh_owned_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base,-z,stack-size=1048576','-lm','-o',wasm)
   w=json.loads(field.run('node',ROOT/'coax/tests/nostr/ecdh_owned_wasm.cjs',wasm,fixture,timeout=1800).stdout);report['modes'].append({'mode':mode,'native_cases':len(rows),'native_failures':0,'native_artifact_sha256':file_sha(libpath),'wasm_artifact_sha256':file_sha(wasm),'wasm':w})
 for name,digest in hashes.items():assert file_sha(pathlib.Path(name))==digest,name
 report['inputs']=hashes;oracle.close();print(json.dumps(report,indent=2))
if __name__=='__main__':main()
