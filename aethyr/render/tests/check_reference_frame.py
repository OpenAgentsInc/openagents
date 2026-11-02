#!/usr/bin/env python3
"""Bounded effects-only capture/package transport for the Coax CPU frame.

A successful measurement is not a CPU/GPU image qualification. Coax owns
package/tree/resource/geometry admission, graph execution and all pixels.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import time
from check_pipeline import ROOT,HERE,sha,run_process,interrupted


def admitted_backend(directory,compiler_hash,compiler_source,root=ROOT):
 """Reuse a completed checked-C emission, never mere artifact presence."""
 directory=directory.resolve();old=json.loads((directory/'measurement.json').read_text())
 if old.get('status') not in ('rendered','refused') or old.get('source_changes') or old.get('compiler_changed') or old.get('artifact_changes') or old.get('input_changes'):
  raise ValueError('backend receipt did not finish with stable inputs')
 if not old.get('compiler_sources_match') or old['compiler_sha256']!=compiler_hash or old['compiler_source_digest']!=compiler_source:
  raise ValueError('backend compiler identity differs')
 sources={name:digest for name,digest in old['source_sha256'].items()if name.endswith('.coax')}
 if 'aethyr/render/tests/reference-frame-native.coax' not in sources:raise ValueError('backend receipt is for another Coax entry')
 for name,digest in sources.items():
  if sha(root/name)!=digest:raise ValueError('backend Coax source changed: '+name)
 artifacts={kind:Path(old.get(kind+'_path',directory/filename))for kind,filename in [('generated_c','reference-frame.c'),('generated_header','reference-frame.h'),('executable','reference-frame')]}
 for kind,path in artifacts.items():
  if sha(path)!=old[kind+'_sha256']:raise ValueError('backend artifact changed: '+kind)
 return old,sources,artifacts



def reuse_native_executable(old,mode,before):
 """A checked emission can be rebuilt for sanitizers; wrapping is distinct."""
 if (old.get('c_mode','checked')=='wrapping')!=(mode=='release'):
  raise ValueError('backend checked/wrapping C profile differs')
 driver_inputs=['aethyr/render/tests/reference-frame-native.c','aethyr/render/tests/frame-input.h']
 return old.get('native_mode','checked-o2')==mode and all(before[name]==old['source_sha256'].get(name)for name in driver_inputs)


def frame_config_paths(text):
 """Inventory the shared RFI whitespace-token grammar; C owns admission."""
 tokens=text.split();at=0;rows=[];refs=[];tree_rows=[]
 def take(n):
  nonlocal at
  if n<0 or at+n>len(tokens):raise ValueError('truncated frame transport')
  row=tokens[at:at+n];at+=n;rows.append(row);return len(rows)-1
 def number():
  j=take(1);n=int(rows[j][0])
  if not 0<=n<=len(tokens):raise ValueError('truncated transport count')
  return n
 take(13);counts=list(map(int,rows[0]))
 for _ in range(5):refs.append((take(1),0))
 for _ in range(number()):
  j=take(2);refs.append((j,1));tree_rows.append(j)
 for _ in range(number()):refs.append((take(8),1))
 while at<len(tokens):
  label=tokens[at]
  if label=='textures':
   j=take(2);n=int(rows[j][1])
   if not 0<=n<=len(tokens):raise ValueError('invalid textures count')
   for _ in range(n):
    row=take(5);levels=int(rows[row][3]);refs.append((row,4))
    if not 0<=levels<=len(tokens):raise ValueError('invalid texture level count')
    for _ in range(levels):take(5)
  elif label=='media_images':
   j=take(2);n=int(rows[j][1])
   if not 0<=n<=len(tokens):raise ValueError('invalid media image count')
   for _ in range(n):take(5)
  elif label=='media_text':refs.append((take(2),1))
  else:raise ValueError('unknown transport suffix')
 return rows,refs,counts,tree_rows


def snapshot_frame_config(source,package_path,inputs,config):
 text=source.read_text();rows,refs,counts,tree_rows=frame_config_paths(text);files={};copies={}
 def original(token):return (source.parent/token).resolve()
 if sha(original(rows[5][0]))!=sha(package_path):raise ValueError('recorded package differs from requested package')
 for row,column in refs:
  token=rows[row][column]
  if token=='-':continue
  path=original(token)
  if path not in copies:
   data=path.read_bytes();name=sha(path)[:16]+'-'+path.name;dest=inputs/name;dest.write_bytes(data)
   if sha(dest)!=sha(path):raise ValueError('input changed during snapshot: '+str(path))
   copies[path]=dest;files[str(path)]=dest
  rows[row][column]=str(copies[path])
 capture_copy=inputs/'original-frame.config';capture_copy.write_text(text);config.write_text('\n'.join(' '.join(row)for row in rows)+'\n')
 package_copy=Path(rows[5][0]);tree_files=[(int(rows[j][0]),Path(rows[j][1]))for j in tree_rows]
 return files,package_copy,tree_files,capture_copy,counts


def main():
 parser=argparse.ArgumentParser();parser.add_argument('--capture',type=Path);parser.add_argument('--frame-config',type=Path);parser.add_argument('--frame-provenance',type=Path);parser.add_argument('--package',type=Path,required=True);parser.add_argument('--work-dir',type=Path,required=True);parser.add_argument('--coarse',action='store_true');parser.add_argument('--prepare-only',action='store_true');parser.add_argument('--reuse-backend',type=Path);parser.add_argument('--raster-rounding',choices=['analytic','nearest-even','nearest-positive']);parser.add_argument('--subpixel-bits',type=int,default=0);parser.add_argument('--raster-provenance',type=Path);parser.add_argument('--xy-clipping',choices=['viewport','guard-band'],default='viewport');parser.add_argument('--guard-pixels',type=int,default=0);parser.add_argument('--screen-format',choices=['rgba8','rgba16f'],default='rgba8');parser.add_argument('--work-profile',choices=['standard','large-world','complex-world'],default='standard');parser.add_argument('--filter-profile',choices=['analytic','metal-unorm8','nvidia-unorm8-integer'],default='analytic');parser.add_argument('--filter-provenance',type=Path);parser.add_argument('--mode',choices=['checked-o2','debug','release','sanitized','sanitized-o1'],default='checked-o2');a=parser.parse_args()
 if bool(a.capture)==bool(a.frame_config):parser.error('provide exactly one of --capture or --frame-config')
 if a.frame_config and a.coarse:parser.error('--coarse cannot change a recorded frame.config')
 if not a.prepare_only and a.raster_rounding is None:parser.error('--raster-rounding is required for rendering')
 if not a.prepare_only and a.filter_profile!='analytic' and a.filter_provenance is None:parser.error('measured --filter-profile requires --filter-provenance')
 initial_tool_sources={str(path.resolve()):sha(path)for path in [Path(__file__),HERE/'frame-input.h']}
 signal.signal(signal.SIGTERM,interrupted);started=time.monotonic();work=a.work_dir.resolve();work.mkdir(parents=True,exist_ok=True);compiler=ROOT/'coax/bin/coax'
 commands=[]
 def run(argv,**kwargs):
  left=830-(time.monotonic()-started)
  if left<=0:raise TimeoutError('CPU frame measurement command deadline')
  commands.append(list(map(str,argv)));return run_process(argv,timeout=left,**kwargs)
 compiler_hash=sha(compiler);compiler_source=run([compiler,'source-digest',ROOT])[0].decode().strip();recorded=(compiler.parent/'source-digest').read_text().strip()
 package_path=a.package.resolve();package=json.loads(package_path.read_text());profile=package['profile_index'];inputs=work/'inputs';inputs.mkdir(exist_ok=True)
 assert package['compiler_sha256']==compiler_hash,'package and frame compiler identities differ'
 if a.frame_config:
  config=work/'frame.config';files,package_copy,tree_files,capture_copy,counts=snapshot_frame_config(a.frame_config.resolve(),package_path,inputs,config);assert counts[0]==profile,'recorded profile differs from package';width,height=counts[1:3]
 else:
  capture_path=a.capture.resolve();capture=json.loads(capture_path.read_text())
  assert capture['schema']=='aethyr.render.producer-capture.v1'
  files={}
  for name,info in capture['files'].items():
   assert Path(name).name==name and not any(c.isspace() for c in name)
   source=capture_path.parent/name;assert source.stat().st_size==info['bytes'] and sha(source)==info['sha256']
   dest=inputs/name;dest.write_bytes(source.read_bytes());files[name]=dest
  capture_copy=inputs/'capture.json';capture_copy.write_bytes(capture_path.read_bytes());package_copy=inputs/'render.json';package_copy.write_bytes(package_path.read_bytes());tree_files=[]
  for entry in package['entries']:
   source=package_path.parent/entry['tree_file'];assert sha(source)==entry['tree_sha256'];dest=inputs/entry['tree_file'];dest.write_bytes(source.read_bytes());tree_files.append((entry['entry_index'],dest))
  c=capture['counts'];width,height=map(int,capture['options']['size'].split('x'));counts=[profile,width,height,c['hud'],capture['vertex_stride'],c['draws'],c['water'],c['grass'],c['particles'],c['surface'],c['caster_vertices'],c['caster_instances'],int(a.coarse)]
  text=' '.join(map(str,counts))+'\n'
  for name in ['u.bin','hud.bin','draws.bin','index.bin']:text+=str(files.get(name,'-'))+'\n'
  text+=str(package_copy)+'\n'+str(len(tree_files))+'\n'+''.join(f'{index} {path}\n'for index,path in tree_files)
  observations={s['file']:s for s in capture['stores']}
  if capture['particles'].get('file'):observations[capture['particles']['file']]=capture['particles']
  names=[name for name in files if name.endswith('.bin') and not name.endswith('.upload.bin')];text+=str(len(names))+'\n'
  for name in names:
   n=files[name].stat().st_size;observed=observations.get(name)
   # These are copied producer observations, not resource-role decisions.
   values=[observed['capacity'],observed['retained_bytes'],observed['highwater'],observed['last_update_bytes'],observed['physical_bytes'],1] if observed else [n,n,n,0,n,1]
   text+=name+' '+str(files[name])+' '+' '.join(map(str,values))+'\n'
  config=work/'frame.config';config.write_text(text)
 provenance_copy=None
 if a.frame_provenance:provenance_copy=inputs/'frame-provenance.json';provenance_copy.write_bytes(a.frame_provenance.read_bytes())
 raster_provenance_sha=sha(a.raster_provenance)if a.raster_provenance else None
 filter_provenance_sha=sha(a.filter_provenance)if a.filter_provenance else None
 if a.prepare_only:
  tool_changes=[name for name,digest in initial_tool_sources.items()if sha(Path(name))!=digest]
  record=dict(schema='aethyr.render.frame-input-preparation.v1',status='prepared',qualification=False,scope='Authenticated immutable producer/package transport only; no Coax renderer compilation, frame admission or execution.',compiler_sha256=compiler_hash,compiler_source_digest=compiler_source,compiler_sources_match=recorded==compiler_source,profile=profile,slug=package['slug'],width=width,height=height,capture_sha256=sha(capture_copy),package_sha256=sha(package_copy),config_sha256=sha(config),source_sha256=initial_tool_sources,source_changes=tool_changes,frame_provenance_sha256=sha(provenance_copy)if provenance_copy else None,inputs={str(path):sha(path)for path in files.values()},trees={str(path):sha(path)for _,path in tree_files},commands=commands)
  if tool_changes:record['status']='inputs-changed'
  (work/'preparation.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({k:record[k]for k in ['status','slug','width','height']}))
  if tool_changes:raise RuntimeError('preparation tool inputs changed')
  return
 fixture=HERE/'reference-frame-native.coax';old=None;reused_emission=None;reused_executable=None
 if a.reuse_backend:
  old,coax_sources,artifacts=admitted_backend(a.reuse_backend,compiler_hash,compiler_source)
  deps=[ROOT/name for name in coax_sources];source=artifacts['generated_c'];header=artifacts['generated_header'];reused_emission=str(a.reuse_backend.resolve()/'measurement.json')
 else:
  deps=json.loads(run([compiler,'deps',fixture])[0])['sources'];source=work/'reference-frame.c';header=work/'reference-frame.h'
 source_paths=[Path(x)for x in deps]+[Path(__file__),HERE/'check_pipeline.py',HERE/'reference-frame-native.c',HERE/'frame-input.h'];before={str(p.resolve().relative_to(ROOT)):sha(p)for p in source_paths}
 if old is None:
  out,err=run([compiler,'c',fixture,'--library','--header',header]+(['--release']if a.mode=='release'else []));source.write_bytes(out);(work/'emission.log').write_bytes(err)
 if old is not None and reuse_native_executable(old,a.mode,before):exe=artifacts['executable'];reused_executable=reused_emission
 else:
  exe=work/'reference-frame';flags=['-O1','-g']if a.mode=='sanitized-o1'else ['-O0','-g']if a.mode in ['debug','sanitized']else ['-O2']
  if a.mode.startswith('sanitized'):flags+=['-fsanitize=address,undefined','-fno-omit-frame-pointer']
  out,err=run(['clang','-std=c11',*flags,'-ffp-contract=off','-Wall','-Wextra','-I',header.parent,source,HERE/'reference-frame-native.c','-lm','-o',exe]);(work/'compile.log').write_bytes(out+err)
 artifact_before={str(path):sha(path)for path in [source,header,exe]};input_before={str(path):sha(path)for path in set(files.values())|{config,package_copy,capture_copy}|{path for _,path in tree_files}|({provenance_copy}if provenance_copy else set())}
 image=work/'screen.rgba32f';result_path=work/'frame.json';environment=dict(os.environ)
 if a.mode.startswith('sanitized'):environment.update(ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1')
 render_start=time.monotonic();out,err=run([exe,config,image,result_path,a.raster_rounding,str(a.subpixel_bits),a.xy_clipping,str(a.guard_pixels),a.screen_format,a.work_profile,a.filter_profile],env=environment);render_seconds=time.monotonic()-render_start;(work/'render.log').write_bytes(out+err);result=json.loads(result_path.read_text())
 input_changes=[name for name,digest in input_before.items()if sha(Path(name))!=digest];artifact_changes=[name for name,digest in artifact_before.items()if sha(Path(name))!=digest];after={str(p.resolve().relative_to(ROOT)):sha(p)for p in source_paths};changed=[name for name in before if before[name]!=after[name]];compiler_changed=sha(compiler)!=compiler_hash or run([compiler,'source-digest',ROOT])[0].decode().strip()!=compiler_source
 if a.raster_provenance and sha(a.raster_provenance)!=raster_provenance_sha:input_changes.append(str(a.raster_provenance))
 if a.filter_provenance and sha(a.filter_provenance)!=filter_provenance_sha:input_changes.append(str(a.filter_provenance))
 receipt=dict(schema='aethyr.render.cpu-frame-measurement.v1',status='rendered'if result['status']==0 else'refused',qualification=False,scope='Actual captured producer frame and authenticated package trees through normal-C Coax reference frame; no CPU/GPU image comparison yet.',compiler_sha256=compiler_hash,compiler_source_digest=compiler_source,compiler_sources_match=recorded==compiler_source,native_mode=a.mode,sanitizers=['address','undefined']if a.mode.startswith('sanitized')else[],leak_detection=False,c_mode='wrapping'if a.mode=='release'else'checked',compiler_changed=compiler_changed,source_sha256=before,source_changes=changed,package_sha256=sha(package_copy),capture_sha256=sha(capture_copy),profile=profile,work_profile=a.work_profile,filter_profile=dict(name=a.filter_profile,provenance_file=str(a.filter_provenance.resolve())if a.filter_provenance else None,provenance_sha256=filter_provenance_sha),screen_format=a.screen_format,raster_profile=dict(rounding=a.raster_rounding,subpixel_bits=a.subpixel_bits,xy_clipping=a.xy_clipping,guard_pixels=a.guard_pixels,provenance_file=str(a.raster_provenance.resolve())if a.raster_provenance else None,provenance_sha256=raster_provenance_sha),slug=package['slug'],width=width,height=height,render_seconds=round(render_seconds,6),elapsed_seconds=round(time.monotonic()-started,6),result=result,emission_reused_from=reused_emission,executable_reused_from=reused_executable,artifact_changes=artifact_changes,input_sha256=input_before,input_changes=input_changes,frame_provenance_file=str(a.frame_provenance.resolve())if a.frame_provenance else None,frame_provenance_sha256=sha(provenance_copy)if provenance_copy else None,generated_c_path=str(source.resolve()),generated_header_path=str(header.resolve()),executable_path=str(exe.resolve()),generated_c_sha256=artifact_before[str(source)],generated_header_sha256=artifact_before[str(header)],executable_sha256=artifact_before[str(exe)],screen_sha256=sha(image),screen_bytes=image.stat().st_size,commands=commands)
 if changed or compiler_changed or artifact_changes or input_changes:receipt['status']='inputs-changed'
 (work/'measurement.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({k:receipt[k]for k in ['status','slug','render_seconds','elapsed_seconds','result','source_changes','compiler_changed']}))
 if changed or compiler_changed or artifact_changes or input_changes:raise RuntimeError('measurement inputs changed during execution')

if __name__=='__main__':main()
