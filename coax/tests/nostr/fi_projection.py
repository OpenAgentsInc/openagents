#!/usr/bin/env python3
"""FI original-byte and generated projection tests; all keys are public fixtures."""
if not __debug__:raise SystemExit('tests require Python assertions')
import base64,copy,ctypes as C,hashlib,json,os,subprocess,sys,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];compiler=Path(os.environ.get('COAX_TEST_BIN',str(R/'coax/bin/coax')))
subprocess.run([sys.executable,str(R/'coax/tests/nostr/proto_wire_parity.py')],check=True);fixture=R/'coax/tests/nostr/data/fi-jws'
def var(n):
 o=bytearray()
 while n>=128:o.append((n&127)|128);n>>=7
 o.append(n);return bytes(o)
def b(n,x):return var(n*8+2)+var(len(x))+x
def v(n,x):return var(n*8)+var(x)
def un64(s):return base64.urlsafe_b64decode(s+b'='*((-len(s))%4))
def enc64(s):return base64.urlsafe_b64encode(s).rstrip(b'=')
def obj(x):return b(1,x)
def projected(payload,claims,header,token,unknown=b''):
 aud=claims['aud'];audience=b(1,aud.encode()) if isinstance(aud,str) else b(2,b''.join(b(1,x.encode()) for x in aud))
 fields=(b(1,obj(payload))+b(2,claims['iss'].encode())+b(3,claims['sub'].encode())+b(4,bytes.fromhex(claims['nostr_pubkey']))+b(5,audience)+b(6,b(1,str(claims['iat']).encode()))+b(7,b(1,str(claims['exp']).encode())))
 if 'nbf' in claims:fields+=b(8,b(1,str(claims['nbf']).encode()))
 if 'nostr_owner_pubkey' in claims:fields+=b(9,bytes.fromhex(claims['nostr_owner_pubkey']))
 if 'client_id' in claims:fields+=b(10,claims['client_id'].encode())
 return b(1,token)+b(2,obj(header))+b(3,fields)+unknown
real=(fixture/'claims-token.txt').read_bytes().strip();a,p,s=real.split(b'.');header=un64(a);payload=un64(p);claims=json.loads(payload)
assert json.loads(header)['alg']=='RS256'
# Verify this shipped public test vector independently. A forged signature must
# not turn a structurally consistent transport into a verified identity.
def openssl_ok(token):
 a,p,s=token.split(b'.')
 with tempfile.TemporaryDirectory(prefix='coax-fi-openssl-') as tmp:
  d=Path(tmp);(d/'message').write_bytes(a+b'.'+p);(d/'sig').write_bytes(un64(s))
  return subprocess.run(['openssl','dgst','-sha256','-verify',str(fixture/'claims-public.pem'),'-signature',str(d/'sig'),str(d/'message')],capture_output=True).returncode==0
assert openssl_ok(real)
modulus=un64(json.loads((fixture/'claims-jwks.json').read_text())['keys'][0]['n'].encode());assert len(modulus)==256
cases=[]
def add(name,wire,good,signature=None):cases.append({'name':name,'wire':list(wire),'consistent':good,'signature':signature,'modulus':list(modulus)})
add('real signed fixture',projected(payload,claims,header,real),True,True)
add('unknown authority flag',projected(payload,claims,header,real,v(90,1)),True,True)
forged_sig=bytearray(un64(s));forged_sig[0]^=1;forged=a+b'.'+p+b'.'+enc64(forged_sig)
assert not openssl_ok(forged);add('structurally matching forged signature is unverified',projected(payload,claims,header,forged,v(91,1)),True,False)
for key in ('iss','sub','nostr_pubkey','aud','iat','exp','client_id'):
 c=copy.deepcopy(claims)
 if key=='nostr_pubkey':c[key]=('00' if c[key][:2]!='00' else '11')+c[key][2:]
 elif key in ('iat','exp'):c[key]+=1
 else:c[key]+='x'
 add('mismatch '+key,projected(payload,c,header,real),False)
for key,val in [('nbf',1),('nostr_owner_pubkey','11'*32)]:
 c=copy.deepcopy(claims);c[key]=val;add('extra optional '+key,projected(payload,c,header,real),False)
