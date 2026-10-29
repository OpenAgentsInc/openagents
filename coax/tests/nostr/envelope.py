#!/usr/bin/env python3
"""Independent JSON/hash oracle, actual native/Wasm closure and resource limits."""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import random
import struct
import subprocess
import sys
import tempfile
import unittest
from jcs import canonical, run

ROOT=Path(__file__).resolve().parents[3];COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax'))
PURE=ROOT/'coax/lib/nostr/envelope.coax';BRIDGE=ROOT/'coax/lib/nostr/envelope-memory.coax';LIMIT=524288
def encoded(s):return s.encode() if isinstance(s,str) else s
def dump(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def reference(raw,schema,issued,retain):
 c=canonical(json.loads(raw));digest='sha256:'+hashlib.sha256(c.encode()).hexdigest()
 prefix={'v':'openagents.artifact-envelope.v1','requires':[],'artifact':{'digest':digest,'size':len(c.encode()),'media_type':'application/json','schema':schema}}
 return dump(prefix)[:-1]+',"inline":'+raw+',"issued_at":'+str(issued)+',"retain_until":'+str(retain)+'}',len(c.encode())
def cases():
 result=[]
 def add(op,raw,schema='x',issued=0,retain=1,wire=-1,error=0,expected=b'',size=0,label=''):
  result.append(dict(id=label,op=op,input=encoded(raw).hex(),schema=encoded(schema).hex(),issued=issued,retain=retain,wire=wire,error=error,expected=encoded(expected).hex(),canonical=size))
 def build(raw,schema='x',issued=0,retain=1,label=''):
  body,size=reference(raw,schema,issued,retain);add(0,raw,schema,issued,retain,expected=body,size=size,label=label);return body
 def check(body,raw,schema='x',issued=0,retain=1,wire=-1,label=''):
  size=len(canonical(json.loads(raw)).encode());add(1,body,schema,issued,retain,wire,expected=raw.strip(),size=size,label=label)
 fixture=json.loads((ROOT/'coax/tests/nostr/data/envelope-test-deck-v1.json').read_text())
 body=build(fixture['inline_json'],fixture['schema'],fixture['issued_at'],fixture['retain_until'],'actual-legacy-deck')
 assert body==fixture['envelope_json'] and len(fixture['canonical_inline'].encode())==182
 assert 'sha256:'+hashlib.sha256(fixture['canonical_inline'].encode()).hexdigest()==fixture['digest']
 check(body,fixture['inline_json'],fixture['schema'],fixture['issued_at'],fixture['retain_until'],1048576,'actual-legacy-observed')
 simple=build('{}',label='empty');check(simple,'{}',label='body-only');check(simple,'{}',wire=1048576,label='wire-exact')
 for wire in [0,-2,1048577]:add(1,simple,wire=wire,error=12,label='wire-refusal-'+str(wire))
 rng=random.Random(3188)
 for i in range(96):
  value={'z':rng.randrange(-999,999),'a':rng.choice(['\0','é','😀','"','\\','e\u0301']),'n':[True,False,None,{'answer':i}]}
  raw=json.dumps(value,ensure_ascii=i%2==0,indent=1 if i%3==0 else None)
  b=build(raw,'example.v1',i,i+1,'seeded-build-'+str(i));check(b,raw,'example.v1',i,i+1,wire=1+i,label='seeded-check-'+str(i))
 # Wrapper overhead: every exact case must survive actual build -> consume.
 for over in [0,1]:
  raw='{"n":['+','.join('0' for _ in range(2036+over))+']}'
  if over:add(0,raw,error=1,label='reserved-node-over');add(1,reference(raw,'x',0,1)[0],error=2,label='consumer-node-over')
  else:build(raw,label='reserved-node-exact')
  raw='{'+','.join('"'+f'{i:04}'+ '":0' for i in range(1013+over,-1,-1))+'}'
  if over:add(0,raw,error=1,label='reserved-keys-over');add(1,reference(raw,'x',0,1)[0],error=2,label='consumer-keys-over')
  else:build(raw,label='reserved-keys-exact')
  raw='{"'+'a'*(65466+over)+'":0}'
  if over:add(0,raw,error=1,label='reserved-key-arena-over');add(1,reference(raw,'x',0,1)[0],error=2,label='consumer-key-arena-over')
  else:build(raw,label='reserved-key-arena-exact')
  raw='{"a":'+'['*(14+over)+'0'+']'*(14+over)+'}'
  if over:add(0,raw,error=1,label='reserved-depth-over');add(1,reference(raw,'x',0,1)[0],error=2,label='consumer-depth-over')
  else:build(raw,label='reserved-depth-exact')
  keys=['a'*(1011+(i>=2)+(over and i>=40))+f'{i:02}' for i in range(46)]
  raw='{'+','.join(dump(k)+':0' for k in keys)+'}'
  if over:add(0,raw,error=1,label='reserved-work-over');add(1,reference(raw,'x',0,1)[0],error=2,label='consumer-work-over')
  else:build(raw,label='reserved-work-within')
  raw=' '*(LIMIT-len(simple)+over)+'{}';b,_=reference(raw,'x',0,1);assert len(b)==LIMIT+over
  if over:add(0,raw,error=1,label='body-output-over');add(1,b,error=1,label='body-input-over')
  else:build(raw,label='body-output-exact');check(b,raw,label='body-input-exact')
  raw='{"s":"'+'a'*(65536+over)+'"}'
  if over:add(0,raw,error=2,label='string-over')
  else:build(raw,label='string-exact')
  raw='{"s":"'+'\\u0000'*10922+'a'*(4+over)+'"}'
  if over:add(0,raw,error=2,label='raw-string-over')
  else:build(raw,label='raw-string-exact')
  schema='a'*(65536+over)
  if over:add(0,'{}',schema,error=6,label='schema-over')
  else:build('{}',schema,label='schema-exact')
  schema='\0'+'a'*(65530+over)
  if over:add(0,'{}',schema,error=6,label='schema-raw-over')
  else:build('{}',schema,label='schema-raw-exact')
 build('{"v":-0,"s":"\\u00e9"}','schema.😀',9007199254740990,9007199254740991,'safe-times-negative-zero')
 # Independent schema/feature/type/digest cases, one defect per record.
 obj=json.loads(simple)
 def changed(label,error,edit):
  v=json.loads(simple);edit(v);add(1,dump(v),error=error,label=label)
 changed('version',4,lambda v:v.update(v='other'))
 changed('version-type',4,lambda v:v.update(v=1))
 changed('requires-feature',5,lambda v:v.update(requires=['unknown']))
 changed('requires-type',3,lambda v:v.update(requires={}))
 changed('root-unknown',5,lambda v:v.update(effect=True))
 changed('meta-type',3,lambda v:v.update(meta=False))
 v=json.loads(simple);v['meta']={'effect':'inert'};check(dump(v),'{}',label='meta-inert')
 for field in obj:changed('root-missing-'+field,3,lambda v,f=field:v.pop(f))
 for field in obj['artifact']:changed('artifact-missing-'+field,3,lambda v,f=field:v['artifact'].pop(f))
 for field in ['event','sources','effect']:changed('artifact-unsupported-'+field,5,lambda v,f=field:v['artifact'].update({f:[]}))
 for value in ['bad','SHA256:'+obj['artifact']['digest'][7:],0]:changed('digest-'+str(value)[:8],7,lambda v,a=value:v['artifact'].update(digest=a))
 for value in [-1,3]:changed('size-'+str(value),8,lambda v,a=value:v['artifact'].update(size=a))
 changed('size-type',3,lambda v:v['artifact'].update(size='2'))
 for value in ['y',0]:changed('schema-'+str(value),6,lambda v,a=value:v['artifact'].update(schema=a))
 for value in ['APPLICATION/JSON',False]:changed('mime-'+str(value),5,lambda v,a=value:v['artifact'].update(media_type=a))
 changed('artifact-type',3,lambda v:v.update(artifact=[]))
 changed('external-inline-null',5,lambda v:v.update(inline=None))
 for value in [[],0,'x']:changed('inline-type-'+str(value),3,lambda v,a=value:v.update(inline=a))
 for field,value in [('issued_at',-1),('retain_until',0)]:changed(field+'-time',9,lambda v,f=field,a=value:v.update({f:a}))
 for field in ['issued_at','retain_until']:changed(field+'-type',3,lambda v,f=field:v.update({f:'x'}))
 for raw in ['!','{"v":1,"v":2}','{"s":"\\ud800"}','{"s":"\\ufdd0"}','{"x":1e0}']:
  add(0,raw,error=2,label='bad-inline-'+str(len(result)));add(1,raw,error=2,label='bad-body-'+str(len(result)))
 for raw in ['[]','1','null']:add(0,raw,error=3,label='builder-shape-'+raw)
 add(0,'{}','',error=6,label='empty-schema');add(0,'{}','x',-1,1,error=9,label='builder-time');add(0,'{}','x',0,9007199254740992,error=9,label='builder-unsafe-time')
 return result,fixture

class Envelope(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-envelope-');cls.dir=Path(cls.temp.name);cls.cases,fixture=cases();cls.fixture=cls.dir/'vectors.json';cls.fixture.write_text(json.dumps(cls.cases));cls.binary=bytearray()
  for c in cls.cases:
   data=bytes.fromhex(c['input']);schema=bytes.fromhex(c['schema']);expected=bytes.fromhex(c['expected']);cls.binary.extend(struct.pack('<IIIIIIqqq',c['op'],len(data),len(schema),len(expected),c['error'],c['canonical'],c['issued'],c['retain'],c['wire'])+data+schema+expected)
  cls.evidence={'compiler':run(COMPILER,'version').stdout.strip(),'compiler_sha256':hashlib.sha256(COMPILER.read_bytes()).hexdigest(),'records':len(cls.cases),'seed':3188,'fixture_sha256':hashlib.sha256(cls.fixture.read_bytes()).hexdigest(),'legacy_digest':fixture['digest'],'legacy_canonical_bytes':182,'legacy_body_bytes':len(fixture['envelope_json'].encode()),'oracle':json.loads(run('node',ROOT/'coax/tests/nostr/envelope_wasm.cjs','oracle',cls.fixture).stdout),'sources':{},'results':[]}
  paths=[PURE,BRIDGE,Path(__file__),ROOT/'coax/tests/nostr/envelope_native.c',ROOT/'coax/tests/nostr/envelope_wasm.c',ROOT/'coax/tests/nostr/envelope_wasm.cjs',ROOT/'coax/tests/nostr/data/envelope-test-deck-v1.json',ROOT/'coax/lib/nostr/jcs.coax',ROOT/'coax/lib/crypto/sha256.coax',ROOT/'coax/lib/nostr/hex.coax']
  dependencies=json.loads(run(COMPILER,'deps',BRIDGE).stdout)['sources'];paths.extend(Path(p) for p in dependencies)
  assert not any(p.endswith('/json-f64.coax') for p in dependencies)
  original=run('git','-C',ROOT,'show','069e2cb5a3:coax/lib/nostr/json.coax').stdout
  a=original.index('// A number token\'s text as the nearest-ish f64:');b=original.index('// A UTF-8 encoding of cp',a)
  c=original.index('#test "numbers as f64:');d=original.index('#test "bounded starts',c)
  frozen=original[a:b]+original[c:d];opt_in=ROOT/'coax/lib/nostr/json-f64.coax'
  assert opt_in.read_text().split('#load "json.coax";\n\n',1)[1]==frozen
  paths.extend([opt_in,ROOT/'coax/lib/agent/claude/fields.coax'])
  cls.evidence['float_extraction']={'original_commit':'069e2cb5a3','helper_and_checks_sha256':hashlib.sha256(frozen.encode()).hexdigest(),'byte_identical':True,'envelope_imports_float_helper':False}
  cls.evidence['sources']={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths};cls.evidence['native_compiler']=run('cc','--version').stdout.splitlines()[0]
  for label,flags in [('checked',[]),('release',['--release'])]:
   c=cls.dir/(label+'.c');c.write_text(run(COMPILER,'c',BRIDGE,'--library','--header',cls.dir/'envelope.h',*flags).stdout)
 @classmethod
 def tearDownClass(cls):cls.temp.cleanup()
 def test_native_sanitizers_independent_reference_closure_and_transactions(self):
  for label in ['checked','release']:
   exe=self.dir/(label+'-sanitized');run('cc','-std=c11','-O2','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-I'+str(self.dir),self.dir/(label+'.c'),ROOT/'coax/tests/nostr/envelope_native.c','-lm','-o',exe)
   result=subprocess.run([str(exe)],input=self.binary,capture_output=True,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'});self.assertEqual(result.returncode,0,result.stderr.decode());self.evidence['results'].append(dict(target='native-arm64-asan-ubsan',profile=label,records=len(self.cases),observation=result.stdout.decode().strip()));print(label+': '+result.stdout.decode().strip())
 def test_actual_wasm_independent_reference_closure_and_transactions(self):
  clang=Path(os.environ.get('COAXIAL_CLANG','/opt/homebrew/opt/llvm/bin/clang'));sysroot=Path(os.environ.get('COAXIAL_SYSROOT','/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'));resources=Path(os.environ.get('COAXIAL_RESOURCES','/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'));environment={**os.environ,'PATH':'/opt/homebrew/opt/lld/bin:'+os.environ.get('PATH','')}
  for label in ['checked','release']:
   module=self.dir/(label+'.wasm');run(clang,'--target=wasm32-wasip1','--sysroot='+str(sysroot),'-resource-dir='+str(resources),'-std=c11','-O2','-ffp-contract=off','-fno-fast-math','-I'+str(self.dir),'-mexec-model=reactor',self.dir/(label+'.c'),ROOT/'coax/tests/nostr/envelope_wasm.c','-Wl,--import-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=2097152,--gc-sections','-o',module,env=environment)
   result=json.loads(run('node',ROOT/'coax/tests/nostr/envelope_wasm.cjs',module,self.fixture).stdout);result.update(profile=label,wasm_compiler=run(clang,'--version').stdout.splitlines()[0]);self.evidence['results'].append(result);print(f"{label}: {result['records']} actual Wasm envelope records passed")
if __name__=='__main__':
 evidence=None
 if '--evidence'in sys.argv:
  at=sys.argv.index('--evidence');evidence=Path(sys.argv[at+1]);del sys.argv[at:at+2]
 result=unittest.main(exit=False)
 if result.result.wasSuccessful() and evidence:evidence.write_text(json.dumps(Envelope.evidence,indent=2)+'\n')
 sys.exit(not result.result.wasSuccessful())
