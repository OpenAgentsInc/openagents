#!/usr/bin/env python3
"""Source-bound classified root controls; no target or production claims."""
import copy
import ctypes as C
import json
import subprocess
from pathlib import Path
import tempfile
import unittest
from aggregate import AggregateAudit,unique_object
from check import COMPILER,run

class ClassifiedRoot(unittest.TestCase):
 @classmethod
 def setUpClass(cls):cls.tmp=tempfile.TemporaryDirectory(prefix='classified-root-');cls.path=Path(cls.tmp.name);cls.tool=AggregateAudit()
 @classmethod
 def tearDownClass(cls):cls.tool.close();cls.tmp.cleanup()
 def manifest(self,source,labels):
  m=self.path/'map.json';run(COMPILER,'c',source,'--library','--source-map',m)
  meta=json.loads(m.read_text());functions=[f for f in meta['functions'] if f['name']=='candidate'];self.assertEqual(len(functions),1)
  parameters=[v for v in functions[0]['variables'] if v['role']=='parameter'];self.assertEqual(len(parameters),len(labels))
  return {'schema':'coax.secret-classification.v1','entry':'candidate','sources':meta['source_hashes'],'parameters':[{'name':v['name'],'type':v['type'],'classification':label} for v,label in zip(parameters,labels)]}
 def test_duplicate_manifest_keys_refuse(self):
  with self.assertRaises(ValueError):json.loads('{"entry":"candidate","entry":"other"}',object_pairs_hook=unique_object)
 def test_cli_requires_an_unambiguous_manifest(self):
  source=self.path/'cli.coax';source.write_text('candidate :: (x:u64)->u64 #export_c {return x;}')
  manifest=self.manifest(source,['public']);path=self.path/'classification.json';path.write_text(json.dumps(manifest))
  driver=Path(__file__).with_name('aggregate.py');result=subprocess.run(['python3',str(driver),str(source),'--entry','candidate','--classification',str(path)],capture_output=True,text=True)
  self.assertEqual(result.returncode,0,result.stderr);report=json.loads(result.stdout);self.assertTrue(report['accepted']);self.assertEqual(report['classification'],'explicit whole-parameter classification')
  path.write_text('{"entry":"candidate","entry":"other"}');bad=subprocess.run(['python3',str(driver),str(source),'--entry','candidate','--classification',str(path)],capture_output=True,text=True);self.assertNotEqual(bad.returncode,0)
 def test_explicit_identity_and_all_secret_default(self):
  source=self.path/'identity.coax';source.write_text('candidate :: (x:u64)->u64 #export_c {return x;}')
  manifest=self.manifest(source,['public']);default=self.tool.audit(source,'candidate');public=self.tool.audit(source,'candidate',classification=manifest)
  self.assertEqual((default['error'],default['return_dependency_mask']),(0,1));self.assertEqual((public['error'],public['return_dependency_mask']),(0,0))
  secret=copy.deepcopy(manifest);secret['parameters'][0]['classification']='secret';self.assertEqual(self.tool.audit(source,'candidate',classification=secret)['return_dependency_mask'],1)
  bad=[]
  for key,value in [('schema','bad'),('entry','other'),('sources',{}),('parameters',[])]:
   m=copy.deepcopy(manifest);m[key]=value;bad.append(m)
  for key,value in [('name','other'),('type','u32'),('classification','unknown')]:
   m=copy.deepcopy(manifest);m['parameters'][0][key]=value;bad.append(m)
  bad.extend([None,{},'public'])
  for m in bad:
   report=self.tool.audit(source,'candidate',classification=m);self.assertFalse(report['accepted'],m);self.assertEqual(report['error'],13,m)
  source.write_text(source.read_text()+'\n// changed graph\n');self.assertEqual(self.tool.audit(source,'candidate',classification=manifest)['error'],13)
 def classified(self,name,text,labels):
  source=self.path/(name+'.coax');source.write_text(text);manifest=self.manifest(source,labels)
  return self.tool.audit(source,'candidate',classification=manifest)
 def test_public_branches_early_returns_and_secret_refusals(self):
  branch='candidate :: (flag:u64,x:u64)->u64 #export_c {if flag==0 {return x;}return x+1;}'
  r=self.classified('branch',branch,['public','secret']);self.assertEqual((r['error'],r['return_dependency_mask']),(0,2),r)
  r=self.classified('secret_branch',branch,['secret','secret']);self.assertEqual(r['error'],6,r)
  for name,text in [
   ('unsafe_yes','unsafe_vartime :: (x:u64)->u64 {return x;} candidate :: (flag:u64,x:u64)->u64 {if flag==0 {return unsafe_vartime(x);}return x;}'),
   ('unsafe_continuation','unsafe_vartime :: (x:u64)->u64 {return x;} candidate :: (flag:u64,x:u64)->u64 {if flag==0 {return x;}return unsafe_vartime(x);}')]:
   r=self.classified(name,text,['public','secret']);self.assertEqual(r['error'],8,(name,r))
  guarded='candidate :: (flag:u64,x:u64)->u64 {if flag==0 {#assert x!=0;return x;}else {#assert x!=1;return x;}}'
  r=self.classified('guards',guarded,['public','secret']);self.assertEqual(r['error'],14,r);self.assertEqual(len(r['authored_guard_obligations']),2);self.assertEqual({g['origin_mask'] for g in r['authored_guard_obligations']},{2})
 def test_early_return_loops_and_forbidden_effects(self):
  loop='candidate :: (flag:u64,x:u64)->u64 {for i:0..<2 {if flag==cast(u64)i {return x;}}return x+1;}'
  r=self.classified('loop',loop,['public','secret']);self.assertEqual((r['error'],r['return_dependency_mask']),(0,2),r)
  zero='candidate :: (flag:u64,x:u64)->u64 {for i:0..<0 {if flag==cast(u64)i {return x;}}return x;}'
  r=self.classified('zero_loop',zero,['public','secret']);self.assertEqual(r['error'],5,r)
  unsafe='unsafe_vartime :: (x:u64)->u64 {return x;} candidate :: (flag:u64,x:u64)->u64 {for i:0..<2 {if flag==cast(u64)i {return x;}}return unsafe_vartime(x);}'
  r=self.classified('loop_continuation',unsafe,['public','secret']);self.assertEqual(r['error'],8,r)
  direct='candidate :: (flag:u64,x:u64)->u64 {a:=[1]u64.[0];if flag==0 {a[0]=x;}return a[0];}'
  r=self.classified('direct_write',direct,['public','secret']);self.assertEqual(r['error'],5,r)
  alias='set :: (a:[]u64,x:u64)->u64 {a[0]=x;return 0;} candidate :: (flag:u64,x:u64)->u64 {a:=[1]u64.[0];if flag==0 {z:=set(a[..],x);}return a[0];}'
  r=self.classified('alias_write',alias,['public','secret']);self.assertEqual(r['error'],5,r)
  mutating_loop='candidate :: (flag:u64,x:u64)->u64 {a:=[1]u64.[0];for i:0..<2 {a[0]=x;if flag==cast(u64)i {return a[0];}}return x;}'
  r=self.classified('mutating_loop',mutating_loop,['public','secret']);self.assertEqual(r['error'],5,r)
  secret_nested='candidate :: (flag:u64,x:u64)->u64 {if flag==0 {if x==0 {return x;}}return x;}'
  r=self.classified('secret_nested',secret_nested,['public','secret']);self.assertEqual(r['error'],6,r)
  union='helper :: (x:u64)->u64 {#assert x!=0;return x;} candidate :: (flag:u64,x:u64)->u64 {if flag==0 {return helper(flag);}else {return helper(x);}}'
  r=self.classified('guard_union',union,['public','secret']);self.assertEqual(r['error'],14,r);self.assertEqual(len(r['authored_guard_obligations']),1);self.assertEqual(r['authored_guard_obligations'][0]['origin_mask'],2)
  fresh='Pair :: struct {value:u64;} candidate :: (flag:u64,x:u64)->u64 {if flag==0 {q:Pair;q.value=x;return q.value;}return x;}'
  r=self.classified('fresh_local',fresh,['public','secret']);self.assertEqual((r['error'],r['return_dependency_mask']),(0,2),r)
 def test_runtime_confirms_return_and_unsafe_continuations(self):
  source=self.path/'runtime_witness.coax';source.write_text('unsafe_vartime :: (x:u64)->u64 {return x+1;} candidate :: (flag:u64,x:u64)->u64 #export_c {for i:0..<2 {if flag==cast(u64)i {return x;}}return unsafe_vartime(x);}')
  manifest=self.manifest(source,['public','secret']);report=self.tool.audit(source,'candidate',classification=manifest);self.assertEqual(report['error'],8,report)
  for mode,flags in [('checked',[]),('release',['--release'])]:
   c=self.path/(mode+'.c');c.write_text(run(COMPILER,'c',source,'--library',*flags).stdout)
   lib=self.path/(mode+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',lib)
   api=C.CDLL(str(lib));api.candidate.argtypes=[C.c_uint64,C.c_uint64];api.candidate.restype=C.c_uint64
   self.assertEqual([api.candidate(flag,7) for flag in (0,1,2)],[7,7,8])
  source=self.path/'runtime_alias.coax';source.write_text('unsafe_vartime :: (x:u64)->u64 {return x+1;} set :: (a:[]u64,x:u64)->u64 {a[0]=x;return 0;} candidate :: (flag:u64,x:u64)->u64 #export_c {a:=[1]u64.[0];if flag==0 {z:=set(a[..],x);}if a[0]==0 {return 0;}else {return unsafe_vartime(x);}}')
  manifest=self.manifest(source,['public','secret']);report=self.tool.audit(source,'candidate',classification=manifest);self.assertEqual(report['error'],5,report)
  for mode,flags in [('checked',[]),('release',['--release'])]:
   c=self.path/('alias-'+mode+'.c');c.write_text(run(COMPILER,'c',source,'--library',*flags).stdout)
   lib=self.path/('alias-'+mode+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',lib)
   api=C.CDLL(str(lib));api.candidate.argtypes=[C.c_uint64,C.c_uint64];api.candidate.restype=C.c_uint64
   self.assertEqual([api.candidate(flag,7) for flag in (0,1)],[8,0])
if __name__=='__main__':unittest.main()
