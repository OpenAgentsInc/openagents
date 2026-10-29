#!/usr/bin/env python3
"""Independent negative controls for the reviewed source-only binding."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('guard_binding',HERE/'check.py');binding=importlib.util.module_from_spec(spec);spec.loader.exec_module(binding)
spec=importlib.util.spec_from_file_location('guard_launcher',HERE/'run.py');launcher=importlib.util.module_from_spec(spec);spec.loader.exec_module(launcher)
class GuardBinding(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.output=launcher.verify();cls.audit=cls.output['raw_audit'];cls.row=cls.output['registry_match']['projected_identities']
  cls.tmp=tempfile.TemporaryDirectory(prefix='guard-policy-tests-');cls.lib,cls.build=binding.build_policy(Path(cls.tmp.name))
 @classmethod
 def tearDownClass(cls):cls.tmp.cleanup()
 def accepted(self,audit=None,proof=None,model=None,runtime=None,reviews=None):
  try:
   row=binding.project(self.audit if audit is None else audit,self.row['proof'] if proof is None else proof,self.row['model'] if model is None else model,self.row['runtime'] if runtime is None else runtime,self.row['reviews'] if reviews is None else reviews)
   value,keep=binding.bind_value(row);return bool(self.lib.GD_accept(value))
  except (ValueError,KeyError,TypeError,OSError):return False
 def test_real_reviewed_root_keeps_raw_refusal(self):
  d=self.output['source_scope_decision'];self.assertTrue(d['source_scope_accepted']);self.assertTrue(d['conditional']);self.assertFalse(d['target_timing_approved']);self.assertFalse(d['production_crypto_approved'])
  self.assertFalse(self.audit['accepted']);self.assertEqual(self.audit['error_name'],'guard-obligations');self.assertEqual(len(self.audit['authored_guard_obligations']),2);self.assertEqual(self.audit['authored_guard_obligations'][0]['origin_mask'],0);self.assertTrue(self.accepted())
 def test_entry_profile_and_result_mismatches_refuse(self):
  for key,value in [('entry','other'),('compiler_profile','wasm-release'),('compiler','other'),('compiler_sha256','0'*64),('resolved_type_sha256','0'*64),('classification','all entry parameters secret'),('classification_assumption',''),('view_assumption',''),('public_only_scope',''),('error',6),('error_name','secret-branch'),('source_status','refused'),('accepted',True),('reached_procedures',45),('visited_nodes',98079),('return_dependency_mask',0),('object_timing_approved',True),('production_crypto_approved',True),('checked_runtime_guards_proved',True),('compiler_run_steps',100000)]:
   a=copy.deepcopy(self.audit);a[key]=value;self.assertFalse(self.accepted(audit=a),key)
  a=copy.deepcopy(self.audit);del a['compiler_run_steps'];self.assertFalse(self.accepted(audit=a))
  for key in ('entry','compiler','compiler_profile','classification','classified_parameters','source_sha256','audit_input_sha256','authored_guard_obligations','compiler_source_map_sha256','audit_artifact_sha256','view_assumption','public_only_scope'):
   a=copy.deepcopy(self.audit);del a[key];self.assertFalse(self.accepted(audit=a),key)
 def test_source_tool_and_classification_changes_refuse(self):
  for key in ('source_sha256','audit_input_sha256'):
   a=copy.deepcopy(self.audit);first=next(iter(a[key]));a[key][first]='0'*64;self.assertFalse(self.accepted(audit=a),key)
   a=copy.deepcopy(self.audit);a[key].pop(next(iter(a[key])));self.assertFalse(self.accepted(audit=a),key)
   a=copy.deepcopy(self.audit);a[key]['/tmp/outside-source']='0'*64;self.assertFalse(self.accepted(audit=a),key)
   a=copy.deepcopy(self.audit);other=binding.ROOT/'coax/release.coax';a[key][str(other)]=binding.sha(other);self.assertFalse(self.accepted(audit=a),key)
   a=copy.deepcopy(self.audit);first=next(iter(a[key]));path=Path(first);alias=str(path.parent/'..'/path.parent.name/path.name);a[key][alias]=a[key][first];self.assertFalse(self.accepted(audit=a),key)
  for variant in ('public_flip','secret_flip','reorder','wrong_name','wrong_type','missing','extent','access','element'):
   a=copy.deepcopy(self.audit);p=a['classified_parameters']
   if variant=='public_flip':p[0]['classification']='public'
   elif variant=='secret_flip':p[1]['classification']='secret'
   elif variant=='reorder':p.reverse()
   elif variant=='wrong_name':p[1]['name']='other'
   elif variant=='wrong_type':p[1]['type']='u64'
   elif variant=='extent':p[1]['view']['extent']+=1
   elif variant=='access':p[1]['view']['access']='write'
   elif variant=='element':p[1]['view']['element_type']='u64'
   else:p.pop()
   self.assertFalse(self.accepted(audit=a),variant)
 def test_every_guard_identity_is_required(self):
  for variant in ('missing','missing_first','extra','duplicate','mask','mask_second','line','column','node','file','source_hash','kind','classification'):
   a=copy.deepcopy(self.audit);g=a['authored_guard_obligations']
   if variant=='missing':g.pop()
   elif variant=='missing_first':g.pop(0)
   elif variant=='extra':g.append({**g[0],'node':999})
   elif variant=='duplicate':g.append(copy.deepcopy(g[0]))
   elif variant=='mask':g[0]['origin_mask']=2
   elif variant=='mask_second':g[1]['origin_mask']=2
   elif variant=='line':g[0]['line']+=1
   elif variant=='column':g[0]['column']+=1
   elif variant=='node':g[0]['node']+=1
   elif variant=='file':g[0]['file']=str(binding.ROOT/'coax/lib/crypto/secp_group.coax')
   elif variant=='source_hash':g[0]['source_sha256']='0'*64
   elif variant=='kind':g[0]['kind']='other'
   else:g[0]['classification']='secret-derived'
   self.assertFalse(self.accepted(audit=a),variant)
 def test_proof_solver_and_runtime_identities_refuse(self):
  self.assertFalse(self.accepted(proof='0'*64));self.assertFalse(self.accepted(model='0'*64));self.assertFalse(self.accepted(runtime='0'*64));self.assertFalse(self.accepted(reviews='0'*64))
  proof=json.loads(binding.PROOF.read_text());original=binding.digest(binding.model_projection(proof));self.assertEqual(original,self.row['model'])
  for variant in ('query','result','solver','library','script','source','chain','reference','audit'):
   p=copy.deepcopy(proof)
   if variant=='query':p['checks'][0]['query_sha256']='0'*64
   elif variant=='result':p['checks'][0]['result']='sat'
   elif variant=='solver':p['solver']='z3 other'
   elif variant=='library':p['solver_library']['sha256']='0'*64
   elif variant=='script':p['script_sha256']='0'*64
   elif variant=='source':p['source_sha256'][next(iter(p['source_sha256']))]='0'*64
   elif variant=='chain':p['chain'][0][0]+=1
   elif variant=='reference':p['reference_report_sha256']='0'*64
   else:p['audit_report_sha256']='0'*64
   self.assertFalse(self.accepted(model=binding.digest(binding.model_projection(p))),variant)
  p=copy.deepcopy(proof);p['checks'][1]=p['checks'][0]
  with self.assertRaises(ValueError):binding.model_projection(p)
 def test_preflight_refuses_changed_trusted_inputs(self):
  original=binding.sha
  for path in (binding.COMPILER,binding.POLICY,binding.ADAPTER,binding.REVIEW,binding.PROOF,binding.MODEL,binding.REFERENCE_MODEL,binding.REFERENCE_REPORT,*[binding.ROOT/x for x in binding.REVIEW_DOCUMENTS]):
   def altered(p):return '0'*64 if Path(p)==path else original(p)
   with patch.object(binding,'sha',side_effect=altered):
    with self.assertRaises(ValueError):binding.verify()
 def test_real_filesystem_drift_refuses(self):
  with tempfile.TemporaryDirectory(prefix='guard-drift-',dir=binding.ROOT/'coax/bin') as td:
   source=Path(td)/'source';source.write_text('reviewed');graph={str(source):binding.sha(source)}
   self.assertEqual(len(binding.normalized_graph(graph)),1);source.write_text('changed')
   with self.assertRaises(ValueError):binding.normalized_graph(graph)
   badproof=Path(td)/'proof.json';badproof.write_bytes(binding.PROOF.read_bytes()+b' ')
   with patch.object(binding,'PROOF',badproof):
    with self.assertRaises(ValueError):binding.verify()
 def test_refusal_after_audit_keeps_raw_result(self):
  context={}
  with patch.object(binding,'project',side_effect=ValueError('injected mismatch')):
   with self.assertRaises(ValueError):binding.verify(context)
  self.assertFalse(context['raw_audit']['accepted']);self.assertEqual(context['raw_audit']['error_name'],'guard-obligations')
 def test_outer_anchor_and_changed_inner_refuse(self):
  self.assertIn('trusted code anchor',self.output['source_scope_decision']['trust_premises'][0])
  provisional={'raw_audit':copy.deepcopy(self.audit),'registry_match':copy.deepcopy(self.output['registry_match'])}
  for variant in ('raw_accepted','raw_error','guard_missing','inner_hash','policy_hash','match','approval','schema'):
   p=copy.deepcopy(provisional)
   if variant=='raw_accepted':p['raw_audit']['accepted']=True
   elif variant=='raw_error':p['raw_audit']['error']=6
   elif variant=='guard_missing':p['raw_audit']['authored_guard_obligations'].pop()
   elif variant=='inner_hash':p['registry_match']['verifier_sha256']='0'*64
   elif variant=='policy_hash':p['registry_match']['policy_build']['policy_source_sha256']='0'*64
   elif variant=='match':p['registry_match']['matched']=False
   elif variant=='approval':p['registry_match']['production_crypto_approved']=True
   else:p['registry_match']['schema']='other'
   with self.assertRaises(ValueError,msg=variant):launcher.validate(p)
  with tempfile.TemporaryDirectory(prefix='guard-inner-drift-') as td:
   bad=Path(td)/'inner';bad.write_bytes(launcher.INNER.read_bytes()+b' ')
   with patch.object(launcher,'INNER',bad):
    with self.assertRaises(ValueError):launcher.verify()
   policy=Path(td)/'policy';policy.write_bytes(launcher.POLICY.read_bytes()+b' ')
   with patch.object(launcher,'POLICY',policy):
    with self.assertRaises(ValueError):launcher.verify()
 def test_inner_output_is_provisional_only(self):
  result=binding.verify();self.assertTrue(result['registry_match']['matched']);self.assertNotIn('source_scope_decision',result)
 def test_solver_loader_ignores_hostile_working_directory(self):
  with tempfile.TemporaryDirectory(prefix='guard-hostile-cwd-') as td:
   directory=Path(td);marker=directory/'loaded';source=directory/'evil.c';library=directory/'libz3.dylib'
   source.write_text('#include <stdio.h>\n__attribute__((constructor)) static void witness(void){FILE *f=fopen('+json.dumps(str(marker))+',"w");if(f)fclose(f);}')
   subprocess.run(['/usr/bin/cc','-dynamiclib',str(source),'-o',str(library)],check=True,capture_output=True)
   command=[sys.executable,'-I','-S','-B','-X','pycache_prefix='+str(directory/'unused'),str(HERE/'run.py'),'--reviewed-ecdh-entry']
   result=subprocess.run(command,cwd=directory,capture_output=True,text=True)
   self.assertEqual(result.returncode,0,result.stdout+result.stderr);self.assertTrue(json.loads(result.stdout)['source_scope_decision']['source_scope_accepted']);self.assertFalse(marker.exists())
   for key in ('Z3_LIBRARY_PATH','CCC_OVERRIDE_OPTIONS'):
    env=dict(os.environ);env[key]=str(directory)
    result=subprocess.run(command,cwd=directory,capture_output=True,text=True,env=env)
    self.assertNotEqual(result.returncode,0);self.assertFalse(json.loads(result.stdout)['source_scope_decision']['source_scope_accepted']);self.assertFalse(marker.exists())
 def test_legacy_registry_stays_valid_and_default_view_refuses(self):
  old=binding.ROOT/'tools/coax-nostr/public-guard-binding/run.py'
  command=[sys.executable,'-I','-S','-B','-X','pycache_prefix='+str(Path(self.tmp.name)/'old-unused'),str(old),'--reviewed-variable-base']
  result=subprocess.run(command,capture_output=True,text=True);self.assertEqual(result.returncode,0,result.stdout+result.stderr)
  record=json.loads(result.stdout);self.assertFalse(record['raw_audit']['accepted']);self.assertTrue(record['source_scope_decision']['source_scope_accepted'])
  sys.path.insert(0,str(binding.ROOT/'tools/coax-nostr/public-view-check'))
  from aggregate import AggregateAudit
  tool=AggregateAudit(compiler=binding.COMPILER)
  try:raw=tool.audit(binding.ROOT/'coax/lib/crypto/ecdh.coax','ecdh_stage')
  finally:tool.close()
  self.assertFalse(raw['accepted']);self.assertEqual(raw['error_name'],'unsupported')
 def test_opt_in_and_isolation_are_required(self):
  plain=subprocess.run([sys.executable,str(HERE/'run.py'),'--reviewed-ecdh-entry'],capture_output=True,text=True)
  self.assertNotEqual(plain.returncode,0);self.assertFalse(json.loads(plain.stdout)['source_scope_decision']['source_scope_accepted'])
  absent=subprocess.run([sys.executable,'-I','-S','-B','-X','pycache_prefix='+str(Path(self.tmp.name)/'unused'),str(HERE/'run.py')],capture_output=True,text=True)
  self.assertNotEqual(absent.returncode,0)
  env=dict(os.environ);env['CPATH']='/tmp'
  overridden=subprocess.run([sys.executable,'-I','-S','-B','-X','pycache_prefix='+str(Path(self.tmp.name)/'unused-env'),str(HERE/'run.py'),'--reviewed-ecdh-entry'],capture_output=True,text=True,env=env)
  self.assertNotEqual(overridden.returncode,0);self.assertFalse(json.loads(overridden.stdout)['source_scope_decision']['source_scope_accepted'])
if __name__=='__main__':unittest.main()
