# Firm reason: 3 (runs protoc 29.3, the reference protobuf implementation, to produce PL and RS fixtures)
# PushLease and ReadStateEvent fixtures: an original event (synthetic id and
# signature) with its event projection, and the envelope projection, then one
# alteration per case. --check compares with the committed files.
import json,os,pathlib,subprocess,sys,tempfile
R=pathlib.Path(__file__).resolve().parents[3]
temp=tempfile.TemporaryDirectory(prefix='pl-rs-coax-fixture-')
gen=pathlib.Path(temp.name);protoc=os.environ.get('PROTOC','protoc')
assert subprocess.check_output([protoc,'--version']).strip()==b'libprotoc 29.3'
subprocess.run([protoc,'-I'+str(R/'proto'),'--python_out='+str(gen),*[str(R/'proto'/x) for x in ['openagents/options/v1alpha1/options.proto','nostr/event/v1alpha1/event.proto','nostr/block/pl/v1alpha1/lease.proto','nostr/block/rs/v1alpha1/read_state.proto']]],check=True)
sys.path.insert(0,str(gen))
from nostr.block.pl.v1alpha1.lease_pb2 import PushLease
from nostr.block.rs.v1alpha1.read_state_pb2 import ReadStateEvent
def original(m,kind,tags,rawtags=None):
 e={'id':'00'*32,'pubkey':'11'*32,'created_at':1769990000,'kind':kind,'tags':tags if rawtags is None else rawtags,'content':'AgCiphertext','sig':'22'*64}
 m.original.original_json=json.dumps(e,ensure_ascii=False,separators=(',',':')).encode()
 v=m.original.event;v.id=bytes(32);v.pubkey=bytes([17])*32;v.created_at=1769990000;v.kind=kind;v.content=e['content'];v.sig=bytes([34])*64
 for tag in tags:v.tags.add().values.extend(tag)
I='f3a1c9e04b7d2a5e8c6f1b3d9e2a7c40';X='exec-key-1'
lease=[['d',I],['expiration','1772582000'],['exec',X],['alt','Push lease']]
def pl(name,tags,kind=30350,alter=None,rawtags=None):
 p=PushLease();original(p,kind,tags,rawtags)
 for t in tags:
  if len(t)<2:continue
  if t[0]=='d':p.installation=t[1]
  elif t[0]=='expiration' and t[1].isdigit() and int(t[1])<2**64:p.expiration=int(t[1])
  elif t[0]=='exec':p.executor_key=t[1]
  elif t[0]=='alt':p.alt=t[1]
 if not p.HasField('expiration'):p.expiration=1772582000
 if alter:alter(p)
 return name,p.SerializeToString(deterministic=True)
L=lease;nd=[t for t in L if t[0]!='alt']
plc=[pl('OK',L),pl('NO_ALT',nd),pl('ZERO',[['d',I],['expiration','0'],['exec',X]]),pl('MAX',[['d',I],['expiration','18446744073709551615'],['exec',X]]),
 pl('D64',[['d','a'*64],['expiration','1'],['exec',X]]),
 pl('BAD_KIND',L,kind=30078),pl('BAD_ORIGINAL',L,rawtags=nd),
 pl('BAD_D',L,alter=lambda p:setattr(p,'installation','other')),pl('BAD_EXPIRATION',L,alter=lambda p:setattr(p,'expiration',1772582001)),
 pl('BAD_EXEC',L,alter=lambda p:setattr(p,'executor_key','x')),pl('BAD_ALT',L,alter=lambda p:setattr(p,'alt','x')),
 pl('ALT_ONLY_PROJECTED',nd,alter=lambda p:setattr(p,'alt','Push lease')),pl('ALT_NOT_PROJECTED',L,alter=lambda p:p.ClearField('alt')),
 pl('NO_D',[t for t in L if t[0]!='d'],alter=lambda p:setattr(p,'installation',I)),pl('NO_EXPIRATION',[t for t in L if t[0]!='expiration']),
 pl('NO_EXEC',[t for t in L if t[0]!='exec'],alter=lambda p:setattr(p,'executor_key',X)),
 pl('DUP_D',L+[['d',I]]),pl('DUP_EXPIRATION',L+[['expiration','1772582000']]),pl('DUP_EXEC',L+[['exec',X]]),pl('DUP_ALT',L+[['alt','Push lease']]),
 pl('EXTRA_TAG',L+[['p','11'*32]]),pl('EMPTY_TAG',L+[[]]),pl('EXTRA_VALUE',[['d',I,'x']]+L[1:]),pl('EXTRA_EXEC_VALUE',L[:2]+[['exec',X,'relay']]+L[3:]),
 pl('NO_VALUE',L[:2]+[['exec']]+L[3:],alter=lambda p:setattr(p,'executor_key',X)),
 pl('LEADING_ZERO',[['d',I],['expiration','01772582000'],['exec',X]]),pl('SIGNED',[['d',I],['expiration','-1'],['exec',X]],alter=lambda p:setattr(p,'expiration',1)),
 pl('PLUS',[['d',I],['expiration','+1'],['exec',X]],alter=lambda p:setattr(p,'expiration',1)),
 pl('OVERFLOW',[['d',I],['expiration','18446744073709551616'],['exec',X]],alter=lambda p:setattr(p,'expiration',0)),
 pl('EMPTY_EXPIRATION',[['d',I],['expiration',''],['exec',X]],alter=lambda p:setattr(p,'expiration',0)),
 pl('EMPTY_D',[['d',''],['expiration','1'],['exec',X]]),
 pl('UNKNOWN_AS_D',[['x',I],['expiration','1772582000'],['exec',X]],alter=lambda p:setattr(p,'installation',I))]
