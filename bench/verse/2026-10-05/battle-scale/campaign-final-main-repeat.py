import hashlib,json,os,pathlib,subprocess,tempfile,time
p=pathlib.Path('bench/verse/2026-10-05/battle-scale')
def sha(path):
 h=hashlib.sha256()
 with open(path,'rb') as f:
  for b in iter(lambda:f.read(65536),b''):h.update(b)
 return h.hexdigest()
tracked=subprocess.check_output(['git','diff','HEAD','--','crates','scripts/bench/verse-delayed-route.py',':(exclude)**/*.md'])
new=[pathlib.Path('crates/verse/examples/battle_scale.rs'),*pathlib.Path('crates/verse/examples/common').glob('*.rs'),*pathlib.Path('scripts/bench/tests').glob('*.py')]
patch=tracked
for path in sorted(new):
 result=subprocess.run(['git','diff','--no-index','--','/dev/null',str(path)],stdout=subprocess.PIPE)
 if result.returncode not in (0,1):raise RuntimeError(path)
 patch+=result.stdout
patchpath=p/'implementation-final-main-repeat.patch';patchpath.write_bytes(patch)
source={'schema':'verse.battle.source.v1','base_revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'source_patch':patchpath.name,'source_patch_sha256':sha(patchpath),'scope':'Tracked engine, authority, client, example, and route changes plus new Rust drivers and route tests; documentation and receipts are excluded.'}
(p/'source-manifest-final-main-repeat.json').write_text(json.dumps(source,indent=2)+'\n')
exepath=pathlib.Path('/home/christopherdavid/work/openagents-target-agent1/debug/examples/battle_scale')
manifest={'schema':'verse.battle.execution.v1','source':source,'environment':{'VERSE_QUALITY':'low','VERSE_GPU_TIMING':'1','DISPLAY':'unset','WAYLAND_DISPLAY':'unset','HOME':'temporary directory per run'},'executables':{'battle_scale':sha(exepath)},'runs':[]}
manifestpath=p/'execution-manifest-final-main-repeat.json'
def run(mode,name,seconds):
 receipt=p/(name+'.json');cmd=[str(exepath),mode,str(receipt),str(seconds)];env=os.environ.copy();env.update(VERSE_QUALITY='low',VERSE_GPU_TIMING='1');env.pop('DISPLAY',None);env.pop('WAYLAND_DISPLAY',None)
 started=time.monotonic()
 with tempfile.TemporaryDirectory(prefix='verse-battle-home-') as home:
  env['HOME']=home
  with (p/(name+'.log')).open('w') as log:code=subprocess.run(cmd,env=env,stdout=log,stderr=subprocess.STDOUT).returncode
 record={'receipt':receipt.name,'executable':'battle_scale','argv':cmd,'exit_code':code,'wall_seconds':time.monotonic()-started,'status':'passed' if code==0 else 'failed'}
 if receipt.is_file():record['receipt_sha256']=sha(receipt)
 manifest['runs'].append(record);manifestpath.write_text(json.dumps(manifest,indent=2)+'\n');print(name,code,flush=True)
 return code==0
def isolated():
 exe=exepath.with_name('renderer_budget');manifest['executables']['renderer_budget']=sha(exe)
 receipt=p/'renderer-final-main-repeat.json';cmd=[str(exe),str(receipt),'300'];env=os.environ.copy();env.update(VERSE_QUALITY='low',VERSE_GPU_TIMING='1');env.pop('DISPLAY',None);env.pop('WAYLAND_DISPLAY',None)
 started=time.monotonic()
 with tempfile.TemporaryDirectory(prefix='verse-renderer-home-') as home:
  env['HOME']=home
  with (p/'renderer-final-main-repeat.log').open('w') as log:code=subprocess.run(cmd,env=env,stdout=log,stderr=subprocess.STDOUT).returncode
 failures=[]
 if code:failures.append('Renderer process failed')
 if not receipt.is_file():failures.append('Renderer receipt missing')
 else:
  result=json.loads(receipt.read_text());steady=result.get('measurements',{}).get('steady',{})
  if not result.get('all_required_visuals_preserved'):failures.append('Required actor roots or mounts absent')
  device=result.get('device',{})
  if device.get('quality')!='low' or not device.get('timestamp_query_enabled'):failures.append('Low quality and valid GPU timing required')
  for metric in ['draw_cpu_ms','gpu_scene_ms']:
   value=steady.get(metric,{})
   if value.get('samples',0)<120 or value.get('invalid',0) or value.get('p95',float('inf'))>1000/60:failures.append(metric+' missing, invalid, or above 16.667 ms')
 record={'receipt':receipt.name,'executable':'renderer_budget','argv':cmd,'exit_code':code,'wall_seconds':time.monotonic()-started,'status':'passed' if not failures else 'failed','acceptance_failures':failures}
 if receipt.is_file():record['receipt_sha256']=sha(receipt)
 manifest['runs'].append(record);manifestpath.write_text(json.dumps(manifest,indent=2)+'\n');print('renderer-final-main-repeat',record['status'],flush=True)
 return not failures
ok=True
for i in range(1,4):
 ok=run('combined',f'combined-final-main-repeat-{i:02}',60)
 if not ok:break
if ok:ok=isolated()
if ok:ok=run('authority','authority-final-main-repeat',600)
if ok:ok=run('network','network-final-main-repeat',60)
if ok:ok=run('combined','combined-final-main-repeat-soak',600)
else:manifest['soak']='not run because short acceptance gates failed'
manifest['aggregate_status']='passed' if ok else 'failed';manifestpath.write_text(json.dumps(manifest,indent=2)+'\n')
raise SystemExit(0 if ok else 1)
