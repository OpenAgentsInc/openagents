#!/usr/bin/env python3
# Firm reason: 2 (independent unbounded-integer endpoint oracle, compared with the generated native Coax ABI).
"""Independent mathematical range oracle through the actual normal-C ABI.

Python integers do not wrap. A span is admitted by mathematical endpoints,
independently of the subtraction guards used by the shipped Coax policy.
This qualifies frame ranges and transport, not arbitrary shader instructions.
"""
from __future__ import annotations
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import random
import re
import struct
import subprocess
import time

ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
U32=2**32; U64=2**64
FIELDS={
 "FrameCounts":"width height hud_vertices vertex_stride draw_count water_draws grass_instances particle_instances surface_vertices caster_vertices caster_instances".split(),
 "RenderFrameLimits":"max_width max_height max_draws max_uploads max_vertex_stride index_work".split(),
 "RenderDirectDomains":"hud_vertices grass_instances particle_instances surface_vertices caster_vertices caster_instances".split(),
 "RenderUpload":"source_address source_base source_bytes bytes gpu_offset gpu_bytes alignment".split(),
 "RenderDrawDomain":"vertex_begin vertices vertex_alt_begin vertex_alt_end instance_begin instances indices".split(),
 "RenderFrameResult":"status bad_upload bad_draw bad_direct checked_indices admitted_draws".split(),
 "RenderDecodedDraw":"kind first count base_instance instances".split(),
}
FAULTS=["status","slot","work","publication","preflight","decode"]

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def header_inventory(header):
 for name,fields in FIELDS.items():
  match=re.search(r"struct coax_t_"+name+r" \{([^}]+)\};",header)
  assert match,name
  actual=[re.search(r"\b(\w+)\s*$",x.strip())[1] for x in match[1].split(";") if x.strip()]
  assert actual==fields,(name,actual,fields)

def base():
 return dict(c=[320,180,0,0,0,0,0,0,0,0,0],l=[16384,16384,4096,48,64,16777216],
  d=[0]*6,u=[],w=[],g=[],i=[],record=0)
def limits_ok(l):return all(a<=x<=b for x,a,b in zip(l,[1,1,0,0,0,0],[65536,65536,65536,48,64,16777216]))
def upload_ok(u):
 a,b,n,count,offset,capacity,alignment=u
 return (alignment in [4,8,16,32,64,128,256] and b+n<U64 and b<=a<=b+n
  and a+count<=b+n and 0<=offset<=capacity and offset+count<=capacity
  and a%alignment==0 and offset%alignment==0 and count%4==0)
def result(status=0,upload=-1,draw=-1,direct=-1,checked=0,admitted=0):return [status,upload,draw,direct,checked,admitted]
def preflight(v):
 if not limits_ok(v['l']):return result(1)
 if len(v['u'])>v['l'][3]:return result(4)
 for k,u in enumerate(v['u']):
  if not upload_ok(u):return result(5,upload=k)
 return result()
def decoded(v):
 k=v['record'];w=v['w']
 if k<0 or k>=len(w)//4:return [3,0,0,0,0]
 first,count,base_instance,instances=w[k*4:k*4+4]
 kind=0 if count==0 or instances==0 else (2 if base_instance>=2**31 else 1)
 if kind==2:base_instance-=2**31
 return [kind,first,count,base_instance,instances]
def interval(a,n,lo,hi):return lo<=a and a+n<=hi
def vertex(g,a,n):return interval(a,n,g[0],g[1]) or interval(a,n,g[2],g[3])
def admit(v):
 c,l,d=v['c'],v['l'],v['d']
 if not limits_ok(l):return result(1)
 if not (1<=c[0]<=l[0] and 1<=c[1]<=l[1]):return result(2)
 count_fields=[2,4,5,6,7,8,9,10]
 if (any(not 0<=c[x]<U32 for x in count_fields) or c[5]>c[4]
  or not 0<=c[3]<=l[4] or (c[2]>0 and c[3]==0)):return result(3)
 for k,requested in enumerate([c[x] for x in [2,6,7,8,9,10]]):
  if d[k]>U32:return result(7,direct=k)
  if requested>d[k]:return result(10,direct=k)
 if c[4]>l[2] or len(v['w'])//4<c[4] or len(v['g'])<c[4]:return result(4)
 memory=preflight(v)
 if memory[0]:return memory
 checked=active=0
 for k in range(c[4]):
  kind,first,count,instance,instances=decoded(dict(v,record=k))
  if kind==0:continue
  g=v['g'][k]
  if not (g[0]<=g[1]<=U32 and g[2]<=g[3]<=U32 and g[4]<g[5]<=U32 and g[6]<=U32):return result(7,draw=k,checked=checked)
  if not interval(instance,instances,g[4],g[5]):return result(6,draw=k,checked=checked)
  if kind==2:
   if not vertex(g,first,count):return result(6,draw=k,checked=checked)
  else:
   if first+count>min(g[6],len(v['i'])):return result(6,draw=k,checked=checked)
   for index in v['i'][first:first+count]:
    if checked==l[5]:return result(9,draw=k,checked=checked)
    checked+=1
    if not vertex(g,index,1):return result(8,draw=k,checked=checked)
  active+=1
 return result(checked=checked,admitted=active)

