"""Keep daily verification targeted; full coverage needs release opt-in."""
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


class DevelopmentPlanTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        source = Path(__file__).resolve().parents[2]
        (self.root / "scripts").mkdir()
        for name in ["verify-rust.sh", "verify-changed.py"]:
            shutil.copyfile(source / "scripts" / name, self.root / "scripts" / name)
        manifest = self.root / "crates" / "example" / "Cargo.toml"
        manifest.parent.mkdir(parents=True)
        manifest.write_text('[package]\nname = "example"\n')
        self.git("init", "-q")
        self.git("add", ".")
        self.git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                 "commit", "-qm", "Fixture")
        self.git("update-ref", "refs/remotes/origin/main", "HEAD")

    def git(self, *args):
        subprocess.run(["git", *args], cwd=self.root, check=True,
                       capture_output=True, text=True)

    def plan(self, *args):
        run = subprocess.run(["bash", "scripts/verify-rust.sh", "--print", *args],
                             cwd=self.root, capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stderr)
        return run.stdout

    def test_default_checks_only_changed_package_once(self):
        (self.root / "crates/example/change.rs").write_text("// changed\n")
        plan = self.plan()
        self.assertIn("cargo fmt -p example --check", plan)
        self.assertIn("cargo clippy --locked -p example", plan)
        self.assertEqual(plan.count("cargo test "), 1)
        self.assertNotIn("test-postgres.sh", plan)
        self.assertNotIn("--features", plan)

    def test_lockfile_does_not_expand_development_scope(self):
        (self.root / "Cargo.lock").write_text("# fixture\n")
        (self.root / "crates/example/change.rs").write_text("// changed\n")
        plan = self.plan()
        self.assertIn("development checks remain scoped", plan)
        self.assertIn("cargo test --locked -p example", plan)
        self.assertNotIn("test-postgres.sh", plan)

    def test_no_crate_changes_does_not_run_workspace_cargo(self):
        (self.root / "README.md").write_text("Documentation only.\n")
        plan = self.plan()
        self.assertNotIn("cargo test ", plan)
        self.assertNotIn("cargo clippy ", plan)
        self.assertNotIn("cargo fmt ", plan)
        self.assertIn("no affected crates", plan)

    def test_explicit_crates_and_phases_stay_narrow(self):
        plan = self.plan("--crates", "example", "--phases", "tests")
        self.assertIn("cargo test --locked -p example", plan)
        self.assertNotIn("cargo fmt ", plan)
        self.assertNotIn("test-postgres.sh", plan)

    def test_release_opt_in_selects_workspace_matrix(self):
        plan = self.plan("--release")
        self.assertIn("Release gate:", plan)
        self.assertEqual(plan.count("cargo test "), 2)
        self.assertIn("--features", plan)
        self.assertIn("test-postgres.sh", plan)
        self.assertNotIn("-p example", plan)


if __name__ == "__main__":
    unittest.main()
