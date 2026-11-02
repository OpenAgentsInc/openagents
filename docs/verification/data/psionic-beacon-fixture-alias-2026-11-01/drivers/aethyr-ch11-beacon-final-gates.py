from pathlib import Path
import subprocess,hashlib,json,os,signal,time
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');binary=Path('/Users/christopherdavid/work/openagents-aethyr-check-budgets-current/coax/bin/coax');out=Path('/tmp/aethyr-ch11-beacon-alias/final');out.mkdir(exist_ok=True);sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();inputs=[root/'psionic/lib/pylon/beacon.coax',root/'psionic/lib/pylon/template.coax',root/'coax/lib/nostr/json.coax',root/'psionic/tests/pylon/beacon_alias_probe.coax',root/'psionic/tests/pylon/beacon_alias_checks.coax'];before={str(p):sha(p)for p in inputs};compiler=sha(binary);assert compiler=='1fac3750bcffbb735ef26f4a065d16414b911068fd7d68bc3ca112565d2432aa';results=[];start=time.monotonic()
def run(args,name):
 command=[str(binary),*args];began=time.monotonic();p=subprocess.Popen(command,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True)
 try:stdout,_=p.communicate(timeout=max(1,805-(time.monotonic()-start)))
 except BaseException:
  os.killpg(p.pid,signal.SIGTERM)
  try:p.wait(timeout=3)
  except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait(timeout=3)
  raise
 (out/(name+'.log')).write_bytes(stdout);r={'command':command,'returncode':p.returncode,'seconds':round(time.monotonic()-began,3)};results.append(r);print(json.dumps(r),flush=True);assert p.returncode==0
run(['test','psionic/lib/pylon/beacon.coax','--library','--evidence',str(out/'checks.evidence.json')],'checks')
run(['mutate','psionic/lib/pylon/beacon.coax','--library','--root-only','--jobs','4','--evidence',str(out/'mutation.evidence.json')],'mutation')
changes=[p for p,h in before.items()if sha(Path(p))!=h];assert not changes and sha(binary)==compiler
q={'schema':'aethyr.beacon.fixture-alias-final-gates.v1','status':'pass','compiler_sha256':compiler,'source_sha256':before,'source_changes':changes,'results':results,'seconds':round(time.monotonic()-start,3),'scope':'Productbeacon functions unchanged; selected fixture owner corrected. Full imported baseline and root-entry mutations run without equivalence marks.'};(out/'gates.json').write_text(json.dumps(q,indent=2)+'\n');print('PASS',flush=True)
