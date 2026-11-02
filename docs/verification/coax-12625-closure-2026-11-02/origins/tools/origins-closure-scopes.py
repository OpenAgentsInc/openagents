# Firm reason: 2 (independent exact-source mutation scope and receipt oracle).
from pathlib import Path
import re,shutil,hashlib,json,sys,subprocess,time,os,signal
ROOT=Path('/tmp/openagents-12625-origins-closure-20261102').resolve();BASE=Path('/tmp/coax-12625-origins-closure-scopes');COMPILER=Path('/tmp/coax-12625-native/final-v4/coax')
def sha(raw):return hashlib.sha256(raw).hexdigest()
def span(text,name):
 m=re.search(r'^'+re.escape(name)+r' :: ',text,re.M);assert m,name
 end=re.search(r'^(?:[A-Za-z_]\w* :: |#test|#forall)',text[m.end():],re.M)
 return m.start(),m.end()+end.start() if end else len(text)
def prepare(name,version):
 d=BASE/(name+'-'+version);d.mkdir(parents=True);shutil.copytree(ROOT/'coax/lib',d/'coax/lib')
 f=d/'coax/lib/compiler/checker.coax';t=f.read_text();a,b=span(t,name);body=t[a:b];f.write_text(t[:a]+t[b:])
 tests=''
 for filename in ['results-origin-policy.coax','results-borrow-policy.coax']:
  tests+=(ROOT/'coax/tests'/filename).read_text().replace('#load "../lib/compiler/checker.coax";','').replace('#authority;','')+'\n'
 (d/'scope.coax').write_text('#authority;\n'+tests+'\n'+body+'\n#load "coax/lib/compiler/checker.coax";\n')
 pins={str(p.relative_to(d)):sha(p.read_bytes()) for p in d.rglob('*.coax')}
 meta={'schema':1,'scope':name,'source_root':str(ROOT),'source_head':'92d8efaafd','adaptation':'Exact one production procedure moved to root, no edits. New origin endpoint checks first, original results-borrow endpoint checks and all checker/dependency baseline checks retained. Stock root-only mutation; no mutator/filter/quota changes.','production_body_sha256':sha(body.encode()),'pins':pins,'compiler_sha256':sha(COMPILER.read_bytes()),'jobs':2}
 (d/'setup.json').write_text(json.dumps(meta,indent=2)+'\n');return d
if __name__=='__main__':
 name=sys.argv[1];version=sys.argv[2];d=prepare(name,version);out=d/'observation';out.mkdir();cmd=[str(COMPILER),'mutate',str(d/'scope.coax'),'--root-only','--jobs','2','--evidence',str(out/'evidence.json')];start=time.monotonic()
 with (out/'stdout').open('wb') as so,(out/'stderr').open('wb') as se:
  p=subprocess.Popen(cmd,cwd=d,stdout=so,stderr=se,start_new_session=True);(out/'launch.json').write_text(json.dumps({'argv':cmd,'pid':p.pid,'pgid':p.pid})+'\n');timeout=False
  try:rc=p.wait(timeout=840)
  except subprocess.TimeoutExpired:
   timeout=True;os.killpg(p.pid,signal.SIGTERM)
   try:rc=p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);rc=p.wait()
 setup=json.loads((d/'setup.json').read_text());record={'argv':cmd,'exit_status':rc,'timeout':timeout,'seconds':time.monotonic()-start,'pins_unchanged':all(sha((d/p).read_bytes())==h for p,h in setup['pins'].items()),'compiler_unchanged':sha(COMPILER.read_bytes())==setup['compiler_sha256'],'stdout_sha256':sha((out/'stdout').read_bytes()),'stderr_sha256':sha((out/'stderr').read_bytes())};(out/'process.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'scope':name,**record}));print((out/'stdout').read_text()[-6500:]);print((out/'stderr').read_text()[-1800:])
