#!/usr/bin/env python3
"""Wide product: exact128-bit native and actual wasm32 differential checks."""
import ctypes as C
import json, os, random, shutil, subprocess, tempfile, unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'bin/coax'))
SOURCE='''
product :: (a:u64,b:u64)->CoaxU64Product #export_c {return mul_wide(a,b);}
hi :: (a:u64,b:u64)->u64 #export_c {return mul_wide(a,b).hi;}
lo :: (a:u64,b:u64)->u64 #export_c {return mul_wide(a,b).lo;}
one :: (s:[]u64)->u64 {s[0]+=1;return s[0];}
two :: (s:[]u64)->u64 {s[0]+=2;return s[0];}
ordered :: ()->u64 #export_c {s:[1]u64;return mul_wide(one(s[..]),two(s[..])).lo;}
#assert #run mul_wide(cast(u64)18446744073709551615,cast(u64)18446744073709551615).hi==18446744073709551614;
#assert #run mul_wide(cast(u64)18446744073709551615,cast(u64)18446744073709551615).lo==1;
#assert #run size_of(CoaxU64Product)==16;
#assert #run field_info(CoaxU64Product,0).name=="hi";
#assert #run field_info(CoaxU64Product,1).name=="lo";
#assert #run field_info(CoaxU64Product,1).offset==8;
'''
class Pair(C.Structure):_fields_=[('hi',C.c_uint64),('lo',C.c_uint64)]
def cases(n=4000):
    rng=random.Random(0x128340)
    edge=[0,1,2,2**16-1,2**32-1,2**32,2**32+1,2**63-1,2**63,2**64-2,2**64-1]
    return [(a,b) for a in edge for b in edge]+[(rng.getrandbits(64),rng.getrandbits(64)) for _ in range(n)]
def emit(d,mode=[]):
    src=d/'wide.coax';src.write_text(SOURCE)
    result=subprocess.run([str(COMPILER),'c',str(src),'--library','--header',str(d/'wide.h'),*mode],capture_output=True,text=True)
    if result.returncode:raise AssertionError(result.stderr)
    (d/'wide.c').write_text(result.stdout)
    return result.stdout
