#!/usr/bin/env python3
"""Independent safe-integer JSON subset; native/Wasm buffers and legacy digest."""
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

ROOT=Path(__file__).resolve().parents[3];COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax'))
PURE=ROOT/'coax/lib/nostr/jcs.coax';BRIDGE=ROOT/'coax/lib/nostr/jcs-memory.coax';LIMIT=524288
def run(*args,**kw):
 r=subprocess.run([str(a) for a in args],capture_output=True,text=True,**kw)
 if r.returncode:raise RuntimeError(f'{args[0]} failed({r.returncode}): {r.stderr or r.stdout}')
 return r
def canonical(value):
 if isinstance(value,dict):return '{'+','.join(json.dumps(k,ensure_ascii=False)+':'+canonical(value[k]) for k in sorted(value,key=lambda k:k.encode('utf-16-be')))+'}'
 if isinstance(value,list):return '['+','.join(canonical(v) for v in value)+']'
 if isinstance(value,int) and not isinstance(value,bool):assert abs(value)<=9007199254740991
 return json.dumps(value,ensure_ascii=False,separators=(',',':'))
def cases():
 result=[]
 def good(text,label):
  value=json.loads(text);expected=canonical(value).encode();result.append(dict(id=label,input=text.encode().hex(),output=expected.hex(),error=0))
 def bad(data,error,label):result.append(dict(id=label,input=(data.encode() if isinstance(data,str) else data).hex(),output='',error=error))
 seeds=['null','true','false','0','-0','9007199254740991','-9007199254740991','""','{}','[]','{"kind":"a","z":0,"a":1}','{"\\ue000":1,"\\ud800\\udc00":2,"":3}','{"é":1,"e\\u0301":2}','{"\\u20ac":1,"\\r":2,"\\ufb33":3,"\\u0001":4,"\\ud83d\\ude00":5,"\\u0080":6,"ö":7}','"\\u0000\\b\\f\\n\\r\\t\\/\\\"\\\\"','[9007199254740991,-9007199254740991,10,-10,0,-0]']
 for i,text in enumerate(seeds):good(text,'fixture-'+str(i))
 good('{"2":"b","10":"a","1":"c"}','numeric-property-names')
 good(json.dumps({'\u20ac':'Euro Sign','\r':'Carriage Return','\ufb33':'Hebrew Letter Dalet With Dagesh','1':'One','😀':'Emoji: Grinning Face','\u0080':'Control','ö':'Latin Small Letter O With Diaeresis'}),'rfc8785-property-order')
 rng=random.Random(8785);alphabet=['a','b','"','\\','\0','\n','é','e\u0301','ࠀ','�','𐀀','😀','\U0010fffd','\u2028','\u2029']
 def value(depth):
  choice=rng.randrange(6 if depth<4 else 4)
  if choice==0:return None
  if choice==1:return bool(rng.randrange(2))
  if choice==2:return rng.choice([0,-1,1,10,-10,9007199254740991,-9007199254740991,rng.randrange(-100000,100000)])
  if choice==3:return ''.join(rng.choices(alphabet,k=rng.randrange(6)))
  if choice==4:return[value(depth+1) for _ in range(rng.randrange(5))]
  return{''.join(rng.choices(alphabet,k=rng.randrange(5))):value(depth+1) for _ in range(rng.randrange(5))}
 for i in range(300):good(json.dumps(value(0),ensure_ascii=i%2==0,indent=1 if i%3==0 else None),'seeded-'+str(i))
 for n in [2047,2048]:
  text='['+','.join('0' for _ in range(n))+']'
  if n==2047:good(text,'node-limit')
  else:bad(text,1,'node-over')
 for n in [1024,1025]:
  text='{'+','.join(f'"{i:05}":0' for i in range(n-1,-1,-1))+'}'
  if n==1024:good(text,'key-limit')
  else:bad(text,1,'key-over')
 good('{"'+'a'*65536+'":0}','key-arena-limit');bad('{"'+'a'*65536+'":0,"b":1}',1,'key-arena-over')
 good('"'+'a'*65536+'"','string-limit');bad('"'+'a'*65537+'"',2,'string-over')
 good(' '*(LIMIT-2)+'{}','input-limit');bad(' '*(LIMIT-1)+'{}',1,'input-over')
 good('['*16+'0'+']'*16,'depth-limit');bad('['*17+'0'+']'*17,2,'depth-over')
 bad('{'+','.join(json.dumps('a'*255+f'{i:04}')+':0' for i in range(100))+'}',7,'comparison-work-over')
 for i,text in enumerate(['{"a":1,"\\u0061":2}','{"😀":1,"\\ud83d\\ude00":2}','{"":0,"":1}','{"x":{"a":1,"a":2}}']):bad(text,4,'duplicate-'+str(i))
 for i,text in enumerate(['1.0','1e0','1E+0','9007199254740992','-9007199254740992','{"unknown":1.5}','{"unknown":1e999}']):bad(text,8,'number-'+str(i))
 for i,text in enumerate(['"\\ud800"','"\\udc00"','{"\\ud800":1}']):bad(text,3,'surrogate-'+str(i))
 for cp in [0xfdd0,0xfdef,0xfffe,0xffff,0x1fffe,0x1ffff,0x10fffe,0x10ffff]:
  char=chr(cp)
  for escaped in [False,True]:
   token=json.dumps(char,ensure_ascii=escaped);bad(token,3,f'noncharacter-value-{cp}-{escaped}');bad('{'+token+':1}',3,f'noncharacter-key-{cp}-{escaped}')
 for i,data in enumerate([b'',b'[1,]',b'{"a":}',b'true false',b'NaN',b'"\xc0\x80"',b'"\xed\xa0\x80"',b'"\xf4\x90\x80\x80"',b'"\x00"']):bad(data,2,'json-'+str(i))
 deck=json.loads((ROOT/'coax/tests/nostr/data/jcs-test-deck-v1.json').read_text())
 assert canonical(deck['deck'])==deck['canonical'] and canonical(deck['envelope'])==deck['envelope_canonical']
 assert 'sha256:'+hashlib.sha256(deck['canonical'].encode()).hexdigest()==deck['digest']
 good(json.dumps(deck['deck'],ensure_ascii=False),'legacy-test-deck');good(json.dumps(deck['envelope'],ensure_ascii=False),'legacy-envelope')
 return result,deck