S='0123456789abcdef0123456789abcdef'
rs=[['d','read-state:'+S],['t','read-state']]
def rsx(name,tags,kind=30078,alter=None,rawtags=None):
 m=ReadStateEvent();original(m,kind,tags,rawtags)
 for t in tags:
  if len(t)>1 and t[0]=='d' and not m.HasField('slot'):m.slot=t[1][len('read-state:'):][:32]
 if not m.HasField('slot'):m.slot=S
 if alter:alter(m)
 return name,m.SerializeToString(deterministic=True)
rsc=[rsx('OK',rs),rsx('MORE_TAGS',[['t','other'],['alt','Read state']]+rs+[['t','misc'],[]]),rsx('EXTRA_D_VALUE',[['d','read-state:'+S,'x'],['t','read-state']]),
 rsx('BAD_KIND',rs,kind=30079),rsx('BAD_ORIGINAL',rs,rawtags=[['d','read-state:'+S]]),rsx('BAD_SLOT',rs,alter=lambda m:setattr(m,'slot','f'+S[1:])),
 rsx('NO_D',[['t','read-state']]),rsx('TWO_D',rs+[['d','read-state:'+S]]),rsx('D_NO_VALUE',[['d'],['t','read-state']]),
 rsx('OTHER_PREFIX',[['d','readstate:'+S],['t','read-state']],alter=lambda m:setattr(m,'slot',S)),
 rsx('UPPER',[['d','read-state:'+S.upper()],['t','read-state']]),rsx('SHORT',[['d','read-state:'+S[:31]],['t','read-state']]),
 rsx('LONG',[['d','read-state:'+S+'0'],['t','read-state']]),rsx('NOT_HEX',[['d','read-state:'+S[:31]+'g'],['t','read-state']]),
 rsx('NO_T',[['d','read-state:'+S]]),rsx('TWO_T',rs+[['t','read-state']]),rsx('T_OTHER_ONLY',[['d','read-state:'+S],['t','readstate']]),
 rsx('T_NO_VALUE',[['d','read-state:'+S],['t'],['t','read-state']]),rsx('VALUE_NOT_T',rs+[['x','read-state']])]
for cases,prefix,name in [(plc,'PL_FIX_','pl_lease_fixtures.coax'),(rsc,'RS_FIX_','rs_read_state_fixtures.coax')]:
 out='// Deterministic protoc-encoded fixtures (generate_pl_rs_fixtures.py). Ids and signatures are synthetic.\n'
 for n,b in cases:out+=prefix+n+' :: ['+str(len(b))+']u8.['+','.join(map(str,b))+'];\n'
 target=R/'coax/tests/nostr'/name
 if '--check' in sys.argv:assert target.read_text()==out,'fixture drift: '+name
 else:target.write_text(out)
 print(len(cases),prefix,'fixtures')
