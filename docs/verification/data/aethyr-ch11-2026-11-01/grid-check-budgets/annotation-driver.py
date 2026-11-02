from pathlib import Path
import hashlib,json,shutil
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');profiles=Path('/tmp/aethyr-ch11-grid-check-profiles');q=json.loads((profiles/'qualification.json').read_text());assert q['status']=='pass'and len(q['results'])==13
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();data=root/'docs/verification/data/aethyr-ch11-2026-11-01/grid-check-budgets';data.mkdir(exist_ok=True)
before={rel:(root/rel).read_text()for rel in q['source_sha256_before']};after=dict(before);annotations=[]
for row in q['results']:
 assert row['status']=='pass'and row['claim']['cases']==1 and row['claim']['kind']=='#test'and row['steps']>20000000 and row['steps']<=400000000 and not row['source_changes']and not row['compiler_changed']
 rel=row['source'];old=row['header'];assert before[rel].splitlines()[row['line']-1]==old and '#steps'not in old
 limit=((row['steps']*5+40000000-1)//40000000)*10000000
 new=old.replace(' {',f' #steps {limit} {{',1);assert old!=new and after[rel].count(old)==1;after[rel]=after[rel].replace(old,new,1)
 annotations.append({'name':row['name'],'source':rel,'line':row['line'],'old_budget':20000000,'measured_steps':row['steps'],'new_budget':limit,'old_header':old,'new_header':new,'profile_record':f'{row["index"]:02}.json','evidence':f'{row["index"]:02}.evidence.json','log':f'{row["index"]:02}.log'})
for rel,h in q['source_sha256_before'].items():assert sha(root/rel)==h
for rel,text in after.items():
 restored=text
 for row in annotations:
  if row['source']==rel:restored=restored.replace(row['new_header'],row['old_header'],1)
 assert restored==before[rel]and len(text.splitlines())==len(before[rel].splitlines())
for rel,text in after.items():(root/rel).write_text(text)
for p in profiles.rglob('*'):
 if p.is_file():dst=data/('measurement.json'if p.relative_to(profiles)==Path('qualification.json')else p.relative_to(profiles));dst.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,dst)
for src,name in [(Path('/tmp/aethyr-ch11-grid-check-profiles-run.log'),'measurement-run.log'),(Path('/tmp/aethyr-ch11-profile-grid-checks.py'),'measurement-driver.py'),(Path(__file__),'annotation-driver.py'),(Path('/tmp/aethyr-ch11-grid-long-checks.json'),'selected-checks.json'),(Path('/tmp/aethyr-ch11-grid-qualified/execution.log'),'default-build-red.log')]:shutil.copyfile(src,data/name)
record={'schema':'aethyr.grid.authored-check-budgets.v1','status':'measured_checks_pass_headers_annotated','compiler':{'path':'/Users/christopherdavid/work/openagents-aethyr-check-budgets-current/coax/bin/coax','sha256':q['compiler_sha256'],'published_dependency_commit':'8008dcc0386f7ba819faa7b1bc8c4454ed460fae'},'entry':q['entry'],'caller_budget':400000000,'old_default_budget':20000000,'selection':'Only the13named checks concretely refused by the ordinary default Grid build. Seven are in newland,one in network,one in walkers,four in walk. Other checks were not repeated.','measurement_seconds':q['elapsed_seconds'],'per_command_deadline_seconds':780,'batch_deadline_seconds':810,'parallel':4,'annotations':annotations,'source_sha256_before':q['source_sha256_before'],'source_sha256_after':{rel:sha(root/rel)for rel in after},'preservation':'Removing exactly the13new #steps header fields restores every byte of all4original files. Bodies, assertions, line numbers,existing comments and the source-documented400M manual command are unchanged.','rounding':'ceil(1.25*measured_steps/10000000)*10000000; all are single #test cases,not forall aggregates.','full_authority_scope':'Selected measurements intentionally do not claim authority execution coverage. The next ordinary default project C/Wasm build runs the full mandatory composed checks under these authored budgets.','files':{str(p.relative_to(data)):sha(p)for p in sorted(data.rglob('*'))if p.is_file()and p.name!='qualification.json'}}
(data/'qualification.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'annotations':[(r['source'],r['line'],r['measured_steps'],r['new_budget'])for r in annotations],'record':str(data/'qualification.json')},indent=2))
