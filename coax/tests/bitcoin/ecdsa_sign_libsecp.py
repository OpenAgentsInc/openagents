#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: libsecp256k1 checks what the Coax signer made)
"""Coax ECDSA signatures through libsecp256k1.

coax/lib/crypto/ecdsa_sign.coax (its generated C, driven by
ecdsa_sign_driver.c) signs seeded random test keys and 32-byte hashes; the
pinned libsecp256k1 v0.8.0 oracle (tools/coax-nostr/fuzz/build_oracle.py)
then, for every signature: parses the DER with
secp256k1_ecdsa_signature_parse_der, checks it is low-S
(secp256k1_ecdsa_signature_normalize leaves it unchanged), verifies it with
secp256k1_ecdsa_verify, and signs the same key and hash with
secp256k1_ecdsa_sign (RFC 6979, low-S): the compact and DER bytes must be
equal. Keys include 1, n-1 and keys with leading zero bytes; hashes include
0, n, and 2^256-1 (reduced mod n). Test keys only.
  --signatures N   (default 1200)   --seed S (default 104)
  --mode M         checked or release (default release)
  --sanitize       build the driver with ASan/UBSan"""
if not __debug__: raise SystemExit('tests require Python assertions')
import argparse,ctypes as C,hashlib,json,random,subprocess,sys,tempfile,time
from pathlib import Path
R=Path(__file__).resolve().parents[3];sys.path.insert(0,str(R/'tools/coax-nostr/fuzz'))
from oracle import N,NativeOracle,DEFAULT_LIBRARY
p=argparse.ArgumentParser();p.add_argument('--signatures',type=int,default=1200);p.add_argument('--seed',type=int,default=104)
p.add_argument('--mode',choices=['checked','release'],default='release');p.add_argument('--sanitize',action='store_true');a=p.parse_args()
rng=random.Random(a.seed)
fixed_keys=[1,N-1,2,N-2,0xff,1<<200]
fixed_hashes=[bytes(32),N.to_bytes(32,'big'),b'\xff'*32,(N-1).to_bytes(32,'big'),b'\0'*31+b'\1']
reqs=[]
for i in range(a.signatures):
 d=fixed_keys[i] if i<len(fixed_keys) else rng.randrange(1,N)>>(8*rng.choice([0,0,0,1,2]))
 d=d or 1
 h=fixed_hashes[i%len(fixed_hashes)] if i<3*len(fixed_hashes) else rng.randbytes(32)
 reqs.append((d.to_bytes(32,'big'),h))
o=NativeOracle(DEFAULT_LIBRARY);L=o.lib;ctx=o.context
for f,args in [('secp256k1_ecdsa_sign',[C.c_void_p,C.c_void_p,C.c_char_p,C.c_char_p,C.c_void_p,C.c_void_p]),
               ('secp256k1_ecdsa_verify',[C.c_void_p,C.c_char_p,C.c_char_p,C.c_void_p]),
               ('secp256k1_ecdsa_signature_parse_der',[C.c_void_p,C.c_void_p,C.c_char_p,C.c_size_t]),
               ('secp256k1_ecdsa_signature_serialize_der',[C.c_void_p,C.c_void_p,C.c_void_p,C.c_void_p]),
               ('secp256k1_ecdsa_signature_serialize_compact',[C.c_void_p,C.c_void_p,C.c_void_p]),
               ('secp256k1_ecdsa_signature_normalize',[C.c_void_p,C.c_void_p,C.c_void_p]),
               ('secp256k1_ec_pubkey_create',[C.c_void_p,C.c_void_p,C.c_char_p])]:
 getattr(L,f).argtypes=args
def ref(d,h):
 sig=C.create_string_buffer(64);assert L.secp256k1_ecdsa_sign(ctx,sig,h,d,None,None)
 der=C.create_string_buffer(72);n=C.c_size_t(72);assert L.secp256k1_ecdsa_signature_serialize_der(ctx,der,C.byref(n),sig)
 c=C.create_string_buffer(64);assert L.secp256k1_ecdsa_signature_serialize_compact(ctx,c,sig);return c.raw,der.raw[:n.value]
def verify(d,h,der):
 sig=C.create_string_buffer(64);pub=C.create_string_buffer(64)
 if not L.secp256k1_ecdsa_signature_parse_der(ctx,sig,der,len(der)):return False
 if L.secp256k1_ecdsa_signature_normalize(ctx,None,sig):return False     # 1 means it was high-S
 assert L.secp256k1_ec_pubkey_create(ctx,pub,d)
 return bool(L.secp256k1_ecdsa_verify(ctx,sig,h,pub))
# The oracle must reject what it should: a planted bad signature.
c0,der0=ref((1).to_bytes(32,'big'),bytes(32))
assert verify((1).to_bytes(32,'big'),bytes(32),der0) and not verify((1).to_bytes(32,'big'),b'\1'+bytes(31),der0) and not verify((2).to_bytes(32,'big'),bytes(32),der0)
start=time.monotonic()
with tempfile.TemporaryDirectory(prefix='coax-ecdsa-libsecp-') as temp:
 t=Path(temp);c=t/'es.c'
 c.write_bytes(subprocess.run([str(R/'coax/bin/coax'),'c',str(R/'coax/lib/crypto/ecdsa_sign.coax'),'--library','--header',str(t/'es.h'),'--run-steps','250000000',*(['--release'] if a.mode=='release' else [])],capture_output=True,check=True).stdout)
 flags=['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all'] if a.sanitize else ['-O2']
 exe=t/'driver';subprocess.run(['cc','-std=c11',*flags,'-I'+str(t),str(c),str(R/'coax/tests/bitcoin/ecdsa_sign_driver.c'),'-o',str(exe)],check=True)
 out=subprocess.run([str(exe)],input=''.join('%s %s\n'%(d.hex(),h.hex()) for d,h in reqs),capture_output=True,text=True,check=True).stdout.splitlines()
 c_sha=hashlib.sha256(c.read_bytes()).hexdigest()
assert len(out)==len(reqs),(len(out),len(reqs))
for (d,h),line in zip(reqs,out):
 if line=='REFUSED':raise SystemExit('REFUSED key=%s hash=%s'%(d.hex(),h.hex()))
 compact,der=(bytes.fromhex(x) for x in line.split())
 rc,rd=ref(d,h)
 if not verify(d,h,der) or compact!=rc or der!=rd:
  raise SystemExit('MISMATCH key=%s hash=%s coax=%s libsecp=%s'%(d.hex(),h.hex(),line,rd.hex()))
o.close()
print(json.dumps({'mode':a.mode,'sanitize':a.sanitize,'seed':a.seed,'signatures':len(reqs),'libsecp_verified':len(reqs),'libsecp_equal_signatures':len(reqs),
 'failures':0,'generated_c_sha256':c_sha,'oracle_sha256':hashlib.sha256(DEFAULT_LIBRARY.read_bytes()).hexdigest(),'seconds':round(time.monotonic()-start,1)}))