c=copy.deepcopy(claims);del c['client_id'];add('missing optional client',projected(payload,c,header,real),False)
c=copy.deepcopy(claims);c['aud']=[c['aud']];add('audience string versus singleton array',projected(payload,c,header,real),False)
array=copy.deepcopy(claims);array['aud']=[claims['aud'],'https://other.test'];array['nbf']=1;array['nostr_owner_pubkey']='11'*32
array_bytes=json.dumps(array,separators=(',',':')).encode();array_token=a+b'.'+enc64(array_bytes)+b'.'+s
add('array and optional claims',projected(array_bytes,array,header,array_token),True)
wrong=copy.deepcopy(array);wrong['aud'].reverse();add('audience order',projected(array_bytes,wrong,header,array_token),False)
add('different original payload',projected(payload+b' ',claims,header,real),False)
add('different original header',projected(payload,claims,header+b' ',real),False)
add('truncated',projected(payload,claims,header,real)[:-1],False)
# Independent protoc parses/re-emits the exact source and projection.
plain=projected(payload,claims,header,real);schema='nostr/block/fi/v1alpha1/identity.proto';cmd=['protoc','-I'+str(R/'proto')]
text=subprocess.run(cmd+['--decode=nostr.block.fi.v1alpha1.Assertion',schema],input=plain,capture_output=True,check=True).stdout
assert subprocess.run(cmd+['--encode=nostr.block.fi.v1alpha1.Assertion',schema],input=text,capture_output=True,check=True).stdout==plain
print('protoc',subprocess.check_output(['protoc','--version'],text=True).strip(),'and OpenSSL signed/forged fixture checks passed',flush=True)
with tempfile.TemporaryDirectory(prefix='coax-fi-projection-') as temp:
 d=Path(temp);case_file=d/'cases.json';case_file.write_text(json.dumps(cases))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  src=d/(mode+'.c');src.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/tests/nostr/fi_projection_adapter.coax'),'--library','--header',str(d/'fi-projection.h'),'--run-steps','250000000',*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),str(src),str(R/'coax/tests/nostr/fi_projection_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);objlib=C.CDLL(str(lib))
  objlib.fpa_test_input.restype=C.POINTER(C.c_uint8);objlib.fpa_test_output.restype=C.POINTER(C.c_uint8);objlib.fpa_test_modulus.restype=C.POINTER(C.c_uint8);objlib.fpa_test_verified.argtypes=[C.c_int64];C.memmove(objlib.fpa_test_modulus(),modulus,len(modulus));objlib.fpa_test_run.argtypes=[C.c_int64];objlib.fpa_test_run.restype=C.c_int64
  assert objlib.fpa_test_run(-1)==-1 and objlib.fpa_test_run(1048577)==-1
  for case in cases:
   data=bytes(case['wire']);C.memmove(objlib.fpa_test_input(),data,len(data));n=objlib.fpa_test_run(len(data));assert (n==len(data))==case['consistent'],case['name']
   if case['consistent']:
    assert bytes(objlib.fpa_test_output()[:n])==data
    if case['signature'] is not None:assert bool(objlib.fpa_test_verified(len(data)))==case['signature'],case['name']
  main=d/'main.c';main.write_text('#include <stdint.h>\n#include <string.h>\nuint8_t*fpa_test_input(void);uint8_t*fpa_test_output(void);int64_t fpa_test_run(int64_t);\nstatic const uint8_t data[]={'+','.join(map(str,plain))+'};\nint main(void){memcpy(fpa_test_input(),data,sizeof data);return fpa_test_run(-1)!=-1 || fpa_test_run(1048577)!=-1 || fpa_test_run(sizeof data)!=(int64_t)sizeof data || memcmp(data,fpa_test_output(),sizeof data);}\n')
  exe=d/(mode+'-asan');subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR')
  if sim:
   sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip();ios=d/(mode+'-ios')
   subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(main),'-o',str(ios)],check=True);subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True)
   print(mode,'iOS Simulator FI fixture',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,-z,stack-size=2097152,--export=fpa_test_input,--export=fpa_test_output,--export=fpa_test_run,--export=fpa_test_modulus,--export=fpa_test_verified'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/fi_projection_wasm.cjs'),str(wasm),str(case_file)],check=True)
  print(mode,len(cases),'FI consistency cases; generated C',hashlib.sha256(src.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
