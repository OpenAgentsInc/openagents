"""Launch the reviewed fixed Flash timing plan only under its owned leases."""
import hashlib,json,os,signal,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parent
TOOLS=Path('<WORKTREE>/crates/psionic/fixtures/clef/tools')
sys.path.insert(0,str(TOOLS))
import mac_quiet_bench as bench
import analyze
proof=json.loads((ROOT/'provenance.json').read_text());config=bench.load_config(ROOT/'config.json')
leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','))
if not {'gpu','memory'}<=leases or 'quiet' in leases:raise SystemExit('GPU and memory leases, without quiet, required')
if (ROOT/'run-receipt.json').exists() or Path(config['output_dir']).exists():raise SystemExit('refuse replacing a timing attempt')
record={'status':'preflight','config_sha256':bench.file_sha(ROOT/'config.json'),'provenance_sha256':bench.file_sha(ROOT/'provenance.json'),'source_sha256':{n:bench.file_sha(ROOT/n) for n in ['run.py','analyze.py','PLAN.md']},'preparation_manifest_sha256':bench.file_sha(ROOT/'preparation-manifest.json'),'deadline_seconds':1800,'preflight_hash_deadline_seconds':120,'conditions':'shared-machine latency; caller must coordinate a window with no task-owned build, CPU model or GPU model overlap'}
def save():(ROOT/'run-receipt.json').write_text(json.dumps(record,indent=2)+'\n')
start=time.monotonic();save();previous={}
def interrupted(signum,_):raise TimeoutError('fixed latency deadline reached') if signum==signal.SIGALRM else KeyboardInterrupt(f'signal {signum}')
def check(path,digest,size=None):
 p=Path(path)
 if size is not None and p.stat().st_size!=size:raise ValueError('artifact size changed: '+str(p))
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):
   if time.monotonic()-start>120:raise TimeoutError('artifact preflight exceeded 120 seconds')
   h.update(b)
 if h.hexdigest()!=digest:raise ValueError('artifact hash changed: '+str(p))
try:
 frozen=json.loads((ROOT/'preparation-manifest.json').read_text())
 analyze.require(set(frozen)=={'config.json','provenance.json','run.py','analyze.py','PLAN.md'},'preparation file set changed')
 for name,digest in frozen.items():check(ROOT/name,digest)
 analyze.require(record['config_sha256']==proof['config_sha256'],'config changed')
 analyze.require((config['rounds'],config['runs'],config['max_rss_gib'],config['minimum_available_memory_gib'],config['startup_timeout'],config['request_timeout'])==(2,5,36,12,180,300),'fixed resources/counts changed')
 for category in ['harness','binary_sha256','input_sha256','correctness_gate_sha256']:
  for path,digest in proof[category].items():check(path,digest)
 for category in ['candidate_source_manifest','candidate_patch','native_model','ollama_manifest']:
  e=proof[category];check(e['path'],e['sha256'],e.get('bytes'))
 unique={e['path']:e for e in proof['ollama_blobs']}
 for e in unique.values():check(e['path'],e['sha256'],e['bytes'])
 record.update(status='running',preflight_hash_seconds=time.monotonic()-start,verified_ollama_blobs=len(unique));save()
 os.environ.update(RAYON_NUM_THREADS='2',OMP_NUM_THREADS='2',VECLIB_MAXIMUM_THREADS='2',OPENBLAS_NUM_THREADS='2')
 for sig in [signal.SIGTERM,signal.SIGINT,signal.SIGALRM]:previous[sig]=signal.signal(sig,interrupted)
 signal.setitimer(signal.ITIMER_REAL,1800)
 summary=bench.run(config)
 signal.setitimer(signal.ITIMER_REAL,0)
 analyze.require(summary['status']=='complete','latency harness did not complete')
 result=analyze.analyze(Path(config['output_dir']));(Path(config['output_dir'])/'analysis.json').write_text(json.dumps(result,indent=2)+'\n')
 record.update(status='complete',analysis_passed=result['passed'])
except BaseException as e:record.update(status='failed',error_type=type(e).__name__,error=str(e))
finally:
 signal.setitimer(signal.ITIMER_REAL,0)
 for sig,handler in previous.items():signal.signal(sig,handler)
 record['seconds']=time.monotonic()-start;save()
print(json.dumps(record,indent=2));raise SystemExit(0 if record['status']=='complete' else 1)
