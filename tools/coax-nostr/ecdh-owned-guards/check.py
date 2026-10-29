#!/usr/bin/env python3
"""Opt-in reviewed source-only guard decision; not target or production approval."""
import argparse
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

ROOT=Path(__file__).resolve().parents[3]
POLICY=ROOT/'tools/coax-nostr/ecdh-owned-guards/policy.coax'
MODEL=ROOT/'tools/coax-nostr/ecdh-owned-guards/prove.py'
ADAPTER=ROOT/'coax/lib/crypto/ecdh_owned_adapter.coax'
ADAPTER_SHA='e52c179ee19e4b038fe924e3ea25c0b7c05bb940f7f6555e82bc564dcefc0d55'
REFERENCE_MODEL=ROOT/'tools/coax-nostr/field-guards/prove.py'
REFERENCE_MODEL_SHA='15d72eb3b45bfe5cf62a968f70cf8f650f88eff360f44bd77c31d2e5c16d3d58'
REFERENCE_REPORT=ROOT/'docs/verification/2026-10-29-secp-field-guard-reproof.json'
REFERENCE_REPORT_SHA='90ce9200475505374f53c9e5315f1125e6193ac7f4514a67418ac222081a0410'
PROOF=ROOT/'docs/verification/2026-10-29-ecdh-owned-migration-proof.json'
REVIEW=ROOT/'docs/verification/2026-10-29-ecdh-owned-migration-audit.json'
COMPILER=ROOT/'coax/bin/coax'
POLICY_SHA='5829577cc9d7786d4170c19a5fdd4f151eee4767a6c02728635a4b58ff5b04e1'
REVIEW_SHA='8c9071dc55ac70e9a82142b1e8b832ca29cf8805fbd1d5be5532eac1053741d2'
COMPILER_SHA='9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7'
PROOF_SHA='5255415450a74987136b47932ce11f0590270c22b032979e07c84c6a553eac82'
MODEL_SHA='814e429e50414b60bb21b2e6de3e24ec0a58bd09a9b4d8daf59b1985d3a9d8f9'
RUNTIME_SHA='858b9965d5dff972a1bd15664f12d97a23a92ca87c48dd7dfcb23dd3f6fade6a'
SOLVER_SHA='bf9c9e020f19228523ee7bbf8595181328905158fe37e4ecead5f6cec9651be6'
REVIEW_DOCUMENTS={'docs/verification/2026-10-29-ecdh-owned-migration-review.md':'ec09dfe7352060b957a04c42a140e96415ecb39f60d1a862dd5f5fb71b847275','docs/verification/2026-10-29-ecdh-public-guards-review.md':'901757721eef22afa25049467a7d1018ebafcbfdcb5ceee6ff17cf1b056929d1','docs/verification/2026-10-29-secp-field-guard-reproof.md':'f654820cbc8e5563b0fbc2a02d916c8b9e178a724e293be201fbea293c9d4a87'}
Z3_DIR=Path('/opt/homebrew/lib/python3.15/site-packages/z3')
TRUST=['reviewed local verifier and registry code','reviewed exact byte-load and field-chain source-to-model correspondence, including the manual multiply post-fold bound','caller supplies a public peer and secret scalar with live, aligned, initialized, exclusive and disjoint fixed-extent views','pinned compiler, native toolchain, Python, solver, stable files and local operating system']
class String(C.Structure):_fields_=[('data',C.c_void_p),('count',C.c_int64)]
class Input(C.Structure):_fields_=[(x,String) for x in ('entry','profile','compiler','compiler_version','source','tool','types','parameters','guards','proof','model','runtime','reviews','status','classification')]+[('error',C.c_uint32),('raw_accepted',C.c_bool),('default_steps',C.c_bool),('reached',C.c_int64),('visits',C.c_int64),('return_mask',C.c_uint64),('target_approved',C.c_bool),('production_approved',C.c_bool)]
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def digest(value):return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(',',':'),allow_nan=False).encode()).hexdigest()
def run(*args,**kwargs):
 result=subprocess.run([str(x) for x in args],capture_output=True,text=True,**kwargs)
 if result.returncode:raise ValueError('trusted tool failed: '+str(args[0])+': '+result.stderr[-2000:])
 return result

def isolated():
 prefix=sys.pycache_prefix
 if not sys.flags.isolated or not sys.flags.no_site or not sys.dont_write_bytecode or not prefix or Path(prefix).exists():raise ValueError('requires isolated Python without site initialization and an unused bytecode cache prefix')
 # The checked compiler driver uses these names. Pin their resolution and
 # remove environment-controlled dynamic-loader and compiler configuration.
 for key in os.environ:
  if key.startswith(('DYLD_','LD_','PYTHON')) or key in ('DEVELOPER_DIR','SDKROOT','MACOSX_DEPLOYMENT_TARGET','CPATH','C_INCLUDE_PATH','CPLUS_INCLUDE_PATH','LIBRARY_PATH','Z3_LIBRARY_PATH','CCC_OVERRIDE_OPTIONS'):
   raise ValueError('unreviewed loader or compiler environment')
 os.environ.clear();os.environ.update({'PATH':'/usr/bin:/bin','LC_ALL':'C'})

