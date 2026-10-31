#!/usr/bin/env python3
"""Independent runtime arithmetic, native optimization and BUILD_DEBUG profiles.

The checked evaluator remains the authority gate under every runtime profile.
Native failure cases use quiet_crash so expected faults do not open dialogs.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(os.environ.get("COAX_TEST_ROOT", Path(__file__).resolve().parents[1])).resolve()
COMPILER = Path(os.environ.get("COAX_TEST_COMPILER", ROOT / "bin/coax")).resolve()
sys.path.insert(0, str(ROOT / "tests"))
from quiet_crash import run as run_quiet_crash


SOURCE = '''#import "Basic";
Cell :: struct { value:u32=7; #if BUILD_DEBUG { stamp:u64=2; } }
sum :: (a:u8,b:u8)->u8 #export_c {return a+b;}
main :: () {
 cell:Cell;
 #if BUILD_DEBUG {print("% %\\n",size_of(Cell),cell.value+cast(u32)cell.stamp);}
 else {print("% %\\n",size_of(Cell),cell.value);}
}
'''


class SemanticProfiles(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-semantic-profile-")
        self.addCleanup(self.tmp.cleanup)
        self.dir = Path(self.tmp.name)
        self.serial = 0

    def source(self, text, name="entry.coax"):
        path = self.dir / name
        path.write_text(text)
        return path

    def call(self, *arguments, env=None, native_fault=False):
        command = [str(COMPILER), *map(str, arguments)]
        run = run_quiet_crash if native_fault else subprocess.run
        return run(command, capture_output=True, text=True, timeout=60, env=env)

    def report(self, command, source, *flags, env=None, output=None, native_fault=False):
        self.serial += 1
        path = self.dir / ("profile-" + str(self.serial) + ".json")
        args = [command, source, *flags, "--profile-report", path]
        if output is not None:
            args += ["-o", output]
        result = self.call(*args, env=env, native_fault=native_fault)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        data = json.loads(path.read_text())
        self.assertEqual(data["schema_version"], 1)
        self.assertEqual(data["command"], command)
        self.assertEqual(data["interpreter_arithmetic"], "checked")
        self.assertIsInstance(data["debug_info"], bool)
        self.assertIsInstance(data["backend"], str)
        self.assertTrue(data["backend"])
        return result, data

    def captured_compiler(self):
        # Observe the actual C driver arguments; metadata alone is insufficient
        # evidence that --opt selected native compilation rather than emission.
        real_cc = shutil.which(os.environ.get("COAX_CC", "cc"))
        self.assertIsNotNone(real_cc)
        log = self.dir / "native-arguments.jsonl"
        wrapper = self.dir / "capture-cc"
        wrapper.write_text("#!" + sys.executable + "\n" +
            "import json, os, sys\n" +
            "with open(os.environ['COAX_PROFILE_CC_LOG'], 'a') as f: f.write(json.dumps(sys.argv[1:])+'\\n')\n" +
            "os.execv(os.environ['COAX_PROFILE_REAL_CC'], [os.environ['COAX_PROFILE_REAL_CC'], *sys.argv[1:]])\n")
        wrapper.chmod(0o755)
        return {**os.environ, "COAX_CC": str(wrapper), "COAX_PROFILE_REAL_CC": real_cc,
                "COAX_PROFILE_CC_LOG": str(log)}, log

    def test_native_optimization_arithmetic_and_layout_matrix(self):
        source = self.source(SOURCE)
        env, log = self.captured_compiler()
        for arithmetic in ("checked", "wrapping"):
            for layout, expected in (("development", "16 9\n"), ("release", "4 7\n")):
                for level in range(4):
                    with self.subTest(arithmetic=arithmetic, layout=layout, optimization=level):
                        log.write_text("")
                        binary = self.dir / "program"
                        _, data = self.report("build", source, "--arithmetic", arithmetic,
                            "--build-profile", layout, "--opt", str(level), env=env, output=binary)
                        self.assertEqual(data["runtime_arithmetic"], arithmetic)
                        self.assertEqual(data["build_profile"], layout)
                        self.assertEqual(data["native_optimization_flag"], "-O" + str(level))
                        calls = [json.loads(line) for line in log.read_text().splitlines()]
                        compile_call = next(call for call in calls if "-c" in call)
                        self.assertEqual([arg for arg in compile_call if arg.startswith("-O")], ["-O" + str(level)])
                        self.assertIn("-ffp-contract=off", compile_call)
                        run = subprocess.run([str(binary)], capture_output=True, text=True, timeout=30)
                        self.assertEqual((run.returncode, run.stdout), (0, expected), run.stderr)
                        self.assertEqual(data["artifact_sha256"], hashlib.sha256(binary.read_bytes()).hexdigest())

    def test_legacy_release_defaults_and_explicit_override_order(self):
        source = self.source(SOURCE)
        for flags, arithmetic, layout, optimization, expected in (
            ([], "checked", "development", "-O0", "16 9\n"),
            (["--release"], "wrapping", "release", "-O2", "4 7\n"),
        ):
            with self.subTest(flags=flags):
                result, data = self.report("run", source, *flags)
                self.assertEqual(result.stdout, expected)
                self.assertEqual((data["runtime_arithmetic"], data["build_profile"], data["native_optimization_flag"]),
                                 (arithmetic, layout, optimization))
        explicit = ["--arithmetic", "checked", "--build-profile", "development", "--opt", "0"]
        first, a = self.report("run", source, "--release", *explicit)
        second, b = self.report("run", source, *explicit, "--release")
        self.assertEqual(first.stdout, "16 9\n")
        self.assertEqual(second.stdout, first.stdout)
        for key in ("runtime_arithmetic", "interpreter_arithmetic", "build_profile", "native_optimization_flag", "debug_info", "backend"):
            self.assertEqual(a[key], b[key], key)
        # --build-profile changes only conditional source selection; an
        # explicit development layout does not undo the legacy release bundle's
        # default omission of native source debugging.
        self.assertFalse(a["debug_info"])

    def test_native_library_profile_records_the_compiled_object(self):
        source = self.source(SOURCE)
        env, log = self.captured_compiler()
        obj = self.dir / "kernel.o"
        _, data = self.report("lib", source, "--arithmetic", "wrapping", "--opt", "3", env=env, output=obj)
        self.assertEqual(data["runtime_arithmetic"], "wrapping")
        self.assertEqual(data["native_optimization_flag"], "-O3")
        self.assertEqual(data["artifact_sha256"], hashlib.sha256(obj.read_bytes()).hexdigest())
        calls = [json.loads(line) for line in log.read_text().splitlines()]
        self.assertEqual(len(calls), 1)
        self.assertIn("-c", calls[0])
        self.assertIn("-O3", calls[0])
        host = self.source("#include <stdint.h>\nuint8_t sum(uint8_t,uint8_t);\nint main(void){return sum(250,10)==4?0:1;}\n", "host.c")
        binary = self.dir / "library-host"
        build = subprocess.run([env["COAX_PROFILE_REAL_CC"], str(host), str(obj), "-lm", "-o", str(binary)],
                               capture_output=True, text=True, timeout=30)
        self.assertEqual(build.returncode, 0, build.stderr)
        run = subprocess.run([str(binary)], capture_output=True, text=True, timeout=30)
        self.assertEqual(run.returncode, 0, run.stderr)

    def test_plain_emission_keeps_legacy_bytes_for_equivalent_axes(self):
        source = self.source(SOURCE)
        for legacy, explicit in (
            ([], ["--arithmetic", "checked", "--build-profile", "development"]),
            (["--release"], ["--arithmetic", "wrapping", "--build-profile", "release"]),
        ):
            with self.subTest(legacy=legacy):
                before = self.call("c", source, *legacy)
                after = self.call("c", source, *legacy, *explicit)
                self.assertEqual(before.returncode, 0, before.stderr)
                self.assertEqual(after.returncode, 0, after.stderr)
                self.assertEqual(after.stdout, before.stdout)

    def test_overflow_policy_is_independent_of_optimization(self):
        cases = (
            ("unsigned addition", "work :: (a:u8,b:u8)->u8 {return a+b;}", "work(250,10)", "4\n"),
            ("signed negation", "work :: (a:s8)->s8 {return -a;}", "work(-128)", "-128\n"),
            ("signed subtraction", "work :: (a:s8,b:s8)->s8 {return a-b;}", "work(-128,1)", "127\n"),
            ("signed multiplication", "work :: (a:s8,b:s8)->s8 {return a*b;}", "work(64,4)", "0\n"),
        )
        for name, procedure, call, expected in cases:
            source = self.source('#import "Basic";\n' + procedure + '\nmain :: () {print("%\\n",' + call + ');}\n')
            for optimization in ("0", "2"):
                with self.subTest(operation=name, optimization=optimization):
                    checked = self.call("run", source, "--opt", optimization, "--arithmetic", "checked", native_fault=True)
                    self.assertNotEqual(checked.returncode, 0)
                    self.assertIn("overflow", checked.stderr)
                    wrapping = self.call("run", source, "--opt", optimization, "--arithmetic", "wrapping")
                    self.assertEqual((wrapping.returncode, wrapping.stdout), (0, expected), wrapping.stderr)

    def test_cast_shift_and_division_failures_are_never_disabled(self):
        expressions = (
            ("cast(u8)value", 256, "cast out of range"),
            ("1 << value", 64, "shift amount out of range"),
            ("1 / value", 0, "division by zero"),
        )
        for expression, value, diagnostic in expressions:
            source = self.source("work :: (value:int)->int {return cast(int)(" + expression + ");}\nmain :: () {out:=work(" + str(value) + ");}\n")
            for arithmetic in ("checked", "wrapping"):
                for optimization in ("0", "2"):
                    with self.subTest(expression=expression, arithmetic=arithmetic, optimization=optimization):
                        result = self.call("run", source, "--arithmetic", arithmetic, "--opt", optimization, native_fault=True)
                        self.assertNotEqual(result.returncode, 0)
                        self.assertIn(diagnostic, result.stderr)

    def test_ordered_places_and_cleanup_match_at_both_native_optimization_levels(self):
        source = self.source('''#import "Basic";
next :: (counter:[]int)->int {value:=counter[0];counter[0]+=1;return value;}
work :: (counter:[]int,out:[]int)->int {
 defer out[0]=out[0]*10+1;
 defer out[0]=out[0]*10+2;
 out[next(counter)]+=next(counter);
 return out[0]+out[1]*10+counter[0]*100;
}
main :: () {
 counter:=[1]int.[0];out:=[2]int.[4,7];
 result:=work(counter[..],out[..]);
 print("% % % %\\n",result,out[0],out[1],counter[0]);
}
''')
        for arithmetic in ("checked", "wrapping"):
            for optimization in ("0", "2"):
                with self.subTest(arithmetic=arithmetic, optimization=optimization):
                    result = self.call("run", source, "--arithmetic", arithmetic, "--opt", optimization)
                    self.assertEqual((result.returncode, result.stdout), (0, "275 521 7 2\n"), result.stderr)

    def test_existing_exact_float_and_noncontracted_operations_are_preserved(self):
        # Values supplied as bits avoid deciding decimal midpoint behavior.
        # The product rounds to 1 before subtraction; contraction would retain
        # a nonzero residual. This pins today's separate-operation contract.
        source = self.source('''#import "Basic";
separate :: (a:f32,b:f32)->f32 {return a*b-1.0f32;}
exact :: (a:f32,b:f32)->f32 {return a+b;}
#assert #run f32_bits(separate(f32_from_bits(1065353217),f32_from_bits(1065353215)))==0;
#assert #run f32_bits(exact(1.5f32,2.25f32))==1081081856;
main :: () {
 print("% %\\n",f32_bits(separate(f32_from_bits(1065353217),f32_from_bits(1065353215))),f32_bits(exact(1.5f32,2.25f32)));
}
''')
        for optimization in ("0", "2"):
            with self.subTest(optimization=optimization):
                result = self.call("run", source, "--arithmetic", "checked", "--opt", optimization)
                self.assertEqual((result.returncode, result.stdout), (0, "0 1081081856\n"), result.stderr)

    def test_typed_f32_decimal_literals_pin_the_existing_two_step_contract(self):
        # These are independent midpoint/adjacent-decimal vectors for today's
        # decimal->binary64->binary32 path. In particular, the short second
        # spelling rounds to the binary64 midpoint first. Direct decimal->f32
        # rounding remains a separate design/migration decision.
        source = self.source('''#import "Basic";
#authority;
literal_bits :: (which:int)->u32 #export_c {
 if which==0 {value:=1.000000059604644775390625f32;return f32_bits(value);}
 if which==1 {value:=1.0000000596046448f32;return f32_bits(value);}
 if which==2 {value:=1.000000059604645f32;return f32_bits(value);}
 value:=1.000000178813934326171875f32;return f32_bits(value);
}
#forall which:0..<4 {
 expected:=[4]u32.[1065353216,1065353216,1065353217,1065353218];
 #assert literal_bits(which)==expected[which];
}
main :: () {print("% % % %\\n",literal_bits(0),literal_bits(1),literal_bits(2),literal_bits(3));}
''')
        expected = "1065353216 1065353216 1065353217 1065353218\n"
        for arithmetic in ("checked", "wrapping"):
            for optimization in ("0", "2"):
                with self.subTest(arithmetic=arithmetic, optimization=optimization):
                    result = self.call("run", source, "--arithmetic", arithmetic, "--opt", optimization)
                    self.assertEqual((result.returncode, result.stdout), (0, expected), result.stderr)

    def test_compile_time_and_authority_checks_remain_checked(self):
        compile_time = self.source("bad :: (a:u8,b:u8)->u8 {return a+b;}\nVALUE :: #run bad(250,10);\nmain :: () {}\n", "constant.coax")
        authority = self.source('''#authority;
bad :: (a:u8,b:u8)->u8 #export_c {return a+b;}
#test "authority must execute checked arithmetic" {value:=bad(250,10);#assert value==4;}
main :: () {}
''', "authority.coax")
        for source in (compile_time, authority):
            for flags in ([], ["--release"], ["--release", "--arithmetic", "checked"], ["--arithmetic", "wrapping"]):
                with self.subTest(source=source.name, flags=flags):
                    result = self.call("c", source, "--json", *flags)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn('"E0407"', result.stderr)

    def test_profile_report_records_loaded_source_content_and_emission(self):
        helper = self.source("answer :: ()->int {return 7;}\n", "helper.coax")
        source = self.source('#load "helper.coax";\nmain :: () {value:=answer();}\n')
        first, data = self.report("c", source, "--arithmetic", "checked", "--build-profile", "release")
        self.assertIsNone(data["native_optimization_flag"])
        self.assertEqual(data["emitted_sha256"], hashlib.sha256(first.stdout.encode()).hexdigest())
        by_path = {row["path"]: row["sha256"] for row in data["sources"]}
        for path in (source, helper):
            self.assertEqual(by_path[str(path.resolve())], hashlib.sha256(path.read_bytes()).hexdigest())
        helper.write_text("answer :: ()->int {return 9;}\n")
        second, changed = self.report("c", source, "--arithmetic", "checked", "--build-profile", "release")
        self.assertNotEqual(changed["emitted_sha256"], data["emitted_sha256"])
        self.assertNotEqual(changed["sources"], data["sources"])
        self.assertEqual(changed["emitted_sha256"], hashlib.sha256(second.stdout.encode()).hexdigest())

    def test_invalid_choices_duplicates_and_unsupported_optimization_refuse(self):
        source = self.source(SOURCE)
        bad_options = (
            ["--arithmetic"], ["--arithmetic", "fast"], ["--arithmetic", "checked", "--arithmetic", "checked"],
            ["--build-profile"], ["--build-profile", "debug"], ["--build-profile", "release", "--build-profile", "development"],
            ["--opt"], ["--opt", "-1"], ["--opt", "4"], ["--opt", "2x"], ["--opt", "2", "--opt", "2"],
            ["--profile-report"], ["--profile-report", str(self.dir / "a.json"), "--profile-report", str(self.dir / "b.json")],
        )
        for flags in bad_options:
            with self.subTest(flags=flags):
                result = self.call("build", source, *flags, "-o", self.dir / "invalid")
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.dir / "invalid").exists())
        for command in ("c", "header", "layout", "deps", "check", "test", "mutate", "metal", "cuda", "metal-compute"):
            with self.subTest(command=command):
                result = self.call(command, source, "--opt", "2")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("--opt", result.stderr)

    def test_mutation_shards_bind_resolved_profiles_even_for_identical_sites(self):
        source = self.source('''#authority;
adjust :: (value:int)->int #export_c {return value+1;}
#forall value:0..<3 {#assert adjust(value)==value+1;}
''')
        combinations = (
            (["--build-profile", "development"], ["--build-profile", "release"]),
            (["--arithmetic", "checked"], ["--arithmetic", "wrapping"]),
        )
        for left_flags, right_flags in combinations:
            with self.subTest(left=left_flags, right=right_flags):
                records = []
                for shard, flags in (("1/2", left_flags), ("2/2", right_flags)):
                    result = self.call("mutate", source, "--jobs", "1", "--shard", shard, *flags)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    record = self.dir / ("shard-" + shard[0] + ".rec")
                    record.write_text(result.stdout)
                    records.append(record)
                merged = self.call("mutate", source, "--merge", records[0], "--merge", records[1], *left_flags)
                self.assertNotEqual(merged.returncode, 0, "mixed-profile receipts were merged")
                self.assertRegex(merged.stderr.lower(), "profile|fingerprint|differ")

    def test_report_frontend_failure_records_no_native_or_artifact(self):
        source = self.source("main :: () {value:=missing;}\n")
        env, log = self.captured_compiler()
        report = self.dir / "failure.json"
        artifact = self.dir / "not-built"
        result = self.call("build", source, "--opt", "2", "--profile-report", report, "-o", artifact, env=env)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("E0306", result.stderr)
        data = json.loads(report.read_text())
        self.assertFalse(data["success"])
        self.assertEqual(data["exit_status"], result.returncode)
        self.assertIsNone(data["native_optimization_flag"])
        self.assertIsNone(data["emitted_sha256"])
        self.assertIsNone(data["artifact_sha256"])
        self.assertFalse(artifact.exists())
        self.assertFalse(log.exists())

    def test_report_failed_compiler_start_does_not_claim_applied_optimization(self):
        source = self.source(SOURCE)
        report = self.dir / "compiler-start.json"
        artifact = self.dir / "not-built"
        env = {**os.environ, "COAX_CC": str(self.dir / "missing-native-compiler")}
        result = self.call("build", source, "--opt", "2", "--profile-report", report, "-o", artifact, env=env)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("cannot start", result.stderr)
        data = json.loads(report.read_text())
        self.assertFalse(data["success"])
        self.assertIsNone(data["native_optimization_flag"])
        self.assertIsNotNone(data["emitted_sha256"])
        self.assertIsNone(data["artifact_sha256"])
        self.assertFalse(artifact.exists())

    def test_check_report_has_no_emitted_or_native_artifact(self):
        source = self.source(SOURCE)
        _, data = self.report("check", source)
        self.assertTrue(data["success"])
        self.assertIsNone(data["native_optimization_flag"])
        self.assertIsNone(data["emitted_sha256"])
        self.assertIsNone(data["artifact_sha256"])

    def test_incomplete_descriptor_graph_refuses_to_publish_a_report(self):
        source = self.source('#import_proto "missing.pb" as P;\nmain :: () {}\n')
        report = self.dir / "descriptor-failure.json"
        result = self.call("c", source, "--profile-report", report)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("E0502", result.stderr)
        self.assertFalse(report.exists(), "an incomplete input graph must not publish a report into an unadmitted destination")

    def test_profile_report_hashes_consumed_descriptor_bytes(self):
        descriptor = self.dir / "profile.pb"
        descriptor.write_bytes((ROOT.parent / "proto/descriptors/ios-home.pb").read_bytes())
        source = self.source('#import_proto "profile.pb" as P;\nmain :: () {}\n')
        result, data = self.report("c", source)
        row = next(row for row in data["descriptors"] if Path(row["path"]).resolve() == descriptor.resolve())
        self.assertEqual(row["sha256"], hashlib.sha256(descriptor.read_bytes()).hexdigest())
        self.assertEqual(data["emitted_sha256"], hashlib.sha256(result.stdout.encode()).hexdigest())

    def test_report_destinations_cannot_overwrite_inputs(self):
        for kind in ("root", "loaded", "symlink", "hardlink", "partial-load"):
            with self.subTest(kind=kind):
                helper = self.source("answer :: ()->int {return 7;}\n", kind + "-helper.coax")
                text = '#load "' + helper.name + '";\n'
                if kind == "partial-load":
                    text += '#load "missing.coax";\n'
                source = self.source(text + "main :: () {value:=answer();}\n", kind + ".coax")
                destination = source if kind == "root" else helper
                if kind in ("symlink", "hardlink"):
                    destination = self.dir / (kind + "-alias")
                    if kind == "symlink":
                        destination.symlink_to(source)
                    else:
                        os.link(helper, destination)
                expected = {path: path.read_bytes() for path in (source, helper)}
                result = self.call("c", source, "--profile-report", destination)
                self.assertNotEqual(result.returncode, 0, "a report overwrote an input")
                for path, contents in expected.items():
                    self.assertEqual(path.read_bytes(), contents, str(path))
        descriptor = self.dir / "profile.pb"
        descriptor.write_bytes((ROOT.parent / "proto/descriptors/ios-home.pb").read_bytes())
        original = descriptor.read_bytes()
        source = self.source('#import_proto "profile.pb" as P;\nmain :: () {}\n', "descriptor.coax")
        result = self.call("c", source, "--profile-report", descriptor)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(descriptor.read_bytes(), original)

    def test_report_destinations_cannot_alias_declared_or_native_outputs(self):
        sentinel = b"existing output must survive rejected profile configuration\n"
        for kind in ("c-output", "native-output", "implicit-native-output", "native-c", "native-object", "header", "map", "timings"):
            with self.subTest(kind=kind):
                source = self.source(SOURCE, kind + ".coax")
                artifact = self.dir / (kind + "-artifact")
                if kind == "c-output":
                    command, extra, destination = "c", ["-o", artifact], artifact
                elif kind == "native-output":
                    command, extra, destination = "build", ["-o", artifact], artifact
                elif kind == "implicit-native-output":
                    command, extra, destination = "build", [], source.with_suffix("")
                elif kind in ("native-c", "native-object"):
                    command, extra = "build", ["-o", artifact, "--keep-c"]
                    destination = Path(str(artifact) + (".c" if kind == "native-c" else ".o"))
                else:
                    option = {"header": "--header", "map": "--source-map", "timings": "--timings"}[kind]
                    destination = self.dir / (kind + ".json")
                    command, extra = "c", [option, destination]
                destination.write_bytes(sentinel)
                result = self.call(command, source, *extra, "--profile-report", destination)
                self.assertNotEqual(result.returncode, 0, "a report overwrote an output")
                self.assertEqual(destination.read_bytes(), sentinel)

    def test_debug_selection_duplicates_and_source_map_conflicts_refuse(self):
        source = self.source(SOURCE)
        map_path = self.dir / "source-map.json"
        options = (
            ["--debug-info", "--debug-info"], ["--no-debug-info", "--no-debug-info"],
            ["--debug-info", "--no-debug-info"], ["--no-debug-info", "--debug-info"],
            ["--source-map", map_path, "--no-debug-info"], ["--no-debug-info", "--source-map", map_path],
        )
        for flags in options:
            with self.subTest(flags=flags):
                artifact = self.dir / "not-built"
                result = self.call("build", source, *flags, "-o", artifact)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(artifact.exists())
                self.assertFalse(map_path.exists())


if __name__ == "__main__":
    unittest.main()
