#!/usr/bin/env python3
"""Selected URI profile: independent component oracle and native/Wasm guards."""
import ctypes as C
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess
import unittest
from urllib.parse import urlsplit

ROOT=Path(__file__).resolve().parents[3];HERE=Path(__file__).resolve().parent;OUT=ROOT/'coax/bin/uri-2026-10-28'
FIELDS=['ok','scheme','host_from','host_to','port','path_from','path_to','has_query','query_from']
class View(C.Structure):_fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class Parts(C.Structure):_fields_=[(k,C.c_bool if k in ['ok','has_query']else C.c_int64)for k in FIELDS]
def run(*args):
 r=subprocess.run([str(x)for x in args],capture_output=True,text=True,timeout=90)
 if r.returncode:raise RuntimeError(r.stderr or r.stdout)
 return r.stdout
def cases():
 rows=[];rng=random.Random(20261028)
 for _ in range(256):
  scheme=rng.choice(['http','https','wss']);host=''.join(rng.choice('abcxyz0123')for _ in range(5))+'.Example';port=rng.choice(['',':1',':80',':443',':65535']);path=rng.choice(['','/','/a%20b','/:@!$&\'()*+,;=-._~','/%2F%FF']);query=rng.choice(['','?','?x=1','?x?y'])
  text=f'{scheme}://{host}{port}{path}{query}';u=urlsplit(text);start=len(scheme)+3;path_at=start+len(u.netloc);has='?'in text;end=text.index('?')if has else len(text)
  expected={'ok':True,'scheme':{'https':1,'http':2,'wss':3}[scheme],'host_from':start,'host_to':start+len(host),'port':u.port or (80 if scheme == 'http' else 443),'path_from':path_at,'path_to':end,'has_query':has,'query_from':end+1 if has else 0}
  assert text[expected['host_from']:expected['host_to']].lower()==u.hostname and text[path_at:end]==u.path
  rows.append({'input':text.encode().hex(),'expected':expected})
 invalid=['','https://','HTTPS://a.test','//a.test','https://u@a.test','https://a.test#f','https://a.test\\b','https://[::1]/','https://127.0.0.1','https://0177.0.0.1','https://0x7f000001','https://2130706433','https://a..test','https://.test','https://a.test.','https://a-.test','https://a_test','https://a:0','https://a:01','https://a:65536','https://a:/','https://a:?x','https://a:%31','https://a.test/%','https://a.test/%1','https://a.test/%3G','https://a.test/%00','https://a.test/%7F','https://a.test/%5c','https://a.test/%41<','https://a.test/a b']
 invalid+=['https://a.test/'+chr(i)for i in range(256)if i<33 or i>=127 or i in [34,35,60,62,91,92,93,94,96,123,124,125]]
 for text in invalid:rows.append({'input':text.encode().hex(),'expected':dict.fromkeys(FIELDS,0)})
 for n in [2048,2049]:
  text='https://a.test/'+'a'*(n-len('https://a.test/'));expected={'ok':True,'scheme':1,'host_from':8,'host_to':14,'port':443,'path_from':14,'path_to':n,'has_query':False,'query_from':0}if n==2048 else dict.fromkeys(FIELDS,0);rows.append({'input':text.encode().hex(),'expected':expected})
 return rows
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
class URI(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  OUT.mkdir(parents=True,exist_ok=True);cls.rows=cases();(OUT/'cases.json').write_text(json.dumps(cls.rows)+'\n');cls.libs=[];cls.report={'schema':'coax.absolute-dns-uri-v1.evidence','scope':'Selected syntax only, no DNS/TLS/SSRF/identity authorization','compiler':run(ROOT/'coax/bin/coax','version').strip(),'compiler_sha256':sha(ROOT/'coax/bin/coax'),'source_sha256':sha(ROOT/'coax/lib/net/uri.coax'),'case_count':len(cls.rows),'cases_sha256':sha(OUT/'cases.json'),'modes':[]}
  for mode,flags in [('checked',[]),('release',['--release'])]:
   source=OUT/(mode+'.c');source.write_text(run(ROOT/'coax/bin/coax','c',ROOT/'coax/lib/net/uri.coax','--library',*flags));binary=OUT/(mode+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',source,'-lm','-o',binary);lib=C.CDLL(str(binary));lib.uri_parse.argtypes=[View];lib.uri_parse.restype=Parts;cls.libs.append((mode,lib));cls.report['modes'].append({'mode':mode,'generated_c_sha256':sha(source),'binary_sha256':sha(binary)})
 @classmethod
 def tearDownClass(cls):(OUT/'report.json').write_text(json.dumps(cls.report,indent=2,sort_keys=True)+'\n')
 def test_native_checked_release_components_and_zero_refusal_metadata(self):
  for mode,lib in self.libs:
   for i,row in enumerate(self.rows):
    data=bytes.fromhex(row['input']);array=(C.c_uint8*len(data))(*data);result=lib.uri_parse(View(array,len(data)));actual={k:getattr(result,k)for k in FIELDS};self.assertEqual(actual,row['expected'],(mode,i,data));self.assertEqual(bytes(array),data)
 def test_sanitized_actual_parser_for_every_case(self):
  inputs=OUT/'input.bin';inputs.write_bytes(b''.join(struct.pack('<I',len(bytes.fromhex(r['input'])))+bytes.fromhex(r['input'])for r in self.rows))
  driver=OUT/'sanitized.c';driver.write_text('''#include "checked.c"
int main(int argc,char**argv){FILE*f=fopen(argv[1],"rb");if(!f)return 2;uint8_t h[4],input[4096];while(fread(h,1,4,f)==4){uint32_t n=(uint32_t)h[0]|(uint32_t)h[1]<<8|(uint32_t)h[2]<<16|(uint32_t)h[3]<<24;if(n>4096||fread(input,1,n,f)!=n)return 3;coax_t_UriParts p=uri_parse((coax_string){(char*)input,n});printf("%d,%lld,%lld,%lld,%lld,%lld,%lld,%d,%lld\\n",p.ok,(long long)p.scheme,(long long)p.host_from,(long long)p.host_to,(long long)p.port,(long long)p.path_from,(long long)p.path_to,p.has_query,(long long)p.query_from);}fclose(f);return 0;}''')
  binary=OUT/'sanitized';run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined',driver,'-lm','-o',binary);results=run(binary,inputs).splitlines();self.assertEqual(len(results),len(self.rows))
  for line,row in zip(results,self.rows):self.assertEqual([int(x)for x in line.split(',')],[int(row['expected'][k])for k in FIELDS])
  self.report['sanitizer']={'cases':len(results),'binary_sha256':sha(binary)}
 def test_actual_wasm_components(self):
  reports=[]
  for mode,_ in self.libs:
   wrapper=OUT/'wasm.c';wrapper.write_text((HERE/'uri_wasm.c').read_text().replace('URI_SOURCE',mode+'.c'));wasm=OUT/(mode+'.wasm');run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor','-Wl,--export-memory',wrapper,'-lm','-o',wasm);result=json.loads(run('bun',HERE/'uri_wasm.cjs',wasm,OUT/'cases.json'));self.assertEqual(result['cases'],len(self.rows));reports.append({'mode':mode,'artifact_sha256':sha(wasm),**result})
  self.report['wasm']=reports
if __name__=='__main__':unittest.main()
