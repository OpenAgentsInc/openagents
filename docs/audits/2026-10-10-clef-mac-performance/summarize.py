"""Summarize retained Mac benchmark samples without selecting minima."""
import collections,json,pathlib,statistics,sys
root=pathlib.Path(sys.argv[1])
rows=[json.loads(x) for x in (root/'records.jsonl').read_text().splitlines()]
requests=[r for r in rows if r['kind']=='request' and r['valid'] and not r['warmup']]
groups=collections.defaultdict(list)
for r in requests:groups[(r['variant'],pathlib.Path(r['workload']).name)].append(r)
summary=json.loads((root/'summary.json').read_text())
out={'source_status':summary['status'],'diagnostic_only':summary['diagnostic_only'],'groups':[]}
for (variant,workload),rs in groups.items():
 rounds=collections.defaultdict(list)
 for r in rs:rounds[r['round']].append(r['elapsed_seconds'])
 tokens=[r['response'].get('usage',{}).get('input_tokens') for r in rs]
 out['groups'].append({'variant':variant,'workload':workload,'samples':len(rs),'median_seconds':statistics.median(r['elapsed_seconds'] for r in rs),'round_medians':{k:statistics.median(v) for k,v in rounds.items()},'input_tokens':sorted(set(tokens))})
paired={}
for r in requests:paired.setdefault((r['round'],pathlib.Path(r['workload']).name,r['sample']),{})[r['variant']]=r
checks=[]
for key,vs in paired.items():
 if 'baseline' in vs and 'merged' in vs:
  a,b=vs['baseline'],vs['merged']
  checks.append({'round':key[0],'workload':key[1],'sample':key[2],'identical_request':a['request_sha256']==b['request_sha256'],'identical_answers':a['response']['answers']==b['response']['answers'],'same_input_tokens':a['usage']['input_tokens']==b['usage']['input_tokens'],'candidate_over_baseline':b['elapsed_seconds']/a['elapsed_seconds']})
out['paired_baseline_candidate']=checks
host=[json.loads(x) for x in (root/'host.jsonl').read_text().splitlines()]
out['host_phases']={}
for phase in sorted(set(h['phase'] for h in host)):
 hs=[h for h in host if h['phase']==phase]
 loads=[h['load_1_5_15'][0] for h in hs if 'load_1_5_15' in h]
 out['host_phases'][phase]={'samples':len(hs),'first_at':hs[0]['at'],'last_at':hs[-1]['at'],'min_load':min(loads),'max_load':max(loads),'gpu_percentages':sorted(set(h.get('agx_utilization',{}).get('Device Utilization %') for h in hs if h.get('agx_utilization'))) }
(root/'analysis.json').write_text(json.dumps(out,indent=2)+'\n')
for g in out['groups']:print(g['variant'],g['workload'],round(g['median_seconds'],4),g['round_medians'],g['input_tokens'])
if checks:print('Identical candidate answers:',sum(c['identical_answers'] for c in checks),'/',len(checks))
print('Source status:',out['source_status'])
