#!/usr/bin/env python3
"""Public experimental gate fixtures: real full-round native/Wasm/browser paths."""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import subprocess
import struct
import tempfile
import time
import unittest

HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[2]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax'))
PURE=ROOT/'coax/lib/access/phrase-gate-worker.coax';PLATFORM=ROOT/'coax/lib/access/platform';ZERO=ROOT/'coax/lib/crypto/platform'
FIXTURE=ROOT/'coax/tests/nostr/data/phrase-gate-draft-2026-10-28.json'
def run(*args,**kwargs):
 r=subprocess.run([str(a) for a in args],capture_output=True,text=True,**kwargs)
 if r.returncode:raise RuntimeError(f'{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}')
 return r
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def dump(v):return json.dumps(v,separators=(',',':'),ensure_ascii=False)
def cases():
 f=json.loads(FIXTURE.read_text());v=f['v2'];pointer=json.loads(bytes.fromhex(v['plaintextHex']));record=dump(v['contextRecord']);entry=dump(v['entry']);rows=[]
 def add(label,message=entry,password=v['phrase'],manifest=record,relay=pointer['relay'],recipient=pointer['recipient'],operation=0,error=0,expected=None):
  rows.append(dict(id=label,message=message,password=password,record=manifest,relay=relay,recipient=recipient,operation=operation,error=error,expected=expected if expected is not None else v['plaintextHex']))
 add('reviewed-open');add('reviewed-seal',bytes.fromhex(v['plaintextHex']).decode(),operation=1,expected=entry.encode().hex())
 add('explicit-normalization',password='Coax demo-phrase2026')
 add('wrong-phrase',password='incorrectphrase2026',error=10)
 for item in f['invalid']:
  import base64
  packet=dump({'v':v['entry']['v'],'payload':base64.b64encode(bytes.fromhex(item['wireHex'])).decode()})
  error=5 if not item['beforeDecrypt'] else 4 if item['case']=='context' else 10 if item['case'] in ['salt','nonce','ciphertext','mac'] else 1 if item['case']=='oversize' else 3
  # A complete >4202-byte cryptogram refuses in the entry projection first.
  if len(bytes.fromhex(item['wireHex']))>4202:error=1
  add('fixture-'+item['case'],packet,error=error)
 add('raw-v2-bang',password='coaxdemophrase2026!',error=2)
 add('unicode-folding-forbidden',password='Coaxdemophrase2026K',error=2)
 add('empty-password',password='',error=2)
 add('password-raw-over',password=' '*129,error=1)
 add('context-absent',manifest='',error=4)
 add('context-empty',manifest='{}',error=4)
 bad=json.loads(record);bad['presentation']='different';add('trusted-context-other',manifest=dump(bad),error=4)
 add('caller-relay-policy',relay='wss://different.example',error=5)
 add('caller-recipient-policy',recipient='0'*64,error=5)
 add('caller-uri-unsupported',relay='wss://127.0.0.1',error=5)
 add('packet-version',message=entry.replace('phrase-gate.v2','phrase-gate.v3'),error=3)
 add('packet-duplicate',message=entry[:-1]+',"v":"openagents.phrase-gate.v2"}',error=3)
 add('packet-unknown',message=entry[:-1]+',"grant":true}',error=3)
 add('packet-padbits',message=dump({'v':v['entry']['v'],'payload':'AR=='}),error=3)
 add('packet-whitespace',message=dump({'v':v['entry']['v'],'payload':v['entry']['payload']+'\n'}),error=3)
 add('packet-raw-over',message=' '*6145,error=1)
 for extra in json.loads(run('node',HERE/'extra-vectors.ts').stdout):
  add(extra.pop('id'),**extra)
 return rows,f

def bind(lib):
 for name in ['password','record','message','relay','recipient','entropy','output']:getattr(lib,'pg_worker_'+name).restype=C.c_size_t
 lib.pg_worker_preflight.argtypes=[C.c_int32]*5+[C.c_uint32];lib.pg_worker_preflight.restype=C.c_uint32
 lib.pg_worker_begin.argtypes=[C.c_int32]*5+[C.c_uint32];lib.pg_worker_begin.restype=C.c_uint32
 lib.pg_worker_advance.argtypes=[C.c_int32];lib.pg_worker_advance.restype=C.c_uint32
 return lib
