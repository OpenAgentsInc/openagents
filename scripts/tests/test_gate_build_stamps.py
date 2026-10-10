"""Build scripts must not rerun on every `git add` (audit CD-02, X-LINT-02).

A build script that watches the Git index reruns whenever any file in the
monorepo is staged, and recompiles its crate and every dependent. Commit
stamps watch `HEAD` and the branch ref only; dirtiness comes from the
environment the release and install scripts set.
"""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class BuildStampTests(unittest.TestCase):
    def build_scripts(self):
        return sorted((ROOT / "crates").glob("*/build.rs"))

    def test_no_build_script_watches_the_git_index(self):
        for script in self.build_scripts():
            code = "\n".join(
                line for line in script.read_text().splitlines()
                if not line.lstrip().startswith("//"))
            self.assertIsNone(
                re.search(r'"index"', code),
                f"{script.relative_to(ROOT)} watches the Git index")

    def test_coder_stamps_take_dirtiness_from_the_environment(self):
        for name in ["coder", "coder-new"]:
            code = (ROOT / "crates" / name / "build.rs").read_text()
            self.assertIn("CODER_BUILD_DIRTY", code)
            self.assertNotIn('"status"', code, name)


if __name__ == "__main__":
    unittest.main()
