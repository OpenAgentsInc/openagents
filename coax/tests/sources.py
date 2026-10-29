#!/usr/bin/env python3
"""Multi-file compilation through the real driver, plus a native policy oracle."""
from pathlib import Path
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from quiet_crash import run as run_quiet_crash

ROOT = Path(__file__).resolve().parents[1]
COAX = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()

class Sources(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-sources-")
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name).resolve()
    def write(self, name, text):
        path = self.base / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        return path
    def command(self, cmd, path, *args, env=None, expect_crash=False):
        runner = run_quiet_crash if expect_crash else subprocess.run
        return runner([str(COAX), cmd, str(path), *args], capture_output=True, text=True, env=env)
    def success(self, cmd, path, *args):
        p = self.command(cmd,path,*args)
        self.assertEqual(p.returncode,0,p.stderr)
        return p.stdout
    def error(self, path, code, file=None):
        p = self.command("check",path,"--json")
        self.assertNotEqual(p.returncode,0,p.stdout)
        records = [json.loads(line) for line in p.stderr.splitlines()]
        self.assertTrue(any(r["code"] == code for r in records),records)
        if file:
            self.assertTrue(any(r["code"] == code and Path(r["file"]) == file for r in records),records)
        return records

    def test_shared_types_forward_names_and_diamond(self):
        self.write("common.coax","Shared :: struct { value: int; }\nshared :: (x:Shared)->int {return x.value;} // no trailing newline")
        self.write("parts/left.coax",'#load "../common.coax";\nleft :: ()->int {return shared(Shared.{value=20});}')
        self.write("parts/right.coax",'#load ".././common.coax";\nright :: ()->int {return shared(Shared.{value=22});}')
        main=self.write("main.coax",'#import "Basic";\n#load "parts/left.coax";\n#load "parts/right.coax";\nmain :: () {print("%\\n", left()+right());}')
        for mode in [[],["--release"]]:
            self.assertEqual(self.success("run",main,*mode),"42\n")
        deps=json.loads(self.success("deps",main))
        self.assertEqual(deps["schema_version"],1)
        self.assertEqual(deps["sources"],[str(main),str(self.base/"parts/left.coax"),str(self.base/"common.coax"),str(self.base/"parts/right.coax")])
        self.assertEqual(deps["descriptors"],[])
        self.assertEqual(self.success("deps",main),self.success("deps",main))

    def test_cycles_symlinks_and_once_only_identity(self):
        main=self.write("main.coax",'#load "b.coax";\nmain :: () {}')
        other=self.write("b.coax",'#load "alias.coax";')
        (self.base/"alias.coax").symlink_to(main)
        self.error(main,"E0602",other)
        other.write_text('answer :: 42;')
        main.write_text('#load "b.coax";\n#load "alias-b.coax";\nmain :: () {#assert answer==42;}')
        (self.base/"alias-b.coax").symlink_to(other)
        self.success("run",main)
        self.assertEqual(len(json.loads(self.success("deps",main))["sources"]),2)

    def test_source_syntax_is_tokens_not_text_search(self):
        main=self.write("main.coax",'// #load "missing.coax";\n/* #load "missing.coax"; */\nmain :: () { s:="#load \\\"missing.coax\\\";"; #assert s.count>0; }')
        self.success("check",main)
        for directive in ['#load "";','#load "x\\0.coax";','#load 4;','#load "x"','#load']:
            main.write_text(directive+'\nmain :: () {}')
            self.error(main,"E0600",main)
        main.write_text('main :: () {#load "missing.coax";}')
        self.error(main,"E0600",main)

    def test_missing_directory_and_unbalanced_units(self):
        main=self.write("main.coax",'#load "missing.coax"; main :: () {}')
        self.error(main,"E0601",main)
        main.write_text('#load "."; main :: () {}')
        self.error(main,"E0601",main)
        main.write_text('#load "part.coax"; main :: () {}')
        part=self.write("part.coax",'work :: () {')
        self.error(main,"E0604",part)
        # No token/brace can escape a loaded file into its parent.
        main.write_text('#load "part.coax"; main :: () {')
        part.write_text('}')
        self.error(main,"E0604")

    def test_declarations_cannot_cross_physical_files(self):
        # Delimiter balance alone is insufficient: a child semicolon must not
        # finish a parent declaration, nor can a child supply its initializer.
        main=self.write("main.coax",'#load "part.coax";\nCOUNT :: 3')
        part=self.write("part.coax",'; main :: () {#assert COUNT==3;}')
        p=self.command("check",main,"--json")
        self.assertNotEqual(p.returncode,0,'a physical source boundary completed a declaration')
        self.assertIn(str(main),p.stderr)
        main.write_text('#load "part.coax";\nCOUNT ::')
        part.write_text('3; main :: () {#assert COUNT==3;}')
        self.assertNotEqual(self.command("check",main).returncode,0)

    def test_original_file_diagnostics_in_parser_checker_and_evaluator(self):
        main=self.write("main.coax",'#load "sub/part.coax";\nmain :: () {}')
        part=self.write("sub/part.coax",'// original line one\nwork :: ()->int {\n return missing;\n}\n')
        records=self.error(main,"E0306",part)
        self.assertEqual(next(r["line"] for r in records if r["code"]=="E0306"),3)
        part.write_text('work :: () {\n value := 1\n}\n')
        p=self.command("check",main)
        self.assertNotEqual(p.returncode,0)
        self.assertIn(str(part)+":2:",p.stderr)
        self.assertIn("value := 1",p.stderr)
        part.write_text('#assert false;\n')
        records=self.error(main,"E0406",part)
        self.assertEqual(records[0]["line"],1)

    def test_file_authority_cannot_be_lost_or_spread(self):
        main=self.write("main.coax",'#load "policy.coax";\n#load "utility.coax";\nmain :: () {}')
        policy=self.write("policy.coax",'#authority;\nallowed :: (n:int)->bool #export_c {return n>0;}\n#test "policy" {#assert allowed(1); #assert !allowed(0);}')
        self.write("utility.coax",'unused :: ()->int {return 99;}')
        self.success("c",main)
        policy.write_text('#authority;\nallowed :: (n:int)->bool #export_c {return n>0;}\n#test "policy" {#assert false;}')
        self.error(main,"E0380",policy)
        policy.write_text('#authority;\nallowed :: (n:int)->bool #export_c {return n>0;}\n#test "policy" {#assert allowed(1)==false;}')
        self.error(main,"E0406",policy)
        policy.write_text('#authority;\nallowed :: (n:int)->bool #export_c {return n>0;}\n#test "dead" {if false {#assert allowed(1);}}')
        self.error(main,"E0380",policy)

    def test_entry_authority_with_non_authority_dependency(self):
        self.write("utility.coax",'unused :: ()->int {return 99;}')
        main=self.write("main.coax",'#authority;\n#load "utility.coax";\nallowed :: (n:int)->bool #export_c {return n>0;}\n#test "boundary" {#assert allowed(1);#assert !allowed(0);}')
        self.success("check",main,"--library")
        self.assertIn("0 survived",self.success("mutate",main))

    def test_loaded_checks_can_cover_entry_authority(self):
        main=self.write("main.coax",'#authority;\n#load "checks.coax";\nallowed :: (n:int)->bool #export_c {return n>0;}')
        self.write("checks.coax",'#test "boundary" {#assert allowed(1);#assert !allowed(0);}')
        self.success("check",main,"--library")
        self.assertIn("0 survived",self.success("mutate",main))

    def test_relative_proto_input_uses_declaring_unit(self):
        profile=ROOT.parent/"proto/descriptors/ios-home.pb"
        (self.base/"adapter").mkdir()
        shutil.copy(profile,self.base/"adapter/types.pb")
        adapter=self.write("adapter/codec.coax",'#import_proto "types.pb" as Wire;\nread :: ()->u64 {return Wire_HudInvocation_get_revision(Wire_HudInvocation_new()).value;}')
        main=self.write("main.coax",'#authority;\n#load "adapter/codec.coax";\nmain :: () {}')
        self.success("c",main)
        deps=json.loads(self.success("deps",main))
        self.assertEqual(deps["sources"],[str(main),str(adapter)])
        self.assertEqual(deps["descriptors"],[str(self.base/"adapter/types.pb")])
        adapter.write_text('#import_proto "missing.pb" as Wire;')
        self.error(main,"E0502",adapter)

    def test_runtime_trap_points_to_loaded_file(self):
        main=self.write("main.coax",'#load "part.coax";\nmain :: () {bad();}')
        part=self.write("part.coax",'bad :: () {\n a:=[1]int.[7];\n i:=2;\n #assert a[i]==7;\n}')
        for mode in [[],["--release"]]:
            p=self.command("run",main,*mode,expect_crash=True)
            self.assertNotEqual(p.returncode,0)
            self.assertIn(str(part)+":4: index out of bounds",p.stderr)

    def test_loaded_path_is_escaped_for_generated_native_code(self):
        name='part\n"quoted".coax'
        part=self.write(name,'bad :: () {#assert false;}')
        spelling=name.replace('\\','\\\\').replace('"','\\"').replace('\n','\\n')
        main=self.write("main.coax",'#load "'+spelling+'"; main :: () {bad();}')
        p=self.command("run",main,expect_crash=True)
        self.assertNotEqual(p.returncode,0)
        self.assertIn(str(part)+':1: assertion failed',p.stderr)

    def test_metal_backend_uses_loaded_functions_and_original_errors(self):
        part=self.write("light.coax",'gain :: (x:f32)->f32 {return x*0.5;}')
        main=self.write("shader.coax",'#load "light.coax"; shade :: (x:f32)->f32 #export_c {return gain(x);}')
        metal=self.success("metal",main)
        self.assertIn('shade(',metal)
        self.assertIn('gain(',metal)
        part.write_text('gain :: (x:f32)->f32 {while false {} return x;}')
        p=self.command("metal",main,"--json")
        self.assertNotEqual(p.returncode,0)
        diagnostics=[json.loads(x) for x in p.stderr.splitlines()]
        self.assertTrue(any(x['code']=='E0377' and Path(x['file'])==part for x in diagnostics),diagnostics)

    def test_graph_depth_and_unique_unit_limits(self):
        for n in range(65):
            self.write(f"u{n}.coax",f'#load "u{n+1}.coax";' if n<64 else '')
        self.error(self.base/"u0.coax","E0603")
        main=self.write("main.coax",'\n'.join(f'#load "u{n}.coax";' for n in range(64))+'\nmain :: () {}')
        for n in range(64):self.write(f"u{n}.coax",'')
        self.error(main,"E0603")
        main.write_text('\n'.join(f'#load "u{n}.coax";' for n in range(63))+'\nmain :: () {}')
        self.success("check",main)

    def test_private_library_checks_need_no_native_exports(self):
        unit=self.write("policy.coax",'#authority; allowed :: (n:int)->bool {return n>0;} #test "edge" {#assert allowed(1);#assert !allowed(0);}')
        self.success("test",unit,"--library")
        self.assertIn("0 survived",self.success("mutate",unit,"--library"))

    def test_native_policy_abi_and_sanitizers(self):
        host = self.write("policy.c", r'''#include <assert.h>
#include <stdint.h>
#include "sources.h"
int main(void) {
  int64_t v[] = {-1,0,1,31,32,63,64,16777215,16777216,16777217,INT64_MAX};
  for(unsigned a=0;a<sizeof(v)/sizeof(v[0]);a++)
  for(unsigned b=0;b<sizeof(v)/sizeof(v[0]);b++)
  for(unsigned c=0;c<sizeof(v)/sizeof(v[0]);c++)
  for(unsigned d=0;d<sizeof(v)/sizeof(v[0]);d++) {
    int expected=v[a]>=0 && v[a]<64 && v[b]>=0 && v[b]<32 && v[c]>=0 && v[d]>=0;
    if (expected) expected=(__uint128_t)v[c]+(__uint128_t)v[d]<=16777216;
    assert(SU_admit(v[a],v[b],v[c],v[d])==expected);
  }
  coax_t_SU_Token tokens[258] = {0}; coax_t_SU_Load loads[1] = {0};
  for(int i=0;i<129;i++) {tokens[i].kind=10;tokens[129+i].kind=11;}
  assert(!SU_scan((coax_v_t_SU_Token){tokens,258},(coax_v_t_SU_Load){loads,1}).balanced);
  for(int open=8;open<=12;open+=2) {
    coax_t_SU_Token pair[2]={{.kind=(uint32_t)open},{.kind=(uint32_t)(open+1)}};
    assert(SU_scan((coax_v_t_SU_Token){pair,2},(coax_v_t_SU_Load){loads,1}).balanced);
    pair[1].kind=8;
    assert(!SU_scan((coax_v_t_SU_Token){pair,2},(coax_v_t_SU_Load){loads,1}).balanced);
  }
  return 0;
}''')
        # The seed is the same Coax module, independently exercised as native C.
        for optimization in ["-O0","-O2"]:
            exe=self.base/"policy"
            p=subprocess.run([os.environ.get("CC","cc"),"-std=c11",optimization,"-g","-fsanitize=address,undefined","-fno-omit-frame-pointer","-I",str(ROOT/"bootstrap"),str(host),str(ROOT/"bootstrap/sources.c"),"-lm","-o",str(exe)],capture_output=True,text=True)
            self.assertEqual(p.returncode,0,p.stderr)
            p=subprocess.run([str(exe)],capture_output=True,text=True)
            self.assertEqual(p.returncode,0,p.stderr)

    def test_relocated_compiler_needs_no_helpers_for_load_and_proto(self):
        shutil.copy(COAX,self.base/"coax")
        profile=ROOT.parent/"proto/descriptors/ios-home.pb"
        self.write("codec.coax",f'#import_proto "{profile}" as Wire;')
        main=self.write("main.coax",'#load "codec.coax";\nanswer :: ()->int #export_c {return 42;}')
        env=dict(os.environ,PATH="/path-with-no-tools")
        p=subprocess.run([str(self.base/"coax"),"c",str(main),"--library"],capture_output=True,text=True,env=env)
        self.assertEqual(p.returncode,0,p.stderr)
        self.assertIn("int64_t answer",p.stdout)

if __name__ == "__main__":unittest.main()
