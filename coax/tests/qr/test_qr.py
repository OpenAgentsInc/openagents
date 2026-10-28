#!/usr/bin/env python3
"""Offline reference grids and actual native/Wasm QR interoperability."""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[3];HERE=Path(__file__).resolve().parent
DIRECTORY=ROOT/'coax/bin/qr-2026-10-28';COMPILER=ROOT/'coax/bin/coax'
def run(*args):
 r=subprocess.run([str(x)for x in args],capture_output=True,text=True,timeout=90)
 if r.returncode:raise RuntimeError(r.stderr or r.stdout)
 return r.stdout
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
class View(C.Structure):_fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class Result(C.Structure):_fields_=[(name,C.c_int64)for name in ['status','version','size','count','mask','penalty']]
def view(array,count=None,offset=0):return View(C.cast(C.byref(array,offset),C.POINTER(C.c_uint8)),len(array)if count is None else count)

class QR(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  DIRECTORY.mkdir(parents=True,exist_ok=True);cls.reference=json.loads((HERE/'reference-2026-10-28.json').read_text());cls.libs=[]
  cls.report={'schema':'coax.qr-evidence.v1','scope':'Byte-mode M versions1..15 only; no authentication, camera optics, ECI or numeric segmentation claim','compiler':run(COMPILER,'version').strip(),'compiler_sha256':sha(COMPILER),'reference_sha256':sha(HERE/'reference-2026-10-28.json'),'modes':[]}
  for name,flags in [('checked',[]),('release',['--release'])]:
   source=DIRECTORY/(name+'.c');source.write_text(run(COMPILER,'c',ROOT/'coax/lib/qr/qr-memory.coax','--library',*flags))
   dylib=DIRECTORY/(name+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',source,'-lm','-o',dylib)
   lib=C.CDLL(str(dylib));lib.qr_encode.argtypes=[View,View];lib.qr_encode.restype=Result;lib.qr_encode_mask.argtypes=[View,View,C.c_int64];lib.qr_encode_mask.restype=Result
   cls.libs.append((name,lib));cls.report['modes'].append({'name':name,'flags':['-std=c11','-O2'],'generated_c_sha256':sha(source),'object_sha256':sha(dylib)})
 @classmethod
 def tearDownClass(cls):
  if hasattr(cls,'report'):(DIRECTORY/'report.json').write_text(json.dumps(cls.report,indent=2,sort_keys=True)+'\n')
 def test_twenty_primary_reference_grids_masks_and_penalties_match(self):
  versions=set()
  for name,lib in self.libs:
   for index,row in enumerate(self.reference['rows']):
    data=(C.c_uint8*len(row['bytes']))(*row['bytes']);out=(C.c_uint8*5930)(*[165]*5930)
    r=lib.qr_encode(view(data),view(out));self.assertEqual(r.status,0,(name,index,r.status))
    self.assertEqual((r.version,r.size,r.count,r.mask,r.penalty),(row['version'],row['size'],row['size']**2,row['mask'],row['penalty']),(name,index))
    expected=bytes(int(x)for line in row['rows']for x in line);self.assertEqual(bytes(out[:r.count]),expected,(name,index));self.assertEqual(bytes(out[r.count:]),bytes([165])*(5930-r.count))
    versions.add(r.version)
  self.assertEqual(versions,set(range(1,16)));self.report['fixed_reference_grids_per_mode']=20;self.report['versions']=sorted(versions)
 def test_capacity_input_masks_and_real_alias_refuse_without_output_writes(self):
  for name,lib in self.libs:
   data=(C.c_uint8*413)(*([65]*413));out=(C.c_uint8*5930)(*([165]*5930));snapshot=bytes(out)
   for n,capacity,mask,status in [(413,5930,-1,1),(70,0,-1,2),(70,1368,-1,2),(70,5930,-2,3),(70,5930,8,3)]:
    r=lib.qr_encode_mask(view(data,n),view(out,capacity),mask);self.assertEqual(r.status,status);self.assertEqual(bytes(out),snapshot);self.assertEqual((r.version,r.size,r.count,r.mask,r.penalty),(0,0,0,0,0))
   r=lib.qr_encode(view(out,70),view(out));self.assertEqual(r.status,4);self.assertEqual(bytes(out),snapshot)
   for mask in range(8):
    r=lib.qr_encode_mask(view(data,70),view(out),mask);self.assertEqual(r.status,0);self.assertEqual(r.mask,mask);self.assertEqual(r.size,37)
 def test_sanitized_reference_driver_and_refusal_boundaries(self):
  records=bytearray()
  for row in self.reference['rows']:
   records+=len(row['bytes']).to_bytes(2,'little')+bytes(row['bytes'])+row['size'].to_bytes(2,'little')+bytes(int(x)for line in row['rows']for x in line)
  inputs=DIRECTORY/'reference.bin';inputs.write_bytes(records)
  driver=DIRECTORY/'sanitized.c';driver.write_text('''#include "checked.c"
int main(int argc,char**argv){FILE*f=fopen(argv[1],"rb");if(!f)return2;unsigned count=0;uint8_t h[2],data[413],out[5930],expected[5929];
while(fread(h,1,2,f)==2){unsigned n=h[0]+256*h[1];if(n>412||fread(data,1,n,f)!=n||fread(h,1,2,f)!=2)return3;unsigned size=h[0]+256*h[1];if(size>77||fread(expected,1,size*size,f)!=size*size)return4;
memset(out,165,sizeof(out));coax_t_QrResult r=qr_encode((coax_v_u8){data,n},(coax_v_u8){out,sizeof(out)});if(r.status||r.size!=size||memcmp(out,expected,size*size))return5;for(unsigned i=size*size;i<sizeof(out);i++)if(out[i]!=165)return6;count++;}
if(count!=20||ferror(f))return7;fclose(f);memset(data,65,sizeof(data));memset(out,165,sizeof(out));if(qr_encode((coax_v_u8){data,413},(coax_v_u8){out,sizeof(out)}).status!=1)return8;if(qr_encode((coax_v_u8){out,70},(coax_v_u8){out,sizeof(out)}).status!=4)return9;for(unsigned i=0;i<sizeof(out);i++)if(out[i]!=165)return10;puts("20 grids and boundary guards passed");return0;}'''.replace('return0','return 0').replace('return1','return 1').replace('return2','return 2').replace('return3','return 3').replace('return4','return 4').replace('return5','return 5').replace('return6','return 6').replace('return7','return 7').replace('return8','return 8').replace('return9','return 9'))
  exe=DIRECTORY/'sanitized';run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined',driver,'-lm','-o',exe)
  self.assertIn('20 grids',run(exe,inputs));self.report['sanitizer']={'grids':20,'binary_sha256':sha(exe),'input_sha256':sha(inputs),'flags':['-O1','-g','-fsanitize=address,undefined']}
 def test_actual_wasm_grid_parity_in_both_modes(self):
  flags=['--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(DIRECTORY),'-mexec-model=reactor','-Wl,--export-memory','-lm'];reports=[]
  for name,_ in self.libs:
   wrapper=DIRECTORY/'wasm.c';wrapper.write_text((HERE/'wasm.c').read_text().replace('QR_SOURCE',name+'.c'))
   wasm=DIRECTORY/(name+'.wasm');run('/opt/homebrew/opt/llvm/bin/clang',*flags,wrapper,'-o',wasm)
   report=json.loads(run('bun',HERE/'wasm.cjs',wasm,HERE/'reference-2026-10-28.json'));self.assertEqual(report['grids'],20);reports.append({'mode':name,'artifact_sha256':sha(wasm),**report})
  self.report['wasm']=reports
if __name__=='__main__':unittest.main()
