# Firm reason: 4 (compiler bootstrap canonical seed refresh after verified fixedpoints).
from pathlib import Path
import json,hashlib,os,shutil,sys
ROOT=Path('/tmp/openagents-12625-complete-20261101').resolve();BASE=Path(os.environ.get('COAX_CLOSURE_NATIVE_OUT','/tmp/coax-12625-closure/native-v6'));FIX=BASE/'fixedpoints'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
record=json.loads((FIX/'record.json').read_text());assert record['passed'];pins=json.loads((BASE/'source-pins.json').read_text())['source_pins'];assert all(sha(ROOT/p)==h for p,h in pins.items())
rows=[]
for module in ['budget','check-domains','profiles','mutation-schedule','lexer','parser','checker','sources']:
 for extension in ['c','h']:
  seed=ROOT/'coax/bootstrap'/(module+'.'+extension);generated=FIX/(module+'.'+extension);rows.append({'path':str(seed.relative_to(ROOT)),'source':str(generated),'before_sha256':sha(seed),'after_sha256':sha(generated),'changed':sha(seed)!=sha(generated)})
apply='--apply' in sys.argv
if apply:
 for row in rows:
  if row['changed']:shutil.copy2(row['source'],ROOT/row['path'])
 assert all(sha(ROOT/row['path'])==row['after_sha256'] for row in rows)
result={'schema':'coax.12625.closure.canonical-seeds.v1','compiler_sha256':sha(BASE/'coax'),'fixedpoint_record_sha256':sha(FIX/'record.json'),'canonical_relative_entry_paths':True,'applied':apply,'normal_release_provenance':False,'files':rows}
(BASE/('seed-refresh.json' if apply else 'seed-comparison.json')).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'applied':apply,'changed_files':[row['path'] for row in rows if row['changed']]}))
