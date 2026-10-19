"""Check benchmark commands and source restoration without compiling Rust."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("build-bench.sh").resolve()


class BuildBenchTests(unittest.TestCase):
    def setUp(self):
        scratch = subprocess.check_output(
            ["openagents", "scratch"], text=True).strip()
        self.temp = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.sources = {}
        packages = []
        for name in ("shared", "consumer", "app"):
            source = self.root / name / "src" / "lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("pub fn original() {}\n")
            self.sources[name] = source
            packages.append({"name": name, "targets": [
                {"kind": ["lib"], "src_path": str(source)}]})
        (self.root / "metadata.json").write_text(json.dumps({"packages": packages}))
        helper = self.root / "scripts" / "dev" / "cargo-build-env.sh"
        helper.parent.mkdir(parents=True)
        helper.write_text("export RUSTC=fixture-rustc\n")
        self.executable("git", '#!/bin/sh\nprintf "%s\\n" "$BENCH_ROOT"\n')
        self.executable("openagents", '#!/bin/sh\nprintf "%s\\n" "$BENCH_ROOT"\n')
        self.executable("cargo", '''#!/usr/bin/python3
import json, os, pathlib, sys
root = pathlib.Path(os.environ["BENCH_ROOT"])
if sys.argv[1] == "metadata":
    print((root / "metadata.json").read_text())
    sys.exit(0)
edited = sorted(p.name for p in root.iterdir() if p.is_dir()
                and (p / "src/lib.rs").exists()
                and "build_bench_edit_" in (p / "src/lib.rs").read_text())
with (root / "commands.jsonl").open("a") as log:
    log.write(json.dumps({"args": sys.argv[1:], "edited": edited}) + "\\n")
if edited and os.environ.get("BENCH_FAIL_EDIT"):
    sys.exit(1)
''')
        self.env = dict(os.environ, BENCH_ROOT=str(self.root),
                        PATH=f"{self.bin}:{os.environ['PATH']}")
        self.out = self.root / "rows.tsv"

    def executable(self, name, source):
        path = self.bin / name
        path.write_text(source)
        path.chmod(0o755)

    def run_bench(self, *args, success=True):
        result = subprocess.run(
            ["bash", str(SCRIPT), "--target-dir", str(self.root / "target"),
             "--out", str(self.out), *args],
            env=self.env, text=True, capture_output=True)
        if success:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)
        for source in self.sources.values():
            self.assertEqual(source.read_text(), "pub fn original() {}\n")
        return result

    def commands(self):
        return [json.loads(line) for line in
                (self.root / "commands.jsonl").read_text().splitlines()]

    def rows(self):
        return [line.split("\t") for line in self.out.read_text().splitlines()]

    def test_dependency_edit_measures_consumers_for_build_and_test(self):
        self.run_bench("--edit-package", "shared", "consumer", "app")
        commands = self.commands()
        self.assertEqual(len(commands), 12)
        for offset, (package, command) in enumerate([
                ("consumer", "build"), ("consumer", "test"),
                ("app", "build"), ("app", "test")]):
            group = commands[offset * 3:offset * 3 + 3]
            for call in group:
                self.assertEqual(call["args"][0], command)
                self.assertIn(package, call["args"])
                self.assertEqual("--no-run" in call["args"], command == "test")
                self.assertNotIn("consumer", call["edited"])
                self.assertNotIn("app", call["edited"])
            self.assertEqual(group[-1]["edited"], ["shared"])
        self.assertEqual(commands[0]["edited"], [])
        self.assertEqual(commands[1]["edited"], [])
        self.assertEqual([row[-1] for row in self.rows()], ["shared"] * 4)
        self.assertEqual([row[2] for row in self.rows()],
                         ["consumer", "consumer", "app", "app"])

    def test_default_edits_the_measured_package(self):
        self.run_bench("--only", "build", "consumer")
        self.assertEqual([call["edited"] for call in self.commands()],
                         [[], [], ["consumer"]])
        self.assertEqual(self.rows()[0][-1], "consumer")

    def test_failed_edit_build_restores_dependency(self):
        self.env["BENCH_FAIL_EDIT"] = "1"
        result = self.run_bench("--edit-package", "shared", "consumer", success=False)
        self.assertIn("failed:", result.stderr)
        self.assertEqual(self.commands()[-1]["edited"], ["shared"])
        self.assertFalse(self.out.exists())

    def test_unknown_edit_package_fails_before_cold_build(self):
        result = self.run_bench("--edit-package", "missing", "--cold",
                                "consumer", success=False)
        self.assertIn("No package named missing", result.stderr)
        self.assertFalse((self.root / "commands.jsonl").exists())

    def test_missing_edit_package_value_is_reported(self):
        result = self.run_bench("consumer", "--edit-package", success=False)
        self.assertIn("Name a package after --edit-package.", result.stderr)

    def test_help_includes_dependency_edit(self):
        result = self.run_bench("--help")
        self.assertIn("--edit-package P", result.stdout)
        self.assertIn("--out FILE", result.stdout)


if __name__ == "__main__":
    unittest.main()
