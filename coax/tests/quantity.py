#!/usr/bin/env python3
"""Focused R12 quantity syntax, checked semantics and native ABI fixtures."""
from pathlib import Path
import os,subprocess,tempfile,unittest
ROOT=Path(__file__).resolve().parents[2]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
PRELUDE='''Metre :: struct @unit(mass=0,length=1,time=0,angle=0,numerator=1,denominator=1) {}
HalfMetre :: struct @unit(mass=0,length=1,time=0,angle=0,numerator=1,denominator=2) {}
Second :: struct @unit(mass=0,length=0,time=1,angle=0,numerator=1,denominator=1) {}
Centimetre :: struct @unit(mass=0,length=1,time=0,angle=0,numerator=1,denominator=100) {}
Quantity :: struct($U:copyable,$R:numeric) @builtin_quantity {value:$R;}
'''
class Quantities(unittest.TestCase):
 def setUp(self):self.temp=tempfile.TemporaryDirectory(prefix='coax-quantity-');self.addCleanup(self.temp.cleanup);self.work=Path(self.temp.name)
 def compile(self,body):
  p=self.work/'case.coax';p.write_text(PRELUDE+body+'\nmain :: () {}\n');return subprocess.run([str(COMPILER),'c',str(p),'--library'],cwd=ROOT,capture_output=True,text=True,timeout=90)
 def test_builtin_arithmetic_and_compile_time_values(self):
  r=self.compile('''calc :: ()->f64 {a:Quantity(Metre,f64)=Quantity.{value=4};b:Quantity(Metre,f64)=Quantity.{value=1};c:=a+b;d:=a-b;e:=c*2.0f64;f:=2.0f64*d;g:=e/2.0f64;#assert a>b && a>=b && b<a && b<=a && a!=b && a==a;return c.value+d.value+e.value+f.value+g.value;}
V :: #run calc(); #assert V==29;''');self.assertEqual(r.returncode,0,r.stderr)
 def test_explicit_binary_scale_conversion(self):
  r=self.compile('calc :: ()->f64 {a:Quantity(HalfMetre,f64)=Quantity.{value=6};b:=convert_unit(Metre,a);return b.value;} V :: #run calc();#assert V==3;');self.assertEqual(r.returncode,0,r.stderr)
 def test_refusals_are_type_directed(self):
  for body,code in [
   ('f :: () {a:Quantity(Metre,f64);b:Quantity(Second,f64);c:=a+b;}','E0822'),
   ('f :: () {a:Quantity(Metre,f32);b:Quantity(Metre,f64);c:=a+b;}','E0823'),
   ('f :: () {a:Quantity(Metre,f64);c:=a*a;}','E0824'),
   ('f :: () {a:Quantity(Metre,int);}','E0821'),
   ('f :: () {a:Quantity(Second,f64);b:=convert_unit(Metre,a);}','E0825'),
   ('f :: () {a:Quantity(Centimetre,f64);b:=convert_unit(Metre,a);}','E0826'),
  ]:
   with self.subTest(body=body):r=self.compile(body);self.assertNotEqual(r.returncode,0);self.assertEqual(r.stdout,'');self.assertIn(code,r.stderr)
 def test_plain_structs_do_not_gain_quantity_operators(self):
  r=self.compile('Plain :: struct {value:f64;} f :: () {a:Plain;b:Plain;c:=a+b;}');self.assertNotEqual(r.returncode,0);self.assertIn('E0309',r.stderr)
 def test_quantities_are_ordered_but_not_numeric_constraints(self):
  r=self.compile('maxq :: (a:$T:ordered,b:$T)->$T {if a<b {return b;}return a;} f :: ()->f64 {a:Quantity(Metre,f64)=Quantity.{value=2};b:Quantity(Metre,f64)=Quantity.{value=3};return maxq(a,b).value;} V :: #run f();#assert V==3;');self.assertEqual(r.returncode,0,r.stderr)
  r=self.compile('closed :: (a:$T:numeric)->$T {return a*a;} f :: () {a:Quantity(Metre,f64);b:=closed(a);}');self.assertNotEqual(r.returncode,0);self.assertIn('E0806',r.stderr)
 def test_schema1_reflection_refuses_without_changing_size(self):
  r=self.compile('S :: #run size_of(Quantity(Metre,f32));#assert S==4;');self.assertEqual(r.returncode,0,r.stderr)
  for query in ['type_info(Quantity(Metre,f32))','field_info(Quantity(Metre,f32),0)']:
   r=self.compile('X :: #run '+query+';');self.assertNotEqual(r.returncode,0);self.assertIn('E0827',r.stderr)
 def test_device_output_is_explicitly_unqualified(self):
  p=self.work/'device.coax';p.write_text(PRELUDE+'k :: (x:Quantity(Metre,f32))->Quantity(Metre,f32) #export_c {return x;}')
  for target in ['metal','metal-compute','cuda','spirv','wgsl']:
   with self.subTest(target=target):
    r=subprocess.run([str(COMPILER),target,str(p),'--library'],cwd=ROOT,capture_output=True,text=True,timeout=90);self.assertNotEqual(r.returncode,0);self.assertIn('E0828',r.stderr)
 def test_malformed_annotations_and_nonunit_tags_are_refused(self):
  for body in [
   'Bad :: struct @unit(mass=9,length=0,time=0,angle=0,numerator=1,denominator=1) {}',
   'Bad :: struct @unit(mass=0,length=0,time=0,angle=0,numerator=1,denominator=0) {}',
   'Bad :: struct @unit(mass=0,length=0,time=0,angle=0,numerator=1,numerator=1) {}',
   'Bad :: struct @unit(mass=0,length=0,time=0,angle=0,numerator=1) {}',
   'Bad :: struct @unit(mass=0,length=0,time=0,angle=0,numerator=1,denominator=1) {x:f32;}',
   'Bad :: struct @something {}',
  ]:
   with self.subTest(body=body):r=self.compile(body);self.assertNotEqual(r.returncode,0);self.assertIn('E0820',r.stderr)
  for body in ['Tag :: struct {} f :: () {x:Quantity(Tag,f32);}','Bad :: struct($A,$B:numeric) @builtin_quantity {value:$A;}']:
   r=self.compile(body);self.assertNotEqual(r.returncode,0);self.assertIn('E0821',r.stderr)
 def test_explicit_conversion_keeps_module_alias_scope(self):
  (self.work/'units.coax').write_text('#module "test.quantity.units";\n'+PRELUDE)
  p=self.work/'client.coax';p.write_text('#module "test.quantity.client"; #load "units.coax" as u; f :: (a:u.Quantity(u.HalfMetre,f32))->u.Quantity(u.Metre,f32) #export_c {return convert_unit(u.Metre,a);}')
  r=subprocess.run([str(COMPILER),'c',str(p),'--library'],cwd=ROOT,capture_output=True,text=True,timeout=90);self.assertEqual(r.returncode,0,r.stderr)
if __name__=='__main__':unittest.main()