def runtime_identity():
 clang=run('/usr/bin/xcrun','--find','clang').stdout.strip();clangxx=run('/usr/bin/xcrun','--find','clang++').stdout.strip()
 return {'platform':platform.platform(),'python_version':sys.version,'python_sha256':sha(Path(sys.executable).resolve()),
  'z3_python_files':{p.name:sha(p) for p in sorted(Z3_DIR.glob('*.py'))},'cc_sha256':sha('/usr/bin/cc'),'cxx_sha256':sha('/usr/bin/c++'),
  'clang_sha256':sha(clang),'clangxx_sha256':sha(clangxx),'cc_version':run('/usr/bin/cc','--version').stdout.splitlines()[0]}

def relative(path):
 p=Path(path).resolve()
 try:return '$REPO/'+str(p.relative_to(ROOT.resolve()))
 except ValueError as error:raise ValueError('source outside reviewed root') from error

def normalized_graph(graph,ordered=False):
 if not isinstance(graph,dict) or not graph:raise ValueError('missing source graph')
 result=[];seen=set()
 for path,h in graph.items():
  if not isinstance(path,str) or not isinstance(h,str) or len(h)!=64 or any(c not in '0123456789abcdef' for c in h):raise ValueError('invalid source identity')
  name=relative(path)
  if name in seen or sha(path)!=h:raise ValueError('duplicate or changed source')
  seen.add(name);result.append((name,h))
 return result if ordered else dict(result)

def normalized_guards(rows):
 if not isinstance(rows,list):raise ValueError('missing guards')
 fields={'node','line','column','origin_mask','kind','file','source_sha256','classification'};out=[];seen=set()
 for row in rows:
  if not isinstance(row,dict) or set(row)!=fields:raise ValueError('invalid guard record')
  item=dict(row);item['file']=relative(item['file']);key=(item['file'],item['line'],item['column'],item['node'])
  if key in seen:raise ValueError('duplicate guard')
  seen.add(key);out.append(item)
 return sorted(out,key=lambda g:(g['file'],g['line'],g['column'],g['node']))

def model_projection(proof):
 fields=('name','result','query_sha256');checks=proof['checks']
 names=['normalize_guard_and_word_bounds','byte_load_is_bounded_and_exact','omitted_load_mask_breaks_bound','all_chain_steps_admitted','invalid_first_chain_index_control']
 if not isinstance(checks,list) or [x['name'] for x in checks]!=names:raise ValueError('invalid proof queries')
 return {'source_sha256':proof['source_sha256'],'checks':[{key:x[key] for key in fields} for x in checks],
  'script_sha256':proof['script_sha256'],'solver':proof['solver'],'solver_library':proof['solver_library'],'chain':proof['chain'],
  'reference_report_sha256':proof['reference_report_sha256'],'reference_model_sha256':proof['reference_model_sha256'],'audit_report_sha256':proof['audit_report_sha256']}

def project(audit,proof_sha,model_digest,runtime_digest,reviews_digest):
 if not isinstance(audit,dict) or audit.get('error_name')!='guard-obligations' or 'compiler_run_steps' not in audit or audit['compiler_run_steps'] is not None:raise ValueError('not a reviewed guard-obligations audit')
 if audit.get('classification_assumption')!='caller-supplied; public parameters must be public at the actual call boundary':raise ValueError('missing caller premise')
 if audit.get('checked_runtime_guards_proved') is not False:raise ValueError('raw audit changed')
 if audit.get('view_assumption')!='caller supplies live aligned fixed-extent views, mutually disjoint from each other and all other reachable storage, with declared access and initial classifications and no concurrent mutation throughout the invocation' or audit.get('public_only_scope')!='all reached public-only bodies and transitive effects are analyzed; no declassification from names':raise ValueError('view premises missing')
 for key in ('compiler_source_map_sha256','audit_artifact_sha256'):
  value=audit.get(key)
  if not isinstance(value,str) or len(value)!=64 or any(c not in '0123456789abcdef' for c in value):raise ValueError('missing compiler or audit artifact identity')
 return {'entry':audit['entry'],'profile':audit['compiler_profile'],'compiler':audit['compiler_sha256'],'compiler_version':audit['compiler'],
  'source':digest(normalized_graph(audit['source_sha256'],True)),'tool':digest(normalized_graph(audit['audit_input_sha256'])),
  'types':audit['resolved_type_sha256'],'parameters':digest(audit['classified_parameters']),'guards':digest(normalized_guards(audit['authored_guard_obligations'])),
  'proof':proof_sha,'model':model_digest,'runtime':runtime_digest,'reviews':reviews_digest,'status':audit['source_status'],'classification':audit['classification'],
  'error':audit['error'],'raw_accepted':audit['accepted'],'default_steps':audit['compiler_run_steps'] is None,'reached':audit['reached_procedures'],
  'visits':audit['visited_nodes'],'return_mask':audit['return_dependency_mask'],'target_approved':audit['object_timing_approved'],'production_approved':audit['production_crypto_approved']}

