import datetime,json,os,pathlib,signal,subprocess,time,urllib.request
root=pathlib.Path('/Users/christopherdavid/work/openagents'); out=pathlib.Path('/tmp/openagents-apple-handoff')
env=dict(os.environ,LEV_OS_BUILD='25E246',STATE_SWEEP_ROWS=str(out/'state-budget-lev-choice.jsonl'))
result={'source':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'started_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'load_before':os.getloadavg(),'retained_rows_before':0,'reason':'Measure the admitted choice adapter on every unchanged state-budget rung.'}
assert not (out/'state-budget-lev-choice.jsonl').exists(), 'Preserve existing measurement before another attempt'
started=time.monotonic();server=None;run=None
try:
 with (out/'state-sweep-choice-server.log').open('w') as log:
  server=subprocess.Popen(['/tmp/openagents-supervision/root-target/debug/lev-serve','--manifest','crates/lev/manifests/lev-adapted-v1.json','--port','11456','--policy-refresh','off'],cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  for _ in range(240):
   if server.poll() is not None:raise RuntimeError('server exited '+str(server.returncode))
   try:
    with urllib.request.urlopen('http://127.0.0.1:11456/v1/models',timeout=1) as response: models=json.load(response)
    card=models['models'][0]
    assert card['manifest']['release']=='lev-adapted@1' and card['samples']==8 and card['seed_base']==0, 'Unexpected measured door'
    (out/'state-sweep-choice-models.json').write_text(json.dumps(models,indent=2)+'\n');break
   except OSError:time.sleep(.5)
  else:raise RuntimeError('server startup deadline exceeded')
  with (out/'state-sweep-choice-run.log').open('w') as log:
   run=subprocess.Popen([str(out/'sweep-choice-target/debug/lev-state-sweep-choice')],cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
   (out/'state-sweep-choice-pid.json').write_text(json.dumps({'server_pid':server.pid,'run_pid':run.pid}))
   try:result['exit_code']=run.wait(timeout=10800)
   except subprocess.TimeoutExpired:
    run.terminate()
    try:run.wait(timeout=10)
    except subprocess.TimeoutExpired:run.kill();run.wait()
    result.update(exit_code=run.returncode,failure='three-hour overall deadline exceeded')
except Exception as error: result['failure']=str(error)
finally:
 if server is not None and server.poll() is None:
  os.killpg(server.pid,signal.SIGTERM)
  try:server.wait(timeout=10)
  except subprocess.TimeoutExpired:os.killpg(server.pid,signal.SIGKILL);server.wait()
 result.update(seconds=time.monotonic()-started,load_after=os.getloadavg(),finished_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
 (out/'state-sweep-choice-result.json').write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps(result),flush=True)

import sys
sys.exit(1 if result.get('failure') or result.get('exit_code') != 0 else 0)