class WideTests(unittest.TestCase):
    def test_native_both_lowerings_match_independent_bigints(self):
        with tempfile.TemporaryDirectory() as t:
            d=Path(t)
            for mode in ([],['--release']):
                emit(d,mode)
                for fallback in (False,True):
                    path=d/('r' if mode else 'c');path=path.with_suffix('.fallback.dylib' if fallback else '.dylib')
                    cmd=['clang','-std=c11','-O2','-shared','-fPIC',str(d/'wide.c'),'-lm','-o',str(path)]
                    if fallback:cmd+=['-U__SIZEOF_INT128__']
                    subprocess.run(cmd,check=True,capture_output=True)
                    lib=C.CDLL(str(path));lib.product.argtypes=[C.c_uint64,C.c_uint64];lib.product.restype=Pair;lib.ordered.restype=C.c_uint64
                    for a,b in cases():
                        value=a*b;r=lib.product(a,b);self.assertEqual((r.hi,r.lo),(value>>64,value&((1<<64)-1)))
                    self.assertEqual(lib.ordered(),3)
    def test_compile_time_result_and_named_record(self):
        rows=cases(100)
        with tempfile.TemporaryDirectory() as t:
            p=Path(t)/'eval.coax';source=[]
            for i,(a,b) in enumerate(rows):
                value=a*b
                source += [f'P{i} :: #run mul_wide(cast(u64){a},cast(u64){b});',f'#assert P{i}.hi==cast(u64){value>>64} && P{i}.lo==cast(u64){value&((1<<64)-1)};']
            p.write_text('\n'.join(source)+'\nmain :: () {}')
            for mode in ([],['--release']):
                r=subprocess.run([str(COMPILER),'c',str(p),*mode],capture_output=True,text=True)
                self.assertEqual(r.returncode,0,r.stderr)
    def test_native_sanitizers_and_native_128_oracle(self):
        with tempfile.TemporaryDirectory() as t:
            d=Path(t);emit(d)
            (d/'main.c').write_text('''#include "wide.h"
#include <stdint.h>
int main(void){uint64_t s=1;for(int i=0;i<10000;i++){
 s=s*UINT64_C(6364136223846793005)+1;uint64_t a=s;
 s=s*UINT64_C(6364136223846793005)+1;uint64_t b=s;
 unsigned __int128 p=(unsigned __int128)a*b;
 coax_t_CoaxU64Product r=product(a,b);
 if(r.hi!=(uint64_t)(p>>64)||r.lo!=(uint64_t)p)return 1;
}return ordered()!=3;}
''')
            for fallback in (False,True):
                cmd=['clang','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all',str(d/'wide.c'),str(d/'main.c'),'-lm','-o',str(d/'test')]
                if fallback:cmd+=['-U__SIZEOF_INT128__']
                subprocess.run(cmd,check=True,capture_output=True)
                r=subprocess.run([str(d/'test')],capture_output=True,text=True);self.assertEqual(r.returncode,0,r.stderr)
    def test_actual_wasm_uses_split_and_matches_bigints(self):
        clang=Path('/opt/homebrew/opt/llvm/bin/clang');sysroot=Path('/opt/homebrew/opt/wasi-libc/share/wasi-sysroot');resources=Path('/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes')
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists());self.assertIsNotNone(shutil.which('bun'))
        with tempfile.TemporaryDirectory() as t:
            d=Path(t)
            for mode in ([],['--release']):
                emit(d,mode)
                subprocess.run([str(clang),'--target=wasm32-wasip1',f'--sysroot={sysroot}',f'-resource-dir={resources}','-std=c11','-O2','-nostartfiles','-Wl,--no-entry','-Wl,--export=hi','-Wl,--export=lo','-Wl,--export=ordered',str(d/'wide.c'),'-lm','-o',str(d/'wide.wasm')],check=True,capture_output=True)
                pre=subprocess.check_output([str(clang),'--target=wasm32-wasip1',f'--sysroot={sysroot}',f'-resource-dir={resources}','-E',str(d/'wide.c')],text=True)
                body=pre.split('coax_t_CoaxU64Product coax_mul_wide',1)[1].split('/*',1)[0]
                self.assertIn('p00',body);self.assertNotIn('unsigned __int128',body)
                (d/'rows.json').write_text(json.dumps([[str(a),str(b),str((a*b)>>64),str((a*b)&((1<<64)-1))] for a,b in cases(1000)]))
                (d/'run.mjs').write_text('''import {readFileSync} from 'node:fs';
const {instance}=await WebAssembly.instantiate(readFileSync(process.argv[2]),{}),x=instance.exports;
for(const [a,b,hi,lo]of JSON.parse(readFileSync(process.argv[3],'utf8'))){
 if(BigInt.asUintN(64,x.hi(BigInt(a),BigInt(b)))!==BigInt(hi)||BigInt.asUintN(64,x.lo(BigInt(a),BigInt(b)))!==BigInt(lo))throw Error('wide mismatch');}
if(x.ordered()!==3n)throw Error('operand order');
''')
                subprocess.run(['bun',str(d/'run.mjs'),str(d/'wide.wasm'),str(d/'rows.json')],check=True,capture_output=True)
    def test_bad_operands_arity_conflicts_and_metal_refuse(self):
        bad=['mul_wide(cast(u32)1,cast(u32)2)','mul_wide(cast(u64)1,cast(u32)2)','mul_wide(1.0,2.0)','mul_wide(cast(f64)1.0,cast(f64)2.0)','mul_wide(true,false)','mul_wide(1)','mul_wide(1,2,3)']
        with tempfile.TemporaryDirectory() as t:
            p=Path(t)/'bad.coax'
            for expression in bad:
                p.write_text('main :: () {x:='+expression+';}')
                r=subprocess.run([str(COMPILER),'c',str(p)],capture_output=True,text=True);self.assertNotEqual(r.returncode,0);self.assertIn('E0350' if expression=='mul_wide(1.0,2.0)' else 'E0378',r.stderr)
            p.write_text('mul_wide :: (a:u64,b:u64)->u64{return a;} main :: () {}')
            r=subprocess.run([str(COMPILER),'c',str(p)],capture_output=True,text=True);self.assertNotEqual(r.returncode,0);self.assertIn('E0335',r.stderr)
            p.write_text('CoaxU64Product :: struct {hi:u32;} main :: () {x:=mul_wide(1,2);}')
            r=subprocess.run([str(COMPILER),'c',str(p)],capture_output=True,text=True);self.assertNotEqual(r.returncode,0);self.assertIn('E0372',r.stderr)
            p.write_text('main :: () {x:=mul_wide(1,2);}')
            r=subprocess.run([str(COMPILER),'metal',str(p)],capture_output=True,text=True);self.assertNotEqual(r.returncode,0);self.assertIn('E0377',r.stderr)
    def test_unused_programs_keep_runtime_unchanged(self):
        with tempfile.TemporaryDirectory() as t:
            p=Path(t)/'plain.coax';p.write_text('main :: () {}')
            r=subprocess.run([str(COMPILER),'c',str(p)],capture_output=True,text=True);self.assertEqual(r.returncode,0,r.stderr)
            self.assertNotIn('coax_mul_wide',r.stdout);self.assertNotIn('CoaxU64Product',r.stdout)
if __name__=='__main__':unittest.main()
