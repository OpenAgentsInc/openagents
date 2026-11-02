#!/usr/bin/env python3
# Firm reason: 2 (independent analytic sRGB and Python IEEE binary16/binary32 decoding oracle for native texel output).
"""Independent analytic/IEEE byte oracle for normal-C texture decoding."""
from __future__ import annotations
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import random
import re
import struct
import subprocess
import time
ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def bits(value):return struct.unpack('<I',struct.pack('<f',value))[0]

def cases():
 result=[]
 def add(fmt,payload,offset=0,length=None):result.append((fmt,payload,offset,len(payload)if length is None else length))
 for code in range(256):
  for fmt in range(4):add(fmt,bytes((code,255-code,code^165,code)))
 for half in range(65536):add(4,struct.pack('<4H',half,0x3c00,0xc000,0x3800))
 rng=random.Random(0x53475242)
 for _ in range(1024):add(5,struct.pack('<4I',*(rng.getrandbits(32)for _ in range(4))))
 for word in (0,0x80000000,1,0x3f000000,0x3f800000,0x3f800001,0xbf000000,0x7f800000,0xff800000,0x7fc00000):add(6,struct.pack('<I',word))
 for fmt in range(7):
  n=4 if fmt<4 or fmt==6 else 8 if fmt==4 else 16
  add(fmt,b'\0'*n,offset=-1)
  add(fmt,b'\0'*n,length=n-1)
  add(fmt,b'!'+b'\0'*n,offset=1)
 add(7,b'')
 return result

def expected(case):
 fmt,data,offset,length=case
 if fmt==7:return 1,[0]*4
 n=4 if fmt<4 or fmt==6 else 8 if fmt==4 else 16
 if offset<0 or offset+n>length:return 2,[0]*4
 payload=data[offset:offset+n]
 if fmt<4:
  values=list(payload)
  if fmt>=2:values[0],values[2]=values[2],values[0]
  colour=[value/255 for value in values]
  if fmt%2:
   colour[:3]=[v/12.92 if v<=0.04045 else((v+0.055)/1.055)**2.4 for v in colour[:3]]
 else:
  colour=list(struct.unpack('<4e'if fmt==4 else '<4f'if fmt==5 else '<f',payload))
  if not all(math.isfinite(value)for value in colour):return 3,[0]*4
  if fmt==6:
   if not 0<=colour[0]<=1:return 4,[0]*4
   colour=[colour[0],0,0,1]
 return 0,[bits(value)for value in colour]

def audit(data,expectations):
 assert len(data)==len(expectations)*20,'missing or extra texel fields'
 for i,(status,words)in enumerate(expectations):
  actual=struct.unpack_from('<5I',data,i*20)
  assert actual==(status,*words),(i,actual,(status,*words))

