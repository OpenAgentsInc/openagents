#!/usr/bin/env python3
import argparse,json,os,signal,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--root',required=True,type=Path);p.add_argument('--stem',required=True,type=Path);a=p.parse_args()
a.stem.parent.mkdir(parents=True,exist_ok=True);start=time.monotonic();log=a.stem.with_suffix('.log')
with log.open('wb') as stream:
 child=subprocess.Popen(['coax/build.sh'],cwd=a.root,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
 try:code=child.wait(timeout=830)
 except subprocess.TimeoutExpired:
  os.killpg(child.pid,signal.SIGTERM)
  try:child.wait(timeout=5)
  except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
  result={'exit_status':'deadline','limit_seconds':830,'process_group_stopped':True,'elapsed_seconds':round(time.monotonic()-start,3),'root':str(a.root),'log':str(log)}
  a.stem.with_suffix('.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));raise SystemExit(1)
result={'exit_status':code,'elapsed_seconds':round(time.monotonic()-start,3),'root':str(a.root),'log':str(log)}
a.stem.with_suffix('.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));raise SystemExit(code)
