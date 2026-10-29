#!/usr/bin/env python3
"""Focused public BIP340/strict Nostr verification in actual native and Wasm."""
import ctypes as C
import csv
import hashlib
import json
import os
from pathlib import Path
import random
import re
import subprocess
import tempfile
import time
import unittest
import schnorr_support as support
from oracle import NativeOracle,DEFAULT_LIBRARY,N,P,schnorr_verify
ROOT=support.ROOT
def official():
 if support.sha(support.VECTORS)!=support.VECTOR_SHA:raise ValueError('upstream BIP340 corpus changed')
 with support.VECTORS.open(newline='') as stream:
  return [{'message':bytes.fromhex(r['message']),'public':bytes.fromhex(r['public key']),'signature':bytes.fromhex(r['signature']),'expected':r['verification result']=='TRUE','index':int(r['index'])} for r in csv.DictReader(stream)]

def body_of(source,name):
 match=re.search(r'\b(?:coax_p_)?'+name+r'\([^;{}]*\)\s*\{',source)
 if not match:raise ValueError('missing actual emitted function:'+name)
 start=match.end();end=source.index('\n}',start);return start,end,source[start:end]
def cbytes(value):return '{'+','.join(str(n) for n in value)+'}'

class Schnorr(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-schnorr-focused-');cls.dir=Path(cls.temp.name);cls.libs,artifacts=support.build(cls.dir);cls.rows=official();cls.native=NativeOracle(DEFAULT_LIBRARY);cls.start=time.monotonic()
  sources=support.source_pins();sources.update({str(p.relative_to(ROOT)):support.sha(p) for p in [Path(__file__),ROOT/'coax/tests/nostr/schnorr_wasm.c',ROOT/'coax/tests/nostr/schnorr_wasm.cjs']})
  cls.report={'schema':'coax.schnorr-focused.v1','scope':'public verification correctness and actual native/Wasm observations; no secret signing, timing security, physical device or production-secret approval','sources':sources,'check_inputs':support.check_inputs(),'official_sha256':support.VECTOR_SHA,'compiler':support.base.run(support.base.COMPILER,'version').stdout.strip(),'compiler_sha256':support.sha(support.base.COMPILER),'native_compiler':support.base.run('cc','--version').stdout.splitlines()[0],'native_target':support.base.run('cc','-dumpmachine').stdout.strip(),'native_artifacts':artifacts,'oracle_artifact_sha256':support.sha(DEFAULT_LIBRARY),'budgets':{'max_message_bytes':1048576,'max_temporary_bytes':33554432,'native_random_cases':256},'native_timing':[]}
 @classmethod
 def tearDownClass(cls):
  cls.native.close();used=sum(p.stat().st_size for p in cls.dir.rglob('*') if p.is_file());cls.report['temporary_bytes']=used;cls.report['elapsed_seconds']=time.monotonic()-cls.start
  if used>33554432:raise AssertionError('temporary artifact budget exceeded')
  support.check_pins(cls.report['sources'])
  if os.environ.get('COAX_SCHNORR_REPORT'):Path(os.environ['COAX_SCHNORR_REPORT']).write_text(json.dumps(cls.report,indent=2,sort_keys=True)+'\n')
  cls.temp.cleanup()
 @classmethod
 def signed(cls,message,key=3):
  secret=key.to_bytes(32,'big');encoded=cls.native.point(secret);signature=cls.native.sign(message,secret,bytes(32))
  if encoded is None or signature is None:raise ValueError('throwaway signed fixture creation')
  return encoded[1:],signature
 def test_all_official_rows_including_exact_variable_message_lengths(self):
  self.assertEqual(len(self.rows),19);self.assertEqual(sorted({len(r['message']) for r in self.rows}),[0,1,17,32,100]);independent=0
  for row in self.rows:
   self.assertEqual(self.native.verify(row['message'],row['public'],row['signature']),row['expected']);self.assertEqual(schnorr_verify(row['message'],row['public'],row['signature']),row['expected']);independent+=1
   for label,lib in self.libs:
    for nostr in [False,True]:
     actual,elapsed=support.verify_timed(lib,row['message'],row['public'],row['signature'],nostr=nostr);self.assertEqual(actual,row['expected'] and (not nostr or len(row['message'])==32),(label,nostr,row['index']))
     self.report['native_timing'].append({'mode':label,'entry':'nostr' if nostr else 'general','official_index':row['index'],'message_bytes':len(row['message']),'valid':actual,'seconds':elapsed})
  self.report['official_rows']=19;self.report['official_comparisons']=76;self.report['independent_affine_verifications']=independent;self.report['official_message_lengths']=[0,1,17,32,100]
 def test_seeded_generated_messages_mutations_and_bounds(self):
  rng=random.Random(340124);count=0
  for i in range(256):
   message=rng.randbytes(rng.choice([0,1,17,32,100,255,1024]));public,signature=self.signed(message,rng.randrange(1,N));mutated=bytearray(signature);mutated[rng.randrange(64)]^=1<<rng.randrange(8)
   for sig in [signature,bytes(mutated),P.to_bytes(32,'big')+signature[32:],signature[:32]+N.to_bytes(32,'big')]:
    expected=self.native.verify(message,public,sig)
    for label,lib in self.libs:self.assertEqual(support.verify(lib,message,public,sig),expected,(label,i));count+=1
  maximum=bytes(range(256))*4096;public,signature=self.signed(maximum)
  self.assertTrue(self.native.verify(maximum,public,signature))
  for label,lib in self.libs:
   self.assertTrue(support.verify(lib,maximum,public,signature));self.assertFalse(support.verify(lib,maximum+b'\0',public,signature));count+=2
  self.report['seeded_native_comparisons']=count;self.report['maximum_message_validated']=1048576
 def test_metadata_refusals_and_positive_readonly_input_alias(self):
  row=self.rows[0];counts=[(-1,32,64),(1048577,32,64),(2**63-1,32,64),(-2**63,32,64),(32,-1,64),(32,31,64),(32,33,64),(32,32,63),(32,32,65),(32,32,2**63-1)];count=0
  for label,lib in self.libs:
   for values in counts:
    self.assertFalse(support.verify(lib,row['message'],row['public'],row['signature'],counts=values));self.assertFalse(support.verify(lib,row['message'],row['public'],row['signature'],nostr=True,counts=values));count+=2
   public,_=self.signed(b'');public,signature=self.signed(public);shared=support.base.array(public);sig=support.base.array(signature)
   for name in ['schnorr_verify_vartime','nostr_verify_vartime']:
    self.assertTrue(getattr(lib,name)(support.base.view(shared),support.base.view(shared),support.base.view(sig)));self.assertEqual(bytes(shared),public);self.assertEqual(bytes(sig),signature);count+=1
  self.report['metadata_and_alias_cases']=count
 def test_real_wasm_official_generated_maximum_and_metadata_cases(self):
  cases=[]
  def add(name,message,public,signature,expected,nostr=False,counts=None):
   row={'name':name,'message':message.hex(),'public':public.hex(),'signature':signature.hex(),'expected':expected,'nostr':nostr}
   if counts is not None:row['counts']=counts
   cases.append(row)
  for row in self.rows:
   for nostr in [False,True]:add('official'+str(row['index']),row['message'],row['public'],row['signature'],row['expected'] and (not nostr or len(row['message'])==32),nostr)
  rng=random.Random(34032)
  for i in range(64):
   message=rng.randbytes(rng.choice([0,1,17,32,100,255]));public,signature=self.signed(message,rng.randrange(1,N));add('valid'+str(i),message,public,signature,True)
   bad=bytearray(signature);bad[i%64]^=1;bad=bytes(bad);add('changed'+str(i),message,public,bad,self.native.verify(message,public,bad))
  maximum=bytes(range(256))*4096;public,signature=self.signed(maximum);add('maximum',maximum,public,signature,True);add('over-maximum',maximum+b'\0',public,signature,False)
  row=self.rows[0]
  for counts in [[-1,32,64],[1048577,32,64],[32,31,64],[32,33,64],[32,32,63],[32,32,65]]:add('metadata',row['message'],row['public'],row['signature'],False,counts=counts)
  fixtures=self.dir/'wasm-fixtures.json';fixtures.write_text(json.dumps(cases));reports=[]
  for label,lib in self.libs:
   wasm=self.dir/(label+'.wasm');support.base.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(self.dir),self.dir/(label+'.c'),ROOT/'coax/tests/nostr/schnorr_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',wasm)
   reports.append({'mode':label,'artifact_sha256':support.sha(wasm),'wasm_compiler':support.base.run('/opt/homebrew/opt/llvm/bin/clang','--version').stdout.splitlines()[0],**json.loads(support.base.run('node',ROOT/'coax/tests/nostr/schnorr_wasm.cjs',wasm,fixtures).stdout)})
  self.report['wasm']=reports
 def test_sanitized_metadata_boundaries_and_readonly_inputs(self):
  row=self.rows[0];probe=self.dir/'sanitized.c';probe.write_text('''#include "checked.c"
#include <assert.h>
int main(void){uint8_t m[32]=M,p[32]=P,s[64]=S;
coax_v_u8 message={m,32},pub={p,32},sig={s,64};
assert(schnorr_verify_vartime(message,pub,sig));assert(nostr_verify_vartime(message,pub,sig));
int64_t bad[]={INT64_MIN,-1,1048577,INT64_MAX};
for(int i=0;i<4;i++){assert(!schnorr_verify_vartime((coax_v_u8){m,bad[i]},pub,sig));assert(!nostr_verify_vartime((coax_v_u8){m,bad[i]},pub,sig));}
assert(!schnorr_verify_vartime(message,(coax_v_u8){p,31},sig));assert(!schnorr_verify_vartime(message,pub,(coax_v_u8){s,65}));return 0;}'''.replace('=M', '= '+cbytes(row['message'])).replace('=P','= '+cbytes(row['public'])).replace('=S','= '+cbytes(row['signature'])))
  exe=self.dir/'sanitized';support.base.run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',probe,'-lm','-o',exe);support.base.run(exe);self.report['sanitizer']='positive official signature plus malformed count/length entrypoints under ASan+UBSan'
 def test_actual_emitted_faults_are_detected_by_official_verification(self):
  source=(self.dir/'checked.c').read_text();faults={};a,b,body=body_of(source,'sv_challenge_vartime')
  missing,n=re.subn(r'l_status = coax_p_sha256_update_checked\([^;]*l_public_key[^;]*;', 'l_status = 0;',body,count=1);self.assertEqual(n,1);faults['missing_challenge_public_key']=source[:a]+missing+source[b:]
  a,b,body=body_of(source,'sv_prepare_vartime');self.assertIn('sc_negate((l_challenge).value)',body);faults['wrong_challenge_sign']=source[:a]+body.replace('sc_negate((l_challenge).value)','(l_challenge).value',1)+source[b:]
  a,b,body=body_of(source,'sv_affine_matches_vartime');self.assertEqual(body.count('UINT64_C(1)'),1);parity=body.replace('UINT64_C(1)','UINT64_C(0)',1)
  faults['missing_even_y_check']=source[:a]+parity+source[b:]
  a,b,body=body_of(source,'sv_prepare_nostr_vartime');self.assertIn('(l_event_id).count != INT64_C(32)',body);faults['nostr_length_bypass']=source[:a]+body.replace('(l_event_id).count != INT64_C(32)','0',1)+source[b:]
  detected=[]
  for name,changed in faults.items():
   self.assertNotEqual(changed,source);path=self.dir/(name+'.c');path.write_text(changed);artifact=self.dir/(name+'.dylib');support.base.run('cc','-std=c11','-O2','-shared','-fPIC',path,'-lm','-o',artifact);lib=C.CDLL(str(artifact))
   for fn in ['schnorr_verify_vartime','nostr_verify_vartime']:getattr(lib,fn).argtypes=[support.base.View]*3;getattr(lib,fn).restype=C.c_bool
   mismatches=[]
   for row in self.rows:
    for nostr in [False,True]:
     expected=row['expected'] and (not nostr or len(row['message'])==32)
     if support.verify(lib,row['message'],row['public'],row['signature'],nostr=nostr)!=expected:mismatches.append([row['index'],nostr])
   self.assertTrue(mismatches,name);detected.append({'fault':name,'caught_by':mismatches})
  self.report['actual_emitted_faults_detected']=detected
 def test_direct_full_positive_interpreter_probe_records_the_default_budget(self):
  row=self.rows[0]
  def array(data):return '['+str(len(data))+']u8.['+','.join(str(n) for n in data)+']'
  probe=self.dir/'positive.coax';probe.write_text('#load "'+str(support.SOURCE)+'";\n#test "direct full positive BIP340 default-budget observation" {\n m:='+array(row['message'])+';p:='+array(row['public'])+';s:='+array(row['signature'])+';\n #assert schnorr_verify_vartime(m[..],p[..],s[..]);\n}\n')
  result=subprocess.run([str(support.base.COMPILER),'test',str(probe)],capture_output=True,text=True)
  if result.returncode:
   self.assertIn('error[E0400]',result.stdout+result.stderr);self.assertIn('exceeded 20000000 steps',result.stdout+result.stderr)
  self.report['direct_positive_interpreter_probe']={'passed_within_default_budget':result.returncode==0,'exit_code':result.returncode,'default_steps':20000000,'error':None if result.returncode==0 else 'E0400','output_sha256':hashlib.sha256((result.stdout+result.stderr).encode()).hexdigest(),'scope':'one real official positive signature; no override, simulation or substitute for native/Wasm/hour validation'}
 def test_component_fixtures_private_boundaries_and_hour_labels(self):
  support.base.run('python3',ROOT/'coax/tests/nostr/make_schnorr_fixtures.py')
  source=support.SOURCE.read_text();exports=re.findall(r'^(\w+)\s*::.*#export_c',source,re.M);self.assertEqual(exports,['schnorr_verify_vartime','nostr_verify_vartime'])
  self.assertTrue(all(name.endswith('_vartime') for name in re.findall(r'^(\w+)\s*::\s*\(',source,re.M)))
  self.report['interpreter_scope']='124 default-budget component/equation checks; no full positive end-to-end BIP340 signature check within20M; native/Wasm official vectors and actual hour are end-to-end evidence, not a proof'
if __name__=='__main__':unittest.main()