def fill(lib,row):
 assert lib.pg_worker_dispose()==0
 values={name:row[name].encode() for name in ['password','record','message','relay','recipient']};lengths=[len(v) for v in values.values()]
 error=lib.pg_worker_preflight(*lengths,row['operation'])
 if error:return lengths,error
 for name,data in values.items():C.memmove(getattr(lib,'pg_worker_'+name)(),data,len(data))
 C.memmove(lib.pg_worker_entropy(),bytes(range(28)),28)
 return lengths,lib.pg_worker_begin(*lengths,row['operation'])
def execute(lib,row,budget=1024):
 lengths,error=fill(lib,row);steps=0;previous=0
 if not error:
  assert lib.pg_worker_output()==0 and lib.pg_worker_written()==0
  while lib.pg_worker_running():
   error=lib.pg_worker_advance(budget);steps+=1;completed=lib.pg_worker_completed();assert previous<=completed<=210000 and completed-previous<=budget;previous=completed
   assert steps<210002
   if error:break
 if not error:error=lib.pg_worker_error()
 result=C.string_at(lib.pg_worker_output(),lib.pg_worker_written()) if lib.pg_worker_emit() else b''
 assert error==row['error'],(row['id'],error,row['error'])
 assert result==(bytes.fromhex(row['expected']) if not error else b''),row['id']
 assert lib.pg_worker_written()==(len(result) if not error else 0)
 assert lib.pg_worker_dispose()==0 and lib.pg_worker_clean()==1
 return dict(id=row['id'],error=error,steps=steps,completed=previous)

