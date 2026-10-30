#!/usr/bin/env python3
"""Independent frame tests. Match flags are untrusted and confer no authority."""
import base64,ctypes as C,hashlib,json,os,pathlib,struct,subprocess,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[3];T=ROOT/'coax/tests/nostr';COMPILER=pathlib.Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax'));SOURCE=T/'store_dedup_rewrite_adapter.coax'
def run(*args):
 p=subprocess.run([str(a) for a in args],capture_output=True,text=True)
 if p.returncode:raise RuntimeError(f'{args[0]} ({p.returncode}): {p.stderr or p.stdout}')
 return p.stdout
def digest(path):return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()
def frame(records):return b'NSLG'+struct.pack('>III',1,len(records),sum(map(len,records)))+b''.join(struct.pack('>I',len(x))+x for x in records)
def b64(b):return base64.b64encode(b).decode()
class View(C.Structure):_fields_=[('data',C.c_void_p),('count',C.c_int64)]
class Result(C.Structure):_fields_=[('ok',C.c_bool),('count',C.c_int64),('bytes',C.c_int64),('written',C.c_int64),('duplicates',C.c_int64)]
rows=[]
def add(name,old,text,flags,ok,expected=b'',cap=1050640,overlap=False,duplicates=None):rows.append({'name':name,'old':b64(old),'text':b64(text),'flags':flags,'ok':ok,'out':b64(expected),'capacity':cap,'overlap':overlap,'duplicates':duplicates})
add('empty frame appends',frame([]),b'a',[],True,frame([b'a']),duplicates=0)
add('two duplicates retain multi-byte unrelated original',frame([b'zz',b'xy',b'abc']),b'a',[1,0,1],True,frame([b'a',b'xy']),duplicates=2)
add('smaller previous wins',frame([b'a']),b'zz',[1],True,frame([b'a']),duplicates=1)
add('new appends',frame([b'a',b'xyz']),b'b',[0,0],True,frame([b'a',b'xyz',b'b']),duplicates=0)
add('512 old matches compact',frame([b'z']*512),b'a',[1]*512,True,frame([b'a']),duplicates=512)
add('growth exceeds output',frame([b'z']),b'aa',[1],False,cap=21)
add('bad frame',b'broken',b'a',[],False)
add('bad match count',frame([b'z']),b'a',[],False)
add('empty candidate',frame([]),b'',[],False)
add('oversized candidate',frame([]),b'a'*262145,[],False)
add('overlap',frame([b'z']),b'a',[1],False,overlap=True)
quota=[b'z']+[b'x'*262144]*3+[b'y'*(262144-1)];assert sum(map(len,quota))==1048576
add('payload quota refuses growing replacement',frame(quota),b'aa',[1,0,0,0,0],False)
quota=[b'zz']+[b'x'*262144]*3+[b'y'*(262144-2)];assert sum(map(len,quota))==1048576
add('payload quota permits shrinking replacement',frame(quota),b'a',[1,0,0,0,0],True,frame([b'a']+quota[1:]),duplicates=1)
inputs=[COMPILER,SOURCE,ROOT/'coax/lib/nostr/store_dedup_rewrite.coax',T/'store_dedup_rewrite_wasm.c',T/'store_dedup_rewrite_wasm.cjs',pathlib.Path(__file__).resolve()];hashes={str(p):digest(p) for p in inputs};reports=[]
with tempfile.TemporaryDirectory(prefix='coax-store-rewrite-') as tmp:
 d=pathlib.Path(tmp);cases=d/'cases.json';cases.write_text(json.dumps(rows));fixture_hash=digest(cases)
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');header=d/'store-rewrite.h';mapping=d/(mode+'-map.json');run(COMPILER,'c',SOURCE,'--library','--header',header,'--source-map',mapping,'--run-steps','250000000','-o',c,*flags)
  for path,h in json.loads(mapping.read_text())['source_hashes'].items():
   if digest(path)!=h or path in hashes and hashes[path]!=h:raise RuntimeError('source identity drift')
   hashes[path]=h
  libfile=d/(mode+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',c,'-o',libfile);lib=C.CDLL(str(libfile));fn=lib.store_rewrite_test;fn.argtypes=[View,View,View,View,C.c_bool];fn.restype=Result
  for row in rows:
   old=base64.b64decode(row['old']);text=base64.b64decode(row['text']);flags=bytes(row['flags']);expected=base64.b64decode(row['out'])
   def buf(n):
    value=(C.c_uint8*(n+16))();C.memset(value,165,n+16);return value
   a=buf(len(old));a[8:8+len(old)]=old;b=buf(len(text));b[8:8+len(text)]=text;f=buf(len(flags));f[8:8+len(flags)]=flags;out=buf(1050640)
   def view(x,n):return View(C.cast(C.byref(x,8),C.c_void_p),n)
   got=fn(view(a,len(old)),view(b,len(text)),view(f,len(flags)),view(out,row['capacity']),row['overlap'])
   assert bool(got.ok)==row['ok'],(mode,row['name'],'status')
   if got.ok:
    assert got.written==len(expected) and bytes(out[8:8+len(expected)])==expected,(mode,row['name'],'frame')
    assert got.duplicates==row['duplicates'],(mode,row['name'],'duplicates')
   else:assert bytes(out[8:-8])==bytes([165])*1050640,(mode,row['name'],'output changed')
   assert bytes(a[8:-8])==old and bytes(b[8:-8])==text and bytes(f[8:-8])==flags,(mode,row['name'],'input changed')
   for x in (a,b,f,out):assert bytes(x[:8])==bytes([165])*8 and bytes(x[-8:])==bytes([165])*8,(mode,row['name'],'guard')
  probe=d/'probe.c';probe.write_text('#include "store-rewrite.h"\n#include <assert.h>\n#include <string.h>\n#include <stdbool.h>\nint main(void){uint8_t old[]={78,83,76,71,0,0,0,1,0,0,0,1,0,0,0,1,0,0,0,1,122};bool flags[]={1};uint8_t out[32];memset(out,165,sizeof out);coax_t_NsDedupFrame r=store_rewrite_test((coax_v_u8){old,sizeof old},(coax_string){"a",1},(coax_v_bool){flags,1},(coax_v_u8){out,21},0);assert(r.ok&&r.count==1&&r.duplicates==1&&out[20]==97);return 0;}\n')
  sanitized=d/(mode+'-asan');run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),c,probe,'-o',sanitized);run(sanitized)
  wasm=d/(mode+'.wasm');run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),c,T/'store_dedup_rewrite_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=16777216,--export=__heap_base,-z,stack-size=1048576','-o',wasm)
  observed=json.loads(run('node',T/'store_dedup_rewrite_wasm.cjs',wasm,cases));reports.append({'mode':mode,'native_cases':len(rows),'native_failures':0,'sanitized':'passed','wasm':observed,'generated_c_sha256':digest(c),'wasm_sha256':digest(wasm)})
for p,h in hashes.items():assert digest(p)==h,p
print(json.dumps({'schema':'coax.nostr-store-dedup-rewrite.v1','scope':'unverified private framing transformation only; caller-supplied match flags carry no authority','fixture_sha256':fixture_hash,'inputs':hashes,'modes':reports},indent=2))
