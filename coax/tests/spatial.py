#!/usr/bin/env python3
"""Focused native-C spatial compiler frontend checks; no release sweep."""
from pathlib import Path
import os,re,subprocess,tempfile,unittest
ROOT=Path(__file__).resolve().parents[2]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
PRELUDE='#load "'+str(ROOT/'coax/lib/quantity.coax')+'" as u;\n'+'''World :: struct @frame(handedness=right,convention=cartesian,continuity=continuous,min_precision=f32) {}
Other :: struct @frame(handedness=right,convention=cartesian,continuity=continuous,min_precision=f32) {}
Deep :: struct @frame(handedness=right,convention=body,continuity=continuous,min_precision=f64) {}
'''
class Spatial(unittest.TestCase):
 def setUp(self):self.tmp=tempfile.TemporaryDirectory(prefix='coax-spatial-');self.addCleanup(self.tmp.cleanup);self.work=Path(self.tmp.name)
 def invoke(self,source,command='c',*flags):
  p=self.work/'case.coax';p.write_text(PRELUDE+source+'\n');return subprocess.run([str(COMPILER),command,str(p),'--library',*flags],cwd=ROOT,text=True,capture_output=True,timeout=120)
 def test_affine_source_results_and_raw_literal_scaling(self):
  r=self.invoke('''calc :: ()->f32 {p:Point3(World,u.Metre,f32);p.x=6;p.y=2;p.z=1;q:Point3(World,u.Metre,f32);q.x=2;q.y=1;v:=p-q;a:=v+p;b:=p-v;c:=2*v;d:=v*2;e:=v/2;n:= -v;return a.x+b.x+c.x+d.x+e.x+n.x;}
V :: #run calc();#assert V==26;''');self.assertEqual(r.returncode,0,r.stderr)
 def test_affine_refusals_do_not_reuse_an_operand_kind(self):
  for expr in ['p+p','v-p','p*2','2*p','p/2','-p','v*v','v/v','p==p','v<v','!v']:
   r=self.invoke('f :: (p:Point3(World,u.Metre,f32),v:Vector3(World,u.Metre,f32)) {x:='+expr+';}');self.assertNotEqual(r.returncode,0,expr);self.assertIn('E0834',r.stderr);self.assertEqual(r.stdout,'')
 def test_minimum_precision_and_point_length_are_static(self):
  for ty in ['Point3(Deep,u.Metre,f32)','Vector3(Deep,u.Metre,f32)','Point3(World,u.Second,f64)','Point3(World,u.Metre,int)','Point3(u.Metre,u.Metre,f32)']:
   r=self.invoke('f :: (x:'+ty+') {}');self.assertNotEqual(r.returncode,0);self.assertIn('E0833',r.stderr)
 def test_frame_unit_and_carrier_mismatch_and_quantity_scale(self):
  for rhs in ['Vector3(Other,u.Metre,f32)','Vector3(World,u.HalfMetre,f32)','Vector3(World,u.Metre,f64)','u.Quantity(UnitOne,f32)']:
   op='*' if 'Quantity' in rhs else '+';r=self.invoke('f :: (a:Vector3(World,u.Metre,f32),b:'+rhs+') {x:=a'+op+'b;}');self.assertNotEqual(r.returncode,0);self.assertIn('E0834',r.stderr)
 def test_vector_any_unit_and_ordinary_aggregate_layout(self):
  r=self.invoke('''f :: (a:Vector3(World,u.Second,f64),b:Vector3(World,u.Second,f64))->Vector3(World,u.Second,f64) #export_c {return a+b;}
S :: #run size_of(Point3(World,u.Metre,f32));D :: #run size_of(Vector3(Deep,u.Second,f64));#assert S==12 && D==24;''');self.assertEqual(r.returncode,0,r.stderr)
 def test_frame_declaration_shapes_and_optical_profile(self):
  for decl in ['Bad :: struct @frame(handedness=left,convention=optical,continuity=continuous,min_precision=f32) {}','Bad :: struct @frame(handedness=right,convention=cartesian,continuity=continuous,min_precision=f32) {x:f32;}','Bad :: struct($T) @frame(handedness=right,convention=cartesian,continuity=continuous,min_precision=f32) {}']:
   r=self.invoke(decl);self.assertNotEqual(r.returncode,0);self.assertIn('E0832',r.stderr)
 def test_intrinsic_names_are_global_types_not_runtime_calls(self):
  for source in ['Point3 :: struct {}','f :: (x:bogus.Point3(World,u.Metre,f32)) {}','f :: () {x:=Point3(World,u.Metre,f32);}']:
   r=self.invoke(source);self.assertNotEqual(r.returncode,0);self.assertEqual(r.stdout,'')
 def test_semantic_reflection_and_device_output_are_refused(self):
  r=self.invoke('f :: () {v:=type_info(Point3(World,u.Metre,f32));}');self.assertNotEqual(r.returncode,0);self.assertIn('E0835',r.stderr)
  for command in ['metal-compute','cuda','spirv']:
   r=self.invoke('f :: (x:Point3(World,u.Metre,f32))->f32 #export_c {return x.x;}',command);self.assertNotEqual(r.returncode,0);self.assertIn('E0836',r.stderr)
 def test_generic_constructor_arguments_remain_explicitly_outside_profile(self):
  r=self.invoke('f :: (p:Point3(World,u.Metre,$R:numeric)) {}');self.assertNotEqual(r.returncode,0);self.assertIn('E0833',r.stderr)
 def test_no_implicit_ordinary_record_or_nominal_frame_assignment(self):
  r=self.invoke('Raw :: struct {x:f32;y:f32;z:f32;} f :: (p:Point3(World,u.Metre,f32))->Raw {return p;}');self.assertNotEqual(r.returncode,0)
  r=self.invoke('f :: (p:Point3(World,u.Metre,f32))->Point3(Other,u.Metre,f32) {return p;}');self.assertNotEqual(r.returncode,0)
 def test_authored_operand_indices_survive_spatial_lowering(self):
  r=subprocess.run([str(COMPILER),'mutate',str(ROOT/'coax/tests/spatial/mutation-operands.coax'),'--library','--root-only','--jobs','1'],cwd=ROOT,text=True,capture_output=True,timeout=120)
  self.assertEqual(r.returncode,0,r.stdout+r.stderr);m=re.search(r'(\d+) mutants, (\d+) killed, (\d+) survived',r.stdout+r.stderr);self.assertIsNotNone(m,r.stdout+r.stderr);sites,killed,survived=map(int,m.groups());self.assertGreaterEqual(sites,2);self.assertEqual(killed,sites);self.assertEqual(survived,0)
 def test_spatial_constraints_remain_copyable_only(self):
  for need in ['numeric','ordered','equatable']:
   r=self.invoke('bound :: (x:$T:'+need+')->$T {return x;} f :: (p:Point3(World,u.Metre,f32)) {x:=bound(p);}')
   self.assertNotEqual(r.returncode,0);self.assertIn('E0806',r.stderr)
if __name__=='__main__':unittest.main()
