#!/usr/bin/env python3
"""Malformed numeric option admission and old-importer refusal controls."""
import os,pathlib,subprocess,tempfile
R=pathlib.Path(__file__).resolve().parents[3]
coax=os.environ.get('COAX_TEST_BIN',str(R/'coax/bin/coax'));old=os.environ.get('COAX_LEGACY_BIN')
def var(x):
 out=bytearray()
 while x>=128:out.append((x&127)|128);x>>=7
 out.append(x);return bytes(out)
def vi(n,x):return var(n*8)+var(x)
def bs(n,x):return var(n*8+2)+var(len(x))+x
def descriptor(kind=13, options=None, message_options=None, label=1):
 if options is None:options=vi(51009,0)
 f=bs(1,b'kind')+vi(3,1)+vi(4,label)+vi(5,kind)+bs(8,options)
 m=bs(1,b'M')+bs(2,f)
 if message_options is not None:m+=bs(7,message_options)
 file=bs(1,b'test.proto')+bs(2,b'test')+bs(4,m)+bs(12,b'proto3')
 return bs(1,file)
def invoke(binary,path):return subprocess.run([str(binary),'proto',str(path),'--prefix','R'],capture_output=True)
with tempfile.TemporaryDirectory(prefix='coax-numeric-policy-') as tmp:
 p=pathlib.Path(tmp)/'test.pb'
 def check(label,raw,accepted,needle=None):
  p.write_bytes(raw);r=invoke(coax,p)
  if (r.returncode==0)!=accepted:raise AssertionError((label,r.returncode,r.stderr))
  if not accepted and r.stdout:raise AssertionError((label,'output on refusal'))
  if needle is not None and needle not in r.stdout:raise AssertionError((label,'expected generated guard missing'))
  return r
 zero=descriptor();check('explicit zero',zero,True,b'if f.raw>0 { return R__pb_Check.{error=R__pb_VALUE}; }')
 if old:
  p.write_bytes(zero);r=invoke(old,p);assert r.returncode and b'unsupported policy option' in r.stderr
 for label,raw in [
  ('wrong wire',descriptor(options=bs(51009,b'\0'))),
  ('duplicate',descriptor(options=vi(51009,0)+vi(51009,1))),
  ('maximum overflow',descriptor(options=vi(51009,2**32))),
  ('unknown future',descriptor(options=vi(51010,1))),
  ('message placement',descriptor(options=b'',message_options=vi(51009,0))),
  ('incompatible byte maximum',descriptor(options=vi(51009,0)+vi(51002,1))),
  *[(f'wrong type {kind}',descriptor(kind=kind)) for kind in (1,3,4,5,6,7,8,9,12,14,15,16,17,18)],
 ]:check(label,raw,False)
 check('repeated',descriptor(label=3,options=vi(51009,65535)),True,b'if f.raw>65535 { return R__pb_Check.{error=R__pb_VALUE}; }')
 maximum=check('maximum uint32',descriptor(options=vi(51009,2**32-1)),True)
 assert maximum.stdout.count(b'if f.raw>4294967295 { return R__pb_Check.{error=R__pb_VALUE}; }')==1
 # Omitted option must not accidentally become an explicit max=0.
 omitted=check('omitted',descriptor(options=b''),True)
 assert b'if f.raw>0 { return R__pb_Check.{error=R__pb_VALUE}; }' not in omitted.stdout
 # A graph containing both versions must use distinct package-qualified names.
 # This is a source-compatibility change for clients that combine the profiles.
 protoc=os.environ.get('PROTOC','protoc')
 mixed=pathlib.Path(tmp)/'mixed.pb'
 subprocess.run([protoc,'-I'+str(R/'proto'),'--include_imports',
                 '--descriptor_set_out='+str(mixed),
                 str(R/'proto/nostr/event/v1alpha1/event.proto'),
                 str(R/'proto/nostr/event/v1alpha2/event.proto')],check=True)
 graph=subprocess.run([str(coax),'proto',str(mixed),'--prefix','Mixed'],capture_output=True,check=True).stdout
 for symbol in (b'Mixed_nostr_event_v1alpha1_Event',b'Mixed_nostr_event_v1alpha2_Event'):
  assert symbol in graph,symbol
print('numeric option admission, malformed policy and legacy-importer refusal passed')
