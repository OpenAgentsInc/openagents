import pathlib,json,statistics,hashlib,sys
root=pathlib.Path(sys.argv[1]);rows=[json.loads(line) for line in (root/'records.jsonl').read_text().split('\n') if line]
summary=json.loads((root/'summary.json').read_text());assert summary['status']=='complete'
requests=[r for r in rows if r['kind']=='request'];assert all(r['valid'] for r in requests)
measured=[r for r in requests if not r['warmup']];groups={};pairs={}
for r in measured:
 key=(r['round'],r['variant'],pathlib.Path(r['workload']).stem);groups.setdefault(key,[]).append(r)
 pairs.setdefault((r['round'],pathlib.Path(r['workload']).stem,r['sample']),{})[r['variant']]=r
variants=sorted(set(r['variant'] for r in measured));paired=[]
for key,group in sorted(pairs.items()):
 assert sorted(group)==variants
 assert len({r['nonce'] for r in group.values()})==1
 assert len({r['request_sha256'] for r in group.values()})==1
 assert len({r['usage']['input_tokens'] for r in group.values()})==1
 paired.append(dict(round=key[0],workload=key[1],sample=key[2],input_tokens=next(iter(group.values()))['usage']['input_tokens'],seconds={k:v['elapsed_seconds'] for k,v in group.items()}))
def timings(rs):
 result={}
 for name in ['encode_ms','queue_ms','prefill_ms','head_ms']:
  vals=[r['response'].get('psionic',{}).get('timing',{}).get(name) for r in rs]
  if all(v is not None for v in vals):result[name]=statistics.median(vals)
 return result
per_round=[dict(round=k[0],variant=k[1],workload=k[2],count=len(v),median_seconds=statistics.median(r['elapsed_seconds'] for r in v),median_reported_phase_ms=timings(v)) for k,v in sorted(groups.items())]
shutdown=[r for r in rows if r['kind']=='shutdown'];assert all(r['owned_group_stopped'] and not r['cleanup_error'] and not r['memory_rejection'] for r in shutdown)
result=dict(source_records_sha256=hashlib.sha256((root/'records.jsonl').read_bytes()).hexdigest(),status='complete',conditions='shared machine; no task-owned build or second model workload; no quiet lease',measured_requests=len(measured),warmups=len(requests)-len(measured),all_actual_token_counts_pair_exactly=True,all_request_bytes_pair_exactly=True,native_prefix_and_answer_cache='disabled; every native response reports zero cached_input_tokens and no cached timing',ollama_cache_reporting='cached_input_tokens is absent; unique state nonces prevent repeated full-request answers, but internal prefix reuse is not directly observed',per_round=per_round,paired_samples=paired,owned_servers_stopped=len(shutdown),peak_accounted_bytes=max(r['peak_accounted_bytes'] for r in shutdown),lowest_available_memory_bytes=min(r['lowest_available_memory_bytes'] for r in shutdown),qualification='Phase timings are ordinary serving timers, not profiling-mode GPU timings. Summaries do not establish idle-machine acceptance or a causal head speedup.')
(root/'analysis.json').write_text(json.dumps(result,indent=2)+'\n')
for g in per_round:print(g['round'],g['variant'],g['workload'],round(g['median_seconds'],6),g['median_reported_phase_ms'])
