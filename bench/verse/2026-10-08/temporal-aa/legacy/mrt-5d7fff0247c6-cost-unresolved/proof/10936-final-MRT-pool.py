from pathlib import Path
import hashlib,json,math,statistics
s=Path(__file__).parent; plan=json.loads((s/'10936-final-MRT-replication-plan.json').read_text())
stems=['10936-final-MRT-high-paired']+plan['additional_prefixes']; reports=[]; provenance=[]
for i,stem in enumerate(stems):
 m=json.loads((s/(stem+'-manifest.json')).read_text()); p=s/stem/'capture.json';r=json.loads(p.read_text())
 assert m['exit']==0 and m['source']==plan['runtime_source'] and m['features']==['capture'] and m['quality']=='high'
 assert r['temporal_texture_diagnostics']['enabled'] is False
 assert r['submission_timing']['submitted_frames']==r['submission_timing']['completed_frames']==960
 assert r['submission_timing']['temporal_baseline_submitted_frames']==r['submission_timing']['temporal_baseline_completed_frames']==960
 assert r['sequence_frames']==[439,484]
 if i: assert plan['registered_unix']<m['start_unix']
 for kind in ('gpu','quiet'):
  v=json.loads((s/(stem+'-'+kind+'.json')).read_text())
  assert v['resource']==kind and v['exit']==0 and v['held_whole_run'] and not v['nested']
  assert v['acquired_at_ms']<=m['start_unix']*1000<=m['end_unix']*1000<=v['released_at_ms']
 if reports: assert m['binary_sha256']==provenance[0]['binary_sha256'] and m['command'][2:]==provenance[0]['command'][2:]
 reports.append(r);provenance.append({'stem':stem,'source':m['source'],'binary_sha256':m['binary_sha256'],'command':m['command'],'report_sha256':hashlib.sha256(p.read_bytes()).hexdigest()})
phases={}
for phase in ('before','swarm','after'):
 rows=[r['temporal_comparison']['phases'][phase]['frame_results'] for r in reports]
 assert all([p['frame'] for p in v]==[p['frame'] for p in rows[0]] for v in rows)
 values=[[p['wall_render_completion_increment_ms'] for p in v] for v in rows]
 means=[statistics.mean(v) for v in values];mean=statistics.mean(means);half=4.303*statistics.stdev(means)/math.sqrt(3)
 blocks=[];counts=[]
 for v in values:
  n=math.ceil(len(v)/60)
  for j in range(n):
   part=v[j*len(v)//n:(j+1)*len(v)//n];blocks.append(statistics.mean(part));counts.append(len(part))
 total=sum(counts);weights=[n/total for n in counts];variance=sum(w*(b-mean)**2 for w,b in zip(weights,blocks));squared=sum(w*w for w in weights)
 df=len(blocks)-1;t={14:2.145,17:2.110}[df];blockhalf=t*math.sqrt(variance/(1-squared)*squared)
 phases[phase]={'samples_per_run':len(values[0]),'all_retained_samples':sum(map(len,values)),'run_means_ms':means,'primary_run_mean_95pct_ci_ms':{'mean':mean,'lower':mean-half,'upper':mean+half,'independent_runs':3,'degrees_of_freedom':2,'student_t_975':4.303},'primary_upper_under_1ms':mean+half<1,'supplementary_balanced_block_ci_ms':{'mean':mean,'lower':mean-blockhalf,'upper':mean+blockhalf,'blocks':len(blocks),'degrees_of_freedom':df},'per_run_bounds_ms':[r['temporal_comparison']['phases'][phase]['wall_render_completion_mean_95pct_ci_ms'] for r in reports],'valid_gpu_pairs':sum(r['temporal_comparison']['phases'][phase]['valid_gpu_frames'] for r in reports),'no_outliers_removed':True}
out={'schema':'openagents.verse.temporal-replication-result.v1','source':plan['runtime_source'],'registered_protocol_sha256':hashlib.sha256((s/'10936-final-MRT-replication-plan.json').read_bytes()).hexdigest(),'runs':provenance,'phases':phases,'strict_high_1ms_gate_pass':all(v['primary_upper_under_1ms'] for v in phases.values()),'method':'Fixed3 independent exact-command runs; primary Studentt df2 interval across equal-size per-phase run means. Single-run flags remain unchanged; all rows/outliers retained. Wall render completion includes encoding, submission, polling, mapping and readback; GPU duration is unavailable.'}
(s/'10936-final-MRT-high-pooled.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
