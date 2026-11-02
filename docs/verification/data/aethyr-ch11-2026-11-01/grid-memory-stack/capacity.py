from pathlib import Path
import json,re,subprocess,hashlib
ROOT=Path('/Users/christopherdavid/work/openagents-aethyr-book');OUT=Path('/tmp/aethyr-ch11-grid-stack')
sha=lambda b:hashlib.sha256(b).hexdigest()
old=(ROOT/'apps/openagents-web/project-fold.coax').read_text()
up=subprocess.check_output(['git','show','origin/main:apps/openagents-web/project-fold.coax'],cwd=ROOT,text=True)
body=lambda s,n:s.split(n+' :: struct {',1)[1].split('\n}',1)[0]
assert body(old,'Fold')==body(up,'Fold') and body(old,'Store')==body(up,'Store')
items=[{'field':m[1],'element':m[2],'element_bytes':8 if m[2]=='int' else 16} for m in re.finditer(r'(\w+)\s*:\s*\[FOLD_ISSUES\]\s*(int|Ref)',body(old,'Fold'))]
delta=512-320;per=sum(x['element_bytes'] for x in items);fold_growth=per*delta;expected_growth=64*delta
mem=json.load(open('/tmp/aethyr-ch11-grid-memory.json'));g=json.load(open(OUT/'callgraph.json'))
local=[{'entry':'apps/openagents-web/newland-network.coax:hn_project','array':'issues','element_bytes':144,'growth_bytes':144*delta},
       {'entry':'apps/openagents-web/newland-network.coax:hn_project','array':'claim','element_bytes':8,'growth_bytes':8*delta},
       {'entry':'apps/openagents-web/newland-network.coax:hn_project','array':'done_rows','element_bytes':8,'growth_bytes':8*delta},
       {'entry':'apps/openagents-web/project-fold.coax:fold','array':'roots','element_bytes':8,'growth_bytes':8*delta}]
page=lambda x:(x+65535)//65536*65536
def predict(stack):
 heap=mem['heapBase']-(8388608-stack)+fold_growth+expected_growth
 base=page(heap)
 need=base+mem['arena']['stateCap']+mem['arena']['vertexBytes']+mem['arena']['uniformBytes']+mem['arena']['tailBytes']
 return {'stack_bytes':stack,'predicted_heap_base':heap,'predicted_minimum_pages':base//65536,'arena_bytes':need,'arena_pages':(need+65535)//65536,'spare_to_64MiB':67108864-need}
result={'schema':'aethyr.grid.capacity-field-growth.v1','scope':'Exact size arithmetic for ONLY the current320→origin512 HOME_ISSUES/FOLD_ISSUES capacity edit, using retained C ABI element sizes. This is not a claim about all later upstream data/code changes or a replacement for measuring the final relinked artifact.',
 'upstream_commit':subprocess.check_output(['git','rev-parse','origin/main'],cwd=ROOT,text=True).strip(),
 'fold_type_body_unchanged':True,'store_type_body_unchanged':True,'fold_per_issue_fields':items,'bytes_per_issue':per,'old_issues':320,'new_issues':512,
 'static_growth':{'Fold':fold_growth,'LfState.expected':expected_growth,'total':fold_growth+expected_growth,'Store':0,'LandState':0},
 'retained_layouts':{'int':8,'Ref':16,'HomeIssue':144,'Fold':1930808,'LfState':13158152,'Store':11206712,'LandState':667752},
 'predicted_layouts':{'Fold':1930808+fold_growth,'LfState':13158152+fold_growth+expected_growth},
 'reachable_local_array_growth':local,'local_growth_sum_upper_bound':sum(x['growth_bytes'] for x in local),
 'static_alignment_note':'All issue arrays have8-byte element alignment; the64-byte ID expansion and all growth totals are multiples of8, so no extra struct padding is introduced.',
 'predictions':[predict(8388608),predict(4194304)],
 'current_stack_conservative_bound':g['all_defined_function_max_bytes'],
 'capacity_growth_added_conservatively_to_stack':g['all_defined_function_max_bytes']+sum(x['growth_bytes'] for x in local),
 'hypothetical_4MiB_stack_slack_after_all_local_growth':4194304-g['all_defined_function_max_bytes']-sum(x['growth_bytes'] for x in local),
 'source_sha256':{'current_project_fold':sha(old.encode()),'upstream_project_fold':sha(up.encode())}}
(OUT/'capacity-growth.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:result[k] for k in ['bytes_per_issue','static_growth','local_growth_sum_upper_bound','predictions','capacity_growth_added_conservatively_to_stack','hypothetical_4MiB_stack_slack_after_all_local_growth']}))