def bind_value(values):
 keep=[];converted={}
 for name,_ in Input._fields_:
  v=values[name]
  if name in ('entry','profile','compiler','compiler_version','source','tool','types','parameters','guards','proof','model','runtime','reviews','status','classification'):
   if not isinstance(v,str):raise ValueError('invalid text')
   b=C.create_string_buffer(v.encode());keep.append(b);converted[name]=String(C.cast(b,C.c_void_p),len(v.encode()))
  elif name in ('raw_accepted','default_steps','target_approved','production_approved'):
   if type(v) is not bool:raise ValueError('invalid boolean')
   converted[name]=v
  else:
   if type(v) is not int or v<0 or v>=(1<<63):raise ValueError('invalid integer')
   converted[name]=v
 return Input(**converted),keep

def build_policy(directory):
 if sha(COMPILER)!=COMPILER_SHA or sha(POLICY)!=POLICY_SHA:raise ValueError('unreviewed compiler or policy')
 c=directory/'policy.c';metadata=directory/'policy-map.json'
 run(COMPILER,'c',POLICY,'--library','--source-map',metadata,'-o',c)
 m=json.loads(metadata.read_text());sources=m['source_hashes']
 if m['sources']!=list(sources) or len(sources)!=1 or next(iter(sources.values()))!=POLICY_SHA or relative(next(iter(sources)))!='$REPO/tools/coax-nostr/ecdh-owned-guards/policy.coax':raise ValueError('policy build identity mismatch')
 libpath=directory/'policy.dylib';run('/usr/bin/cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',libpath)
 lib=C.CDLL(str(libpath));lib.GD_accept.argtypes=[Input];lib.GD_accept.restype=C.c_bool
 return lib,{'policy_source_sha256':POLICY_SHA,'policy_c_sha256':sha(c),'policy_artifact_sha256':sha(libpath),'policy_source_map_sha256':sha(metadata)}

