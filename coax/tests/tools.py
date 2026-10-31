#!/usr/bin/env python3
"""Exercise the tools around the compiler: explain, JSON diagnostics,
layout reports, libraries and headers, and deterministic output."""

import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
COMPILER = Path(os.environ.get("COAX_BIN", ROOT / "bin/coax"))
CC = os.environ.get("COAX_CC", "cc")


def coax(*args, cwd=None):
    return subprocess.run([str(COMPILER), *args], capture_output=True, text=True, timeout=30, cwd=cwd)


class Tools(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-tools-")
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, name, text):
        path = self.dir / name
        path.write_text(text)
        return str(path)

    def test_every_error_code_is_explained(self):
        used = set()
        for source in (ROOT / "src").glob("*.cpp"):
            if source.name == "errors.cpp":
                continue
            used |= set(re.findall(r'"(E0\d{3})"', source.read_text()))
        for source in (ROOT / "lib").rglob("*.coax"):
            used |= set(re.findall(r'"(E0\d{3})"', source.read_text()))
        listed = set(re.findall(r"^(E0\d{3}):", coax("explain", "--all").stdout, re.M))
        self.assertFalse(used - listed, f"codes without an explanation: {sorted(used - listed)}")
        result = coax("explain", "E0350")
        self.assertEqual(result.returncode, 0)
        self.assertIn("does not fit", result.stdout)
        self.assertEqual(coax("explain", "E9999").returncode, 1)

    def test_markdown_table_has_every_code(self):
        table = coax("explain", "--all", "--markdown").stdout
        self.assertEqual(len(re.findall(r"^\| `E0\d{3}` \|", table, re.M)),
                         len(re.findall(r"^E0\d{3}:", coax("explain", "--all").stdout, re.M)))

    def test_json_diagnostics(self):
        path = self.write("bad.coax", "main :: () {\n  x: u8 = 300;\n  y := missing;\n}\n")
        result = coax("check", "--json", path)
        self.assertEqual(result.returncode, 1)
        lines = [json.loads(line) for line in result.stderr.splitlines()]
        self.assertEqual([d["code"] for d in lines], ["E0350", "E0306"])
        self.assertEqual((lines[0]["line"], lines[0]["col"]), (2, 11))
        self.assertTrue(lines[0]["fix"])

    def test_layout_report_shows_padding(self):
        path = self.write("layout.coax", "Mixed :: struct { tag: u8; value: f64; flag: bool; }\nmain :: () {}\n")
        report = coax("layout", path).stdout
        self.assertIn("Mixed  size 24  align 8", report)
        self.assertIn("(padding)", report)
        self.assertIn("8     8  value: f64", report)

    def test_library_header_and_c_plus_plus(self):
        source = self.write("kernel.coax", """
Sample :: struct { value: f32; weight: s32 = 1; }
Mode :: enum u8 { off; on = 3; }
weigh :: (samples: [] Sample, mode: Mode) -> f64 #export_c {
  total := 0.0;
  if mode == Mode.off { return total; }
  for s: samples { total += cast(f64) s.value * cast(f64) s.weight; }
  return total;
}
""")
        self.assertIn("E0336", coax("c", source).stderr)  # a program needs main
        self.assertEqual(coax("check", source).returncode, 0)  # a file that exports does not
        header = self.dir / "kernel.h"
        obj = self.dir / "kernel.o"
        self.assertEqual(coax("header", source, "-o", str(header)).returncode, 0)
        self.assertEqual(coax("lib", source, "-o", str(obj)).returncode, 0)
        host = self.write("host.c", """
#include <stdio.h>
#include "kernel.h"
int main(void) {
  coax_t_Sample s[3] = {{1.5f, 2}, {2.0f, 1}, {0.25f, 4}};
  printf("%.2f %.2f\\n", weigh((coax_v_t_Sample){s, 3}, COAX_Mode_on), weigh((coax_v_t_Sample){s, 3}, COAX_Mode_off));
  return 0;
}
""")
        exe = self.dir / "host"
        build = subprocess.run([CC, "-std=c11", "-I", str(self.dir), "-o", str(exe), host, str(obj)],
                               capture_output=True, text=True)
        self.assertEqual(build.returncode, 0, build.stderr)
        self.assertEqual(subprocess.run([str(exe)], capture_output=True, text=True).stdout, "6.00 0.00\n")
        cxx = subprocess.run(["c++", "-std=c++17", "-fsyntax-only", "-x", "c++", "-I", str(self.dir), "-"],
                             input='#include "kernel.h"\nint main() { return 0; }\n', capture_output=True, text=True)
        self.assertEqual(cxx.returncode, 0, cxx.stderr)

    def test_output_is_deterministic(self):
        for example in ("crc32", "life", "mandelbrot"):
            path = str(ROOT / "examples" / f"{example}.coax")
            self.assertEqual(coax("c", path).stdout, coax("c", path).stdout)

    def test_untyped_constants_take_their_use_type(self):
        ok = self.write("ok.coax", "N :: 200; F :: 0.5;\nmain :: () { a: u8 = N; b: f32 = F; c: [N] u8; d := N; }\n")
        self.assertEqual(coax("check", ok).returncode, 0, coax("check", ok).stderr)
        bad = self.write("bad.coax", "N :: 300;\nmain :: () { a: u8 = N; }\n")
        self.assertIn("error[E0350]", coax("check", bad).stderr)


