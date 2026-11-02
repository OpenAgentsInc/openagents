#!/usr/bin/env python3
"""Actual normal-C checked-tree/raster pipeline against authored pixel math.

This is a synthetic conformance corpus, not a realm screenshot qualification.
Every depth/RGBA/MRT word and unused pixel is independently observed. Source
hashes, actual compiler-emitted tree words and whole ABI inventories bind it.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import struct
import subprocess
import time

ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
FIELDS={'RQColor':['r','g','b','a'],'RRPixel':['depth','colour','aux'],
 'RRPassResult':['status','steps','triangles','quads','fragments','written']}
PAIRS={'plain':('rp_vertex','rp_fragment'),'helper':('rp_vertex','rp_helper_fragment'),
 'depth':('rp_vertex','rp_depth_fragment'),'depth_last':('rp_vertex','rp_depth_last_fragment'),'texture':('rp_vertex','rp_texture_fragment'),'vertex_only':('rp_vertex',''),'void':('rp_vertex','rp_void_fragment'),'masked_aux':('rp_noattrs_vertex','rp_position_fragment')}
FAULTS={'status':0,'work':6,'publication':0,'coverage':0,'colour':0,'depth':0,'aux':0,'vertex_colour':15,'vertex_depth':15,'void_depth':16,'masked_aux':17}

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def bits(value):return struct.unpack('<I',struct.pack('<f',value))[0]
def header_inventory(text):
 for name,fields in FIELDS.items():
  m=re.search(r'struct coax_t_'+name+r' \{([^}]+)\};',text);assert m,name
  actual=[re.search(r'\b(\w+)\s*$',x.strip())[1] for x in m[1].split(';') if x.strip()]
  assert actual==fields,(name,actual,fields)
def pair(scenario):return 'masked_aux' if scenario==17 else 'vertex_only' if scenario==15 else 'void' if scenario==16 else 'helper' if scenario==1 else 'depth_last' if scenario==14 else 'depth' if scenario in (3,4) else 'texture' if scenario in (10,11) else 'plain'

def oracle(scenario,short=False):
 clear=[1,3e38 if scenario==9 else .125,.25,.5,.75,.25,.5,.75,1]
 pixels=[list(clear) for _ in range(16)]
 if short:return [2,0,0,0,0],pixels,0
 if scenario in (5,6,7,8,9,11,12,13):
  # Status/counters are observable even when scratch work is discarded.
  summary={5:[5,2,2,4,0],6:[4,0,0,0,0],7:[5,0,0,0,0],8:[5,1,0,0,0],
   9:[5,1,1,1,0],11:[5,1,1,0,0],12:[3,0,0,0,0],13:[5,0,0,0,0]}[scenario]
  return summary,pixels,1 if scenario==6 else 0 if scenario==12 else None
 size=4 if scenario==2 else 2
 for y in range(size):
  for x in range(size):
   at=y*size+x;first=x+y<size-1
   # Pixel centres of the authored affine plane; the primitive's first
   # vertex supplies its flat tag, with the last drawn instance on top.
   value=(x+.5)*(4/size)+(y+.5)*(8/size)+1
   companion=9-value
   tag=(9 if first else 19)+(3 if scenario==2 else 2)
   if scenario==17:pixels[at]=[1,x+.5,y+.5,.5,1,*clear[5:]]
   elif scenario in (15,16):
    if scenario==15 or x==0:pixels[at][0]=.50000011920928955078125
   elif scenario==1:
    if x==1:continue
    pixels[at]=[.5,10,4,value,1,tag,x+.5,y+.5,1]
   elif scenario in (3,4,14):
    pixels[at]=[.25,.25 if scenario==4 else .75,.50000011920928955078125,companion,1,value,tag,x+.5,y+.5]
   elif scenario==10:pixels[at]=[.5,1.125,.25,.5,1,*clear[5:]]
   else:pixels[at]=[.5,value,4/size,-8/size,1,tag,x+.5,y+.5,companion]
 summary=[0,4,12,32,32] if scenario==2 else [0,2,2,2,2] if scenario in (1,16) else [0,2,2,4,4]
 return summary,pixels,None

def verify(data,scenario,short=False):
 expected,pixels,steps=oracle(scenario,short)
 result=data['result'];assert len(result)==6
 assert [result[0],*result[2:]]==expected,(scenario,'counters',result,expected)
 if steps is not None:assert result[1]==steps,(scenario,'exact work boundary',result[1],steps)
 else:assert 0<result[1]<=1000000,(scenario,'bounded work',result[1])
 expected_words=[[bits(x) for x in p] for p in pixels]
 assert data['pixels']==expected_words,(scenario,'pixel words differ')

def interrupted(signum,frame):
 raise RuntimeError(f'pipeline qualification interrupted by signal {signum}')

def run_process(argv,env=None,timeout=830):
 """Run one effect; cancellation and deadlines remove the whole owned group."""
 child=subprocess.Popen(list(map(str,argv)),cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env,start_new_session=True)
 try:out,err=child.communicate(timeout=timeout)
 except BaseException:
  try:os.killpg(child.pid,signal.SIGKILL)
  except ProcessLookupError:pass
  child.communicate(timeout=5)
  raise
 if child.returncode:raise RuntimeError(f'command failed {child.returncode}: {argv!r}\n'+err.decode(errors='replace')[-2400:])
 return out,err

def main():
 p=argparse.ArgumentParser();p.add_argument('--work-dir',required=True,type=Path);p.add_argument('--receipt',required=True,type=Path)
 p.add_argument('--modes',nargs='+',choices=['debug','release','sanitized'],default=['debug','release','sanitized'])
 p.add_argument('--explore-stale-compiler',action='store_true');a=p.parse_args()
 signal.signal(signal.SIGTERM,interrupted)
 started=time.monotonic();work=a.work_dir.resolve();work.mkdir(parents=True,exist_ok=True);compiler=ROOT/'coax/bin/coax'
 def run(argv,env=None):
  left=830-(time.monotonic()-started)
  if left<=0:raise TimeoutError('pipeline qualification bounded command deadline')
  return run_process(argv,env,timeout=left)
 compiler_hash=sha(compiler);version=run([compiler,'version'])[0].decode().strip()
 recorded=(compiler.parent/'source-digest').read_text().strip();current=run([compiler,'source-digest',ROOT])[0].decode().strip();stale=recorded!=current
 if stale and not a.explore_stale_compiler:raise RuntimeError('compiler source fingerprint differs; rebuild before qualification')
 fixture=HERE/'pipeline-native.coax';shader=HERE/'pipeline-shader.coax'
 sources=set()
 for entry in [fixture,HERE/'pipeline-shader-deps.coax']:sources.update(json.loads(run([compiler,'deps',entry,'--library'])[0])['sources'])
 sources.update([str(HERE/'pipeline-native.c'),str(Path(__file__).resolve())]);paths=sorted(map(Path,sources))
 closure={str(x.relative_to(ROOT)):sha(x) for x in paths}
 if a.receipt.exists():
  old=json.loads(a.receipt.read_text())
  if old.get('source_sha256')==closure and old.get('compiler_sha256')==compiler_hash:raise RuntimeError('unchanged profile already qualified; retain its receipt')
 tree_dir=work/'trees';tree_dir.mkdir(exist_ok=True);trees={};tree_records={}
 authored=(HERE/'reference-pipeline-checks.coax').read_text()+(HERE/'reference-pipeline-trees.coax').read_text()+(HERE/'reference-pipeline-vertex-only-checks.coax').read_text()
 constants={'plain':'RP_TEST_TREE','helper':'RP_HELPER_TREE','depth':'RP_DEPTH_TREE','depth_last':'RP_DEPTH_LAST_TREE','texture':'RP_TEXTURE_TREE','vertex_only':'RP_VERTEX_ONLY_TREE','void':'RP_VOID_TREE','masked_aux':'RP_NOATTR_TREE'}
 for name,(vs,fs) in PAIRS.items():
  command=[compiler,'spirv',shader,'--vertex',vs]+(['--fragment',fs] if fs else [])+['--tree']
  out,err=run(command);words=list(map(int,out.split()))
  match=re.search(constants[name]+r' :: \[(\d+)\]u32\.\[([^]]+)\];',authored);assert match,name
  literal=list(map(int,match[2].split(',')));assert len(literal)==int(match[1]) and words==literal,(name,'checked tree differs from actual compiler emission')
  tree=tree_dir/(name+'.txt');tree.write_bytes(out);(tree_dir/(name+'.log')).write_bytes(err);trees[name]=tree
  tree_records[name]=dict(vertex=vs,fragment=fs,version=words[0],words=len(words),sha256=sha(tree))
 emitted={};emitted_hashes={};records=[]
 for mode in a.modes:
  release=mode=='release'
  if release not in emitted:
   part=work/('wrapping' if release else 'checked');part.mkdir(exist_ok=True);header=part/'pipeline-native.h';source=part/'pipeline-native.c'
   out,err=run([compiler,'c',fixture,'--library','--header',header]+(['--release'] if release else []))
   source.write_bytes(out);(part/'emission.log').write_bytes(err);header_inventory(header.read_text());emitted[release]=(source,header);emitted_hashes[release]=(sha(source),sha(header))
  source,header=emitted[release];assert emitted_hashes[release]==(sha(source),sha(header)),'emitted C or header changed before reuse'
  exe=work/('pipeline-'+mode);flags=['-O2'] if release else ['-O0','-g']
  if mode=='sanitized':flags+=['-fsanitize=address,undefined','-fno-omit-frame-pointer']
  run(['clang','-std=c11','-ffp-contract=off','-Wall','-Wextra',*flags,'-I',header.parent,source,HERE/'pipeline-native.c','-lm','-o',exe]);env=dict(os.environ)
  if mode=='sanitized':env.update(ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1')
  transcript=[]
  for scenario in range(18):
   out,err=run([exe,trees[pair(scenario)],scenario],env=env);observed=json.loads(out);verify(observed,scenario);transcript.append(observed)
  out,err=run([exe,trees['plain'],0,'short'],env=env);observed=json.loads(out);verify(observed,0,short=True);transcript.append(observed)
  for fault,scenario in FAULTS.items():
   out,err=run([exe,trees[pair(scenario)],scenario,fault],env=env)
   try:verify(json.loads(out),scenario)
   except AssertionError:pass
   else:raise AssertionError(('undetected producer fault',mode,fault))
  target=work/(mode+'.json');target.write_text(json.dumps(transcript,indent=2)+'\n')
  records.append(dict(mode=mode,scenarios=19,pixel_words_observed=19*16*9,producer_faults_detected=FAULTS,transcript_sha256=sha(target),generated_c_sha256=sha(source),generated_header_sha256=sha(header),executable_sha256=sha(exe)))
 assert sha(compiler)==compiler_hash,'compiler changed during qualification'
 assert run([compiler,'source-digest',ROOT])[0].decode().strip()==current,'compiler source inputs changed during qualification'
 for release,(source,header) in emitted.items():assert emitted_hashes[release]==(sha(source),sha(header)),'emitted C or header changed during qualification'
 assert closure=={str(x.relative_to(ROOT)):sha(x) for x in paths},'sources changed during qualification'
 receipt=dict(schema='aethyr.render.pipeline.native.v1',status='exploratory' if stale else 'pass',compiler_version=version,compiler_sha256=compiler_hash,compiler_recorded_source_digest=recorded,compiler_current_source_digest=current,compiler_current_sources_match=not stale,source_sha256=closure,header_fields_checked=FIELDS,trees=tree_records,modes=records,elapsed_seconds=round(time.monotonic()-started,3),scope='Synthetic normal-C conformance: actual checked VS/FS trees, indexed/nonindexed/instanced rasterization, clipping/helpers/fine derivatives/demotion, depth-first/last explicit outputs/MRT, vertex-only biased depth, present void-FS demotion and omitted masked auxiliary output, resource mapping and atomic refusal. All nine scalar words in every pixel and the unused tail are observed. Not full-realm GPU image qualification.')
 a.receipt.parent.mkdir(parents=True,exist_ok=True);a.receipt.write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(dict(status=receipt['status'],scenarios=19,modes=a.modes,receipt=str(a.receipt))))
if __name__=='__main__':main()
