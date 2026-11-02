# Firm reason: 2 (independent stock mutation shard execution and receipt).
from pathlib import Path
import sys,json,hashlib,subprocess,time,os,signal
D=Path('/tmp/coax-12625-origins-closure-scopes/CB_query-v6');C=Path('/tmp/coax-12625-native/final-v4/coax');part=sys.argv[1];O=D/('shard-'+part);O.mkdir();cmd=[str(C),'mutate',str(D/'scope.coax'),'--root-only','--shard',part+'/4','--jobs','2','--evidence',str(O/'evidence.json')];start=time.monotonic();timeout=False
with (O/'stdout').open('wb') as so,(O/'stderr').open('wb') as se:
 p=subprocess.Popen(cmd,cwd=D,stdout=so,stderr=se,start_new_session=True);(O/'launch.json').write_text(json.dumps({'pid':p.pid,'pgid':p.pid,'argv':cmd})+'\n')
 try:rc=p.wait(timeout=840)
 except subprocess.TimeoutExpired:
  timeout=True;os.killpg(p.pid,signal.SIGTERM)
  try:rc=p.wait(timeout=5)
  except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);rc=p.wait()
def sha(raw):return hashlib.sha256(raw).hexdigest()
s=json.loads((D/'setup.json').read_text());row={'argv':cmd,'exit_status':rc,'timeout':timeout,'seconds':time.monotonic()-start,'pins_unchanged':all(sha((D/f).read_bytes())==h for f,h in s['pins'].items()),'compiler_unchanged':sha(C.read_bytes())==s['compiler_sha256'],'stdout_sha256':sha((O/'stdout').read_bytes()),'stderr_sha256':sha((O/'stderr').read_bytes())};(O/'process.json').write_text(json.dumps(row,indent=2)+'\n');print(json.dumps(row));print((O/'stdout').read_text()[-6500:]);print((O/'stderr').read_text()[-1500:])