def main():
 ap=argparse.ArgumentParser();ap.add_argument('--work-dir',type=Path,required=True);ap.add_argument('--receipt',type=Path,required=True);ap.add_argument('--modes',nargs='+',choices=['debug','release','sanitized'],default=['debug','release','sanitized']);ap.add_argument('--explore-stale-compiler',action='store_true');a=ap.parse_args()
 started=time.monotonic();work=a.work_dir.resolve();work.mkdir(parents=True,exist_ok=True);compiler=ROOT/'coax/bin/coax'
 def run(argv,env=None):
  remaining=830-(time.monotonic()-started)
  if remaining<=0:raise TimeoutError('texture qualification reached its bounded deadline')
  r=subprocess.run(list(map(str,argv)),cwd=ROOT,env=env,capture_output=True,timeout=remaining)
  if r.returncode:raise RuntimeError(f'command failed{r.returncode}: {argv!r}\n'+r.stderr.decode(errors='replace')[-2000:])
  return r
 compiler_hash=sha(compiler);version=run([compiler,'version']).stdout.decode().strip();recorded=(compiler.parent/'source-digest').read_text().strip();current=run([compiler,'source-digest',ROOT]).stdout.decode().strip();stale=recorded!=current
 if stale and not a.explore_stale_compiler:raise RuntimeError('compiler source digests differ; rebuild before final qualification')
 fixture=HERE/'texture-codec-native.coax';deps=json.loads(run([compiler,'deps',fixture]).stdout);paths=[Path(x)for x in deps['sources']]+[HERE/'texture-codec-native.c',Path(__file__).resolve()];closure={str(p.relative_to(ROOT)):sha(p)for p in paths}
 if a.receipt.exists():
  old=json.loads(a.receipt.read_text())
  if old.get('source_sha256')==closure and old.get('compiler_sha256')==compiler_hash:raise RuntimeError('unchanged codec already qualified; retain its receipt')
 values=cases();expectations=[expected(case)for case in values];payload=work/'cases.bin';payload.write_bytes(b'TEX1'+struct.pack('<I',len(values))+b''.join(struct.pack('<IIq24s',fmt,length,offset,data)for fmt,data,offset,length in values));modes=[];emitted={}
 for mode in a.modes:
  release=mode=='release'
  if release not in emitted:
   part=work/('wrapping'if release else'checked');part.mkdir(exist_ok=True);header=part/'texture.h';r=run([compiler,'c',fixture,'--library','--header',header]+(['--release']if release else[]));source=part/'texture.c';source.write_bytes(r.stdout);(part/'emission.log').write_bytes(r.stderr)
   for name,fields in {'RenderTexelResult':['status','colour'],'RQColor':['r','g','b','a']}.items():
    match=re.search(r'struct coax_t_'+name+r' \{([^}]+)\};',header.read_text());assert match,name;observed=[re.search(r'\b(\w+)\s*$',item.strip())[1]for item in match[1].split(';')if item.strip()];assert observed==fields,(name,observed)
   emitted[release]=(source,header)
  source,header=emitted[release];exe=work/('texture-'+mode);flags=['-O2']if release else['-O0','-g']
  if mode=='sanitized':flags+=['-fsanitize=address,undefined','-fno-omit-frame-pointer']
  run(['clang','-std=c11',*flags,'-I',header.parent,source,HERE/'texture-codec-native.c','-lm','-o',exe]);env=dict(os.environ)
  if mode=='sanitized':env.update(ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1')
  result=run([exe,payload],env);audit(result.stdout,expectations);transcript=work/(mode+'.bin');transcript.write_bytes(result.stdout);faults=[]
  for fault in ('swap-rb','drop-alpha','clear-status'):
   broken=run([exe,payload,fault],env)
   try:audit(broken.stdout,expectations)
   except AssertionError:faults.append(fault)
   else:raise AssertionError('undetected transport fault '+fault)
  modes.append(dict(mode=mode,cases=len(values),faults=faults,transcript=str(transcript),transcript_sha256=sha(transcript),generated_c_sha256=sha(source),generated_header_sha256=sha(header),executable_sha256=sha(exe)))
 assert sha(compiler)==compiler_hash and closure=={str(p.relative_to(ROOT)):sha(p)for p in paths},'source/compiler changed during qualification'
 receipt=dict(schema='aethyr.render.texture-codec.native.v1',status='exploratory'if stale else'pass',compiler_version=version,compiler_sha256=compiler_hash,compiler_recorded_source_digest=recorded,compiler_current_source_digest=current,compiler_current_sources_match=not stale,source_sha256=closure,input_sha256=sha(payload),modes=modes,elapsed_seconds=round(time.monotonic()-started,3),scope='All65536 half words, all256 byte EOTF values in RGBA/BGRA linear/sRGB,1024 independent float-word colors, depth/offset/truncation cases; normal-C transport and scalar decode only. Batch atomic/layout proof is in sharp Coax checks; no GPU image claim.')
 a.receipt.parent.mkdir(parents=True,exist_ok=True);a.receipt.write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(dict(status=receipt['status'],cases=len(values),modes=[x['mode']for x in modes],receipt=str(a.receipt))))

if __name__=='__main__':main()
