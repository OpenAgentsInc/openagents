#!/usr/bin/env python3
"""Independent public event admission fixtures; no full-store authority claim."""
import base64,copy,ctypes as C,hashlib,json,pathlib,random,struct,subprocess,tempfile
import secp_field as base
from oracle import NativeOracle,DEFAULT_LIBRARY,schnorr_verify
from make_event_fixtures import canonical
ROOT=base.ROOT;SOURCE=ROOT/'coax/lib/nostr/store_admit.coax';T=ROOT/'coax/tests/nostr';COMPILER=base.COMPILER
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def run(*args):return subprocess.run([str(x) for x in args],check=True,capture_output=True,text=True).stdout
class View(C.Structure):_fields_=[('data',C.c_void_p),('count',C.c_int64)]
class Admission(C.Structure):_fields_=[('status',C.c_uint32),('id',C.c_uint8*32),('created',C.c_int64),('kind',C.c_int64),('written',C.c_int64)]
def frame(text):return b'NSLG'+struct.pack('>IIII',1,1,len(text),len(text))+text
oracle=NativeOracle(DEFAULT_LIBRARY);rng=random.Random(8120261029);rows=[]
def add(name,text,status,capacity=2048,overlap=False):
 raw=text.encode() if isinstance(text,str) else text;rows.append({'name':name,'text':base64.b64encode(raw).decode(),'frame':base64.b64encode(frame(raw)).decode(),'status':status,'capacity':capacity,'overlap':overlap})
for i in range(16):
 secret=(i+1).to_bytes(32,'big');kind=[0,1,5,10000,20000,30023][i%6];event={'pubkey':oracle.point(secret)[1:].hex(),'created_at':i,'kind':kind,'tags':[['d','α' if i%2 else 'a']], 'content':'case '+str(i)+' é'}
 event['id']=hashlib.sha256(canonical(event)).hexdigest();event['sig']=oracle.sign(bytes.fromhex(event['id']),secret,bytes(32)).hex();assert schnorr_verify(bytes.fromhex(event['id']),bytes.fromhex(event['pubkey']),bytes.fromhex(event['sig']))
 text=json.dumps(event,ensure_ascii=bool(i%2),indent=1 if i%3==0 else None)
 if i%4==0:text=text.replace('"pubkey"','"\\u0070ubkey"')
 add('valid-'+str(i),text,4 if kind==20000 else 0)
 invalid=copy.deepcopy(event);invalid['sig']=('0' if invalid['sig'][0]!='0' else '1')+invalid['sig'][1:];assert not schnorr_verify(bytes.fromhex(invalid['id']),bytes.fromhex(invalid['pubkey']),bytes.fromhex(invalid['sig']));add('bad-signature-'+str(i),json.dumps(invalid,separators=(',',':')),1)
 invalid=copy.deepcopy(event);invalid['content']+='!';add('bad-id-'+str(i),json.dumps(invalid,separators=(',',':')),1)
 if kind!=20000:add('short-output-'+str(i),text,3,1)
add('malformed','{',1);add('missing','{}',1);add('overlap','{}',2,2048,True)
inputs=[COMPILER,SOURCE,ROOT/'coax/lib/nostr/store_policy.coax',ROOT/'coax/lib/nostr/store_snapshot.coax',ROOT/'coax/lib/nostr/store_layout.coax',pathlib.Path(__file__).resolve(),T/'store_admit_wasm.c',T/'store_admit_wasm.cjs',DEFAULT_LIBRARY];hashes={str(p):sha(p) for p in inputs};results=[]
with tempfile.TemporaryDirectory(prefix='coax-store-admit-') as td:
 d=pathlib.Path(td);fixtures=d/'cases.json';fixtures.write_text(json.dumps(rows));fixture_hash=sha(fixtures)
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');h=d/'store-admit.h';mapping=d/(mode+'-map.json');run(COMPILER,'c',SOURCE,'--library','--header',h,'--source-map',mapping,'-o',c,*flags)
  for path,digest in json.loads(mapping.read_text())['source_hashes'].items():
   if sha(path)!=digest or path in hashes and hashes[path]!=digest:raise RuntimeError('source identity drift')
   hashes[path]=digest
  target=d/(mode+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',target);lib=C.CDLL(str(target));lib.ns_admit_one.argtypes=[View,View,View,C.c_bool];lib.ns_admit_one.restype=Admission
  for row in rows:
   text=base64.b64decode(row['text']);expected=base64.b64decode(row['frame']);source=(C.c_uint8*(len(text)+16))(*([165]*(len(text)+16)));source[8:8+len(text)]=text;scratch=(C.c_uint8*1040)(*([165]*1040));out=(C.c_uint8*2064)(*([165]*2064))
   pointer=lambda a,offset:C.cast(C.byref(a,offset),C.c_void_p)
   result=lib.ns_admit_one(View(pointer(source,8),len(text)),View(pointer(scratch,8),1024),View(pointer(out,8),row['capacity']),row['overlap'])
   assert result.status==row['status'],(mode,row['name'],result.status,row['status'])
   if not result.status:assert result.written==len(expected) and bytes(out[8:8+len(expected)])==expected,(mode,row['name'],'frame')
   else:assert result.written==0 and bytes(out[8:-8])==bytes([165])*2048,(mode,row['name'],'refusal')
   assert bytes(source)==bytes([165])*8+text+bytes([165])*8 and bytes(scratch[:8])==bytes([165])*8 and bytes(scratch[-8:])==bytes([165])*8 and bytes(out[:8])==bytes([165])*8 and bytes(out[-8:])==bytes([165])*8
  valid=next(x for x in rows if x['status']==0);text=base64.b64decode(valid['text']).decode();probe=d/'asan.c';probe.write_text('#include "store-admit.h"\n#include <assert.h>\n#include <string.h>\nint main(void){char text[]='+json.dumps(text)+';uint8_t scratch[1040],out[2064];memset(scratch,165,sizeof scratch);memset(out,165,sizeof out);coax_t_NsAdmission r=ns_admit_one((coax_string){text,sizeof(text)-1},(coax_v_u8){scratch+8,1024},(coax_v_u8){out+8,2048},0);assert(r.status==0&&r.written==(int64_t)sizeof(text)-1+20);for(int i=0;i<8;i++)assert(scratch[i]==165&&scratch[1032+i]==165&&out[i]==165&&out[2056+i]==165);return 0;}\n')
  sanitized=d/(mode+'-asan');run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),c,probe,'-lm','-o',sanitized);run(sanitized)
  wasm=d/(mode+'.wasm');run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),c,T/'store_admit_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=16777216,--export=__heap_base,-z,stack-size=2097152','-lm','-o',wasm)
  observed=json.loads(run('node',T/'store_admit_wasm.cjs',wasm,fixtures));results.append({'mode':mode,'native_cases':len(rows),'native_failures':0,'native_sha256':sha(target),'sanitized_fixture':'passed','sanitized_sha256':sha(sanitized),'wasm_sha256':sha(wasm),'wasm':observed})
for p,h in hashes.items():assert sha(p)==h,p
oracle.close();print(json.dumps({'schema':'coax.nostr-store-single-admission.v1','scope':'one-event verification and framing only; no multi-event store, replay or durability claim','seed':8120261029,'fixture_sha256':fixture_hash,'inputs':hashes,'modes':results},indent=2))
