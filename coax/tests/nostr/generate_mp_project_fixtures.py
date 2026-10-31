# Firm reason: 3 (runs protoc 29.3, the reference protobuf implementation, to produce MP fixtures)
import sys,json,os,pathlib,subprocess,tempfile
R=pathlib.Path(__file__).resolve().parents[3]
temp=tempfile.TemporaryDirectory(prefix='mp-coax-fixture-')
gen=pathlib.Path(temp.name);protoc=os.environ.get('PROTOC','protoc')
assert subprocess.check_output([protoc,'--version']).strip()==b'libprotoc 29.3'
subprocess.run([protoc,'-I'+str(R/'proto'),'--python_out='+str(gen),*[str(R/'proto'/x) for x in ['openagents/options/v1alpha1/options.proto','nostr/event/v1alpha1/event.proto','nostr/block/mp/v1alpha1/project.proto']]],check=True)
sys.path.insert(0,str(gen))
from nostr.block.mp.v1alpha1.project_pb2 import Project
key='a'*64
base=[['d','p'],['name','N'],['description','D'],['a','30617:'+key+':repo',''],['a','30617:'+key+':other'],['buzz-channel','C'],['buzz-visibility','unlisted'],['contributions','open'],['x','unknown']]
def fixture(name,tags,kind=30621,alter=None,rawtags=None):
 e={'id':'00'*32,'pubkey':'11'*32,'created_at':0,'kind':kind,'tags':tags if rawtags is None else rawtags,'content':'','sig':'22'*64}
 p=Project();p.original.original_json=json.dumps(e,ensure_ascii=False,separators=(',',':')).encode()
 v=p.original.event;v.id=bytes(32);v.pubkey=bytes([17])*32;v.created_at=0;v.kind=kind;v.content='';v.sig=bytes([34])*64
 for tag in tags:v.tags.add().values.extend(tag)
 for t in tags:
  if len(t)<2:continue
  k,value=t[:2]
  if k=='d':p.identifier=value
  elif k=='name':p.name=value
  elif k=='description':p.description=value
  elif k=='buzz-channel':p.channel=value
  elif k=='buzz-visibility':p.visibility=value
  elif k=='a':
   m=p.members.add();m.coordinate=value
   if len(t)>2:m.relay_hint=t[2]
 if alter:alter(p)
 data=p.SerializeToString(deterministic=True)
 return name,data
cases=[fixture('OK',base),fixture('MIN', [['d','p']]),fixture('EMPTY_TAG',base+[[]]),fixture('BAD_KIND',base,kind=1),fixture('BAD_ORIGINAL',base,rawtags=[['d','other']]),fixture('BAD_D',base,alter=lambda p:setattr(p,'identifier','q')),fixture('BAD_NAME',base,alter=lambda p:setattr(p,'name','x')),fixture('BAD_DESCRIPTION',base,alter=lambda p:setattr(p,'description','x')),fixture('BAD_CHANNEL',base,alter=lambda p:setattr(p,'channel','x')),fixture('BAD_VISIBILITY',base,alter=lambda p:setattr(p,'visibility','x')),fixture('BAD_MEMBER',base,alter=lambda p:setattr(p.members[0],'coordinate','x')),fixture('BAD_HINT',base,alter=lambda p:setattr(p.members[0],'relay_hint','h')),fixture('MISSING_HINT',base,alter=lambda p:p.members[0].ClearField('relay_hint')),fixture('EXTRA_MEMBER',base,alter=lambda p:p.members.add(coordinate='x')),fixture('DUP_D',base+[['d','p']]),fixture('DUP_NAME',base+[['name','N']]),fixture('EMPTY_D',[['d','']]),fixture('BAD_MEMBER_ARITY', [['d','p'],['a','coord','hint','extra']]),
 fixture('A_NO_VALUE',[['d','p'],['a'],['a','x']]),fixture('MISSING_MEMBER',base,alter=lambda p:p.members.pop()),fixture('EXTRA_HINT',base,alter=lambda p:setattr(p.members[1],'relay_hint','h')),
 fixture('NO_D',[['name','N']],alter=lambda p:setattr(p,'identifier','p')),fixture('NAME_ONLY',[['d','p']],alter=lambda p:setattr(p,'name','N')),
 fixture('DESCRIPTION_ONLY',[['d','p']],alter=lambda p:setattr(p,'description','D')),fixture('CHANNEL_ONLY',[['d','p']],alter=lambda p:setattr(p,'channel','C')),
 fixture('VISIBILITY_ONLY',[['d','p']],alter=lambda p:setattr(p,'visibility','unlisted'))]
out='// Deterministic protoc-encoded candidate fixtures. Signatures are synthetic.\n'
for n,b in cases:out+='MP_FIX_'+n+' :: ['+str(len(b))+']u8.['+','.join(map(str,b))+'];\n'
target=R/'coax/tests/nostr/mp_project_fixtures.coax'
if '--check' in sys.argv:
 assert target.read_text()==out,'fixture drift'
else: target.write_text(out)
print(len(cases),'deterministic protoc fixtures',len(out),'bytes')
