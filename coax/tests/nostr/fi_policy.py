#!/usr/bin/env python3
"""Independent IANA/IP/time oracles and actual emitted FI candidate fixtures."""
import csv
import ctypes as C
from functools import cache
import hashlib
import ipaddress as ip
import json
from pathlib import Path
import random
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
OUT=ROOT/'coax/bin/fi-policy-2026-10-28'
IANA=HERE/'data/iana-2026-10-28'
SOURCE=ROOT/'coax/lib/nostr/block/fi-jwks.coax'
# Candidate=0, unavailable=1, rejected=2. No scenario grants authorization.
SCENARIOS=[0,2,1,2,2,2,1,2,2,0,1,1,1,1,1,1,1,1,1,1,2,2,0,0,1,2,0,2,2,2,2,1,1,1,1,1,0,1,0,1,0,1,1]
class View(C.Structure):_fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
def run(*args):
 r=subprocess.run([str(a)for a in args],capture_output=True,text=True,timeout=120)
 if r.returncode:raise RuntimeError(r.stderr or r.stdout)
 return r.stdout
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
@cache
def networks(name):
 with (IANA/name).open() as source:
  return [ip.ip_network(part.strip().split(' ')[0])for row in csv.DictReader(source)for part in row['Address Block'].split(',')]
def oracle(a,transport):
 if transport==1:
  return not any(a in n for n in networks('ipv4.csv')) and not a.is_multicast if a.version==4 else a in ip.ip_network('2000::/3') and not any(a in n for n in networks('ipv6.csv'))
 if transport==2:
  return any(a in ip.ip_network(n)for n in ['10.0.0.0/8','172.16.0.0/12','192.168.0.0/16','100.64.0.0/10','240.0.0.0/4','127.0.0.0/8']) and str(a)!='255.255.255.255' if a.version==4 else a in ip.ip_network('fc00::/7') or a==ip.IPv6Address('::1')
 return False
