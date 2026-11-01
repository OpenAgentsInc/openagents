# Firm reason: 3 (runs protoc 29.3, the reference protobuf implementation, to produce GS fixtures)
# GitSignature fixtures from the NIP-GS test vectors. The two signed vectors are
# the NIP's own; every other case alters armor, JSON or projection. Nothing here
# verifies a signature. --check compares with the committed file.
import base64,json,os,pathlib,subprocess,sys,tempfile
R=pathlib.Path(__file__).resolve().parents[3]
temp=tempfile.TemporaryDirectory(prefix='gs-coax-fixture-')
gen=pathlib.Path(temp.name);protoc=os.environ.get('PROTOC','protoc')
assert subprocess.check_output([protoc,'--version']).strip()==b'libprotoc 29.3'
subprocess.run([protoc,'-I'+str(R/'proto'),'--python_out='+str(gen),*[str(R/'proto'/x) for x in ['openagents/options/v1alpha1/options.proto','nostr/block/oa/v1alpha1/owner.proto','nostr/block/gs/v1alpha1/signature.proto']]],check=True)
sys.path.insert(0,str(gen))
from nostr.block.gs.v1alpha1.signature_pb2 import GitSignature
nip=(R/'nostr/nips/block/NIP-GS.md').read_text()
PK='f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9'
SIG='c35062148d95b820068c18ab9cf69a8dd2322c606890366d084df7617570b96b7a1aca0a8fcabb2eb4032ebbdf5b43e6bf8633e0d85bcecce28a9e08705b875f'
OSIG_PK='79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798'
OSIG='54b97dfd2b7d61c1bc1b5facab9d12a991fe0ac3dcb9044b3176f63bebb6f67340eb0ad866f2d5568b78b58ba234ee9f490f8c41e64a949c200315801520ed25'
SIG_OA='15592857980b8656ff50303d86acaffcbda397b9c0bb40aebd2fb87a723e466fdb1a74404d39f9eb7ac220b4f2e061f27523f1af24cbdf991cf42ff9b47034c0'
def envelope(sig,oa=None,t=1700000000):
 e={'v':1,'pk':PK,'sig':sig,'t':t}
 if oa is not None:e['oa']=oa
 return json.dumps(e,separators=(',',':'))
def armor(text,b64=None,end='\n'):
 line=b64 if b64 is not None else base64.b64encode(text.encode()).decode()
 return ('-----BEGIN SIGNED MESSAGE-----\n'+line+'\n-----END SIGNED MESSAGE-----'+end).encode()
plain=envelope(SIG);owned=envelope(SIG_OA,[OSIG_PK,'',OSIG])
# The NIP's own published strings must be what we rebuild.
assert plain in nip and owned in nip and base64.b64encode(plain.encode()).decode() in nip
def project(text,raw=None,oa=None,sig=SIG,t=1700000000,alter=None):
 g=GitSignature();g.armor=raw if raw is not None else armor(text);g.version=1;g.pubkey=bytes.fromhex(PK);g.signature=bytes.fromhex(sig);g.created_at=t
 if oa is not None:g.owner.owner_pubkey=bytes.fromhex(oa[0]);g.owner.conditions=oa[1];g.owner.signature=bytes.fromhex(oa[2])
 if alter:alter(g)
 return g.SerializeToString(deterministic=True)
