#!/usr/bin/env python3
"""Actual C11 libraries, scalar math, sequencing and Metal admission gates."""
import ctypes
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from quiet_crash import run as run_quiet_crash

ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT / 'bin/coax')).resolve()

class NativeTests(unittest.TestCase):
    def invoke(self, source, command='c', *options, expect_crash=False):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'test.coax'; path.write_text(source)
            runner = run_quiet_crash if expect_crash else subprocess.run
            return runner([str(COMPILER),command,str(path),*options],capture_output=True,text=True,timeout=15)

    def test_borrowed_return_storage_under_sanitizers(self):
        source='''tail :: (input:[]u8)->[]u8 #export_c #effects(all) {alias:=input;return alias[1..<input.count];}
        text :: (input:[]u8)->string #export_c #effects(all) {return cast(string)input;}
        '''
        with tempfile.TemporaryDirectory() as directory:
            d=Path(directory);path=d/'borrow.coax';path.write_text(source)
            r=subprocess.run([str(COMPILER),'c',str(path),'--library','--header',str(d/'borrow.h')],capture_output=True,text=True)
            self.assertEqual(r.returncode,0,r.stderr);(d/'borrow.c').write_text(r.stdout)
            (d/'host.c').write_text('''#include "borrow.h"
            #include <assert.h>
            int main(void){uint8_t a[]={65,66,67};
              coax_v_u8 all={a,3};coax_v_u8 result=tail(all);
              assert(result.data==a+1 && result.count==2);result.data[0]=90;
              coax_string s=text(all);assert(s.data==(const char*)a && s.count==3 && s.data[1]==90);return 0;}
            ''')
            r=subprocess.run([os.environ.get('CC','cc'),'-std=c11','-O1','-g','-fsanitize=address,undefined',str(d/'borrow.c'),str(d/'host.c'),'-lm','-o',str(d/'borrow')],capture_output=True,text=True)
            self.assertEqual(r.returncode,0,r.stderr)
            self.assertEqual(subprocess.run([str(d/'borrow')],env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'}).returncode,0)

    def test_readonly_descriptors_and_static_backing_under_sanitizers(self):
        source = '''TABLE :: [4]int.[3,5,7,9];
        Row :: struct {n:int;}
        Carrier :: struct {data:[] const Row;}
        row_tail :: (input:[] const Row)->Carrier #export_c #effects(all) {return Carrier.{data=input[1..<input.count]};}
        table :: ()->[] const int #export_c #effects(all) {return TABLE[..];}
        tail :: (input:[] const int)->[] const int #export_c #effects(all) {return input[1..<input.count];}
        total :: (input:[] const int)->int #export_c #effects(all) {answer:=0;for x:input {answer+=x;}return answer;}
        bytes :: (input:[] const u8)->string #export_c #effects(all) {return cast(string)input;}
        '''
        with tempfile.TemporaryDirectory() as directory:
            d=Path(directory); path=d/'readonly.coax'; path.write_text(source)
            for mode in ((), ('--release',)):
                r=subprocess.run([str(COMPILER),'c',str(path),'--library','--header',str(d/'readonly.h'),*mode],capture_output=True,text=True,timeout=30)
                self.assertEqual(r.returncode,0,r.stderr); (d/'readonly.c').write_text(r.stdout)
                header=(d/'readonly.h').read_text()
                self.assertIn('const int64_t *data',header)
                self.assertIn('const uint8_t *data',header)
                self.assertIn('const coax_t_Row *data',header)
                (d/'host.c').write_text('''#include "readonly.h"
                #include <assert.h>
                int main(void){const int64_t a[]={2,4,6};coax_rv_s64 v={a,3};
                  assert(total(v)==12);coax_rv_s64 t=tail(v);
                  assert(t.data==a+1 && t.count==2);
                  coax_rv_s64 first=table();coax_rv_s64 second=table();
                  assert(first.data==second.data && total(first)==24);
                  const uint8_t b[]={65,0,66};coax_rv_u8 bv={b,3};coax_string text=bytes(bv);
                  assert(text.count==3 && text.data[2]==66);
                  const coax_t_Row rows[]={{.n=9},{.n=11}};coax_rv_t_Row rv={rows,2};
                  coax_t_Carrier carrier=row_tail(rv);
                  assert(carrier.data.data==rows+1 && carrier.data.count==1 && carrier.data.data[0].n==11);return 0;}
                ''')
                r=subprocess.run([os.environ.get('CC','cc'),'-std=c11','-O1','-g','-fsanitize=address,undefined',str(d/'readonly.c'),str(d/'host.c'),'-lm','-o',str(d/'host')],capture_output=True,text=True,timeout=30)
                self.assertEqual(r.returncode,0,r.stderr)
                r=subprocess.run([str(d/'host')],capture_output=True,text=True,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'},timeout=15)
                self.assertEqual(r.returncode,0,r.stderr)

    def test_readonly_profiles_and_capability_refusals(self):
        cases=[('fill :: (v:[]int) {v[0]=1;} main :: () {a:[1]int;r:[] const int=a[..];fill(r);}', 'E0317'),
               ('Row :: struct {v:[]int;} Carrier :: struct {rows:[] const Row;} main :: () {}', 'E0950'),
               ('main :: () {a:[1]int;r:[] const int=a[..];s:=cast([]int)r;}', 'E0362')]
        for source,code in cases:
            r=self.invoke(source,'check');self.assertNotEqual(r.returncode,0);self.assertIn(code,r.stderr)
        source='run :: (index:int, data:[] const f32) #export_c #effects(all) {}'
        for command in ('cuda','metal-compute','wgsl','spirv','metal'):
            r=self.invoke(source,command);self.assertNotEqual(r.returncode,0);self.assertIn('E0951',r.stderr)

    def test_float_types_and_compile_time(self):
        source = '''#import "Basic";
        A :: #run sqrt(9.0f32); #assert A == 3.0f32;
        #assert #run pow(2.0,3.0) == 8.0;
        #assert #run len("a\\0b") == 3;
        #assert #run byte_at("a\\0b",1) == 0;
        main :: () { a:=1.25f32; a*=2.0f32; b:=f64(a); print("% % % % %\\n", a,b,int(-2.9),len("abc"),byte_at("abc",2)); }
        '''
        for mode in ((),('--release',)):
            r=self.invoke(source,'run',*mode)
            self.assertEqual(r.returncode,0,r.stderr); self.assertEqual(r.stdout,'2.5 2.5 -2 3 99\n')

    def test_numeric_and_abi_diagnostics(self):
        # A bare 1.0 is untyped and fits f32; a suffixed literal is typed.
        cases=[('main :: () {x:f32=1.0f64;}','E0325'),
               ('main :: () {x:=pow(1.0f32,2.0f64);}','E0375'),
               ('main :: () {n:=4; x:=sqrt(n);}','E0375'),
               ('#assert #run sqrt(-1.0) == 0.0; main :: () {}','E0412'),
               ('#assert #run byte_at("a",1)==0; main :: () {}','E0410'),
               ('#assert f32(1e100) == 0.0; main :: () {}','E0350'),
               ('A :: 1e100; #assert #run f32(A * 1.0f64) == 0.0; main :: () {}','E0411'),
               ('x :: () -> int #foreign #effects(all); A :: #run x(); main :: () {}','E0404'),
               ('x :: (s:string) #foreign #effects(all); main :: () {}','E0376'),
               ('x :: () -> string #foreign #effects(all); main :: () {}','E0376'),
               ('coax_x :: () #foreign #effects(all); main :: () {}','E0376'),
               ('sqrt :: (x:f64)->f64 { return x; } main :: () {}','E0335'),
               ('main :: () { x:=1e999; }','E0109'),
               ('main :: () { x:=1e39f32; }','E0109'),
               ('main :: () { x:=1.0f32junk; }','E0109')]
        for source,code in cases:
            with self.subTest(code=code,source=source):
                r=self.invoke(source);self.assertNotEqual(r.returncode,0);self.assertIn(code,r.stderr)

    def test_runtime_guards(self):
        # Run-time values: the same expressions over constants fail while compiling.
        for expr,message in [('int(big)','cast out of range'),('f32(big * big * big)','cast out of range'),
                             ('byte_at("a",minus)','out of bounds'),('1 << (minus + 65)','shift amount out of range'),
                             ('1 >> minus','shift amount out of range')]:
            r=self.invoke('main :: () { big := 1e30; minus := -1; x:='+expr+'; }','run',expect_crash=True)
            self.assertNotEqual(r.returncode,0);self.assertIn(message,r.stderr)

    def test_fixed_abi_and_ordered_effects(self):
        source='''next :: () -> int #foreign #effects(all);
        pair :: (a:int,b:int)->int { return a*10+b; }
        result :: ()->int #export_c #effects(all) {
          a:=pair(next(),next()); b:=next()*10+next();
          skip:=false && next()==99;
          loop:=0; while next()<7 { loop+=1; }
          return a*1000+b*10+loop;
        }
        scalar :: (a:f32,b:f64,c:bool)->f64 #export_c #effects(all) { if c { return f64(a)+b; } return 0.0; }
        '''
        for opt in ('-O0','-O2'):
            with tempfile.TemporaryDirectory() as directory:
                d=Path(directory);path=d/'test.coax';path.write_text(source)
                r=subprocess.run([str(COMPILER),'c',str(path),'--library','--header',str(d/'test.h')],capture_output=True,text=True)
                self.assertEqual(r.returncode,0,r.stderr)
                self.assertNotIn('({',r.stdout)
                (d/'test.c').write_text(r.stdout)
                (d/'host.c').write_text('#include "test.h"\nstatic int64_t n; int64_t next(void){return ++n;}\nint main(void){return result()==12342 && scalar(1.25f,2.5,1)==3.75 ? 0:1;}')
                r=subprocess.run([os.environ.get('CC','cc'),'-std=c11','-pedantic-errors',opt,str(d/'test.c'),str(d/'host.c'),'-lm','-o',str(d/'test')],capture_output=True,text=True)
                self.assertEqual(r.returncode,0,r.stderr)
                self.assertEqual(subprocess.run([str(d/'test')]).returncode,0)

    def test_compound_assignment_through_a_view_under_sanitizers(self):
        # tests/pass/compound_view.coax: places whose indexes call functions
        # stay C lvalues; the program is pedantic C11 and clean under ASan/UBSan.
        source=(ROOT/'tests/pass/compound_view.coax')
        want=(ROOT/'tests/pass/compound_view.out').read_text()
        with tempfile.TemporaryDirectory() as directory:
            d=Path(directory)
            r=subprocess.run([str(COMPILER),'c',str(source)],capture_output=True,text=True,timeout=30)
            self.assertEqual(r.returncode,0,r.stderr)
            (d/'test.c').write_text(r.stdout)
            r=subprocess.run([os.environ.get('CC','cc'),'-std=c11','-pedantic-errors','-O1','-g','-fsanitize=address,undefined',
                              '-fno-omit-frame-pointer','-fno-sanitize-recover=all',str(d/'test.c'),'-lm','-o',str(d/'test')],capture_output=True,text=True)
            self.assertEqual(r.returncode,0,r.stderr)
            run=subprocess.run([str(d/'test')],capture_output=True,text=True,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'})
            self.assertEqual(run.returncode,0,run.stderr);self.assertEqual(run.stdout,want)

    def test_bit_semantics(self):
        source='''#import "Basic";
        #assert (1 << 4 | 3) == 19;
        #assert (-8 >> 2) == -2;
        main :: () {one := 1; print("% % %\\n", -8 >> 2, (7 ^ 3) & 5, one << 63);}
        '''
        r=self.invoke(source,'run');self.assertEqual(r.returncode,0,r.stderr)
        self.assertEqual(r.stdout,'-2 4 -9223372036854775808\n')

    def test_metal_scalar_restrictions(self):
        good='f :: (x:f32)->f32 #export_c #effects(all) { return sin(x)+1.0f32; }'
        r=self.invoke(good,'metal');self.assertEqual(r.returncode,0,r.stderr)
        self.assertIn('metal_stdlib',r.stdout);self.assertNotIn('stdio',r.stdout)
        for source in ['f :: (x:f64)->f64 {return x;}', 'f :: (x:int)->int {return x;}',
                       'f :: () #foreign #effects(all);', 'f :: () {while true {}}',
                       'f :: (x:f32)->f32 {return f(x);}',
                       'f :: () {s:="host";}']:
            r=self.invoke(source,'metal');self.assertNotEqual(r.returncode,0);self.assertIn('E0377',r.stderr)

if __name__=='__main__': unittest.main()
