#!/usr/bin/env python3
"""Source fixture, generated Coax codecs and independent BIP-340 admission."""
if not __debug__: raise SystemExit('tests require Python assertions')
import copy,ctypes as C,hashlib,json,os,subprocess,sys,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];compiler=Path(os.environ.get('COAX_TEST_BIN',str(R/'coax/bin/coax')))
subprocess.run([sys.executable,str(R/'coax/tests/nostr/proto_wire_parity.py')],check=True)
def var(n):
 out=bytearray()
 while n>=128:out.append((n&127)|128);n>>=7
 out.append(n);return bytes(out)
def v(n,x):return var(n*8)+var(x)
def b(n,x):return var(n*8+2)+var(len(x))+x
def event(o):
 return (b(1,bytes.fromhex(o['id']))+b(2,bytes.fromhex(o['pubkey']))+v(3,o['created_at'])+v(4,o['kind'])+
  b''.join(b(5,b''.join(b(1,x.encode()) for x in tag)) for tag in o['tags'])+b(6,o['content'].encode())+b(7,bytes.fromhex(o['sig'])))
def signed(raw,projection,unknown=b''):return b(1,raw)+b(2,projection)+unknown
raw=(R/'proto/nostr/fixtures/owner-attested.event.json').read_bytes();doc=json.loads(raw);fixture=(R/'proto/nostr/fixtures/owner-attested.event.pb').read_bytes()
assert event(doc)==fixture
cases=[]
def add(name,data,consistent,verified=None,owner=None,admitted=None):cases.append({'name':name,'wire':list(data),'consistent':consistent,'verified':verified,'owner':list(owner) if owner is not None else None,'admitted':admitted})
add('real signed source',signed(raw,fixture),True,True,bytes.fromhex(doc['pubkey']),True)
add('unknown protobuf authority flag',signed(raw,fixture+v(90,1),v(91,1)),True,True)
forged=copy.deepcopy(doc);forged.update({'verified':True,'authorized':True,'owner':True});forged['sig']='00'*64
forged_raw=json.dumps(forged,ensure_ascii=False,separators=(',',':')).encode();add('forged authority flags',signed(forged_raw,event(forged),v(92,1)),True,False,bytes.fromhex(doc['pubkey']),False)
claimed=copy.deepcopy(doc);claimed.update({'verified':True,'authorized':True,'owner':True})
add('verified signer is not independently configured owner',signed(json.dumps(claimed,separators=(',',':')).encode(),event(claimed),v(93,1)),True,True,bytes(32),False)
for key in ('id','pubkey','sig','created_at','kind','content','tags'):
 other=copy.deepcopy(doc)
 if key in ('id','pubkey','sig'):other[key]=('00' if other[key][:2]!='00' else '11')+other[key][2:]
 elif key in ('created_at','kind'):other[key]+=1
 elif key=='content':other[key]+='!'
 else:other[key][0]=list(reversed(other[key][0]))
 add('mismatch '+key,signed(raw,event(other)),False)
add('duplicate kind',signed(raw,fixture+v(4,1)),False)
add('missing original',b(2,fixture),False)
add('invalid original',signed(b'{',fixture),False)
add('truncated wire',signed(raw,fixture)[:-1],False)
unicode_case=copy.deepcopy(doc);unicode_case['content']='hello é\n😀';unicode_case['tags']=[['', 'é'], [], ['', 'é']]
unicode_raw=json.dumps(unicode_case,ensure_ascii=True,indent=2).encode()
add('escaped unicode and ordered duplicate/empty tags',signed(unicode_raw,event(unicode_case)),True)
large=copy.deepcopy(doc);large['content']='x'*65537
add('profile content limit',signed(json.dumps(large,separators=(',',':')).encode(),event(large)),False)
duplicate=raw.replace(b'"id":',b'"id":"'+doc['id'].encode()+b'", "id":',1)
add('duplicate JSON key',signed(duplicate,fixture),False)
# Independent protoc parser/encoder consumes the same signed envelope and fixture.
plain=signed(raw,fixture);proto=['protoc','-I'+str(R/'proto')];schema='nostr/event/v1alpha1/event.proto'
text=subprocess.run(proto+['--decode=nostr.event.v1alpha1.SignedEvent',schema],input=plain,capture_output=True,check=True).stdout
assert subprocess.run(proto+['--encode=nostr.event.v1alpha1.SignedEvent',schema],input=text,capture_output=True,check=True).stdout==plain
print('protoc',subprocess.check_output(['protoc','--version'],text=True).strip(),'source fixture and exact envelope round-trip',flush=True)
class View(C.Structure):_fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
with tempfile.TemporaryDirectory(prefix='coax-event-projection-') as temp:
 d=Path(temp);fixture_path=d/'cases.json';fixture_path.write_text(json.dumps(cases))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  src=d/(mode+'.c');src.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/tests/nostr/event_projection_adapter.coax'),'--library','--header',str(d/'projection.h'),'--run-steps','250000000',*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),str(src),str(R/'coax/tests/nostr/event_projection_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  obj.npa_test_input.restype=C.POINTER(C.c_uint8);obj.npa_test_output.restype=C.POINTER(C.c_uint8);obj.npa_test_owner_key.restype=C.POINTER(C.c_uint8);obj.npa_test_admit.argtypes=[C.c_int64];obj.npa_test_run.argtypes=[C.c_int64];obj.npa_test_run.restype=C.c_int64;obj.npa_test_signed.argtypes=[C.c_int64]
  for case in cases:
   data=bytes(case['wire']);C.memmove(obj.npa_test_input(),data,len(data));n=obj.npa_test_run(len(data));assert (n==len(data))==case['consistent'],case['name']
   if case['consistent']:
    assert bytes(obj.npa_test_output()[:n])==data
    if case['verified'] is not None:assert bool(obj.npa_test_signed(len(data)))==case['verified'],case['name']
    if case['owner'] is not None:
     C.memmove(obj.npa_test_owner_key(),bytes(case['owner']),32);obj.npa_test_reset_effect();assert bool(obj.npa_test_admit(len(data)))==case['admitted'],case['name'];assert obj.npa_test_effect_count()==int(case['admitted']),case['name']
  values=','.join(map(str,plain));main=d/'main.c';main.write_text('#include <stdint.h>\n#include <string.h>\nuint8_t*npa_test_input(void);uint8_t*npa_test_output(void);int64_t npa_test_run(int64_t);int npa_test_signed(int64_t);\nstatic const uint8_t data[]={'+values+'};\nint main(void){memcpy(npa_test_input(),data,sizeof data);return npa_test_run(sizeof data)!=(int64_t)sizeof data || memcmp(data,npa_test_output(),sizeof data) || !npa_test_signed(sizeof data);}\n')
  exe=d/(mode+'-asan');subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR')
  if sim:
   sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip();ios=d/(mode+'-ios')
   subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(main),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS Simulator signed fixture',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,-z,stack-size=2097152,--export=npa_test_input,--export=npa_test_output,--export=npa_test_run,--export=npa_test_signed,--export=npa_test_owner_key,--export=npa_test_admit,--export=npa_test_reset_effect,--export=npa_test_effect_count'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/event_projection_wasm.cjs'),str(wasm),str(fixture_path)],check=True)
  print(mode,len(cases),'source/projection cases, real and forged-signature paths; generated C',hashlib.sha256(src.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
