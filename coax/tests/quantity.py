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
 def test_explicit_family_interoperation_on_one_imported_unit(self):
  source='#load "'+str(ROOT/'coax/lib/quantity.coax')+'" as u;\n'+'''A :: struct($U:copyable,$R:numeric) @builtin_quantity {value:$R;}
B :: struct($U:copyable,$R:numeric) @builtin_quantity {value:$R;}
family_ab :: (a:A(u.Metre,f64),b:B(u.Metre,f64))->A(u.Metre,f64) #export_c {return a+b;}
family_ba :: (b:B(u.Metre,f64),a:A(u.Metre,f64))->B(u.Metre,f64) #export_c {return b+a;}
family_scale :: (s:f64,b:B(u.Metre,f64))->B(u.Metre,f64) #export_c {return s*b;}
family_convert_a :: (a:A(u.Metre,f64))->A(u.HalfMetre,f64) #export_c {return convert_unit(u.HalfMetre,a);}
family_convert_b :: (b:B(u.Metre,f64))->B(u.HalfMetre,f64) #export_c {return convert_unit(u.HalfMetre,b);}
family_value :: ()->f64 {a:A(u.Metre,f64)=A.{value=3};b:B(u.Metre,f64)=B.{value=4};x:=family_ab(a,b);y:=family_ba(b,a);#assert x==y;return x.value+y.value+family_scale(2.0,b).value+family_convert_a(a).value+family_convert_b(b).value;}
FamilyAnswer :: #run family_value();#assert FamilyAnswer==36;
'''
  p=self.work/'families.coax';p.write_text(source);header=self.work/'families.h'
  r=subprocess.run([str(COMPILER),'c',str(p),'--library','--header',str(header)],cwd=ROOT,capture_output=True,text=True,timeout=90);self.assertEqual(r.returncode,0,r.stderr)
  h=header.read_text()
  for signature in ['double family_ab(double a, double b);','double family_ba(double b, double a);','double family_scale(double s, double b);','double family_convert_a(double a);','double family_convert_b(double b);']:self.assertIn(signature,h)
  p.write_text(source+'bad_family_assignment :: (a:A(u.Metre,f64))->B(u.Metre,f64) {return a;}\n')
  r=subprocess.run([str(COMPILER),'c',str(p),'--library'],cwd=ROOT,capture_output=True,text=True,timeout=90);self.assertNotEqual(r.returncode,0);self.assertEqual(r.stdout,'');self.assertIn('A(',r.stderr);self.assertIn('B(',r.stderr)
 def test_bound_generic_literals_preserve_quantity_boundary(self):
  common='scale :: (s:$R:numeric,q:Quantity(Metre,$R))->Quantity(Metre,$R) {return q*s;} clamp :: (x:$T:ordered,lo:$T,hi:$T)->$T {if x<lo {return lo;}if x>hi {return hi;}return x;} '
  r=self.compile(common+'calc :: ()->f32 {q:Quantity(Metre,f32)=Quantity.{value=3};lo:Quantity(Metre,f32)=Quantity.{value=1};hi:Quantity(Metre,f32)=Quantity.{value=5};s:=scale(2,q);v:=clamp(s,lo,hi);return s.value+v.value;} Answer :: #run calc();#assert Answer==11;')
  self.assertEqual(r.returncode,0,r.stderr)
  for call in ['clamp(q,0,1)','clamp(0,q,1)']:
   with self.subTest(call=call):
    r=self.compile(common+'bad :: () {q:Quantity(Metre,f32)=Quantity.{value=3};v:='+call+';}')
    self.assertNotEqual(r.returncode,0);self.assertEqual(r.stdout,'');self.assertIn('E',r.stderr)
if __name__=='__main__':unittest.main()
