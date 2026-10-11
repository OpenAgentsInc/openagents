"""Build a frozen candidate and run only nonignored backend Clef tests."""
from pathlib import Path
import hashlib,json,os,shutil,subprocess,time
root=Path(__file__).resolve().parent
repo=Path('<WORKTREE>')
manifest=json.loads((root/'source-manifest.json').read_text())
def sha(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
def frozen():
 for row in manifest['files']:
  if sha(repo/row['path'])!=row['sha256']:raise RuntimeError('source changed: '+row['path'])
frozen()
env={k:v for k,v in os.environ.items() if not k.startswith('PSIONIC_CLEF_')}
env.update(CARGO_TARGET_DIR='<TARGET>',CARGO_BUILD_JOBS='2',RAYON_NUM_THREADS='2',OMP_NUM_THREADS='2',VECLIB_MAXIMUM_THREADS='2')
base=['cargo','+1.97.1']
commands=[('server',base+['build','--release','--manifest-path','crates/psionic/Cargo.toml','-p','psionic-serve','--bin','psionic-openai-server','--message-format=json-render-diagnostics','-j','2']),('backend-build',base+['test','--release','--manifest-path','crates/psionic/Cargo.toml','-p','psionic-backend-metal','--lib','--no-run','--message-format=json-render-diagnostics','-j','2'])]
rows=[];artifacts={}
for name,command in commands:
 frozen();start=time.monotonic();print('Starting '+name,flush=True)
 with (root/(name+'.log')).open('w') as log:
  result=subprocess.run(command,cwd=repo,env=env,stdout=log,stderr=subprocess.STDOUT)
 row={'name':name,'command':command,'exit':result.returncode,'seconds':time.monotonic()-start};rows.append(row)
 (root/'checks.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(row),flush=True)
 if result.returncode:raise SystemExit(result.returncode)
 matches=[]
 for line in (root/(name+'.log')).read_text().splitlines():
  try:obj=json.loads(line)
  except json.JSONDecodeError:continue
  expected='psionic-openai-server' if name=='server' else 'psionic_backend_metal'
  if obj.get('reason')=='compiler-artifact' and obj.get('target',{}).get('name')==expected and obj.get('executable'):
   if name=='server' or obj.get('profile',{}).get('test'):matches.append(Path(obj['executable']))
 if len(set(matches))!=1:raise RuntimeError('expected exactly one executable for '+name)
 artifacts[name]=matches[0];frozen()
command=[str(artifacts['backend-build']),'clef','--test-threads=1']
start=time.monotonic()
with (root/'backend-tests.log').open('w') as log:result=subprocess.run(command,cwd=repo,env=env,stdout=log,stderr=subprocess.STDOUT)
rows.append({'name':'backend-tests','command':command,'exit':result.returncode,'seconds':time.monotonic()-start,'ignored_GPU_tests_run':False})
(root/'checks.json').write_text(json.dumps(rows,indent=2)+'\n')
print('\n'.join((root/'backend-tests.log').read_text().splitlines()[-45:]),flush=True)
if result.returncode:raise SystemExit(result.returncode)
frozen();outputs={}
for name,source in artifacts.items():
 destination=root/('psionic-openai-server' if name=='server' else 'psionic-metal-tests')
 if destination.exists():raise RuntimeError('refuse replacing frozen binary')
 digest=sha(source);shutil.copy2(source,destination)
 if sha(destination)!=digest:raise RuntimeError('binary copy differs')
 outputs[name]={'name':destination.name,'source':str(source),'sha256':digest,'bytes':destination.stat().st_size,'source_manifest_unchanged':True}
(root/'binaries.json').write_text(json.dumps(outputs,indent=2)+'\n')
print(json.dumps({'status':'complete','binaries':outputs}),flush=True)
