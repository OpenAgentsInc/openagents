#!/usr/bin/env python3
"""Defined unsigned wrapping/rotation at compile time and emitted targets."""
from pathlib import Path
import ctypes
import os
import json
import shutil
import random
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',str(ROOT/'bin/coax')))
OPS=('add_wrap','sub_wrap','mul_wrap','rotl','rotr')
SOURCE='\n'.join(f'w_{op}_{bits} :: (a:u{bits},b:u{bits})->u{bits} #export_c {{return {op}(a,b);}}' for bits in (32,64) for op in OPS)+'''
#assert #run add_wrap(cast(u64)18446744073709551615,cast(u64)1)==0;
#assert #run sub_wrap(cast(u32)0,cast(u32)1)==4294967295;
#assert #run mul_wrap(cast(u64)18446744073709551615,cast(u64)18446744073709551615)==1;
#assert #run rotl(cast(u32)1,cast(u32)32)==1;
#assert #run rotr(cast(u64)1,cast(u64)1)==9223372036854775808;
next :: (s:[]u64)->u64 {v:=s[0];s[0]=v+1;return v;}
order :: ()->u64 #export_c {s:[1]u64;return sub_wrap(next(s[..]),next(s[..]));}
'''

def oracle(op,a,b,bits):
    mask=(1<<bits)-1;k=b&(bits-1)
    if op=='add_wrap':return (a+b)&mask
    if op=='sub_wrap':return (a-b)&mask
    if op=='mul_wrap':return (a*b)&mask
    if op=='rotl':return ((a<<k)|(a>>((-k)&(bits-1))))&mask
    return ((a>>k)|(a<<((-k)&(bits-1))))&mask