class FI(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  OUT.mkdir(parents=True,exist_ok=True);rng=random.Random(981998)
  addresses=[ip.ip_address(r['address'])for r in json.loads((IANA/'address-cases.json').read_text())]
  addresses += [ip.IPv4Address(rng.getrandbits(32))for _ in range(2048)] + [ip.IPv6Address(rng.getrandbits(128))for _ in range(2048)]
  cls.rows=[{'hex':a.packed.hex(),'transport':mode,'expected':oracle(a,mode)}for a in addresses for mode in [1,2]]
  cls.fixture={'addresses':cls.rows,'scenarios':SCENARIOS};(OUT/'cases.json').write_text(json.dumps(cls.fixture)+'\n')
  cls.libs=[];cls.report={'schema':'coax.fi-candidate-policy.v1.evidence','scope':'Typed candidate policy only; no signature verification, live network enforcement, full AssertionPolicyId or identity grant','compiler':run(ROOT/'coax/bin/coax','version').strip(),'compiler_sha256':sha(ROOT/'coax/bin/coax'),'sources':{str(p.relative_to(ROOT)):sha(p)for p in [SOURCE,ROOT/'coax/lib/nostr/block/fi-policy.coax',ROOT/'coax/lib/nostr/block/fi-addresses.coax',ROOT/'coax/lib/net/uri.coax']},'cases_sha256':sha(OUT/'cases.json'),'modes':[]}
  for mode,flags in [('checked',[]),('release',['--release'])]:
   generated=OUT/(mode+'.c');generated.write_text(run(ROOT/'coax/bin/coax','c',SOURCE,'--library',*flags));wrapper=OUT/(mode+'_fixture.c');wrapper.write_text((HERE/'fi_policy_fixture.c').read_text().replace('FI_SOURCE',mode+'.c'));libpath=OUT/(mode+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC','-Wno-unknown-attributes',wrapper,'-lm','-o',libpath);lib=C.CDLL(str(libpath));lib.fi_scenario.argtypes=[C.c_int32];lib.fi_scenario.restype=C.c_int32;lib.fi_address_allowed.argtypes=[View,C.c_int64];lib.fi_address_allowed.restype=C.c_bool;lib.fi_time_bounds.argtypes=[C.c_int64,C.c_int64,C.c_int64,C.c_bool,C.c_int64,C.c_int64,C.c_int64];lib.fi_time_bounds.restype=C.c_bool;cls.libs.append((mode,lib));cls.report['modes'].append({'mode':mode,'generated_c_sha256':sha(generated),'binary_sha256':sha(libpath)})
 @classmethod
 def tearDownClass(cls):(OUT/'report.json').write_text(json.dumps(cls.report,indent=2,sort_keys=True)+'\n')
 def test_independent_registry_and_mesh_oracle(self):
  for mode,lib in self.libs:
   for row in self.rows:
    data=bytes.fromhex(row['hex']);b=(C.c_uint8*len(data))(*data)
    self.assertEqual(lib.fi_address_allowed(View(b,len(data)),row['transport']),row['expected'],(mode,row))
  self.report['addresses_per_mode']=len(self.rows)
 def test_actual_policy_chain_and_independent_numericdate_oracle(self):
  rng=random.Random(20261028);safe=2**53-1;times=[-1,0,1,99,100,101,109,110,safe-1,safe,safe+1];count=0
  for mode,lib in self.libs:
   for i,status in enumerate(SCENARIOS):self.assertEqual(lib.fi_scenario(i),status,(mode,i))
   for _ in range(4096):
    now,issued,expires,nbf=(rng.choice(times)for _ in range(4));has_nbf=rng.choice([True,False]);age=rng.choice([0,1,10,31536000,31536001]);skew=rng.choice([-1,0,1,300,301])
    valid=lambda n:0<=n<=safe
    wanted=all(valid(n)for n in [now,issued,expires]) and 1<=age<=31536000 and 0<=skew<=300 and now<expires and issued<=now+skew and now<issued+age and (not has_nbf or valid(nbf) and nbf<=now+skew)
    self.assertEqual(lib.fi_time_bounds(now,issued,expires,has_nbf,nbf,age,skew),wanted,(mode,now,issued,expires,has_nbf,nbf,age,skew));count+=1
  self.report['candidate_scenarios_per_mode']=len(SCENARIOS);self.report['time_comparisons']=count
 def test_sanitizers_and_real_wasm(self):
  binary=OUT/'addresses.bin';binary.write_bytes(b''.join(bytes([len(bytes.fromhex(r['hex'])),r['transport'],int(r['expected'])])+bytes.fromhex(r['hex']).ljust(16,b'\0')for r in self.rows))
  main=OUT/'sanitized.c';main.write_text('#include "checked_fixture.c"\n#include <assert.h>\nint main(int argc,char**argv){int expected[]={' + ','.join(map(str,SCENARIOS)) + '};for(int i=0;i<43;i++)assert(fi_scenario(i)==expected[i]);FILE*f=fopen(argv[1],"rb");assert(f);uint8_t r[19];while(fread(r,1,19,f)==19){memcpy(input,r+3,16);assert(fi_address_probe(r[0],r[1])==r[2]);}assert(feof(f));fclose(f);return 0;}')
  exe=OUT/'sanitized';run('cc','-std=c11','-O1','-g','-Wno-unknown-attributes','-fsanitize=address,undefined',main,'-lm','-o',exe);run(exe,binary)
  reports=[]
  for mode,_ in self.libs:
   wasm=OUT/(mode+'.wasm');run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',OUT/(mode+'_fixture.c'),'-lm','-o',wasm);result=json.loads(run('bun',HERE/'fi_policy_wasm.cjs',wasm,OUT/'cases.json'));self.assertEqual(result['addresses'],len(self.rows));self.assertEqual(result['scenarios'],len(SCENARIOS));reports.append({'mode':mode,'artifact_sha256':sha(wasm),**result})
  self.report['sanitizer']={'addresses':len(self.rows),'scenarios':len(SCENARIOS),'binary_sha256':sha(exe)};self.report['wasm']=reports
if __name__=='__main__':unittest.main()