class Gate(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-phrase-gate-');cls.directory=Path(cls.temp.name);cls.rows,cls.fixture=cases();cls.libs=[];cls.modules={};cls.report={'schema':'coax.phrase-gate-experimental.v1','compiler':run(COMPILER,'version').stdout.strip(),'compiler_sha256':sha(COMPILER),'fixture_sha256':sha(FIXTURE),'scope':'Experimental public-fixture/staging path only. No enabled production format switch or full-memory erasure assurance.','native':[],'wasm':[],'browser':[]}
  deps=json.loads(run(COMPILER,'deps',PURE).stdout)['sources'];paths=[Path(p)for p in deps]+[PLATFORM/'phrase-gate-worker.c',PLATFORM/'phrase-gate-worker.js',ZERO/'zeroize.c',ZERO/'zeroize.h',Path(__file__),FIXTURE,ROOT/'coax/web/unlock/gate.ts',ROOT/'coax/web/unlock/inject.ts']
  cls.report['sources']={str(p.relative_to(ROOT)):sha(p)for p in paths}
  for mode,flags in [('checked',[]),('release',['--release'])]:
   source=cls.directory/(mode+'.c');source.write_text(run(COMPILER,'c',PURE,'--library','--header',cls.directory/'zeroize-policy.h',*flags).stdout)
   library=cls.directory/(mode+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC','-DCOAX_PG_TEST_OBSERVE','-I'+str(cls.directory),'-I'+str(ZERO),source,PLATFORM/'phrase-gate-worker.c',ZERO/'zeroize.c','-lm','-o',library)
   cls.libs.append((mode,bind(C.CDLL(str(library)))))
 @classmethod
 def tearDownClass(cls):
  destination=os.environ.get('COAX_PHRASE_GATE_REPORT')
  if destination:Path(destination).write_text(json.dumps(cls.report,indent=2)+'\n')
  cls.temp.cleanup()
 def test_full_round_native_exact_fixtures_and_negative_matrix(self):
  for mode,lib in self.libs:
   started=time.monotonic();observations=[execute(lib,row)for row in self.rows]
   self.report['native'].append(dict(mode=mode,records=len(self.rows),elapsed_ms=round((time.monotonic()-started)*1000,2),observations=observations));print(mode+': '+str(len(self.rows))+' full-round/admission native cases')
 def test_native_cancellation_cleanup_failure_and_scalar_abi(self):
  row=self.rows[0]
  for mode,lib in self.libs:
   lengths,error=fill(lib,row);self.assertEqual(error,0);self.assertEqual(lib.pg_worker_advance(256),0);self.assertEqual(lib.pg_worker_completed(),256)
   self.assertEqual(lib.pg_worker_cancel(),12);self.assertEqual(lib.pg_worker_emit(),0);self.assertEqual(lib.pg_worker_output(),0);self.assertEqual(lib.pg_worker_dispose(),0);self.assertEqual(lib.pg_worker_clean(),1)
   lengths,error=fill(lib,row);self.assertEqual(error,0);lib.pg_worker_test_fault(1)
   while lib.pg_worker_running():error=lib.pg_worker_advance(1024)
   self.assertEqual(error,13);self.assertEqual(lib.pg_worker_emit(),0);self.assertEqual(lib.pg_worker_output(),0)
   self.assertEqual(lib.pg_worker_dispose(),13);lib.pg_worker_test_fault(0);self.assertEqual(lib.pg_worker_dispose(),0);self.assertEqual(lib.pg_worker_clean(),1)
   for budget in [0,-1,1025]:
    lengths,error=fill(lib,row);self.assertEqual(error,0);self.assertNotEqual(lib.pg_worker_advance(budget),0);self.assertEqual(lib.pg_worker_output(),0);self.assertEqual(lib.pg_worker_dispose(),0);self.assertEqual(lib.pg_worker_clean(),1)
   self.assertEqual(lib.pg_worker_preflight(-1,1,1,7,64,0),1);self.assertEqual(lib.pg_worker_preflight(1,1,6145,7,64,0),1)
   self.assertEqual(lib.pg_worker_attempts(4,4),0);self.assertEqual(lib.pg_worker_attempts(4,5),1)
 def test_actual_legacy_adapter_source_and_entries_are_unchanged(self):
  self.assertEqual(sha(ROOT/'coax/web/unlock/gate.ts'),self.fixture['legacy']['sourceSha256'])
  for path in ['coax/web/unlock/gate.ts','coax/web/unlock/inject.ts']:
   original=subprocess.check_output(['git','-C',str(ROOT),'show','fa594f441d:'+path]);self.assertEqual((ROOT/path).read_bytes(),original)
 def test_native_ASan_UBSan_full_round_and_lifecycle(self):
  data=bytearray()
  for row in self.rows:
   fields=[row[name].encode()for name in ['password','record','message','relay','recipient']];expected=bytes.fromhex(row['expected'])if not row['error']else b''
   data.extend(struct.pack('<8I',row['operation'],row['error'],*(len(x)for x in fields),len(expected))+b''.join(fields)+expected)
  for mode in ['checked','release']:
   executable=self.directory/(mode+'-sanitized');run('cc','-std=c11','-O2','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-DCOAX_PG_TEST_OBSERVE','-I'+str(self.directory),'-I'+str(ZERO),self.directory/(mode+'.c'),PLATFORM/'phrase-gate-worker.c',ZERO/'zeroize.c',HERE/'native.c','-lm','-o',executable)
   result=subprocess.run([str(executable)],input=data,capture_output=True,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'});self.assertEqual(result.returncode,0,result.stderr.decode());print(mode+': '+result.stdout.decode().strip())
   self.report['native'].append(dict(mode=mode,target='asan-ubsan',records=len(self.rows),observation=result.stdout.decode().strip()))
 def test_actual_Wasm_full_round_and_lifecycle(self):
  fixture=self.directory/'rows.json';fixture.write_text(json.dumps(self.rows));clang=Path('/opt/homebrew/opt/llvm/bin/clang')
  for mode in ['checked','release']:
   target=self.directory/(mode+'.wasm');run(clang,'--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-DCOAX_PG_TEST_OBSERVE','-I'+str(self.directory),'-I'+str(ZERO),'-mexec-model=reactor',self.directory/(mode+'.c'),PLATFORM/'phrase-gate-worker.c',ZERO/'zeroize.c','-Wl,--export-memory,--initial-memory=8388608,--max-memory=16777216,-z,stack-size=2097152,--gc-sections','-lm','-o',target)
   result=json.loads(run('node',HERE/'wasm.cjs',target,fixture).stdout);result['mode']=mode;self.report['wasm'].append(result);print(mode+': '+str(result['records'])+' actual Wasm full-round/admission cases')

if __name__=='__main__':unittest.main()
