from pathlib import Path
import concurrent.futures,hashlib,json,os,re,subprocess,time,signal,sys
root=Path('/Users/christopherdavid/work/openagents-aethyr-book'); work=Path('/tmp/aethyr-ch11-everglade-water-spray-build');work.mkdir(exist_ok=True)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def build(app):
 started=time.monotonic(); admitted=work/'admitted'/app;out=work/(app+'.wasm')
 files=list(root.rglob('*.coax'))+list((root/'apps/openagents-web').glob('*.[ch]'))+list((root/'coax/web').glob('*.[ch]'))+[root/'coax/web/wasm.sh',root/'coax/web/toolchain.sh',root/'verse/native/build.sh']
 before={str(p.relative_to(root)):sha(p)for p in files}; compiler=sha(root/'coax/bin/coax')
 libraries=subprocess.check_output(['verse/native/build.sh','--libraries',app],cwd=root,text=True).splitlines()
 command=['coax/web/wasm.sh',f'apps/openagents-web/{app}.coax',f'apps/openagents-web/{app}-face.c',str(out),*libraries]
 env=dict(os.environ,COAXIAL_ADMITTED_DIR=str(admitted));env.pop('COAXIAL_RENDER',None)
 try:
  child=subprocess.Popen(command,cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
  try:
   stdout,stderr=child.communicate(timeout=810)
  except subprocess.TimeoutExpired:
   os.killpg(child.pid,signal.SIGTERM)
   try: stdout,stderr=child.communicate(timeout=5)
   except subprocess.TimeoutExpired:
    os.killpg(child.pid,signal.SIGKILL); stdout,stderr=child.communicate()
   (work/(app+'.log')).write_bytes(stdout+stderr)
   raise RuntimeError('owned build process group stopped at810second deadline')
  p=subprocess.CompletedProcess(command,child.returncode,stdout,stderr)
  (work/(app+'.log')).write_bytes(p.stdout+p.stderr)
  deps=set()
  for source in admitted.glob('*.c'):
   preamble=source.read_text().split('static const int coax_source_ranges_count')[0][:50000]
   deps.update(re.findall(r'\{\d+,\d+,\d+,\d+,"([^"]+)"\}',preamble))
  deps.update(k for k in before if not k.endswith('.coax'))
  changes=[k for k in deps if k not in before or sha(root/k)!=before[k]]
  record={'app':app,'status':'pass'if p.returncode==0 and not changes else'fail','command':command,'environment':{'COAXIAL_RENDER':'default auto (unset)','COAXIAL_ADMITTED_DIR':str(admitted)},'elapsed_seconds':round(time.monotonic()-started,3),'returncode':p.returncode,'compiler_sha256':compiler,'source_changes':changes,'inputs':{k:before[k]for k in sorted(deps)if k in before},'emitted':{str(f.relative_to(admitted)):sha(f)for f in admitted.glob('*')},'wasm_sha256':sha(out)if out.exists()else None}
 except Exception as e:record={'app':app,'status':'fail','error':str(e)}
 (work/(app+'.json')).write_text(json.dumps(record,indent=2)+'\n'); print(app,record['status'],flush=True);return record
result=build(sys.argv[1])
if result['status']!='pass': raise SystemExit(1)
