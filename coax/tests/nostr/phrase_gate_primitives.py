#!/usr/bin/env python3
"""Public draft fixture versus existing Coax primitives; no gate implementation."""
import ctypes as C
import hashlib
import hmac
import json
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[3];DIRECTORY=ROOT/'coax/bin/phrase-gate-format'
class View(C.Structure):_fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class Progress(C.Structure):_fields_=[('error',C.c_uint32),('phase',C.c_uint32),('completed',C.c_int64),('total',C.c_int64)]
class Cipher(C.Structure):_fields_=[('written',C.c_int64),('error',C.c_uint32)]
def run(*args):
 r=subprocess.run([str(x)for x in args],capture_output=True,text=True,timeout=90)
 if r.returncode:raise RuntimeError(r.stderr or r.stdout)
 return r.stdout
def array(b):return(C.c_uint8*len(b))(*b)
def view(b):return View(C.cast(b,C.POINTER(C.c_uint8)),len(b))
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 DIRECTORY.mkdir(parents=True,exist_ok=True);fixture=ROOT/'coax/tests/nostr/data/phrase-gate-draft-2026-10-28.json';f=json.loads(fixture.read_text())['v2'];header=bytes.fromhex(f['headerHex']);salt=header[46:62];nonce=header[62:74];context=header[14:46]
 labels={name:('openagents.phrase-gate.v2/'+name+'\0').encode('ascii')for name in ['kdf','keys','mac','context']};assert {k:v.hex()for k,v in labels.items()}==f['labels']
 canonical_record=json.dumps(f['contextRecord'],sort_keys=True,separators=(',',':')).encode();assert hashlib.sha256(labels['context']+canonical_record).digest()==context
 assert len(header)==74 and header[:6]==b'PG\x02\x00\x01\x01' and int.from_bytes(header[6:10],'big')==210000 and int.from_bytes(header[10:14],'big')==len(bytes.fromhex(f['plaintextHex']))
 assert salt==bytes(range(16))and nonce==bytes(range(16,28))
 master=hashlib.pbkdf2_hmac('sha256',f['phrase'].encode(),labels['kdf']+salt,210000,32);assert master.hex()==f['masterHex']
 prk=hmac.digest(salt,master,'sha256');assert prk.hex()==f['prkHex'];t1=hmac.digest(prk,labels['keys']+context+b'\x01','sha256');t2=hmac.digest(prk,t1+labels['keys']+context+b'\x02','sha256');assert(t1+t2).hex()==f['keysHex']
 report={'schema':'coax.phrase-gate-draft-primitive-evidence.v1','scope':'Public reference fixture only, not a product gate codec, secure erasure or production approval','fixture_sha256':sha(fixture),'compiler':run(ROOT/'coax/bin/coax','version').strip(),'compiler_sha256':sha(ROOT/'coax/bin/coax'),'modes':[]}
 source=DIRECTORY/'primitives.coax';source.write_text(''.join(f'#load "{ROOT}/coax/lib/crypto/{name}";\n'for name in ['pbkdf2-memory.coax','hkdf-memory.coax','chacha20-memory.coax']))
 for name,flags in [('checked',[]),('release',['--release'])]:
  generated=DIRECTORY/(name+'.c');generated.write_text(run(ROOT/'coax/bin/coax','c',source,'--library',*flags));binary=DIRECTORY/(name+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',generated,'-lm','-o',binary);lib=C.CDLL(str(binary))
  lib.pbkdf2_init.argtypes=[View,View,View,C.c_int64,C.c_int64];lib.pbkdf2_init.restype=C.c_uint32;lib.pbkdf2_step.argtypes=[View,C.c_int64];lib.pbkdf2_step.restype=Progress;lib.pbkdf2_result.argtypes=[View,View];lib.pbkdf2_result.restype=C.c_uint32
  lib.hkdf_extract.argtypes=[View,View,View];lib.hkdf_extract.restype=C.c_uint32;lib.hkdf_expand.argtypes=[View,View,View,C.c_int64];lib.hkdf_expand.restype=C.c_uint32;lib.hmac_sha256.argtypes=[View,View,View];lib.hmac_sha256.restype=C.c_uint32;lib.chacha20_xor.argtypes=[View,View,C.c_uint32,View,View];lib.chacha20_xor.restype=Cipher
  state=array(bytes(832));password=array(f['phrase'].encode());pb_salt=array(labels['kdf']+salt);out=array(bytes(32));assert lib.pbkdf2_init(view(state),view(password),view(pb_salt),210000,32)==0
  steps=0
  while True:
   p=lib.pbkdf2_step(view(state),1024);steps+=1;assert p.error==0
   if p.phase==2:break
   assert steps<207
  assert p.completed==p.total==210000;assert lib.pbkdf2_result(view(state),view(out))==0 and bytes(out)==master
  k_prk=array(bytes(32));assert lib.hkdf_extract(view(array(salt)),view(out),view(k_prk))==0 and bytes(k_prk)==prk
  material=array(bytes(64));assert lib.hkdf_expand(view(k_prk),view(array(labels['keys']+context)),view(material),64)==0 and bytes(material)==t1+t2
  plaintext=array(bytes.fromhex(f['plaintextHex']));encrypted=array(bytes(len(plaintext)));r=lib.chacha20_xor(view(array(bytes(material[:32]))),view(array(nonce)),1,view(plaintext),view(encrypted));assert r.error==0 and r.written==len(plaintext)and bytes(encrypted).hex()==f['ciphertextHex']
  mac_input=labels['mac']+header+bytes(encrypted);tag=array(bytes(32));assert lib.hmac_sha256(view(array(bytes(material[32:]))),view(array(mac_input)),view(tag))==0 and bytes(tag).hex()==f['tagHex']
  report['modes'].append({'mode':name,'generated_c_sha256':sha(generated),'binary_sha256':sha(binary),'pbkdf2_rounds':p.completed,'bounded_steps':steps,'master_prk_keys_ciphertext_tag_match':True})
 (DIRECTORY/'evidence.json').write_text(json.dumps(report,indent=2,sort_keys=True)+'\n');print('Python/Node/OpenSSL fixture agrees with actual checked/release Coax PBKDF2, HKDF, ChaCha20 and HMAC')
if __name__=='__main__':main()