class HeaderComposition(unittest.TestCase):
    """Two real libraries must share compatible C types without host renaming.

    Incompatible public spellings must be diagnosed, including when their
    storage size happens to match. Include guards must not hide a second
    entry file merely because both entries have the same basename.
    """

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-header-composition-")
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def source(self, name, text):
        path = self.dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        return path

    def library(self, source, name, flags=()):
        header = self.dir / (name + ".h")
        emitted = coax("c", str(source), "--library", "--header", str(header), *flags)
        self.assertEqual(emitted.returncode, 0, emitted.stderr)
        c = self.source(name + ".c", emitted.stdout)
        obj = self.dir / (name + ".o")
        result = subprocess.run([CC, "-std=c11", "-O2", "-c", str(c), "-o", str(obj)],
                                capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        return header, obj

    def host(self, headers, body, objects=(), succeeds=True, diagnostic=None):
        includes = "".join('#include "' + header.name + '"\n' for header in headers)
        for compiler, language in ((CC, "c11"), ("c++", "c++17")):
            with self.subTest(language=language, order=[header.name for header in headers]):
                source = self.source("host." + ("c" if language == "c11" else "cpp"), includes + body)
                output = self.dir / ("host-" + language)
                command = [compiler, "-std=" + language, "-Werror", "-pedantic-errors", "-I", str(self.dir)]
                if succeeds:
                    command += [str(source), *map(str, objects), "-lm", "-o", str(output)]
                else:
                    command += ["-fsyntax-only", str(source)]
                result = subprocess.run(command, capture_output=True, text=True, timeout=30)
                if succeeds:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    run = subprocess.run([str(output)], capture_output=True, text=True, timeout=30)
                    self.assertEqual(run.returncode, 0, run.stderr)
                else:
                    self.assertNotEqual(result.returncode, 0, "incompatible headers were accepted")
                    self.assertIn(diagnostic, result.stderr)

    def test_independent_libraries_share_fixed_arrays_and_array_views(self):
        for flags in ((), ("--release",)):
            with self.subTest(flags=flags):
                libraries = []
                for suffix, index, amount in (("a", 1, 1), ("b", 2, 2)):
                    source = self.source(suffix + ".coax", f"""
array_{suffix} :: (value:[3]u16)->[3]u16 #export_c {{
  result:=value;result[{index}]+={amount};return result;
}}
sum_{suffix} :: (rows:[] [3]u16)->int #export_c {{
  total:=0;for row:rows {{for value:row {{total+=cast(int)value;}}}}return total;
}}
""")
                    libraries.append(self.library(source, suffix, flags))
                headers, objects = zip(*libraries)
                body = """
int main(void) {
  coax_a3_u16 value = {{2, 3, 5}};
  coax_a3_u16 first = array_a(value), second = array_b(first);
  coax_a3_u16 rows[] = {first, second};
  coax_v_a3_u16 view = {rows, 2};
  return first.e[1] == 4 && second.e[2] == 7 && sum_a(view) == 24 && sum_b(view) == 24 ? 0 : 1;
}
"""
                self.host(headers, body, objects)
                self.host(tuple(reversed(headers)), body, objects)

    def test_shared_loaded_structs_enums_and_borrowed_rows_compose(self):
        self.source("common.coax", """
Mode :: enum u8 { idle; ready = 3; }
Cell :: struct { tag:u8; samples:[3]u16; mode:Mode; }
Batch :: struct { rows:[]Cell; mode:Mode; }
Node :: struct { value:int; children:[]Node; }
""")
        a = self.source("shared-a.coax", """
#load "common.coax";
cell_a :: (cell:Cell)->Cell #export_c {result:=cell;result.tag+=1;return result;}
batch_a :: (batch:Batch)->int #export_c {return batch.rows.count+cast(int)batch.mode;}
node_a :: (node:Node)->int #export_c {return node.value+node.children.count;}
""")
        b = self.source("shared-b.coax", """
#load "common.coax";
cell_b :: (cell:Cell)->Cell #export_c {result:=cell;result.samples[2]+=2;return result;}
batch_b :: (batch:Batch)->int #export_c {return batch.rows.count+cast(int)batch.mode;}
node_b :: (node:Node)->int #export_c {return node.value+node.children.count;}
""")
        for flags in ((), ("--release",)):
            with self.subTest(flags=flags):
                libraries = [self.library(a, "shared-a", flags), self.library(b, "shared-b", flags)]
                headers, objects = zip(*libraries)
                body = """
int main(void) {
  coax_t_Cell cell = {2, {{3, 5, 7}}, COAX_Mode_ready};
  coax_t_Cell next = cell_b(cell_a(cell));
  coax_t_Batch batch = {{&next, 1}, COAX_Mode_ready};
  coax_t_Node node = {7, {0, 0}};
  return next.tag == 3 && next.samples.e[2] == 9 && batch_a(batch) == 4 && batch_b(batch) == 4 && node_a(node) == 7 && node_b(node) == 7 ? 0 : 1;
}
"""
                self.host(headers, body, objects)
                self.host(tuple(reversed(headers)), body, objects)

    def test_equal_size_incompatible_public_types_are_explicitly_rejected(self):
        variants = (
            ("value:u32;", "value:f32;"),
            ("left:u16;right:u16;", "right:u16;left:u16;"),
            ("value:u32;#if BUILD_DEBUG {stamp:u32;}", "value:u32;stamp:u32;"),
        )
        for index, (left, right) in enumerate(variants):
            with self.subTest(variant=index):
                a = self.source("conflict-a.coax", "Collision :: struct {" + left + "}\nread_a :: (v:Collision)->int #export_c {return 1;}")
                b = self.source("conflict-b.coax", "Collision :: struct {" + right + "}\nread_b :: (v:Collision)->int #export_c {return 2;}")
                # The last pair differs only after the left producer selects
                # release; mixed-profile includes must not silently coalesce.
                left_flags = ("--release",) if index == 2 else ()
                headers = [self.library(a, "conflict-a", left_flags)[0], self.library(b, "conflict-b")[0]]
                self.host(headers, "int main(void) { return 0; }\n", succeeds=False,
                          diagnostic="incompatible Coax C type")
                self.host(tuple(reversed(headers)), "int main(void) { return 0; }\n", succeeds=False,
                          diagnostic="incompatible Coax C type")

    def test_conflicting_enum_member_values_are_explicitly_rejected(self):
        a = self.source("enum-a.coax", "Mode :: enum u8 { idle; ready = 3; }\nmode_a :: (mode:Mode)->int #export_c {return cast(int)mode;}")
        b = self.source("enum-b.coax", "Mode :: enum u8 { idle; ready = 4; }\nmode_b :: (mode:Mode)->int #export_c {return cast(int)mode;}")
        headers = [self.library(a, "enum-a")[0], self.library(b, "enum-b")[0]]
        self.host(headers, "int main(void) { return 0; }\n", succeeds=False,
                  diagnostic="incompatible Coax C type")
        self.host(tuple(reversed(headers)), "int main(void) { return 0; }\n", succeeds=False,
                  diagnostic="incompatible Coax C type")

    def test_same_basename_entries_do_not_hide_each_others_exports(self):
        a = self.source("left/kernel.coax", "from_left :: (value:int)->int #export_c {return value+1;}")
        b = self.source("right/kernel.coax", "from_right :: (value:int)->int #export_c {return value+2;}")
        libraries = [self.library(a, "left"), self.library(b, "right")]
        headers, objects = zip(*libraries)
        body = "int main(void) { return from_left(3) == 4 && from_right(3) == 5 ? 0 : 1; }\n"
        self.host(headers, body, objects)
        self.host(tuple(reversed(headers)), body, objects)

    def test_repeated_header_includes_remain_valid(self):
        source = self.source("repeated.coax", """
Entry :: struct { values:[2]u8; }
read_entry :: (entries:[]Entry)->int #export_c {return cast(int)entries[0].values[1];}
""")
        header, obj = self.library(source, "repeated")
        body = """
int main(void) {
  coax_t_Entry entry = {{{2, 7}}};
  coax_v_t_Entry entries = {&entry, 1};
  return read_entry(entries) == 7 ? 0 : 1;
}
"""
        self.host((header, header), body, (obj,))

    def test_defaults_empty_records_and_source_paths_do_not_change_c_compatibility(self):
        libraries = []
        for side, default in (("left", 3), ("right", 5)):
            source = self.source(side + "/defaults.coax", f"""
Empty :: struct {{}}
Box :: struct {{value:int={default}; marker:Empty;}}
make_{side} :: ()->Box #export_c {{box:Box;return box;}}
""")
            libraries.append(self.library(source, "defaults-" + side))
        headers, objects = zip(*libraries)
        body = "int main(void) { return make_left().value == 3 && make_right().value == 5 ? 0 : 1; }\n"
        self.host(headers, body, objects)
        self.host(tuple(reversed(headers)), body, objects)

    def test_long_record_schemas_compose_without_truncated_macro_identity(self):
        # Full direct-schema encoding must work for real records whose schema
        # identifier exceeds the C standard's minimum significant-name limit.
        names = ["field_" + str(i) + "_with_a_descriptive_public_interface_name" for i in range(48)]
        fields = "".join(name + ":u32;" for name in names)
        libraries = []
        for side in ("left", "right"):
            source = self.source("long-" + side + ".coax", "Long :: struct {" + fields + "}\nread_" + side + " :: (value:Long)->int #export_c {return cast(int)value." + names[0] + "+cast(int)value." + names[-1] + ";}")
            libraries.append(self.library(source, "long-" + side))
        headers, objects = zip(*libraries)
        body = "int main(void) { coax_t_Long value = {0}; value." + names[0] + " = 5; value." + names[-1] + " = 7; return read_left(value) == 12 && read_right(value) == 12 ? 0 : 1; }\n"
        self.host(headers, body, objects)
        self.host(tuple(reversed(headers)), body, objects)


if __name__ == "__main__":
    unittest.main()