class Wrapping(unittest.TestCase):
    def test_native_matches_independent_integer_oracle_in_both_modes(self):
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp);src=d/'w.coax';src.write_text(SOURCE)
            for mode in ([],['--release']):
                p=subprocess.run([str(COMPILER),'c',str(src),'--library',*mode],capture_output=True,text=True)
                self.assertEqual(p.returncode,0,p.stderr)
                (d/'w.c').write_text(p.stdout)
                binary=d/('release.dylib' if mode else 'checked.dylib')
                subprocess.run(['clang','-std=c11','-O2','-shared','-fPIC',str(d/'w.c'),'-lm','-o',str(binary)],check=True,capture_output=True)
                lib=ctypes.CDLL(str(binary))
                rng=random.Random(0x340)
                for bits in (32,64):
                    word=ctypes.c_uint32 if bits==32 else ctypes.c_uint64
                    maximum=(1<<bits)-1
                    values=[0,1,2,bits-1,bits,bits+1,maximum>>1,maximum]
                    pairs=[(a,b) for a in values for b in values]+[(rng.getrandbits(bits),rng.getrandbits(bits)) for _ in range(2000)]
                    for op in OPS:
                        fn=getattr(lib,f'w_{op}_{bits}');fn.argtypes=[word,word];fn.restype=word
                        for a,b in pairs:self.assertEqual(fn(a,b),oracle(op,a,b,bits),(mode,op,bits,a,b))
                lib.order.restype=ctypes.c_uint64
                self.assertEqual(lib.order(),(1<<64)-1)
    def test_compile_time_matches_integer_oracle(self):
        rng=random.Random(0x34044);lines=[]
        for bits in (32,64):
            mask=(1<<bits)-1
            pairs=[(0,mask),(mask,1),(mask,mask),(1,bits),(1,0)]+[(rng.getrandbits(bits),rng.getrandbits(bits)) for _ in range(25)]
            for op in OPS:
                for a,b in pairs:
                    lines.append(f'#assert #run {op}(cast(u{bits}){a},cast(u{bits}){b}) == cast(u{bits}){oracle(op,a,b,bits)};')
        with tempfile.TemporaryDirectory() as tmp:
            src=Path(tmp)/'eval.coax';src.write_text('\n'.join(lines)+'\nmain :: () {}')
            for mode in ([],['--release']):
                p=subprocess.run([str(COMPILER),'c',str(src),*mode],capture_output=True,text=True)
                self.assertEqual(p.returncode,0,p.stderr)
    def test_emitted_native_under_sanitizers(self):
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp);src=d/'wrap.coax';src.write_text(SOURCE)
            p=subprocess.run([str(COMPILER),'c',str(src),'--library','--header',str(d/'wrap.h')],capture_output=True,text=True)
            self.assertEqual(p.returncode,0,p.stderr);(d/'wrap.c').write_text(p.stdout)
            lines=['#include <stdint.h>','#include "wrap.h"','int main(void){']
            rng=random.Random(0x65)
            for bits in (32,64):
                mask=(1<<bits)-1
                pairs=[(0,0),(0,mask),(mask,1),(mask,mask)]+[(rng.getrandbits(bits),rng.getrandbits(bits)) for _ in range(50)]
                for op in OPS:
                    for a,b in pairs:
                        want=oracle(op,a,b,bits)
                        lines.append(f'if(w_{op}_{bits}(UINT{bits}_C({a}),UINT{bits}_C({b}))!=UINT{bits}_C({want}))return 1;')
            lines+=['return order()!=UINT64_MAX;}'];(d/'host.c').write_text('\n'.join(lines))
            subprocess.run(['clang','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all',str(d/'wrap.c'),str(d/'host.c'),'-lm','-o',str(d/'host')],check=True,capture_output=True)
            p=subprocess.run([str(d/'host')],capture_output=True,text=True)
            self.assertEqual(p.returncode,0,p.stderr)
    def test_actual_wasm_matches_oracle(self):
        clang=Path(os.environ.get('COAXIAL_CLANG','/opt/homebrew/opt/llvm/bin/clang'))
        sysroot=Path(os.environ.get('COAXIAL_SYSROOT','/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'))
        resources=Path(os.environ.get('COAXIAL_RESOURCES','/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'))
        self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(),'required wasm32 toolchain unavailable')
        self.assertIsNotNone(shutil.which('bun'),'required Wasm executor unavailable')
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp);src=d/'wrap.coax';src.write_text(SOURCE)
            p=subprocess.run([str(COMPILER),'c',str(src),'--library'],capture_output=True,text=True)
            self.assertEqual(p.returncode,0,p.stderr);(d/'wrap.c').write_text(p.stdout)
            exports=[f'-Wl,--export=w_{op}_{bits}' for bits in (32,64) for op in OPS]
            subprocess.run([str(clang),'--target=wasm32-wasip1',f'--sysroot={sysroot}',f'-resource-dir={resources}','-std=c11','-O2','-nostartfiles','-Wl,--no-entry',*exports,str(d/'wrap.c'),'-lm','-o',str(d/'wrap.wasm')],check=True,capture_output=True)
            rng=random.Random(0x44);rows=[]
            for bits in (32,64):
                mask=(1<<bits)-1
                pairs=[(0,mask),(mask,1),(mask,mask),(1,bits),(1,bits+1),(1,0)]+[(rng.getrandbits(bits),rng.getrandbits(bits)) for _ in range(100)]
                for op in OPS:
                    rows.extend([f'w_{op}_{bits}',bits,str(a),str(b),str(oracle(op,a,b,bits))] for a,b in pairs)
            (d/'cases.json').write_text(json.dumps(rows))
            (d/'run.mjs').write_text("""import {readFileSync} from 'node:fs';
const {instance}=await WebAssembly.instantiate(readFileSync(process.argv[2]),{});
const rows=JSON.parse(readFileSync(process.argv[3],'utf8'));
for(const [name,bits,a,b,want] of rows){
 const v=instance.exports[name](bits===64?BigInt(a):Number(a),bits===64?BigInt(b):Number(b));
 const got=bits===64?BigInt.asUintN(64,v).toString():String(v>>>0);
 if(got!==want)throw new Error(`${name} ${a} ${b}: ${got} != ${want}`);
}
console.log(rows.length);
""")
            p=subprocess.run(['bun',str(d/'run.mjs'),str(d/'wrap.wasm'),str(d/'cases.json')],capture_output=True,text=True)
            self.assertEqual(p.returncode,0,p.stderr);self.assertEqual(p.stdout.strip(),str(len(rows)))
    def test_rejects_wrong_width_signed_float_and_arity(self):
        with tempfile.TemporaryDirectory() as tmp:
            src=Path(tmp)/'bad.coax'
            for op in OPS:
                for args in ('','cast(u32)1','cast(u32)1,cast(u64)2','cast(u8)1,cast(u8)2','cast(s64)1,cast(s64)2','1.0,2.0','cast(u32)1,cast(u32)2,cast(u32)3'):
                    src.write_text(f'main :: () {{x:={op}({args});}}')
                    p=subprocess.run([str(COMPILER),'c',str(src)],capture_output=True,text=True)
                    self.assertNotEqual(p.returncode,0,(op,args))
                    self.assertIn('E0378',p.stderr,(op,args,p.stderr))
if __name__=='__main__':unittest.main()