def cases():
 out=[base()]
 def put(v):out.append(copy.deepcopy(v))
 # Each scalar has its own boundary probe; signed counts and unsigned ends
 # are transported as their actual declared width.
 for k,ends in enumerate([[-1,0,1,65536,65537],[-1,0,1,65536,65537],[-1,0,65536,65537],[-1,0,48,49],[-1,0,64,65],[-1,0,16777216,16777217]]):
  for value in ends:v=base();v['l'][k]=value;put(v)
 for k in range(11):
  for value in [-1,0,1,U32-1,U32]:
   v=base();v['c'][k]=value;v['d']=[U32]*6;put(v)
 for k in range(6):
  for cap in [0,1,U32,U32+1,U64-1]:
   v=base();v['d'][k]=cap;v['c'][[2,6,7,8,9,10][k]]=1;v['c'][3]=4;put(v)
 for alignment in [0,1,2,3,4,8,16,32,64,128,256,512,U64-1]:
  v=base();v['u']=[[256,0,512,12,256,512,alignment]];put(v)
 for k in range(7):
  for value in [0,1,3,4,8,12,16,32,64,256,512,U64-4,U64-1]:
   v=base();v['u']=[[16,8,24,12,4,16,4]];v['u'][0][k]=value;put(v)
 v=base();v['u']=[[0,0,0,0,0,0,4]]*49;put(v)
 v=base();v['u']=[[4,0,4,0,4,4,4],[U64-1,U64-1,1,0,0,0,4]];put(v)
 # Full encoded instance prefixes, separated procedural vertices, exact
 # upper endpoints and index bytes vs initialized GPU index domains.
 for nonindexed in [False,True]:
  for first in [0,1,2,3,9,10,11,U32-1]:
   for count in [0,1,2,3,U32-1]:
    for instance in [0,2,3,7,2**31-1]:
     v=base();v['c'][4]=1;v['w']=[first,count,instance+(2**31 if nonindexed else 0),1]
     v['g']=[[0,3,10,12,2,8,3]];v['i']=[0,10,2];put(v)
 for k in range(7):
  for value in [0,1,2,3,4,U32-1,U32,U32+1,U64-1]:
   v=base();v['c'][4]=1;v['w']=[0,3,0,1];v['g']=[[0,3,10,12,0,1,3]];v['i']=[0,2,1];v['g'][0][k]=value;put(v)
 for count in [0,1,2,3,4]:
  v=base();v['c'][4]=2;v['l'][5]=count;v['w']=[0,2,2**31,1,0,3,0,1];v['g']=[[0,3,0,0,0,1,3]]*2;v['i']=[0,2,1];put(v)
 for value in [-1,0,1,2,2**63-1]:
  v=base();v['record']=value;v['w']=[1,2,2**31+7,3,8,0,9,10,99];put(v)
 # Deterministic mixed-frame corpus. The oracle examines actual referenced
 # index values rather than trusting any producer's maximum.
 rng=random.Random(0xA37E11)
 for _ in range(1200):
  v=base();n=rng.randrange(5);v['c'][4]=n;v['c'][5]=rng.randrange(n+1)
  v['l'][5]=rng.randrange(20);v['record']=rng.randrange(-1,n+2)
  v['i']=[rng.randrange(20) for _ in range(rng.randrange(17))]
  for j in range(n):
   nonindexed=rng.choice([0,2**31]);v['w'] += [rng.randrange(19),rng.randrange(7),nonindexed+rng.randrange(5),rng.randrange(4)]
   v['g'].append([0,rng.randrange(12),14,rng.randrange(14,21),0,rng.randrange(5),rng.randrange(19)])
  if rng.randrange(3)==0:v['u']=[[0,0,64,rng.randrange(18)*4,0,64,4]]
  if rng.randrange(8)==0:v['g']=v['g'][:-1]
  if rng.randrange(8)==0:v['w']=v['w'][:-1]
  put(v)
 return out

def encode(values):
 data=bytearray(struct.pack('<I',len(values)))
 for v in values:
  data+=struct.pack('<11q6q6Q4Iq',*v['c'],*v['l'],*v['d'],len(v['u']),len(v['w']),len(v['g']),len(v['i']),v['record'])
  for row in v['u']:data+=struct.pack('<7Q',*row)
  data+=struct.pack('<'+'I'*len(v['w']),*v['w'])
  for row in v['g']:data+=struct.pack('<7Q',*row)
  data+=struct.pack('<'+'I'*len(v['i']),*v['i'])
 return bytes(data)

