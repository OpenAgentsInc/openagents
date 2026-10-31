#!/usr/bin/env python3
"""Focused derived-unit compiler semantics, independent of release/cache gates."""
from pathlib import Path
import json,os,subprocess,tempfile,unittest
ROOT=Path(__file__).resolve().parents[2]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
PRELUDE='#load "'+str(ROOT/'coax/lib/quantity.coax')+'" as u;\n'+'''Speed :: struct @unit(mass=0,length=1,time=-1,angle=0,numerator=1,denominator=1) {}
OtherMetre :: struct @unit(mass=0,length=1,time=0,angle=0,numerator=1,denominator=1) {}
Count :: struct @unit(mass=0,length=0,time=0,angle=0,numerator=1,denominator=1) {}
'''
class DerivedQuantities(unittest.TestCase):
 def setUp(self):self.temp=tempfile.TemporaryDirectory(prefix='coax-unit-derived-');self.addCleanup(self.temp.cleanup);self.work=Path(self.temp.name)
 def invoke(self,source,command='c',*extra):
  p=self.work/'case.coax';p.write_text(PRELUDE+source+'\n');return subprocess.run([str(COMPILER),command,str(p),'--library',*extra],cwd=ROOT,capture_output=True,text=True,timeout=120)
 def test_concrete_product_quotient_and_formal_cancellation(self):
  r=self.invoke('''product :: (a:u.Quantity(u.Metre,f32),b:u.Quantity(u.Second,f32))->u.Quantity(UnitProduct(u.Second,u.Metre),f32) #export_c {return a*b;}
speed :: (a:u.Quantity(u.Metre,f32),b:u.Quantity(u.Second,f32))->u.Quantity(UnitQuotient(u.Metre,u.Second),f32) #export_c {return a/b;}
back :: (a:u.Quantity(UnitQuotient(u.Metre,u.Second),f32),b:u.Quantity(u.Second,f32))->u.Quantity(u.Metre,f32) #export_c {return a*b;}
ratio :: (a:u.Quantity(u.Metre,f32),b:u.Quantity(u.Metre,f32))->u.Quantity(UnitOne,f32) #export_c {return a/b;}
calc :: ()->f32 {a:u.Quantity(u.Metre,f32)=u.Quantity.{value=6};b:u.Quantity(u.Second,f32)=u.Quantity.{value=2};return product(a,b).value+speed(a,b).value+back(speed(a,b),b).value+ratio(a,a).value;}
V :: #run calc();#assert V==22;''');self.assertEqual(r.returncode,0,r.stderr)
 def test_generic_representation_keeps_bound_scalar_literals(self):
  r=self.invoke('''speed :: (a:u.Quantity(u.Metre,$R:numeric),b:u.Quantity(u.Second,$R))->u.Quantity(UnitQuotient(u.Metre,u.Second),$R) {return a/b;}
scale :: (s:$R:numeric,x:u.Quantity(UnitQuotient(u.Metre,u.Second),$R))->u.Quantity(UnitQuotient(u.Metre,u.Second),$R) {return s*x;}
calc :: ()->f32 {a:u.Quantity(u.Metre,f32)=u.Quantity.{value=6};b:u.Quantity(u.Second,f32)=u.Quantity.{value=2};return scale(2,speed(a,b)).value;}
V :: #run calc();#assert V==6;''');self.assertEqual(r.returncode,0,r.stderr)
 def test_concrete_unit_mismatch_is_not_hidden_by_generic_binding(self):
  r=self.invoke('''scale :: (s:$R:numeric,x:u.Quantity(UnitQuotient(u.Metre,u.Second),$R))->u.Quantity(UnitQuotient(u.Metre,u.Second),$R) {return s*x;} wrong :: (x:u.Quantity(UnitQuotient(u.Metre,u.Radian),f32)) {v:=scale(2,x);}''')
  self.assertNotEqual(r.returncode,0);self.assertIn('E0317',r.stderr)
 def test_nominal_atoms_and_named_derived_tags_stay_distinct(self):
  for body in ['f :: (a:u.Quantity(u.Metre,f64),b:u.Quantity(u.Second,f64))->u.Quantity(Speed,f64) {return a/b;}','f :: (a:u.Quantity(OtherMetre,f64),b:u.Quantity(u.Second,f64))->u.Quantity(UnitProduct(u.Metre,u.Second),f64) {return a*b;}','f :: (a:u.Quantity(UnitOne,f64))->u.Quantity(Count,f64) {return a;}']:
   r=self.invoke(body);self.assertNotEqual(r.returncode,0);self.assertEqual(r.stdout,'')
 def test_conversions_are_explicit_in_both_directions(self):
  r=self.invoke('''to_named :: (a:u.Quantity(UnitQuotient(u.Metre,u.Second),f64))->u.Quantity(Speed,f64) #export_c {return convert_unit(Speed,a);}
to_derived :: (a:u.Quantity(Speed,f64))->u.Quantity(UnitQuotient(u.Metre,u.Second),f64) #export_c {return convert_unit(UnitQuotient(u.Metre,u.Second),a);}
to_one :: (a:u.Quantity(u.HalfMetre,f64),b:u.Quantity(u.Metre,f64))->u.Quantity(UnitOne,f64) #export_c {return convert_unit(UnitOne,a/b);}
calc :: ()->f64 {x:u.Quantity(Speed,f64)=u.Quantity.{value=3};h:u.Quantity(u.HalfMetre,f64)=u.Quantity.{value=6};m:u.Quantity(u.Metre,f64)=u.Quantity.{value=3};return to_named(to_derived(x)).value+to_one(h,m).value;}
V :: #run calc();#assert V==4;''');self.assertEqual(r.returncode,0,r.stderr)
 def test_result_keeps_left_family_and_same_carrier(self):
  base='B :: struct($U:copyable,$R:numeric) @builtin_quantity {value:$R;}\n'
  r=self.invoke(base+'f :: (a:B(u.Metre,f64),b:u.Quantity(u.Second,f64))->B(UnitProduct(u.Metre,u.Second),f64) #export_c {return a*b;}');self.assertEqual(r.returncode,0,r.stderr)
  r=self.invoke('f :: (a:u.Quantity(u.Metre,f32),b:u.Quantity(u.Second,f64)) {x:=a*b;}');self.assertNotEqual(r.returncode,0);self.assertIn('E0823',r.stderr)
 def test_physical_dimension_and_reduced_scale_refusals(self):
  for meta in ['length=8,numerator=1','length=0,numerator=1048576']:
   tag='Huge :: struct @unit(mass=0,'+meta+',time=0,angle=0,denominator=1) {}\n'
   body='f :: (a:u.Quantity(Huge,f64),b:u.Quantity('+('u.Metre' if 'length=8' in meta else 'Huge')+',f64)) {x:=a*b;}'
   r=self.invoke(tag+body);self.assertNotEqual(r.returncode,0);self.assertIn('E0829',r.stderr)
 def test_form_capacity_and_power_refusals(self):
  tags=''.join('U'+str(i)+' :: struct @unit(mass=0,length=0,time=0,angle=0,numerator=1,denominator=1) {}\n' for i in range(9))
  args=','.join('a'+str(i)+':u.Quantity(U'+str(i)+',f64)' for i in range(9));expr='*'.join('a'+str(i) for i in range(9))
  r=self.invoke(tags+'f :: ('+args+') {x:='+expr+';}');self.assertNotEqual(r.returncode,0);self.assertIn('E0831',r.stderr)
  body='f :: (x:u.Quantity(Count,f64)) {v0:=x;'+''.join('v'+str(i)+':=v'+str(i-1)+'*v'+str(i-1)+';' for i in range(1,32))+'}'
  r=self.invoke(body);self.assertNotEqual(r.returncode,0);self.assertIn('E0831',r.stderr)
 def test_type_constructor_roles_and_concrete_unit_profile(self):
  for body in ['f :: (x:u.Quantity(UnitProduct(u.Metre),f64)) {}','f :: (x:u.Quantity(UnitProduct(u.Metre,f64),f64)) {}','f :: (x:u.Quantity(UnitProduct($U:copyable,u.Metre),f64)) {}','f :: () {x:=UnitProduct(u.Metre,u.Second);}']:
   r=self.invoke(body);self.assertNotEqual(r.returncode,0);self.assertIn('E0830',r.stderr)
  r=self.invoke('UnitOne :: struct {}');self.assertNotEqual(r.returncode,0);self.assertIn('E0372',r.stderr)
 def test_generated_unit_namespace_cannot_collide_with_a_nominal_name(self):
  r=self.invoke('CoaxUnitone :: struct @unit(mass=0,length=0,time=0,angle=0,numerator=1,denominator=1) {} accept :: (x:u.Quantity(UnitOne,f32))->f32 {return x.value;} wrong :: (x:u.Quantity(CoaxUnitone,f32))->f32 {return accept(x);}')
  self.assertNotEqual(r.returncode,0);self.assertEqual(r.stdout,'');self.assertIn('E0317',r.stderr)
 def test_qualified_intrinsic_names_do_not_ignore_the_alias(self):
  for target in ['bogus.UnitProduct(u.Metre,u.Second)','bogus.UnitOne','UnitProduct(u.Metre,bogus.UnitOne)']:
   r=self.invoke('f :: (x:u.Quantity('+target+',f64)) {}');self.assertNotEqual(r.returncode,0);self.assertEqual(r.stdout,'')
   r=self.invoke('f :: (a:u.Quantity(Speed,f64)) {x:=convert_unit('+target+',a);}');self.assertNotEqual(r.returncode,0);self.assertEqual(r.stdout,'')
 def test_type_constructor_target_has_no_runtime_call_effect(self):
  r=self.invoke('f :: (a:u.Quantity(Speed,f64))->u.Quantity(UnitQuotient(u.Metre,u.Second),f64) #export_c {return convert_unit(UnitQuotient(u.Metre,u.Second),a);}','deps','--effects','--proc','f');self.assertEqual(r.returncode,0,r.stderr)
  v=json.loads(r.stdout);self.assertEqual(v['edge_count'],0);self.assertEqual(len(v['procedures']),1);self.assertEqual(v['procedures'][0]['effects_mask'],0);self.assertEqual(v['procedures'][0]['reachable_procedures'],1)
 def test_native_verification_runtime_preserves_derived_units(self):
  fixture=ROOT/'coax/tests/quantity/native-verification-integration.coax';record=self.work/'native-record.json'
  r=subprocess.run(['python3',str(ROOT/'coax/tools/native_check_execution.py'),str(fixture),'--compiler',str(COMPILER),'--keep',str(self.work/'native-artifacts'),'--out',str(record)],cwd=ROOT,capture_output=True,text=True,timeout=120)
  self.assertEqual(r.returncode,0,r.stdout+r.stderr);value=json.loads(record.read_text());self.assertEqual(value['outcome'],'passed');self.assertEqual(len(value['files']),1)
if __name__=='__main__':unittest.main()
