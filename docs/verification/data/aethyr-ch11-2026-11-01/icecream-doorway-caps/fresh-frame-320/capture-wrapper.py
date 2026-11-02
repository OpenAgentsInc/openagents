from pathlib import Path
import hashlib,json,os,signal,subprocess,time
root=Path('/Users/christopherdavid/work/openagents-aethyr-book-main');out=Path('/tmp/aethyr-ch11-icecream-doorway-main-half-320');out.mkdir(exist_ok=False)
args=['/Users/christopherdavid/.bun/bin/bun','aethyr/tests/check_render_host.ts','/tmp/aethyr-ch11-icecream-doorway-main-7c9289b4-producer/icecream.wasm','/tmp/aethyr-ch11-packages-current/ice-cream-man','apps/openagents-web/realms/ice-cream-man/index.html',str(out),'--half','--start','--require-draws','--size=320x180']
start=time.monotonic();child=None;q={'command':args,'deadline_seconds':180,'status':'failed'}
try:
 with(out/'capture.log').open('wb')as log:
  child=subprocess.Popen(args,cwd=root,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  q['exit']=child.wait(timeout=180)
 assert q['exit']==0
 q['status']='pass'
finally:
 if child is not None and child.poll()is None:
  os.killpg(child.pid,signal.SIGKILL);child.wait()
 q['elapsed_seconds']=time.monotonic()-start;q['log_sha256']=hashlib.sha256((out/'capture.log').read_bytes()).hexdigest();(out/'command.json').write_text(json.dumps(q,indent=2)+'\n')
print(json.dumps(q))