def main():
 p=argparse.ArgumentParser();p.add_argument('--work-dir',required=True,type=Path);p.add_argument('--receipt',required=True,type=Path)
 p.add_argument('--coax',type=Path,default=ROOT/'coax/bin/coax')
 p.add_argument('--modes',nargs='+',choices=['debug','release','sanitized'],default=['debug','release','sanitized'])
 p.add_argument('--explore-stale-compiler',action='store_true');a=p.parse_args()
 start=time.monotonic();work=a.work_dir.resolve();work.mkdir(parents=True,exist_ok=True);compiler=a.coax.resolve(strict=True)
 def run(argv,input=None,env=None):
  left=830-(time.monotonic()-start)
  if left<=0:raise TimeoutError('bounded frame qualification deadline')
  r=subprocess.run(list(map(str,argv)),input=input,cwd=ROOT,capture_output=True,env=env,timeout=left)
  if r.returncode:raise RuntimeError(f'command failed {r.returncode}: {argv!r}\n'+r.stderr.decode(errors='replace')[-2400:])
  return r
 compiler_hash=sha(compiler);version=run([compiler,'version']).stdout.decode().strip()
 recorded=(compiler.parent/'source-digest').read_text().strip();current=run([compiler,'source-digest',ROOT]).stdout.decode().strip();stale=recorded!=current
 if stale and not a.explore_stale_compiler:raise RuntimeError('compiler source fingerprint differs; rebuild before qualification')
 deps=json.loads(run([compiler,'deps',HERE/'frame-native.coax']).stdout)
 paths=[Path(x) for x in deps['sources']]+[HERE/'frame-native.c',Path(__file__).resolve()]
 closure={str(x.relative_to(ROOT)):sha(x) for x in paths}
 if a.receipt.exists():
  old=json.loads(a.receipt.read_text())
  if old.get('source_sha256')==closure and old.get('compiler_sha256')==compiler_hash:raise RuntimeError('unchanged profile already qualified; retain prior receipt')
 values=cases();payload=encode(values);expected=[admit(v)+preflight(v)+decoded(v) for v in values]
 (work/'inputs.bin').write_bytes(payload);(work/'expected.json').write_text(json.dumps(expected)+'\n')
 records=[];emitted={}
 for mode in a.modes:
  release=mode=='release'
  if release not in emitted:
   part=work/('wrapping' if release else 'checked');part.mkdir(exist_ok=True);header=part/'frame.h';source=part/'frame.c'
   r=run([compiler,'c',HERE/'frame-native.coax','--library','--header',header]+(['--release'] if release else []))
   source.write_bytes(r.stdout);(part/'emission.log').write_bytes(r.stderr);header_inventory(header.read_text());emitted[release]=(source,header)
  source,header=emitted[release];exe=work/('frame-'+mode);flags=['-O2'] if release else ['-O0','-g']
  if mode=='sanitized':flags+=['-fsanitize=address,undefined','-fno-omit-frame-pointer']
  run(['clang','-std=c11',*flags,'-I',header.parent,source,HERE/'frame-native.c','-lm','-o',exe]);env=dict(os.environ)
  if mode=='sanitized':env.update(ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1')
  transcript=run([exe],input=payload,env=env).stdout
  actual=[[int(x) for x in line.split()] for line in transcript.splitlines()]
  if actual!=expected:
   mismatch=next((i for i in range(min(len(actual),len(expected))) if actual[i]!=expected[i]),-1)
   raise AssertionError((mode,mismatch,values[mismatch],actual[mismatch],expected[mismatch]))
  (work/(mode+'.txt')).write_bytes(transcript)
  for fault in FAULTS:
   broken=run([exe,fault],input=payload,env=env).stdout
   assert broken!=transcript,('missed producer fault',mode,fault)
  records.append(dict(mode=mode,cases=len(values),producer_faults_detected=FAULTS,transcript_sha256=hashlib.sha256(transcript).hexdigest(),generated_c_sha256=sha(source),generated_header_sha256=sha(header),executable_sha256=sha(exe)))
 assert sha(compiler)==compiler_hash,'compiler changed during qualification'
 assert closure=={str(x.relative_to(ROOT)):sha(x) for x in paths},'source changed during qualification'
 receipt=dict(schema='aethyr.render.frame.native.v1',status='exploratory' if stale else 'pass',compiler_version=version,compiler_sha256=compiler_hash,compiler_recorded_source_digest=recorded,compiler_current_source_digest=current,compiler_current_sources_match=not stale,source_sha256=closure,header_fields_checked=FIELDS,input_sha256=hashlib.sha256(payload).hexdigest(),expected_sha256=sha(work/'expected.json'),cases=len(values),modes=records,elapsed_seconds=round(time.monotonic()-start,3),scope='Mathematical integer endpoint oracle for whole-frame ranges, typed normal-C transport, four-u32 Draw and atomic admission/refusal. Borrowed views themselves must be valid; geometry/profile admission and arbitrary shader data access are separate checks.')
 a.receipt.parent.mkdir(parents=True,exist_ok=True);a.receipt.write_text(json.dumps(receipt,indent=2)+'\n')
 print(json.dumps(dict(status=receipt['status'],cases=len(values),modes=a.modes,receipt=str(a.receipt))))
if __name__=='__main__':main()
