#!/usr/bin/env python3
"""Explicit RFC/independent native oracle checks for Coax HMAC and HKDF."""
import ctypes as C
import hashlib
import hmac
import json
import os
from pathlib import Path
import random
import re
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[3]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax'))
SOURCE=ROOT/'coax/lib/crypto/hkdf-memory.coax'
def run(*args,**kw):
 r=subprocess.run([str(a) for a in args],capture_output=True,text=True,**kw)
 if r.returncode:raise RuntimeError(f'{args[0]} failed({r.returncode}): {r.stderr or r.stdout}')
 return r
class View(C.Structure):_fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class Info(C.Structure):_fields_=[('error',C.c_uint32),('phase',C.c_uint32),('total',C.c_uint64),('buffered',C.c_uint32)]
def array(data):return(C.c_uint8*len(data))(*data)
def view(data,at=0,count=None):return View(C.cast(C.byref(data,at),C.POINTER(C.c_uint8)),len(data)-at if count is None else count)

def hmac_vectors():
 text=(ROOT/'coax/tests/vectors/upstream/rfc/rfc4231.txt').read_text()
 offsets=list(re.finditer(r'\n4\.\d+\.\s+Test Case [1-7]\s*\n',text))
 keys=[b'\x0b'*20,b'Jefe',b'\xaa'*20,bytes(range(1,26)),b'\x0c'*20,b'\xaa'*131,b'\xaa'*131]
 messages=[b'Hi There',b'what do ya want for nothing?',b'\xdd'*50,b'\xcd'*50,b'Test With Truncation',b'Test Using Larger Than Block-Size Key - Hash Key First',b'This is a test using a larger than block-size key and a larger than block-size data. The key needs to be hashed before being used by the HMAC algorithm.']
 assert len(offsets)==7
 for i,start in enumerate(offsets):
  section=text[start.end():offsets[i+1].start() if i+1<len(offsets) else len(text)]
  line=re.search(r'HMAC-SHA-256\s*=\s*([0-9a-f]+)((?:\n[ \t]+[0-9a-f]{16,}[ \t]*)*)',section)
  digest=bytes.fromhex(''.join(re.findall('[0-9a-f]+',line[0].split('=',1)[1])))
  yield keys[i],messages[i],digest

def hkdf_vectors():
 text=(ROOT/'coax/tests/vectors/upstream/rfc/rfc5869.txt').read_text()
 offsets=list(re.finditer(r'\nA\.([1-4])\.\s+Test Case [1-4]\s*\n',text))
 assert len(offsets)==4
 for i,start in enumerate(offsets[:3]):
  section=text[start.end():offsets[i+1].start()]
  def field(name):
   m=re.search(r'^\s*'+name+r'\s*=\s*0x([0-9a-f]+)((?:\n[ \t]+[0-9a-f]+[ \t]*)*)',section,re.M)
   if not m:
    assert re.search(name+r'\s*=\s*\(0 octets\)',section);return b''
   return bytes.fromhex(m[1]+''.join(re.findall('[0-9a-f]+',m[2])))
  yield field('salt'),field('IKM'),field('info'),field('PRK'),field('OKM')

