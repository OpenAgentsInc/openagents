#!/usr/bin/env python3
"""Actual build-profile parser, native layouts and target/diagnostic boundaries."""
import ctypes as C
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
COMPILER=Path(os.environ.get('COAX_BIN',ROOT/'bin/coax'))
def run(*args,ok=True):
 env=os.environ.copy()
 if '--target=wasm32-wasip1' in args:
  env['PATH']='/opt/homebrew/opt/lld/bin:'+env.get('PATH','')
 r=subprocess.run([str(a)for a in args],capture_output=True,text=True,timeout=120,env=env)
 if ok and r.returncode:raise AssertionError(r.stderr or r.stdout)
 return r
PROFILE='''
Cell :: struct { value:u32 = 7; #if BUILD_DEBUG { magnitude:u64 = 2; } }
profile_identity :: (c:Cell)->Cell #export_c {return c;}
profile_size :: ()->int #export_c {return size_of(Cell);}
profile_value :: ()->int #export_c {
 c:Cell;#if BUILD_DEBUG {#assert c.magnitude<8;return cast(int)c.value+cast(int)c.magnitude;}
 else {return cast(int)c.value+100;}
}
profile_fields :: ()->int #export_c {return type_info(Cell).field_count;}
#test "selected evaluator layout and body" {
 #if BUILD_DEBUG {#assert size_of(Cell)==16 && profile_value()==9 && profile_fields()==2;}
 else {#assert size_of(Cell)==4 && profile_value()==107 && profile_fields()==1;}
}
'''
class BuildProfiles(unittest.TestCase):
 def setUp(self):self.tmp=tempfile.TemporaryDirectory(prefix='coax-build-profile-');self.dir=Path(self.tmp.name)
 def tearDown(self):self.tmp.cleanup()
 def source(self,text,name='case.coax'):
  p=self.dir/name;p.write_text(text);return p
 def test_actual_native_layout_defaults_and_conditional_bodies(self):
  source=self.source(PROFILE)
  for mode,flags,size,value,fields in [('development',[],16,9,2),('release',['--release'],4,107,1)]:
   with self.subTest(mode=mode):
    c=self.dir/(mode+'.c');h=self.dir/(mode+'.h');result=run(COMPILER,'c',source,'--library','--header',h,*flags);c.write_text(result.stdout)
    self.assertIn('magnitude',result.stdout)if mode=='development'else self.assertNotIn('magnitude',result.stdout)
    self.assertIn('magnitude',h.read_text())if mode=='development'else self.assertNotIn('magnitude',h.read_text())
    dylib=self.dir/(mode+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',dylib);lib=C.CDLL(str(dylib))
    for function,wanted in [('profile_size',size),('profile_value',value),('profile_fields',fields)]:
     f=getattr(lib,function);f.argtypes=[];f.restype=C.c_int64;self.assertEqual(f(),wanted)
    run(COMPILER,'test',source,*flags)
    # A real C consumer checks the selected generated public header/layout too.
    probe=self.dir/(mode+'-probe.c');probe.write_text(f'#include "{mode}.h"\n#include <assert.h>\nint main(void){{assert(sizeof(coax_t_Cell)=={size});coax_t_Cell c={{.value=5}};coax_t_Cell d=profile_identity(c);assert(d.value==5);assert(profile_value()=={value});return 0;}}')
    exe=self.dir/(mode+'-probe');run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined',c,probe,'-lm','-o',exe);run(exe)
 def test_only_selected_names_types_and_compiletime_work_are_resolved(self):
  source=self.source('''S :: struct { a:u8; #if BUILD_DEBUG { missing:NoSuchType; } }
f :: ()->int #export_c {#if BUILD_DEBUG {return no_such_procedure();}else{return 7;}}
''')
  self.assertNotEqual(run(COMPILER,'c',source,'--library',ok=False).returncode,0)
  release=run(COMPILER,'c',source,'--library','--release').stdout;self.assertNotIn('NoSuchType',release);self.assertNotIn('no_such_procedure',release)
  source=self.source('''bad :: (value:int)->int {return 1/value;}
f :: ()->int #export_c {#if BUILD_DEBUG {return 7;}else{return #run bad(0);}}
''')
  run(COMPILER,'c',source,'--library');r=run(COMPILER,'c',source,'--library','--release',ok=False);self.assertNotEqual(r.returncode,0);self.assertIn('E0407',r.stderr)
  # An inactive reflection/wide reference must not inject helper declarations.
  source=self.source('''f :: ()->int #export_c {#if BUILD_DEBUG {x:=type_info(int);p:=mul_wide(cast(u64)2,cast(u64)3);return x.size+cast(int)p.lo;}else{return 7;}}''')
  release=run(COMPILER,'c',source,'--library','--release').stdout
  for name in ['CoaxTypeInfo','CoaxFieldInfo','CoaxEnumInfo','CoaxU64Product','coax_mul_wide']:self.assertNotIn(name,release)
 def test_both_branches_parse_and_scope_remains_lexical(self):
  for text in ['f :: () {#if BUILD_DEBUG {x:=1} else {}}','S :: struct {#if BUILD_DEBUG {} else {x:u8}}']:
   source=self.source(text)
   for flags in [[],['--release']]:self.assertNotEqual(run(COMPILER,'check',source,*flags,ok=False).returncode,0)
  source=self.source('f :: ()->int #export_c {#if BUILD_DEBUG {x:=1;}else{x:=2;}return x;}')
  for flags in [[],['--release']]:
   result=run(COMPILER,'c',source,'--library',*flags,ok=False);self.assertNotEqual(result.returncode,0);self.assertIn('E0306',result.stderr)
  for text in ['FLAG :: true; f :: () {#if FLAG {}}','#if BUILD_DEBUG {x :: 1;}','S :: struct {#if true {x:u8;}}']:
   source=self.source(text)
   for flags in [[],['--release']]:
    result=run(COMPILER,'check',source,*flags,ok=False);self.assertNotEqual(result.returncode,0);self.assertIn('E0210',result.stderr)
 def test_nested_conditions_duplicate_fields_and_original_locations(self):
  source=self.source('''S :: struct {#if BUILD_DEBUG {#if BUILD_DEBUG {x:u8;}}else{#if BUILD_DEBUG {missing:Missing;}else{x:u8;}}}
f :: ()->int #export_c {s:S;s.x=9;return cast(int)s.x;}''')
  for flags in [[],['--release']]:run(COMPILER,'c',source,'--library',*flags)
  # A struct whose only field is development-only becomes a valid empty
  # struct in release, including in emitted native code and reflection.
  source=self.source('Empty :: struct {#if BUILD_DEBUG {debug:u64;}}\nsize :: ()->int #export_c {return size_of(Empty);}\nfields :: ()->int #export_c {return type_info(Empty).field_count;}')
  for mode,flags,size,fields in [('debug',[],8,1),('release',['--release'],1,0)]:
   c=self.dir/('empty-'+mode+'.c');c.write_text(run(COMPILER,'c',source,'--library',*flags).stdout)
   binary=self.dir/('empty-'+mode+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',binary);lib=C.CDLL(str(binary))
   self.assertEqual(lib.size(),size);self.assertEqual(lib.fields(),fields)
  source=self.source('S :: struct {#if BUILD_DEBUG {x:u8;} x:u8;} f :: () #export_c {}')
  self.assertNotEqual(run(COMPILER,'c',source,'--library',ok=False).returncode,0);run(COMPILER,'c',source,'--library','--release')
  source=self.source('f :: () #export_c {\n#if BUILD_DEBUG {} else {\n missing();\n}\n}')
  result=run(COMPILER,'c',source,'--library','--release',ok=False);self.assertNotEqual(result.returncode,0);self.assertIn(str(source)+':3:',result.stderr)
  # Syntax still consumes the existing nesting budget in a disabled branch.
  source=self.source('S :: struct {'+'#if BUILD_DEBUG {'*140+'x:u8;'+'}'*140+'} f :: () #export_c {}')
  for flags in [[],['--release']]:
   result=run(COMPILER,'c',source,'--library',*flags,ok=False);self.assertNotEqual(result.returncode,0);self.assertIn('E0221',result.stderr)
 def test_loaded_units_and_real_wasm_and_metal_profiles(self):
  self.source(PROFILE,'loaded.coax');source=self.source('#load "loaded.coax";')
  for flags in [[],['--release']]:run(COMPILER,'c',source,'--library',*flags)
  # Inactive host code must not enter the Metal scalar checker.
  source=self.source('f :: (x:f32)->f32 {#if BUILD_DEBUG {bad:string="host";return x;}else{return x+1.0f32;}}')
  self.assertNotEqual(run(COMPILER,'metal',source,ok=False).returncode,0);release=run(COMPILER,'metal',source,'--release').stdout;self.assertNotIn('host',release)
  source=self.source(PROFILE)
  for mode,flags,want in [('development',[],[16,9,2]),('release',['--release'],[4,107,1])]:
   c=self.dir/(mode+'.c');c.write_text(run(COMPILER,'c',source,'--library',*flags).stdout)
   wrapper=self.dir/(mode+'-wasm.c');wrapper.write_text(f'#include "{mode}.c"\n__attribute__((export_name("size"))) int s(void){{return (int)profile_size();}}\n__attribute__((export_name("value"))) int v(void){{return (int)profile_value();}}\n__attribute__((export_name("fields"))) int f(void){{return (int)profile_fields();}}')
   wasm=self.dir/(mode+'.wasm');run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',wrapper,'-lm','-o',wasm)
   js=self.dir/(mode+'.cjs');js.write_text("const fs=require('node:fs');(async()=>{const no=()=>{throw Error('Unexpected host effect')};const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[2]),{wasi_snapshot_preview1:{fd_write:no,fd_seek:no,fd_fdstat_get:no,proc_exit:no}});const x=instance.exports;x._initialize?.();process.stdout.write(JSON.stringify([x.size(),x.value(),x.fields()]));})().catch(e=>{console.error(e);process.exitCode=1;});")
   self.assertEqual(json.loads(run('bun',js,wasm).stdout),want)
if __name__=='__main__':unittest.main()
