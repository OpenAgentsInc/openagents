#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: libsecp256k1 checks what the Coax signer made)
"""Coax-signed BIP-340 signatures and Nostr events through libsecp256k1.

schnorr_sign.coax and event_sign.coax (through event_sign.coax's generated C
and schnorr_sign_driver.c) sign seeded random test keys, messages and aux;
the pinned libsecp256k1 v0.8.0 oracle (tools/coax-nostr/fuzz/build_oracle.py)
then runs secp256k1_schnorrsig_verify on every signature, and also signs the
same key, message and aux with secp256k1_schnorrsig_sign_custom: BIP-340
signing is deterministic given aux, so the two signatures must be equal byte
for byte. Event ids are recomputed from an independent NIP-01 serializer.
Test keys only.
  --signatures N   random BIP-340 signatures (default 10000)
  --events N       random signed Nostr events (default 300)
  --seed S         RNG seed (default 13)
  --shard I/N      sign only part I of N (1-based) of each stream
  --mode M         checked or release (default release)
  --sanitize       build the driver with ASan/UBSan"""
if not __debug__: raise SystemExit('tests require Python assertions')
import argparse,hashlib,json,random,subprocess,sys,tempfile,time
from pathlib import Path
R=Path(__file__).resolve().parents[3];sys.path.insert(0,str(R/'tools/coax-nostr/fuzz'))
from oracle import N,NativeOracle,DEFAULT_LIBRARY
p=argparse.ArgumentParser();p.add_argument('--signatures',type=int,default=10000);p.add_argument('--events',type=int,default=300)
p.add_argument('--seed',type=int,default=13);p.add_argument('--shard',default='1/1');p.add_argument('--mode',choices=['checked','release'],default='release')
p.add_argument('--sanitize',action='store_true');a=p.parse_args()
part,parts=map(int,a.shard.split('/'));assert 1<=part<=parts
H=lambda b:b.hex() or '-'
dump=lambda v:json.dumps(v,separators=(',',':'),ensure_ascii=False)
rng=random.Random(a.seed)
# Signatures: message lengths cover empty, one byte, the 32-byte Nostr id
# size, odd lengths and a few kilobytes; keys and aux are uniform.
sigs=[]
for i in range(a.signatures):
 d=rng.randrange(1,N).to_bytes(32,'big');aux=rng.randbytes(32);m=rng.randbytes(rng.choice([0,1,31,32,33,64,100,255,1024,4096]))
 if i%parts==part-1:sigs.append((d,aux,m))
alphabet=['a','Z','0',' ','"','\\','\n','\t','\r','\x00','\x01','\x1f','\x7f','é','€','😀','𝄞','\u2028','/','<']
text=lambda k:''.join(rng.choice(alphabet) for _ in range(rng.randrange(k+1)))
events=[]
for i in range(a.events):
 created=rng.choice([0,1,9007199254740991,rng.randrange(9007199254740992)]);kind=rng.choice([0,1,3,10002,20000,30617,65535,rng.randrange(65536)])
 tags=[[text(8) for _ in range(rng.randrange(4))] for _ in range(rng.randrange(4))];content=text(rng.choice([0,1,40,300]))
 d=rng.randrange(1,N).to_bytes(32,'big');aux=rng.randbytes(32)
 if i%parts==part-1:events.append((d,aux,created,kind,tags,content))
requests=['S %s %s %s'%(d.hex(),aux.hex(),H(m)) for d,aux,m in sigs]
requests+=['E %s %s %d %d %s %s'%(d.hex(),aux.hex(),created,kind,H(dump(tags).encode()),H(content.encode())) for d,aux,created,kind,tags,content in events]
oracle=NativeOracle(DEFAULT_LIBRARY);start=time.monotonic()
# The oracle must reject what it should: a planted bad signature.
d0,aux0,m0=(1).to_bytes(32,'big'),bytes(32),b'control';good=oracle.sign(m0,d0,aux0);pub0=oracle.point(d0)[1:]
assert oracle.verify(m0,pub0,good) and not oracle.verify(m0,pub0,bytes([good[0]^1])+good[1:]) and not oracle.verify(m0+b'!',pub0,good)
with tempfile.TemporaryDirectory(prefix='coax-sign-libsecp-') as temp:
 t=Path(temp);c=t/'es.c'
 c.write_bytes(subprocess.run([str(R/'coax/bin/coax'),'c',str(R/'coax/lib/nostr/event_sign.coax'),'--library','--header',str(t/'es.h'),'--run-steps','250000000',*(['--release'] if a.mode=='release' else [])],capture_output=True,check=True).stdout)
 flags=['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all'] if a.sanitize else ['-O2']
 exe=t/'driver';subprocess.run(['cc','-std=c11',*flags,'-I'+str(t),str(c),str(R/'coax/tests/nostr/schnorr_sign_driver.c'),'-o',str(exe)],check=True)
 out=subprocess.run([str(exe)],input='\n'.join(requests)+'\n',capture_output=True,text=True,check=True).stdout.split()
 c_sha=hashlib.sha256(c.read_bytes()).hexdigest()
assert len(out)==len(requests),(len(out),len(requests))
for (d,aux,m),got in zip(sigs,out):
 pub=oracle.point(d)[1:];ref=oracle.sign(m,d,aux)
 if got=='REFUSED' or not oracle.verify(m,pub,bytes.fromhex(got)) or bytes.fromhex(got)!=ref:
  raise SystemExit('MISMATCH signature key=%s aux=%s msg=%s coax=%s libsecp=%s'%(d.hex(),aux.hex(),m.hex(),got,ref.hex()))
for (d,aux,created,kind,tags,content),got in zip(events,out[len(sigs):]):
 assert got!='REFUSED',('event refused',d.hex())
 event=json.loads(bytes.fromhex(got));pub=oracle.point(d)[1:]
 assert list(event)==['id','pubkey','created_at','kind','tags','content','sig']
 assert (event['pubkey'],event['created_at'],event['kind'],event['tags'],event['content'])==(pub.hex(),created,kind,tags,content)
 eid=hashlib.sha256(dump([0,pub.hex(),created,kind,tags,content]).encode()).digest();sig=bytes.fromhex(event['sig'])
 if event['id']!=eid.hex() or not oracle.verify(eid,pub,sig) or sig!=oracle.sign(eid,d,aux):
  raise SystemExit('MISMATCH event key=%s aux=%s event=%s'%(d.hex(),aux.hex(),bytes.fromhex(got).decode()))
oracle.close()
print(json.dumps({'mode':a.mode,'sanitize':a.sanitize,'seed':a.seed,'shard':a.shard,'signatures':len(sigs),'events':len(events),
 'libsecp_verified':len(sigs)+len(events),'libsecp_equal_signatures':len(sigs)+len(events),'failures':0,
 'generated_c_sha256':c_sha,'oracle_sha256':hashlib.sha256(DEFAULT_LIBRARY.read_bytes()).hexdigest(),'seconds':round(time.monotonic()-start,1)}))
