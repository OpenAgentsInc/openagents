import json, os, signal, subprocess, time
from pathlib import Path
s=Path(__file__).parent
root=Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents')
pid=55287
expected='/bin/bash /Users/christopherdavid/.openagents/dev-host/checkout/scripts/desktop/dev-host.sh follow-build'
actual=subprocess.check_output(['ps','-p',str(pid),'-o','args='],text=True).strip()
assert actual==expected and os.getpgid(pid)==pid, (actual, 'process group changed')
record={'schema':'openagents.verse.capture-resource-pause.v1','source':'fb9a5fd280db673926cfd0d649f924da3091a24b','process_group':pid,'leader_command':actual,'reason':'Unleased automatic development-host compiler; suspend its build group only while the registered final capture batch runs. Running host is a separate process group.','pause_unix':time.time(),'resume_unix':None,'batch_exit':None}
path=s/'10936-final-copy-background-build-pause.json'
assert not path.exists()
os.killpg(pid,signal.SIGSTOP)
path.write_text(json.dumps(record,indent=2)+'\n')
print('Automatic compiler group paused',flush=True)
try:
 for name in ['10936-final-copy-all-cases.py','10936-final-copy-replicate-cases.py']:
  result=subprocess.run(['python3',str(s/name)],cwd=root)
  if result.returncode:
   record['batch_exit']=result.returncode
   raise SystemExit(result.returncode)
 record['batch_exit']=0
finally:
 os.killpg(pid,signal.SIGCONT)
 record['resume_unix']=time.time()
 path.write_text(json.dumps(record,indent=2)+'\n')
 print('Automatic compiler group resumed',flush=True)
