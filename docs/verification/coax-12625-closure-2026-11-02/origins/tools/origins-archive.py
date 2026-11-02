# Firm reason: 2 (archive exact source closures and unmodified stock receipts).
from pathlib import Path
import sys, json, hashlib, tarfile, gzip, io, shutil, re
PRIVATE=Path('/tmp/openagents-12625-origins-closure-20261102')
SHARED=Path('/tmp/openagents-12625-complete-20261101')
BASE=Path('/tmp/coax-12625-origins-closure-scopes')
OUT=PRIVATE/'docs/verification/coax-12625-closure-2026-11-02/origins'
sys.path.insert(0,str(PRIVATE/'coax/tests'));import b2_support
OUT.mkdir(parents=True,exist_ok=True)
def sha(raw):return hashlib.sha256(raw).hexdigest()
def freeze(d,label):
 dst=OUT/'scopes'/label;dst.mkdir(parents=True,exist_ok=True)
 files=b2_support.closure(d/'scope.coax',root=d)
 inventory={str(p.relative_to(d.resolve())):{'bytes':len(raw),'sha256':sha(raw)} for p,raw in files.items()}
 reports=list(d.glob('observation/evidence.json'))+list(d.glob('shard-*/evidence.json'))
 expected={str(p.resolve()):sha(raw) for p,raw in files.items()}
 for report in reports:
  evidence=json.loads(report.read_text());assert {str(Path(x['path']).resolve()):x['sha256'] for x in evidence['sources']}==expected,(label,report)
 tar=io.BytesIO()
 with tarfile.open(fileobj=tar,mode='w') as t:
  for p,raw in sorted(files.items()):
   ti=tarfile.TarInfo(str(p.relative_to(d.resolve())));ti.size=len(raw);ti.mode=0o644;ti.mtime=0;t.addfile(ti,io.BytesIO(raw))
 archive=gzip.compress(tar.getvalue(),mtime=0);(dst/'loaded-source.tar.gz').write_bytes(archive)
 (dst/'loaded-source.json').write_text(json.dumps({'original_root':str(d),'sources':inventory,'archive_sha256':sha(archive),'stock_source_receipts_verified':len(reports)},indent=2)+'\n')
 if (d/'setup.json').exists():shutil.copyfile(d/'setup.json',dst/'setup.json')
 for pat in ['observation/*','shard-*/*','merge-*']:
  for p in d.glob(pat):
   if p.is_file():
    q=dst/p.relative_to(d);q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
 return {'scope':label,'source_files':len(files),'loaded_source_sha256':sha(archive),'manifest_sha256':sha((dst/'loaded-source.json').read_bytes())}
rows=[]
for name in ['CB_variant','CB_leaf_count','CB_discover','CB_expr']:
 rows.append(freeze(Path('/tmp/coax-12625-source/scoped-mutation')/('borrow-'+name+'-resume'),'original-'+name))
for d in sorted(BASE.iterdir()):
 if d.is_dir():rows.append(freeze(d,d.name))
(OUT/'negative-control').mkdir(exist_ok=True)
for name in ['scope.coax','setup.json','control-receipt.json']:
 shutil.copyfile(Path('/tmp/coax-12625-origins-projection-negative')/name,OUT/'negative-control'/name)
(OUT/'checks-first').mkdir(exist_ok=True)
for p in Path('/tmp').glob('origins-*'):
 if p.is_file() and p.suffix in ['.json','.stdout','.stderr']:
  shutil.copyfile(p,OUT/'checks-first'/p.name)
(OUT/'tools').mkdir(exist_ok=True)
for n in ['origins-closure-scopes.py','origins-query-shard.py','origins-query-shard-v3.py','origins-query-shard-v5.py','origins-query-shard-v6.py','origins-archive.py']:
 shutil.copyfile(Path('/tmp')/n,OUT/'tools'/n)
shutil.copyfile(PRIVATE/'coax/tests/results-origin-policy.coax',OUT/'results-origin-policy.coax')
t=SHARED.joinpath('coax/lib/compiler/checker.coax').read_text();m=re.search(r'^CB_query :: ',t,re.M);n=re.search(r'^(?:[A-Za-z_]\w* :: |#test|#forall)',t[m.end():],re.M);body=t[m.start():m.end()+n.start() if n else len(t)]
(OUT/'CB_query-final.coax').write_text(body)
old=body.replace('head:=b.c.nodes[decl].borrow_query;link:=head;bucket:int;if b.query_index','head:=b.c.nodes[decl].borrow_query;link:=head;bucket:=0;if b.query_index');assert body!=old
(OUT/'CB_query-before.coax').write_text(old)
record={'schema':1,'scope':'#12625 origins mutation closure only','producer':{'path':'/tmp/coax-12625-native/final-v4/coax','sha256':sha(Path('/tmp/coax-12625-native/final-v4/coax').read_bytes())},'shared_checker_sha256':sha(SHARED.joinpath('coax/lib/compiler/checker.coax').read_bytes()),'origin_policy_sha256':sha((OUT/'results-origin-policy.coax').read_bytes()),'new_origin_groups':12,'new_origin_cases':42,'production_change':'ONE dead initializer bucket:=0 changed to bucket:int; every indexed use is preceded by unconditional CB_index_bucket assignment; no read when index is disabled. No guard/helper/quota/path/depth/work change.','scopes':rows,'passing_nonquery':{'CB_variant-v1':46,'CB_leaf_count-v3':74,'CB_discover-v1':132,'CB_expr-v1':226},'old_query':{'scope':'CB_query-v3','mutants':372,'killed':365,'survived':7},'final_query':{'scope':'CB_query-v6','status':'pending until all four stock reports and stock merge are present'},'trial_status':{'CB_query-v5':'complete 371-site stock inventory, 370 killed and one removed-publication call survived; single-block fuel fixture did not distinguish. Final v6 uses both real layouts with two admitted field blocks.', 'CB_query-v1':'840s owned-process timeout, not passing','CB_query-v2':'canceled to rebind to final sibling cache modules, not passing','CB_query-v4':'checks-first snapshot; selected baseline had malformed unused path tail in write-only fixture; corrected in final policy before production edit','CB_leaf_count-v1':'73/74; depth-zero mutant survived','CB_leaf_count-v2':'deep borrowed fixture evaluator E0403; corrected to deep owned payload in v3'},'no_waivers':True,'postfatal_contract':'Only compositional helper sentinel values consumed by existing callers before fatal barrier are asserted after rejection; no unspecified private failed cache output asserted.'}
(OUT/'record.json').write_text(json.dumps(record,indent=2)+'\n')
print(OUT);print('scopes',len(rows));print('bytes',sum(p.stat().st_size for p in OUT.rglob('*') if p.is_file()))