def expand_oracle(prk,info,length):
 result=b'';previous=b''
 for index in range(1,(length+31)//32+1):
  previous=hmac.digest(prk,previous+info+bytes([index]),'sha256');result+=previous
 return result[:length]

class HmacHkdf(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-hmac-hkdf-');cls.dir=Path(cls.temp.name);cls.libs=[]
  for label,mode in [('checked',[]),('release',['--release'])]:
   c=cls.dir/(label+'.c');c.write_text(run(COMPILER,'c',SOURCE,'--library','--header',cls.dir/'hkdf.h',*mode).stdout)
   run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',cls.dir/(label+'.dylib'))
   lib=C.CDLL(str(cls.dir/(label+'.dylib')))
   for name,signature in [('hmac_sha256',[View,View,View]),('hmac_sha256_init',[View,View]),('hmac_sha256_update',[View,View]),('hmac_sha256_final',[View,View]),('hkdf_extract',[View,View,View]),('hkdf_expand',[View,View,View,C.c_int64])]:
    fn=getattr(lib,name);fn.argtypes=signature;fn.restype=C.c_uint32
   lib.hmac_sha256_info.argtypes=[View];lib.hmac_sha256_info.restype=Info
   lib.sha256_init.argtypes=[View];lib.sha256_init.restype=C.c_uint32
   cls.libs.append(lib)
 @classmethod
 def tearDownClass(cls):cls.temp.cleanup()
 def test_fixture_pins_and_unchanged_shared_kernel(self):
  base=ROOT/'coax/tests/vectors'
  manifest=json.loads((base/'manifest.json').read_text())
  pins={row['path']:row for row in manifest['files']}
  for path in ['upstream/rfc/rfc4231.txt','upstream/rfc/rfc5869.txt','upstream/nip44/nip44.vectors.json']:
   data=(base/path).read_bytes();self.assertEqual(len(data),pins[path]['bytes'])
   self.assertEqual(hashlib.sha256(data).hexdigest(),pins[path]['sha256'])
  self.assertEqual(hashlib.sha256((ROOT/'coax/lib/proto/fingerprint.coax').read_bytes()).hexdigest(),'94142391e7e8122c4314443a878d65e61a1318cc5de2430f7d1ac826d620728a')
 def mac(self,lib,key,message):
  k=array(key);m=array(message);out=array([0xa5]*40)
  self.assertEqual(lib.hmac_sha256(view(k),view(m),view(out)),0)
  self.assertEqual(bytes(out)[32:],b'\xa5'*8);return bytes(out)[:32]
 def expand(self,lib,prk,info,length):
  key=array(prk);data=array(info);out=array([0xa5]*(length+8))
  self.assertEqual(lib.hkdf_expand(view(key),view(data),view(out),length),0)
  self.assertEqual(bytes(out)[length:],b'\xa5'*8);return bytes(out)[:length]
 def test_rfc4231_cases_one_to_seven(self):
  for key,message,published in hmac_vectors():
   expected=hmac.digest(key,message,'sha256');self.assertEqual(expected[:len(published)],published)
   for lib in self.libs:self.assertEqual(self.mac(lib,key,message),expected)
 def test_rfc5869_sha256_cases_one_to_three(self):
  for salt,ikm,info,prk,okm in hkdf_vectors():
   self.assertEqual(hmac.digest(salt,ikm,'sha256'),prk);self.assertEqual(expand_oracle(prk,info,len(okm)),okm)
   for lib in self.libs:
    s=array(salt);k=array(ikm);out=array([0xa5]*40)
    self.assertEqual(lib.hkdf_extract(view(s),view(k),view(out)),0)
    self.assertEqual(bytes(out)[:32],prk);self.assertEqual(bytes(out)[32:],b'\xa5'*8)
    self.assertEqual(self.expand(lib,prk,info,len(okm)),okm)
 def test_independent_random_and_boundary_lengths(self):
  rng=random.Random(5869)
  for key_length in [0,1,20,32,63,64,65,131,1025]:
   key=rng.randbytes(key_length)
   for size in [0,1,55,56,63,64,65,129,1000]:
    message=rng.randbytes(size)
    for lib in self.libs:self.assertEqual(self.mac(lib,key,message),hmac.digest(key,message,'sha256'))
  for length in [0,1,16,31,32,33,42,64,76,82,255,8160]:
   key=rng.randbytes(32);info=rng.randbytes(17)
   for lib in self.libs:self.assertEqual(self.expand(lib,key,info,length),expand_oracle(key,info,length))
 def test_hmac_streaming_lifecycle(self):
  key=array(bytes(range(80)));message=array(bytes(range(129)));expected=hmac.digest(bytes(key),bytes(message),'sha256')
  for lib in self.libs:
   for chunk in [1,7,64,129]:
    state=array([0]*224);out=array([0xa5]*32)
    self.assertEqual(lib.hmac_sha256_init(view(state),view(key)),0)
    for at in range(0,129,chunk):self.assertEqual(lib.hmac_sha256_update(view(state),view(message,at,min(chunk,129-at))),0)
    info=lib.hmac_sha256_info(view(state));self.assertEqual((info.error,info.phase,info.total,info.buffered),(0,1,129,1))
    self.assertEqual(lib.hmac_sha256_final(view(state),view(out)),0);self.assertEqual(bytes(out),expected)
    saved=bytes(state);self.assertEqual(lib.hmac_sha256_final(view(state),view(out)),0);self.assertEqual(bytes(state),saved)
    self.assertNotEqual(lib.hmac_sha256_update(view(state),view(message)),0);self.assertEqual(bytes(state),saved)
 def test_alias_capacity_and_error_preservation(self):
  for lib in self.libs:
   key=array(range(64));message=array(range(128));out=array([0xa5]*100);state=array([0xa5]*224)
   self.assertNotEqual(lib.hmac_sha256_init(view(state,count=223),view(key)),0);self.assertEqual(bytes(state),b'\xa5'*224)
   self.assertEqual(lib.hmac_sha256_init(view(state),view(key)),0)
   for operation in [lambda:lib.hmac_sha256_update(view(state),view(state,0,1)),lambda:lib.hmac_sha256_update(view(state),view(message,count=-1)),lambda:lib.hmac_sha256_final(view(state),view(state,0,32)),lambda:lib.hmac_sha256_final(view(state),view(out,count=31))]:
    before=bytes(state);self.assertNotEqual(operation(),0);self.assertEqual(bytes(state),before)
   for operation in [lambda:lib.hmac_sha256(view(key),view(message),view(message,0,32)),lambda:lib.hmac_sha256(view(key,count=-1),view(message),view(out)),lambda:lib.hkdf_extract(view(key),view(message),view(message,0,32)),lambda:lib.hkdf_expand(view(key,count=31),view(message),view(out),42),lambda:lib.hkdf_expand(view(key),view(message,count=1025),view(out),42),lambda:lib.hkdf_expand(view(key),view(message),view(out),-1),lambda:lib.hkdf_expand(view(key),view(message),view(out),8161),lambda:lib.hkdf_expand(view(key),view(message),view(key),42),lambda:lib.hkdf_expand(view(key),view(message),view(out,count=41),42)]:
    saved=(bytes(out),bytes(key),bytes(message));self.assertNotEqual(operation(),0);self.assertEqual((bytes(out),bytes(key),bytes(message)),saved)
   # Two valid bare SHA contexts do not establish a valid keyed HMAC context.
   self.assertEqual(lib.sha256_init(view(state,0,112)),0);self.assertEqual(lib.sha256_init(view(state,112,112)),0)
   self.assertNotEqual(lib.hmac_sha256_info(view(state)).error,0)

 def test_nip44_message_keys_are_exact_76_byte_expansions(self):
  record=json.loads((ROOT/'coax/tests/vectors/upstream/nip44/nip44.vectors.json').read_text())['v2']['valid']['get_message_keys']
  prk=bytes.fromhex(record['conversation_key'])
  for row in record['keys']:
   info=bytes.fromhex(row['nonce']);expected=bytes.fromhex(row['chacha_key']+row['chacha_nonce']+row['hmac_key'])
   self.assertEqual(len(expected),76);self.assertEqual(expand_oracle(prk,info,76),expected)
   for lib in self.libs:self.assertEqual(self.expand(lib,prk,info,76),expected)

 def test_native_sanitizers_and_context_aliases(self):
  host=self.dir/'sanitized.c';host.write_text('''#include "hkdf.h"
#include <assert.h>
int main(void){
 uint8_t state[224],key[80],message[129],out[8200];
 for(int i=0;i<80;i++)key[i]=(uint8_t)i;
 for(int i=0;i<129;i++)message[i]=(uint8_t)i;
 for(int i=0;i<8200;i++)out[i]=165;
 coax_v_u8 s={state,224},k={key,80},m={message,129},o={out,8200};
 assert(hmac_sha256_init(s,k)==0);
 assert(hmac_sha256_update(s,m)==0);
 assert(hmac_sha256_final(s,o)==0);
 assert(hmac_sha256_init((coax_v_u8){state,-1},k)!=0);
 assert(hmac_sha256_init(s,k)==0);
 assert(hmac_sha256_update(s,(coax_v_u8){state+112,1})!=0);
 assert(hmac_sha256_final(s,(coax_v_u8){state+32,32})!=0);
 assert(hkdf_extract(k,m,o)==0);
 assert(hkdf_expand((coax_v_u8){out,32},(coax_v_u8){message,32},(coax_v_u8){out+32,8160},8160)==0);
 for(int i=8192;i<8200;i++)assert(out[i]==165);
 assert(hkdf_expand(k,m,o,8161)!=0);
 assert(hkdf_expand((coax_v_u8){key,31},m,o,32)!=0);
 assert(hkdf_expand(k,(coax_v_u8){message,-1},o,32)!=0);
 assert(hkdf_expand(k,m,(coax_v_u8){key,32},32)!=0);
 return 0;
}''')
  run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',self.dir/'checked.c',host,'-lm','-o',self.dir/'sanitized')
  run(self.dir/'sanitized')

 def test_actual_wasm_rfc_and_nip44_vectors(self):
  clang=Path(os.environ.get('COAXIAL_CLANG','/opt/homebrew/opt/llvm/bin/clang'));sysroot=Path(os.environ.get('COAXIAL_SYSROOT','/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'));resources=Path(os.environ.get('COAXIAL_RESOURCES','/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'))
  self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(),'actual Wasm toolchain required; see coax/web/wasm.sh')
  fixture={'hmac':[{'key':key.hex(),'input':message.hex(),'digest':hmac.digest(key,message,'sha256').hex(),'published':digest.hex()} for key,message,digest in hmac_vectors()],'hkdf':[{'salt':salt.hex(),'ikm':ikm.hex(),'info':info.hex(),'prk':prk.hex(),'okm':okm.hex()} for salt,ikm,info,prk,okm in hkdf_vectors()]}
  message_keys=json.loads((ROOT/'coax/tests/vectors/upstream/nip44/nip44.vectors.json').read_text())['v2']['valid']['get_message_keys'];prk=message_keys['conversation_key']
  fixture['expand']=[{'prk':prk,'info':v['nonce'],'okm':v['chacha_key']+v['chacha_nonce']+v['hmac_key']} for v in message_keys['keys']]
  for length in [0,1,31,32,33,8160]:fixture['expand'].append({'prk':prk,'info':'','okm':expand_oracle(bytes.fromhex(prk),b'',length).hex()})
  path=self.dir/'wasm-rfc.json';path.write_text(json.dumps(fixture));env=os.environ.copy();env['PATH']='/opt/homebrew/opt/lld/bin:'+env.get('PATH','')
  for label in ['checked','release']:
   module=self.dir/(label+'.wasm')
   run(clang,'--target=wasm32-wasip1','--sysroot='+str(sysroot),'-resource-dir='+str(resources),'-std=c11','-O2','-ffp-contract=off','-fno-fast-math','-I'+str(self.dir),'-mexec-model=reactor',self.dir/(label+'.c'),ROOT/'coax/tests/nostr/hmac_hkdf_wasm.c','-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections','-o',module,env=env)
   report=json.loads(run('node',ROOT/'coax/tests/nostr/hmac_hkdf_wasm.cjs',module,path).stdout)
   self.assertEqual(report['hmac'],7);self.assertEqual(report['hkdf'],3);self.assertEqual(report['expand'],len(fixture['expand']))

if __name__=='__main__':unittest.main()
