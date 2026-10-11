"""Validate every fixed latency sample before computing paired comparisons."""
import argparse,collections,hashlib,json,math,statistics,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parent
TOOLS=Path('<WORKTREE>/crates/psionic/fixtures/clef/tools')
sys.path.insert(0,str(TOOLS))
import mac_quiet_bench as bench
import image_parity
NAMES=['stage6-key32','candidate-qk128-key32','ollama']
PHASES=['encode_ms','queue_ms','prefill_ms','head_ms']
def require(ok,message):
 if not ok:raise ValueError(message)
def load(p):return json.loads(Path(p).read_text())
def unique(rows,key,expected):
 keys=[key(r) for r in rows]
 require(len(keys)==len(expected) and len(set(keys))==len(keys) and set(keys)==expected,'missing or duplicate sample coordinates')
def canonical_binary_hashes(pinned):
 result={}
 for alias,digest in pinned.items():
  canonical=str(Path(alias).resolve(strict=True))
  require(canonical not in result,'duplicate canonical binary identity')
  require(bench.file_sha(canonical)==digest,'pinned binary hash changed')
  result[canonical]=digest
 return result
def analyze(directory):
 config=bench.load_config(ROOT/'config.json');proof=load(ROOT/'provenance.json');summary=load(directory/'summary.json');metadata=load(directory/'metadata.json')
 binaries=canonical_binary_hashes(proof['binary_sha256'])
 require(summary['status']=='complete' and summary['measurement_mode']=='shared' and not summary['diagnostic_only'],'measurement did not complete as shared latency')
 require(metadata['config']==bench.uncached_config(config),'effective config differs')
 require(metadata['measurement_mode']=='shared' and not metadata['quiet_lease'] and not metadata['self_test'],'unexpected measurement mode')
 rows=[json.loads(x) for x in (directory/'records.jsonl').read_text().splitlines() if x]
 samples=[r for r in rows if r['kind']=='request'];workloads=config['requests']
 expected={(r,v,w,s) for r in range(2) for v in NAMES for w in workloads for s in range(6)}
 unique(samples,lambda r:(r['round'],r['variant'],r['workload'],r['sample']),expected)
 starts=[r for r in rows if r['kind']=='startup'];stops=[r for r in rows if r['kind']=='shutdown']
 order=[(r,v) for r in range(2) for v in (NAMES if r==0 else list(reversed(NAMES)))]
 require([(r['round'],r['variant']) for r in starts]==order,'startup order differs')
 require([(r['round'],r['variant']) for r in stops]==order,'shutdown order differs')
 for row in stops:
  require(row['owned_group_stopped'] and not any(row.get(k) for k in ['cleanup_error','memory_rejection','sampling_error']),'owned cleanup or resource check failed')
  require(row['max_rss_bytes']==36*1024**3 and row['minimum_available_memory_bytes']==12*1024**3,'guard limits differ')
 for row in starts:
  require(row['owned'] and row['status']=='ready' and row['startup_required_available_memory_bytes']==48*1024**3,'startup guard differs')
  variant=next(v for v in config['variants'] if v['name']==row['variant'])
  for k,v in variant['env'].items():require(row['effective_env'].get(k)==v,'effective environment differs')
 for row in metadata['variants']:
  variant=next(v for v in config['variants'] if v['name']==row['name'])
  require(row['binary_sha256']==binaries[str(Path(variant['binary']).resolve(strict=True))] and row['profile_mode']=='disabled','binary or profile mode differs')
 pairs=collections.defaultdict(dict)
 ranges={'len1k':(950,1200),'len4k':(3500,4500),'len16k':(15000,17000)}
 for row in samples:
  require(row['valid'] and row['warmup']==(row['sample']==0),'invalid sample or warmup label')
  require(isinstance(row['elapsed_seconds'],(int,float)) and math.isfinite(row['elapsed_seconds']) and row['elapsed_seconds']>0,'invalid elapsed time')
  require(row['input_sha256']==proof['input_sha256'][row['workload']],'original workload differs')
  base=load(row['workload']);state=base['state'] if isinstance(base['state'],str) else bench.encoded(base['state']).decode()
  body=dict(base,state=f"RUN {row['nonce']}\n\n{state}",model='clef-flash')
  require(row['request_sha256']==bench.sha(bench.encoded(body)),'reconstructed exact request differs')
  bench.validate_uncached_response(row['http_status'],row['response']);image_parity.validate_response(row['response'],body)
  tokens=row['response']['usage']['input_tokens'];lo,hi=ranges[Path(row['workload']).stem]
  require(type(tokens) is int and lo<=tokens<=hi,'actual token count outside fixed workload range')
  if row['variant']!='ollama':
   meta=row['response']['psionic'];require(meta['backend']=='metal' and meta['truncated_state_tokens']==0,'wrong backend or truncated state')
   require(row['response']['usage'].get('cached_input_tokens')==0,'native cache use')
   require(all(isinstance(meta['timing'].get(k),(int,float)) and math.isfinite(meta['timing'][k]) and meta['timing'][k]>=0 for k in PHASES),'missing or invalid serving phase time')
  pairs[row['round'],row['workload'],row['sample']][row['variant']]=row
 require(len(pairs)==36,'pair coverage differs')
 nonces=[];paired=[]
 for key,pair in sorted(pairs.items()):
  require(set(pair)==set(NAMES),'missing paired variant')
  require(len({r['nonce'] for r in pair.values()})==len({r['request_sha256'] for r in pair.values()})==len({r['usage']['input_tokens'] for r in pair.values()})==1,'paired bytes or token counts differ')
  nonces.append(pair[NAMES[0]]['nonce'])
  paired.append({'round':key[0],'workload':Path(key[1]).stem,'sample':key[2],'warmup':key[2]==0,'actual_tokens':pair[NAMES[0]]['usage']['input_tokens'],'seconds':{v:r['elapsed_seconds'] for v,r in pair.items()},'native_phase_ms':{v:{k:pair[v]['response']['psionic']['timing'][k] for k in PHASES} for v in NAMES[:2]}})
 require(len(set(nonces))==36,'nonce reused across samples')
 measured=[r for r in paired if not r['warmup']];groups=[]
 targets={'len1k':.35,'len4k':1.3,'len16k':7.5}
 for workload in ['len1k','len4k','len16k']:
  subset=[r for r in measured if r['workload']==workload];require(len(subset)==10,'measured workload coverage differs')
  medians={v:statistics.median(r['seconds'][v] for r in subset) for v in NAMES}
  groups.append({'workload':workload,'measured_pairs':10,'actual_tokens':sorted({r['actual_tokens'] for r in subset}),'median_seconds':medians,'candidate_over_stage6_paired_ratios':[r['seconds'][NAMES[1]]/r['seconds'][NAMES[0]] for r in subset],'candidate_over_ollama_paired_ratios':[r['seconds'][NAMES[1]]/r['seconds']['ollama'] for r in subset],'median_native_phase_ms':{v:{k:statistics.median(r['native_phase_ms'][v][k] for r in subset) for k in PHASES} for v in NAMES[:2]},'shared_machine_target_seconds':targets[workload],'candidate_meets_shared_machine_target':medians[NAMES[1]]<=targets[workload],'per_round_median_seconds':{str(i):{v:statistics.median(r['seconds'][v] for r in subset if r['round']==i) for v in NAMES} for i in range(2)}})
 return {'passed':True,'offline_analyzer_version':2,'reverified_canonical_binary_sha256':binaries,'scope':'shared-machine latency; actual token counts retained; no idle-machine or causal hardware claim','records_sha256':bench.file_sha(directory/'records.jsonl'),'requests':108,'measured_requests':90,'warmups':18,'owned_servers_stopped':6,'all_exact_request_bytes_and_actual_token_counts_pair':True,'native_prefix_and_answer_cache':'prefix disabled; all native cached counts zero; no cached timing','ollama_cache_limit':'unique leading nonces; internal prefix reuse is not directly observed when cached_input_tokens is absent','groups':groups,'paired_samples_including_warmups':paired,'peak_accounted_bytes':max(r['peak_accounted_bytes'] for r in stops),'lowest_available_memory_bytes':min(r['lowest_available_memory_bytes'] for r in stops)}
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('directory',type=Path);args=p.parse_args()
 try:result=analyze(args.directory)
 except Exception as e:result={'passed':False,'error_type':type(e).__name__,'error':str(e)}
 (args.directory/'analysis-v2.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2));raise SystemExit(0 if result['passed'] else 1)
