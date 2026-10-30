#!/usr/bin/env python3
"""Native end-to-end signing: schnorr_sign.coax and event_sign.coax looped to
completion through the generated C, checked and release, plain and
ASan/UBSan, against the pinned BIP-340 vectors and an independent
pure-Python BIP-340 verifier and NIP-01 serializer."""
if not __debug__: raise SystemExit('tests require Python assertions')
import csv,hashlib,json,random,subprocess,sys,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];sys.path.insert(0,str(R/'tools/coax-nostr/fuzz'))
from oracle import N,point_mul,schnorr_verify
compiler=R/'coax/bin/coax';rng=random.Random(340)
rows=[r for r in csv.DictReader(open(R/'coax/tests/vectors/upstream/bip340/test-vectors.csv')) if r['secret key']]  # signing rows 0-3 and 15-18 (0, 1, 17, 100-byte messages)
H=lambda b:b.hex() or '-'
requests=[];expect=[]
for row in rows:
 requests.append('S %s %s %s'%(row['secret key'],row['aux_rand'],H(bytes.fromhex(row['message']))));expect.append(('vector',row))
for i in range(24):
 d=rng.randrange(1,N);m=rng.randbytes([0,1,32,33,64,200][i%6]);aux=rng.randbytes(32)
 requests.append('S %064x %s %s'%(d,aux.hex(),H(m)));expect.append(('random',(d,m)))
for bad in ['00'*32,'%064x'%N,'ff'*32]:
 requests.append('S %s %s %s'%(bad,'00'*32,'00'*32));expect.append(('refused',None))
dump=lambda v:json.dumps(v,separators=(',',':'),ensure_ascii=False)
events=[(1730000000,1,[['e','00'*32],['p','11'*32,'wss://r.example']],'hello\n"world" \\ é 😀'),
 (0,0,[],''),(9007199254740991,65535,[[],['a','30023:%s:x'%('ab'*32)]],'\t\r\x08\x0c\x01 control'),
 (5,30617,[['d','repo'],['t','x']*3],'x'*5000)]
for created,kind,tags,content in events:
 d=rng.randrange(1,N);requests.append('E %064x %s %d %d %s %s'%(d,rng.randbytes(32).hex(),created,kind,H(dump(tags).encode()),H(content.encode())))
 expect.append(('event',(d,created,kind,tags,content)))
for created,kind,tags in [(-1,1,'[]'),(0,65536,'[]'),(0,1,'[["a",1]]'),(0,1,'[[]'),(9007199254740992,1,'[]')]:
 requests.append('E %064x %s %d %d %s -'%(3,'00'*32,created,kind,H(tags.encode())));expect.append(('refused',None))
def check(lines):
 assert len(lines)==len(expect)
 for got,(what,data) in zip(lines,expect):
  if what=='vector':assert got.upper()==data['signature'],(got,data['index'])
  elif what=='random':
   d,m=data;pub=point_mul(d)[0].to_bytes(32,'big');assert got!='REFUSED' and schnorr_verify(m,pub,bytes.fromhex(got))
  elif what=='refused':assert got=='REFUSED',got
  else:
   d,created,kind,tags,content=data;event=json.loads(bytes.fromhex(got));pub=point_mul(d)[0].to_bytes(32,'big').hex()
   assert list(event)==['id','pubkey','created_at','kind','tags','content','sig']
   assert (event['pubkey'],event['created_at'],event['kind'],event['tags'],event['content'])==(pub,created,kind,tags,content)
   eid=hashlib.sha256(dump([0,pub,created,kind,tags,content]).encode()).digest()
   assert event['id']==eid.hex() and schnorr_verify(eid,bytes.fromhex(pub),bytes.fromhex(event['sig']))
with tempfile.TemporaryDirectory(prefix='coax-sign-') as temp:
 d=Path(temp)
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/nostr/event_sign.coax'),'--library','--header',str(d/'es.h'),'--run-steps','250000000',*flags],capture_output=True,check=True).stdout)
  for build,extra in [('plain',['-O2']),('sanitized',['-O1','-fsanitize=address,undefined','-fno-sanitize-recover=all'])]:
   exe=d/(mode+'-'+build);subprocess.run(['cc','-std=c11',*extra,'-I'+str(d),str(c),str(R/'coax/tests/nostr/schnorr_sign_driver.c'),'-o',str(exe)],check=True)
   run=subprocess.run([str(exe)],input='\n'.join(requests)+'\n',capture_output=True,text=True,check=True)
   check(run.stdout.split());print(mode,build,len(requests),'requests: %d BIP-340 vectors, 24 random signatures, %d events, 8 refusals'%(len(rows),len(events)),'C',hashlib.sha256(c.read_bytes()).hexdigest()[:16],flush=True)
