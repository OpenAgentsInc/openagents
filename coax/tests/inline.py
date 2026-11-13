#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: the host C compiler and nm decide what the emitted C does, outside Coax)
"""#inline procedures: the emitted C, the same results with and without the note, and the refusals."""
from pathlib import Path
import ctypes
import os
import shutil
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',str(ROOT/'bin/coax')))
CC=os.environ.get('CC') or shutil.which('cc') or 'clang'
# mix is large enough that the C compiler would not inline it into both callers on its own.
SOURCE='''
mix :: (a:u64,b:u64)->u64 NOTE #effects(all) {
  x:=a;
  for i:0..<8 {p:=mul_wide(x^b,0x9E3779B97F4A7C15+cast(u64)i);x=rotl(p.hi^p.lo,cast(u64)(i+7))^add_wrap(x,b);}
  return x;
}
next :: (s:[]u64)->u64 #effects(all) {v:=s[0];s[0]=v+1;return v;}
fold :: (a:u64,b:u64)->u64 #export_c #effects(all) {return mix(mix(a,b),mix(b,a));}
twice :: (a:u64)->u64 #export_c #effects(all) {return mix(a,a);}
order :: ()->u64 #export_c #effects(all) {s:[1]u64;s[0]=5;return mix(next(s[..]),next(s[..]));}
MIXED :: #run mix(1,2);
#assert MIXED==#run mix(1,2) && MIXED!=#run mix(2,1);
#test "an inline procedure is an ordinary procedure to a check" {#assert mix(3,4)==mix(3,4) && mix(0,0)!=mix(0,1);}
'''
REFUSED={
 'with #export_c':('f :: (a:u64)->u64 #inline #export_c #effects(all) {return a;}','#inline is for a procedure only this program calls, not #export_c or #foreign'),
 'with #foreign':('f :: (a:u64)->u64 #foreign #inline #effects(all);','#inline is for a procedure only this program calls, not #export_c or #foreign'),
 'on main':('main :: () #inline {}','#inline is not supported on main'),
 'on a procedure that calls itself':('down :: (a:u64)->u64 #inline #effects(all) {if a==0 {return 0;}return down(a-1);}\ng :: (a:u64)->u64 #export_c #effects(all) {return down(a);}','#inline is not supported on a procedure that calls itself, directly or through others'),
 'on a procedure that reaches itself':('even :: (a:u64)->bool #inline #effects(all) {if a==0 {return true;}return odd(a-1);}\nodd :: (a:u64)->bool #effects(all) {if a==0 {return false;}return even(a-1);}\ng :: (a:u64)->bool #export_c #effects(all) {return even(a);}','#inline is not supported on a procedure that calls itself, directly or through others'),
}

def emit(directory,note,*mode):
    src=directory/('noted.coax' if note else 'plain.coax');src.write_text(SOURCE.replace('NOTE',note))
    p=subprocess.run([str(COMPILER),'c',str(src),'--library',*mode],capture_output=True,text=True)
    assert p.returncode==0,p.stderr
    return p.stdout

