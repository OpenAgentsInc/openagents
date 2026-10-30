#!/usr/bin/env python3
"""End-to-end signing: schnorr_sign.coax and event_sign.coax looped to
completion through the generated C, checked and release, plain and
ASan/UBSan, and the release build as Wasm (wasm32-wasip1 under Node's WASI),
against the pinned BIP-340 vectors and an independent pure-Python BIP-340
verifier and NIP-01 serializer.
  --round-trips N  also signs N random events (Unicode, control characters,
                   empty fields, edge kinds and times) in the release build
  --no-wasm        skips the Wasm build
COAX_IOS_SIMULATOR=<device> (on a Mac) also runs the checked and release
builds in that iOS Simulator (arm64), as coax/tests/nostr/aead_host.py does."""
if not __debug__: raise SystemExit('tests require Python assertions')
import csv,hashlib,json,os,random,subprocess,sys,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];sys.path.insert(0,str(R/'tools/coax-nostr/fuzz'))
from oracle import N,point_mul,schnorr_verify
compiler=R/'coax/bin/coax';rng=random.Random(340)
args=sys.argv[1:];trips=int(args[args.index('--round-trips')+1]) if '--round-trips' in args else 0;wasm='--no-wasm' not in args
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
def check(lines,expect=expect):
 assert len(lines)==len(expect),(len(lines),len(expect))
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
# Random events: Unicode, escapes, control characters, empty fields and edge
# kinds and times, each signed under a fresh random key.
def round_trips(n):
 pick=random.Random(10000);alphabet=['a','Z','0',' ','"','\\','\n','\t','\r','\x00','\x01','\x1f','\x7f','é','€','😀','𝄞','\u2028','/','<']
 text=lambda k:''.join(pick.choice(alphabet) for _ in range(pick.randrange(k+1)))
 req=[];exp=[]
 for i in range(n):
  created=pick.choice([0,1,9007199254740991,pick.randrange(9007199254740992)]);kind=pick.choice([0,1,3,10002,20000,30617,65535,pick.randrange(65536)])
  tags=[[text(8) for _ in range(pick.randrange(4))] for _ in range(pick.randrange(4))];content=text(pick.choice([0,1,40,300]))
  d=pick.randrange(1,N);req.append('E %064x %s %d %d %s %s'%(d,pick.randbytes(32).hex(),created,kind,H(dump(tags).encode()),H(content.encode())));exp.append(('event',(d,created,kind,tags,content)))
 return req,exp
with tempfile.TemporaryDirectory(prefix='coax-sign-') as temp:
 d=Path(temp)
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/nostr/event_sign.coax'),'--library','--header',str(d/'es.h'),'--run-steps','250000000',*flags],capture_output=True,check=True).stdout)
  for build,extra in [('plain',['-O2']),('sanitized',['-O1','-fsanitize=address,undefined','-fno-sanitize-recover=all'])]:
   exe=d/(mode+'-'+build);subprocess.run(['cc','-std=c11',*extra,'-I'+str(d),str(c),str(R/'coax/tests/nostr/schnorr_sign_driver.c'),'-o',str(exe)],check=True)
   run=subprocess.run([str(exe)],input='\n'.join(requests)+'\n',capture_output=True,text=True,check=True)
   check(run.stdout.split());print(mode,build,len(requests),'requests: %d BIP-340 vectors, 24 random signatures, %d events, 8 refusals'%(len(rows),len(events)),'C',hashlib.sha256(c.read_bytes()).hexdigest()[:16],flush=True)
   if mode=='release' and build=='plain' and trips:
    trip_requests,trip_expect=round_trips(trips)
    run=subprocess.run([str(exe)],input='\n'.join(trip_requests)+'\n',capture_output=True,text=True,check=True)
    check(run.stdout.split(),trip_expect);print(mode,build,trips,'random signed-event round trips agree with the independent serializer and verifier',flush=True)
  if mode=='release' and wasm:
   cache=Path.home()/'.cache/coax-wasm-toolchain';w=d/'release.wasm'
   subprocess.run([os.environ.get('COAXIAL_CLANG',str(cache/'bin/clang')),'--sysroot='+os.environ.get('COAXIAL_SYSROOT',str(cache/'sysroot')),'-resource-dir='+os.environ.get('COAXIAL_RESOURCES',str(cache/'resource')),
    '--target=wasm32-wasip1','-std=c11','-O2','-I'+str(d),str(c),str(R/'coax/tests/nostr/schnorr_sign_driver.c'),'-Wl,-z,stack-size=8388608,--max-memory=134217728','-lm','-o',str(w)],check=True,env={**os.environ,'PATH':str(cache/'bin')+':'+os.environ['PATH']})
   run=subprocess.run(['node',str(R/'coax/tests/nostr/schnorr_sign_wasm.cjs'),str(w)],input='\n'.join(requests)+'\n',capture_output=True,text=True,check=True)
   check(run.stdout.split());print(mode,'wasm',len(requests),'requests agree with native','wasm',hashlib.sha256(w.read_bytes()).hexdigest()[:16],flush=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR')
  if sim:
   sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip();ios=d/(mode+'-ios')
   subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-I'+str(d),str(c),str(R/'coax/tests/nostr/schnorr_sign_driver.c'),'-o',str(ios)],check=True)
   run=subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],input='\n'.join(requests)+'\n',capture_output=True,text=True,check=True)
   check(run.stdout.split());print(mode,'iOS Simulator',len(requests),'requests agree with native',hashlib.sha256(ios.read_bytes()).hexdigest()[:16],flush=True)
