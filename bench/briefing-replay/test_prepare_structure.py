"""Synthetic preparation tests use an inert executable and a disposable Git repo."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("prepare_structure", Path(__file__).with_name("prepare_structure.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class PreparationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="structure-preparation-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        home = self.root / "home"
        home.mkdir()
        env = dict(os.environ, HOME=str(home), GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")
        def git(*args):
            return subprocess.check_output([
                "git", "-C", str(self.repo), "-c", "user.name=Fixture",
                "-c", "user.email=fixture@example.invalid", "-c", "core.hooksPath=/dev/null",
                "-c", "commit.gpgsign=false", *args,
            ], env=env, stderr=subprocess.PIPE, text=True).strip()
        git("init", "--quiet")
        (self.repo / "source.rs").write_text("pub fn example() {}\n")
        git("add", ".")
        git("commit", "--quiet", "-m", "Fixture")
        self.commit = git("rev-parse", "HEAD")
        self.issue = self.root / "issue.json"
        self.issue.write_text(json.dumps({"title": "Example", "body": "$(touch EXECUTED)"}))
        self.index = self.root / "index.json"
        self.index.write_text(json.dumps({"commit": self.commit}))
        self.binary = self.root / "fake-briefing"
        self.install_binary()

    def install_binary(self, size=24):
        script = f'''#!{sys.executable}
import json, pathlib, sys
args=sys.argv[1:]
def value(flag): return args[args.index(flag)+1]
if args[0]=="index":
    pathlib.Path(value("--output")).write_text(json.dumps({{"commit":value("--rev"),"index_ms":1.0}}))
else:
    out=pathlib.Path(value("--output-dir"));out.mkdir()
    if "--explicit-structure" in args:
        assert "--no-lexical" not in args and "--no-symbols" not in args
        payload="x"*{size}
    else:
        assert "--focused" in args and "--no-lexical" in args and "--no-symbols" in args
        payload="old focused payload\\n"
    pack={{"schema":"openagents.briefing-lab.explicit-structure.v1","byte_budget":16384,"packed_bytes":len(payload.encode()),"markdown":payload,"source_bytes":0,"selections":[],"omissions":[]}}
    (out/"focused.md").write_text(payload)
    (out/"briefing.json").write_text(json.dumps({{"commit":value("--rev"),"focused":pack,"evidence":[],"timings_ms":{{"assembly":0.5}}}}))
    print("Complete local preview including output: 2.000 ms")
'''
        self.binary.write_text(script)
        self.binary.chmod(0o755)

    def args(self, **overrides):
        values = dict(binary=self.binary, binary_sha256=module.digest(self.binary.read_bytes()), repo=self.repo,
                      rev=self.commit, issue_file=self.issue, issue_sha256=None, index=self.index,
                      index_sha256=None, output_dir=self.root / "output", prior_focused=None)
        values.update(overrides)
        return argparse.Namespace(**values)

    def test_payload_hashes_timings_and_frozen_policy_comparison(self):
        prior = self.root / "prior-focused.md"
        prior.write_text("old focused payload\n")
        result = module.prepare(self.args(prior_focused=prior))
        output = self.root / "output"
        payload = (output / "treatment.md").read_bytes()
        self.assertEqual(payload, (output / "preview" / "focused.md").read_bytes())
        self.assertEqual(result["hashes"]["treatment_sha256"], module.digest(payload))
        self.assertEqual(result["preview_flags"], ["--explicit-structure"])
        self.assertEqual(result["model_calls"], 0)
        self.assertEqual(result["external_git_probes"], 0)
        self.assertTrue(result["v2_comparison"]["identical"])
        self.assertGreater(result["timings_ms"]["cold_index_build_wall"], 0)
        self.assertGreater(result["timings_ms"]["warm_preview_wall"], 0)
        self.assertFalse((self.root / "EXECUTED").exists())
        self.assertFalse((self.repo / "EXECUTED").exists())

    def test_pin_mismatch_and_in_repo_output_are_rejected_before_creation(self):
        with self.assertRaisesRegex(ValueError, "Binary SHA-256"):
            module.prepare(self.args(binary_sha256="0" * 64))
        with self.assertRaisesRegex(ValueError, "outside"):
            module.prepare(self.args(output_dir=self.repo / "artifacts"))
        self.assertFalse((self.root / "output").exists())
        self.assertFalse((self.repo / "artifacts").exists())

    def test_oversized_payload_is_recorded_as_failed_and_not_copied(self):
        self.install_binary(size=16385)
        with self.assertRaisesRegex(ValueError, "maximum"):
            module.prepare(self.args())
        output = self.root / "output"
        self.assertFalse((output / "treatment.md").exists())
        self.assertEqual(json.loads((output / "preparation.json").read_text())["status"], "failed")

    def test_changed_frozen_output_is_not_accepted(self):
        prior = self.root / "prior-focused.md"
        prior.write_text("different output\n")
        with self.assertRaisesRegex(ValueError, "Frozen V2"):
            module.prepare(self.args(prior_focused=prior))
        self.assertFalse((self.root / "output" / "treatment.md").exists())


if __name__ == "__main__":
    unittest.main()