O=[OSIG_PK,'',OSIG]
cases=[('OK',project(plain)),('OWNER',project(owned,oa=O,sig=SIG_OA)),
 ('TRAILING_LF',project(plain,raw=armor(plain,end='\n\n'))),
 ('ZERO_T',project(envelope(SIG,t=0),t=0)),('MAX_T',project(envelope(SIG,t=4294967295),t=4294967295)),
 ('CONDITIONS',project(envelope(SIG_OA,[OSIG_PK,'kind=1&created_at<1800000000',OSIG]),oa=[OSIG_PK,'kind=1&created_at<1800000000',OSIG],sig=SIG_OA)),
 ('NO_FINAL_LF',project(plain,raw=armor(plain,end=''))),('TWO_EXTRA_LF',project(plain,raw=armor(plain,end='\n\n\n'))),
 ('CRLF',project(plain,raw=armor(plain).replace(b'\n',b'\r\n'))),('EXTRA_BYTE',project(plain,raw=armor(plain,end='\nx'))),
 ('WRAPPED',project(plain,raw=armor(plain,b64=base64.b64encode(plain.encode()).decode()[:64]+'\n'+base64.b64encode(plain.encode()).decode()[64:]))),
 ('BAD_BEGIN',project(plain,raw=armor(plain).replace(b'BEGIN SIGNED',b'BEGIN PGP'))),('BAD_END',project(plain,raw=armor(plain).replace(b'END SIGNED',b'END PGP'))),
 ('SPACE',project(plain,raw=armor(plain.replace(',"t"',', "t"')))),('ORDER',project(plain,raw=armor(json.dumps({'pk':PK,'v':1,'sig':SIG,'t':1700000000},separators=(',',':'))))),
 ('UPPER_HEX',project(plain,raw=armor(plain.replace(PK,PK.upper())))),('FLOAT_T',project(plain,raw=armor(plain.replace('1700000000','1.7e9')))),
 ('EXTRA_KEY',project(plain,raw=armor(plain[:-1]+',"x":1}'))),('DUP_KEY',project(plain,raw=armor(plain[:-1]+',"t":1700000000}'))),
 ('BAD_PK',project(plain,alter=lambda g:setattr(g,'pubkey',bytes(32)))),('BAD_SIG',project(plain,alter=lambda g:setattr(g,'signature',bytes(64)))),
 ('BAD_T',project(plain,t=1700000001)),('BAD_VERSION',project(plain.replace('"v":1','"v":2'),alter=lambda g:setattr(g,'version',2))),
 ('VERSION_ONLY',project(plain,alter=lambda g:setattr(g,'version',2))),
 ('OWNER_MISSING',project(owned,sig=SIG_OA)),('OWNER_EXTRA',project(plain,oa=O)),
 ('BAD_OWNER_KEY',project(owned,oa=O,sig=SIG_OA,alter=lambda g:setattr(g.owner,'owner_pubkey',bytes(32)))),
 ('BAD_OWNER_SIG',project(owned,oa=O,sig=SIG_OA,alter=lambda g:setattr(g.owner,'signature',bytes(64)))),
 ('BAD_CONDITIONS',project(owned,oa=O,sig=SIG_OA,alter=lambda g:setattr(g.owner,'conditions','kind=1'))),
 ('SELF_OWNER',project(envelope(SIG_OA,[PK,'',OSIG]),oa=[PK,'',OSIG],sig=SIG_OA)),
 ('QUOTE_CONDITIONS',project(envelope(SIG_OA,[OSIG_PK,'a"b',OSIG]),oa=[OSIG_PK,'a"b',OSIG],sig=SIG_OA)),
 ('ESCAPED_CONDITIONS',project(envelope(SIG_OA,[OSIG_PK,'\\u0026',OSIG]).replace('\\\\u0026','\\u0026'),oa=[OSIG_PK,'&',OSIG],sig=SIG_OA)),
 ('CONTROL_CONDITIONS',project(envelope(SIG_OA,[OSIG_PK,'\t',OSIG]),oa=[OSIG_PK,'\t',OSIG],sig=SIG_OA)),
 ('DEL_CONDITIONS',project(envelope(SIG_OA,[OSIG_PK,'\x7f',OSIG]),oa=[OSIG_PK,'\x7f',OSIG],sig=SIG_OA)),
 ('PAD_BITS',project(plain,raw=armor(plain,b64=base64.b64encode(plain.encode()).decode()[:-2]+'R=')))]
# The 2048-byte decoded JSON limit, exactly and one past it.
fit=[OSIG_PK,'k'*(2048-len(envelope(SIG_OA,[OSIG_PK,'',OSIG]))),OSIG];assert len(envelope(SIG_OA,fit))==2048
cases.append(('FULL',project(envelope(SIG_OA,fit),oa=fit,sig=SIG_OA)))
big=[OSIG_PK,fit[1]+'k',OSIG];assert len(envelope(SIG_OA,big))==2049
cases.append(('OVERSIZE',project(envelope(SIG_OA,big),oa=big,sig=SIG_OA)))
out='// Deterministic protoc-encoded NIP-GS fixtures (generate_gs_signature_fixtures.py). Only OK and OWNER carry the NIP\'s signatures; no case is verified here.\n'
for n,b in cases:out+='GS_FIX_'+n+' :: ['+str(len(b))+']u8.['+','.join(map(str,b))+'];\n'
target=R/'coax/tests/nostr/gs_signature_fixtures.coax'
if '--check' in sys.argv:
 assert target.read_text()==out,'fixture drift'
else: target.write_text(out)
print(len(cases),'deterministic protoc fixtures',len(out),'bytes')