def verify(context=None):
 isolated();driver_sha=sha(__file__);runtime=runtime_identity();runtime_digest=digest(runtime)
 if runtime_digest!=RUNTIME_SHA or sha(COMPILER)!=COMPILER_SHA or sha(POLICY)!=POLICY_SHA or sha(ADAPTER)!=ADAPTER_SHA:raise ValueError('unreviewed runtime, compiler, policy or adapter')
 if sha(REVIEW)!=REVIEW_SHA or sha(PROOF)!=PROOF_SHA or sha(MODEL)!=MODEL_SHA or sha(REFERENCE_MODEL)!=REFERENCE_MODEL_SHA or sha(REFERENCE_REPORT)!=REFERENCE_REPORT_SHA:raise ValueError('reviewed record or proof changed')
 review=json.loads(REVIEW.read_text())['real_ecdh_audit'];expected=json.loads(PROOF.read_text())
 reviews={path:sha(ROOT/path) for path in REVIEW_DOCUMENTS}
 if reviews!=REVIEW_DOCUMENTS:raise ValueError('manual review record changed')
 if sha(expected['solver_library']['path'])!=SOLVER_SHA:raise ValueError('solver library changed')
 for graph in (review['source_sha256'],review['audit_input_sha256']):
  for path,h in graph.items():
   if not path.startswith('$REPO/') or sha(ROOT/path[6:])!=h:raise ValueError('reviewed source or tool changed')
 # The audit and proof are run here, not supplied by the caller.
 sys.path.insert(0,str(ROOT/'tools/coax-nostr/public-view-check'))
 from aggregate import AggregateAudit
 sources={str(ROOT/path[6:]):h for path,h in review['source_sha256'].items() if path.startswith('$REPO/')}
 if len(sources)!=len(review['source_sha256']):raise ValueError('reviewed graph malformed')

 with tempfile.TemporaryDirectory(prefix='coax-guard-binding-') as td:
  directory=Path(td);lib,build=build_policy(directory);tool=AggregateAudit(compiler=COMPILER)
  try:
   manifest={'schema':'coax.secret-classification-views.v2','entry':'ecdh_stage','sources':sources,'parameters':review['classified_parameters'],'views_disjoint':True,'live_aligned_exclusive':True,'public_only':True,'compiler_sha256':tool.compiler_fingerprint,'audit_inputs':tool.inputs,'audit_artifact_sha256':tool.artifact_fingerprint,'profile':'native-c-checked-library'}
   audit=tool.audit(ROOT/'coax/lib/crypto/ecdh.coax','ecdh_stage',classification=manifest)
  finally:tool.close()
  if context is not None:context['raw_audit']=audit
  proof_result=directory/'proof.json'
  solver=Path(expected['solver_library']['path']).resolve();solver_dir=directory/'solver-cwd';solver_dir.mkdir()
  # Restrict z3core's library search before importing it. Never load a
  # candidate until its resolved path and bytes match the reviewed library.
  bootstrap="""import builtins,ctypes,hashlib,os,runpy,sys
package,solver,expected,model,report=sys.argv[1:]
solver=os.path.realpath(solver);builtins.Z3_LIB_DIRS=[os.path.dirname(solver)]
real_cdll=ctypes.CDLL;loaded=[]
def reviewed_cdll(path,*args,**kwargs):
 if not isinstance(path,str) or os.path.realpath(path)!=solver:raise OSError('unreviewed solver path')
 with open(solver,'rb') as f:
  if hashlib.sha256(f.read()).hexdigest()!=expected:raise OSError('unreviewed solver bytes')
 lib=real_cdll(solver,*args,**kwargs);loaded.append(os.path.realpath(lib._name));return lib
ctypes.CDLL=reviewed_cdll
sys.path.insert(0,package)
import z3.z3core
if loaded!=[solver]:raise RuntimeError('solver load path mismatch')
ctypes.CDLL=real_cdll
sys.argv=[model,'--report',report];runpy.run_path(model,run_name='__main__')"""
  run(sys.executable,'-I','-S','-B','-X','pycache_prefix='+str(directory/'unused-cache'),'-c',bootstrap,Z3_DIR.parent,solver,SOLVER_SHA,MODEL,proof_result,cwd=solver_dir)
  actual=json.loads(proof_result.read_text());expected_digest=digest(model_projection(expected));actual_digest=digest(model_projection(actual))
  if actual_digest!=expected_digest:raise ValueError('proof rerun mismatch')
  row=project(audit,sha(PROOF),actual_digest,runtime_digest,digest(reviews));value,keep=bind_value(row)
  accepted=bool(lib.GD_accept(value))
  # The values are checked once more after both live computations.
  if sha(COMPILER)!=COMPILER_SHA or sha(POLICY)!=POLICY_SHA or sha(ADAPTER)!=ADAPTER_SHA or sha(REVIEW)!=REVIEW_SHA or sha(PROOF)!=PROOF_SHA or sha(MODEL)!=MODEL_SHA or sha(REFERENCE_MODEL)!=REFERENCE_MODEL_SHA or sha(REFERENCE_REPORT)!=REFERENCE_REPORT_SHA or sha(__file__)!=driver_sha or any(sha(ROOT/path)!=h for path,h in REVIEW_DOCUMENTS.items()) or sha(expected['solver_library']['path'])!=SOLVER_SHA or digest(runtime_identity())!=runtime_digest:raise ValueError('tool changed during verification')
  normalized_graph(audit['source_sha256'],True);normalized_graph(audit['audit_input_sha256'])
  if digest(model_projection(json.loads(PROOF.read_text())))!=actual_digest or sha(MODEL)!=actual['script_sha256']:raise ValueError('proof changed during verification')
  decision={'schema':'coax.reviewed-guard-registry-match.v1','matched':accepted,'provisional':True,'trust_premises':TRUST,
   'target_timing_approved':False,'erasure_approved':False,'production_crypto_approved':False,'policy_build':build,'verifier_sha256':driver_sha,'runtime':runtime,
   'proof_report_sha256':sha(PROOF),'proof_rerun_sha256':sha(proof_result),'audit_artifact_sha256':audit['audit_artifact_sha256'],'projected_identities':row}
  return {'raw_audit':audit,'registry_match':decision}

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--reviewed-ecdh-entry',action='store_true');parser.add_argument('--report',type=Path);args=parser.parse_args()
 if not args.reviewed_ecdh_entry:parser.error('explicit --reviewed-ecdh-entry is required')
 context={}
 try:output=verify(context)
 except (ValueError,KeyError,TypeError,OSError,json.JSONDecodeError) as error:
  output={**context,'registry_match':{'matched':False,'provisional':True,'reason':str(error),'target_timing_approved':False,'erasure_approved':False,'production_crypto_approved':False}}
 text=json.dumps(output,indent=2)+'\n'
 if args.report:
  with args.report.open('x') as stream:stream.write(text)
 else:print(text,end='')
 return 0 if output['registry_match']['matched'] else 1
if __name__=='__main__':raise SystemExit(main())