class Inline(unittest.TestCase):
    def test_the_emitted_c_marks_only_the_noted_procedure(self):
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp);noted=emit(d,'#inline');plain=emit(d,'')
            self.assertNotIn('COAX_INLINE',plain)
            self.assertIn('#define COAX_INLINE static inline __attribute__((always_inline))',noted)
            self.assertIn('#if defined(__GNUC__) && !defined(__wasm__)',noted)
            self.assertIn('#define COAX_INLINE static\n',noted)
            # The declaration and the definition, and nothing else: next keeps its plain static.
            self.assertEqual(noted.count('COAX_INLINE uint64_t coax_p_mix('),2)
            self.assertEqual(noted.count('COAX_INLINE '),4)   # two definitions of the macro, two uses
            self.assertIn('static uint64_t coax_p_next(',noted)
            self.assertLess(noted.index('#define COAX_INLINE static inline'),noted.index('COAX_INLINE uint64_t coax_p_mix('))
            # Apart from the macro and the two markers, the C is the same.
            start=noted.index('/* #inline');end=noted.index('#endif\n',start)+len('#endif\n')
            self.assertEqual((noted[:start]+noted[end:]).replace('COAX_INLINE uint64_t coax_p_mix(','static uint64_t coax_p_mix(').replace('noted.coax','plain.coax'),plain)
    def test_results_are_the_same_and_the_body_is_copied_at_every_level(self):
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp);values=[0,1,2,0xFFFFFFFFFFFFFFFF,0x0123456789ABCDEF,1<<63]
            for mode in ([],['--release']):
                got={}
                for note in ('','#inline'):
                    for level in ('-O0','-O2'):
                        c=d/'unit.c';c.write_text(emit(d,note,*mode))
                        obj=d/'unit.o';subprocess.run([CC,'-std=c11',level,'-c',str(c),'-o',str(obj)],check=True,capture_output=True)
                        symbols=subprocess.run(['nm',str(obj)],check=True,capture_output=True,text=True).stdout
                        # With the note no copy of mix stands alone, even unoptimized; without it, -O0 keeps the call.
                        if note: self.assertNotIn('coax_p_mix',symbols,(mode,level))
                        elif level=='-O0': self.assertIn('coax_p_mix',symbols,mode)
                        lib=d/(('n' if note else 'p')+level+('r' if mode else 'c')+'.dylib')
                        subprocess.run([CC,'-std=c11',level,'-shared','-fPIC',str(c),'-lm','-o',str(lib)],check=True,capture_output=True)
                        h=ctypes.CDLL(str(lib))
                        for f in (h.fold,h.twice,h.order):f.restype=ctypes.c_uint64
                        h.fold.argtypes=[ctypes.c_uint64,ctypes.c_uint64];h.twice.argtypes=[ctypes.c_uint64]
                        got[(note,level)]=[h.fold(a,b) for a in values for b in values]+[h.twice(a) for a in values]+[h.order()]
                first=got[('','-O0')]
                for key,result in got.items():self.assertEqual(result,first,(mode,key))
                self.assertGreater(len(set(first)),30)
    def test_each_instance_of_a_generic_procedure_carries_the_note(self):
        with tempfile.TemporaryDirectory() as tmp:
            src=Path(tmp)/'generic.coax'
            src.write_text('pick :: (a:$T,b:$T,first:bool)->$T #inline {return ifx first then a else b;}\n'
                           'g :: (a:u64,b:u32)->u64 #export_c #effects(all) {return pick(a,a+1,false)+cast(u64)pick(b,b+1,true);}\n')
            p=subprocess.run([str(COMPILER),'c',str(src),'--library'],capture_output=True,text=True)
            self.assertEqual(p.returncode,0,p.stderr)
            self.assertEqual(p.stdout.count('COAX_INLINE uint64_t coax_p_pick'),2)
            self.assertEqual(p.stdout.count('COAX_INLINE uint32_t coax_p_pick'),2)
    def test_checks_run_an_inline_procedure(self):
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp);src=d/'noted.coax';src.write_text(SOURCE.replace('NOTE','#inline'))
            p=subprocess.run([str(COMPILER),'test',str(src),'--library'],capture_output=True,text=True)
            self.assertEqual(p.returncode,0,p.stdout+p.stderr)
            self.assertIn('1 check, 1 passed, 0 failed',p.stdout)
    def test_refusals(self):
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp)
            for name,(source,message) in REFUSED.items():
                src=d/'refused.coax';src.write_text(source+'\n')
                p=subprocess.run([str(COMPILER),'c',str(src),'--library'],capture_output=True,text=True)
                self.assertNotEqual(p.returncode,0,name)
                self.assertIn('E0210',p.stderr,name);self.assertIn(message,p.stderr,name)

if __name__=='__main__':
    unittest.main()