class View(C.Structure):_fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class String(C.Structure):_fields_=[('data',C.c_char_p),('count',C.c_int64)]
class Result(C.Structure):_fields_=[('written',C.c_int64),('error',C.c_uint32)]
def view(a,n=None):return View(C.cast(a,C.POINTER(C.c_uint8)),len(a) if n is None else n)
class Jcs(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-jcs-');cls.dir=Path(cls.temp.name);cls.cases,cls.deck=cases();cls.fixture=cls.dir/'vectors.json';cls.fixture.write_text(json.dumps(cls.cases));cls.libs=[]
  cls.oracle=json.loads(run('node',ROOT/'coax/tests/nostr/jcs_wasm.cjs','oracle',cls.fixture).stdout)
  binary=bytearray()
  for c in cls.cases:
   data=bytes.fromhex(c['input']);expected=bytes.fromhex(c['output']);binary.extend(struct.pack('<III',len(data),len(expected),c['error'])+data+expected)
  cls.binary=bytes(binary)
  cls.evidence={'compiler':run(COMPILER,'version').stdout.strip(),'compiler_sha256':hashlib.sha256(COMPILER.read_bytes()).hexdigest(),'seed':8785,'records':len(cls.cases),'oracle':cls.oracle,'test_deck_digest':cls.deck['digest'],'test_deck_bytes':len(cls.deck['canonical'].encode()),'sources':{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [PURE,BRIDGE,ROOT/'coax/lib/nostr/json.coax',ROOT/'coax/lib/nostr/adapters/json-policy.coax',Path(__file__),ROOT/'coax/tests/nostr/jcs_wasm.c',ROOT/'coax/tests/nostr/jcs_wasm.cjs',ROOT/'coax/tests/nostr/jcs_native.c',ROOT/'coax/tests/nostr/data/jcs-test-deck-v1.json']},'results':[]}
  cls.evidence.update(native_compiler=run('cc','--version').stdout.splitlines()[0],fixture_sha256=hashlib.sha256(cls.fixture.read_bytes()).hexdigest())
  for label,flags in [('checked',[]),('release',['--release'])]:
   c=cls.dir/(label+'.c');c.write_text(run(COMPILER,'c',BRIDGE,'--library','--header',cls.dir/'jcs.h',*flags).stdout)
   libfile=cls.dir/(label+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',libfile);lib=C.CDLL(str(libfile));lib.jcs_canonicalize.argtypes=[String,View];lib.jcs_canonicalize.restype=Result;cls.libs.append(lib)
 @classmethod
 def tearDownClass(cls):cls.temp.cleanup()
 def test_native_independent_and_legacy_digest(self):
  for lib in self.libs:
   for c in self.cases:
    data=bytes.fromhex(c['input']);expected=bytes.fromhex(c['output']);capacity=LIMIT if c['error'] else len(expected);out=(C.c_uint8*(capacity+8))(*([165]*(capacity+8)));r=lib.jcs_canonicalize(String(data,len(data)),view(out,capacity))
    self.assertEqual((r.error,r.written),(c['error'],0 if c['error'] else len(expected)),c['id']);self.assertEqual(bytes(out),b'\xa5'*(capacity+8) if c['error'] else expected+b'\xa5'*8,c['id'])
 def test_native_sanitizers_and_transactional_capacities(self):
  for label in ['checked','release']:
   exe=self.dir/(label+'-sanitized');run('cc','-std=c11','-O2','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-I'+str(self.dir),self.dir/(label+'.c'),ROOT/'coax/tests/nostr/jcs_native.c','-lm','-o',exe)
   result=subprocess.run([str(exe)],input=self.binary,capture_output=True,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'});self.assertEqual(result.returncode,0,result.stderr.decode());print(label+': '+result.stdout.decode().strip())
 def test_actual_wasm_independent_canonical_bytes(self):
  clang=Path(os.environ.get('COAXIAL_CLANG','/opt/homebrew/opt/llvm/bin/clang'));sysroot=Path(os.environ.get('COAXIAL_SYSROOT','/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'));resources=Path(os.environ.get('COAXIAL_RESOURCES','/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'));environment={**os.environ,'PATH':'/opt/homebrew/opt/lld/bin:'+os.environ.get('PATH','')}
  for label in ['checked','release']:
   module=self.dir/(label+'.wasm');run(clang,'--target=wasm32-wasip1','--sysroot='+str(sysroot),'-resource-dir='+str(resources),'-std=c11','-O2','-ffp-contract=off','-fno-fast-math','-I'+str(self.dir),'-mexec-model=reactor',self.dir/(label+'.c'),ROOT/'coax/tests/nostr/jcs_wasm.c','-Wl,--import-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections','-o',module,env=environment)
   result=json.loads(run('node',ROOT/'coax/tests/nostr/jcs_wasm.cjs',module,self.fixture).stdout);result.update(profile=label,optimization='O2',wasm_compiler=run(clang,'--version').stdout.splitlines()[0]);self.evidence['results'].append(result);print(f"{label}: {result['records']} actual Wasm canonicalization records passed")
if __name__=='__main__':
 evidence=None
 if '--evidence'in sys.argv:
  at=sys.argv.index('--evidence');evidence=Path(sys.argv[at+1]);del sys.argv[at:at+2]
 result=unittest.main(exit=False)
 if result.result.wasSuccessful() and evidence:evidence.write_text(json.dumps(Jcs.evidence,indent=2)+'\n')
 sys.exit(not result.result.wasSuccessful())
