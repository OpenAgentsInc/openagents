import os, subprocess, json, time, hashlib
from pathlib import Path
from datetime import datetime, timezone
root=Path('/Users/christopherdavid/work/openagents')
out=Path('/tmp/openagents-apple-handoff')
def utc(): return datetime.now(timezone.utc).isoformat()
def write(name,data): (out/name).write_text(json.dumps(data,indent=2)+'\n')
with (out/'final-gate-claim.json').open('x') as f: json.dump({'pid':os.getpid(),'started_utc':utc()},f)
env=dict(os.environ)
removed=['KEV_TEST_DEVICE','KEV_VARIANT','KEV_ARTIFACT_DIR','KEV_BASE_DIR','LEV_BRIDGE_ALLOW_UNSIGNED','LEV_BRIDGE_DEADLINE_MS']
for k in removed: env.pop(k,None)
env['CARGO_TARGET_DIR']='/tmp/openagents-supervision/root-target'
env['LEV_BRIDGE_BIN']=str(root/'swift/lev-bridge/.build/release/lev-bridge')
env['LEV_OS_BUILD']=subprocess.check_output(['sw_vers','-buildVersion'],text=True).strip()
command=['./scripts/verify-rust.sh','--with-metal']
r={'command':command,'source':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'git_status':subprocess.check_output(['git','status','--short'],cwd=root,text=True),'started_utc':utc(),'environment_removed':removed,'environment_overrides':{k:env[k] for k in ['CARGO_TARGET_DIR','LEV_BRIDGE_BIN','LEV_OS_BUILD']},'load_before':os.getloadavg()}
write('final-gate-progress.json',r)
start=time.monotonic()
with (out/'final-gate.log').open('x') as log:
 child=subprocess.Popen(command,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
 write('final-gate-pid.json',{'controller_pid':os.getpid(),'gate_pid':child.pid})
 r['exit_code']=child.wait()
r.update(finished_utc=utc(),seconds=time.monotonic()-start,load_after=os.getloadavg(),log_sha256=hashlib.sha256((out/'final-gate.log').read_bytes()).hexdigest())
write('final-gate-result.json',r)
