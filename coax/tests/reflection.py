#!/usr/bin/env python3
"""Target metadata through the real compiler, with an independent C ABI oracle.
Python is test tooling only; reflection/bootstrap/app compilation do not use it.
"""
from pathlib import Path
import json
import os
import random
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
COAX = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()

class Reflection(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-reflection-")
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
    def write(self, name, text):
        p = self.base / name
        p.write_text(text)
        return p
    def command(self, cmd, path, *args):
        return subprocess.run([str(COAX), cmd, str(path), *args], capture_output=True, text=True)
    def success(self, cmd, path, *args):
        p = self.command(cmd, path, *args)
        self.assertEqual(p.returncode, 0, p.stderr)
        return p.stdout
    def error(self, text, code):
        path = self.write("bad.coax", text)
        p = self.command("check", path, "--library", "--json")
        self.assertNotEqual(p.returncode, 0, p.stdout)
        errors = [json.loads(line) for line in p.stderr.splitlines()]
        self.assertTrue(any(e["code"] == code for e in errors), errors)

    def test_checks_compile_time_tables_and_both_runtime_modes(self):
        src = ROOT / "tests/pass/reflection.coax"
        self.success("test", src)
        want = src.with_suffix(".out").read_text()
        for mode in [[], ["--release"]]:
            self.assertEqual(self.success("run", src, *mode), want)
        c = self.success("c", src)
        self.assertNotIn("coax_p_type_info", c)
        self.assertNotIn("coax_p_field_info", c)
        self.assertIn("Values computed at compile time", c)

    def test_cross_unit_queries_and_layout_during_type_resolution(self):
        self.write("types.coax", 'S :: struct {flag:u8;value:f64;}\n')
        src = self.write("main.coax", '''#import "Basic";
#load "types.coax";
Buffer :: struct {bytes:[type_info(S).size]u8;}
main :: () {
 info:=type_info(Buffer);
 #assert info.size==16 && info.field_count==1;
 #assert field_info(Buffer,0).type_name=="[16] u8";
 print("%\\n", info.name);
}''')
        self.assertEqual(self.success("run", src), "Buffer\n")

    def test_metadata_values_are_ordinary_records(self):
        src = self.write("records.coax", '''#import "Basic";
S :: struct {a:u8;b:f64;}
inspect :: (info:CoaxTypeInfo)->int {return info.size;}
main :: () {
 infos:=[2]CoaxTypeInfo.[type_info(S),type_info([] S)];
 f:=field_info(S,1);e:=enum_info(E,0);
 #assert inspect(infos[0])==16 && infos[1].kind=="view";
 #assert f.index==1 && f.schema_version==1 && f.alignment==8;
 #assert e.schema_version==1 && e.value_bits==18446744073709551615;
 print("% %\\n", f.name,e.name);
}
E :: enum u64 {maximum=18446744073709551615;}
''')
        self.assertEqual(self.success("run", src), "b maximum\n")

    def test_scalar_empty_array_and_enum_boundaries(self):
        src = self.write("edges.coax", '''#import "Basic";
Empty :: struct {}
E :: enum s64 {minimum=-9223372036854775808;maximum=9223372036854775807;}
main :: () {
 #assert type_info(Empty).size==1 && type_info(Empty).field_count==0;
 #assert type_info(E).member_count==2 && type_info(E).bits==64;
 #assert enum_info(E,0).value_bits==9223372036854775808;
 #assert enum_info(E,1).value_bits==9223372036854775807;
 #assert type_info(bool).kind=="bool" && type_info(bool).size==1;
 #assert type_info(string).kind=="string" && type_info(string).size==16;
 #assert type_info(f32).kind=="float" && type_info(f32).bits==32;
 #assert type_info([2][3]u16).element_name=="[3] u16";
 #assert type_info([]u8).element_count==0;
 print("ok\\n");
}''')
        self.assertEqual(self.success("run", src), "ok\n")

    def test_invalid_categories_indices_names_and_syntax(self):
        for expr, code in [
            ("field_info(u8,0)", "E0382"), ("enum_info(S,0)", "E0382"),
            ("field_info(S,1)", "E0374"), ("field_info(S,-1)", "E0374"),
            ("field_info(S,18446744073709551615)", "E0350"),
            ("field_info(S,true)", "E0356"), ("enum_info(E,0)", "E0382"),
            ("field_info(Empty,0)", "E0382"), ("type_info(Missing)", "E0302"),
            ("type_info(u8,0)", "E0201"), ("field_info(S)", "E0201"),
        ]:
            with self.subTest(expr=expr):
                self.error('S :: struct {a:u8;} Empty :: struct {} E :: enum u8 {} f :: () {x:='+expr+';}', code)
        self.error('field_info :: () {}', "E0335")
        self.error('enum_info :: 1;', "E0345")
        self.error('CoaxTypeInfo :: struct {} f :: () {x:=type_info(u8);}', "E0372")

    def test_dynamic_bounds_trap_in_debug_and_release(self):
        for query, decl in [("field_info", "S :: struct {a:u8;b:f64;}"),
                            ("enum_info", "S :: enum u8 {a;b;}")]:
            for index in [-1, 2]:
                src = self.write("bounds.coax", decl+f'\nf :: (i:int) {{x:={query}(S,i);}} main :: () {{f({index});}}')
                for mode in [[], ["--release"]]:
                    p = self.command("run", src, *mode)
                    self.assertNotEqual(p.returncode, 0)
                    self.assertIn("index", p.stderr)

    def test_independent_native_padding_offsets_and_arrays(self):
        rng = random.Random(0xC0A012)
        scalars = [("u8", "uint8_t"), ("s16", "int16_t"), ("u32", "uint32_t"),
                   ("f64", "double"), ("bool", "bool"), ("f32", "float")]
        coax, c, probes = [], ['#include <stddef.h>\n#include <stdint.h>\n#include <stdbool.h>\n#include <assert.h>\n#include "oracle.h"'], []
        for i in range(12):
            name = f"S{i}"
            fields, native, checks = [], [], []
            for j in range(1+rng.randrange(6)):
                t, ct = rng.choice(scalars)
                if i and j == 0:
                    t, ct = f"S{i-1}", f"struct O{i-1}"
                count = 1+rng.randrange(4)
                fields.append(f"f{j}:[{count}]{t};")
                native.append(f"{ct} f{j}[{count}];")
                checks += [f'assert(probe_{i}({j},2)==offsetof(struct O{i},f{j}));',
                           f'assert(probe_{i}({j},3)==sizeof(((struct O{i}*)0)->f{j}));']
            coax += [f'{name} :: struct {{'+''.join(fields)+'}',
                     f'probe_{i} :: (index:int,part:int)->int #export_c {{',
                     f'if part==0 {{return type_info({name}).size;}}',
                     f'if part==1 {{return type_info({name}).alignment;}}',
                     f'info:=field_info({name},index);if part==2 {{return info.offset;}}return info.size;}}']
            c.append(f'struct O{i} {{'+''.join(native)+'};')
            probes += [f'assert(probe_{i}(0,0)==sizeof(struct O{i}));',
                       f'assert(probe_{i}(0,1)==_Alignof(struct O{i}));', *checks]
        src = self.write("oracle.coax", '\n'.join(coax))
        # C owns the oracle's declarations. It does not use Coax's field offsets.
        host = self.write("host.c", '\n'.join(c)+'\nint main(void) {'+''.join(probes)+'return 0;}')
        for mode in [[], ["--release"]]:
            generated = self.success("c", src, "--library", "--header", str(self.base/"oracle.h"), *mode)
            self.write("oracle.c", generated)
            cmd = [os.environ.get("CC", "cc"), "-std=c11", "-O1", "-g", "-fsanitize=address,undefined", "-fno-omit-frame-pointer", "-I", str(self.base), str(self.base/"oracle.c"), str(host), "-lm", "-o", str(self.base/"oracle")]
            p = subprocess.run(cmd, capture_output=True, text=True)
            self.assertEqual(p.returncode, 0, p.stderr)
            p = subprocess.run([str(self.base/"oracle")], capture_output=True, text=True)
            self.assertEqual(p.returncode, 0, p.stderr)

    def test_mutation_targets_source_logic_not_synthetic_metadata(self):
        src = self.write("mutate.coax", '''#authority;
S :: struct {a:u8;b:f64;}
f :: (index:int)->int #export_c {return field_info(S,index).offset+1;}
#forall "source decisions" i:0..<2 {expected:=[2]int.[1,9];#assert f(i)==expected[i];}
''')
        output = self.success("mutate", src)
        self.assertIn("0 survived", output)
        self.assertNotIn("no reachable", output)
        # User-authored values of the same record type still supply sites.
        src.write_text('''#authority;
f :: ()->int #export_c {x:=CoaxFieldInfo.{offset=7};return x.offset;}
g :: () {unused:=type_info(u8);}
#test "authored metadata" {#assert f()==7;g();}
''')
        self.assertIn("0 survived", self.success("mutate", src))

    def test_large_dynamic_table_refuses_bounded_expansion(self):
        fields = ''.join(f'f{i}:u8;' for i in range(9000))
        self.error('Large :: struct {'+fields+'} f :: (index:int)->int {return field_info(Large,index).offset;}', "E0381")

    def test_relocated_compiler_needs_no_helpers(self):
        compiler = self.base/"coax"
        shutil.copy2(COAX, compiler)
        env = {**os.environ, "PATH":"/nonexistent"}
        p = subprocess.run([str(compiler), "c", str(ROOT/"tests/pass/reflection.coax")], capture_output=True, text=True, env=env)
        self.assertEqual(p.returncode, 0, p.stderr)

if __name__ == "__main__":
    unittest.main()
