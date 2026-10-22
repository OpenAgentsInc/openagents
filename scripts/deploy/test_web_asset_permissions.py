"""Keep exported browser assets readable under a private agent umask."""

import os
from pathlib import Path
import re
import shlex
import shutil
import stat
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


class AssetPermissions(unittest.TestCase):
    @unittest.skipUnless(shutil.which("gcloud"), "gcloud verifies the filtered build context")
    def test_the_filtered_context_includes_only_the_embedded_editor_catalog(self):
        with tempfile.TemporaryDirectory(dir=os.environ["OPENAGENTS_SCRATCH"]) as temporary:
            root = Path(temporary)
            source = root / "apps/haunt-preview/src"
            source.mkdir(parents=True)
            catalog = source / "assets.json"
            catalog.write_bytes((ROOT / "apps/haunt-preview/src/assets.json").read_bytes())
            (source / "main.ts").write_text("unrelated editor source")
            (source.parent / ".env").write_text("unrelated private settings")
            (root / ".gcloudignore").write_bytes(
                (ROOT / "crates/openagents-web/web.gcloudignore").read_bytes()
            )
            files = subprocess.run(
                ["gcloud", "meta", "list-files-for-upload"], cwd=root,
                check=True, capture_output=True, text=True,
            ).stdout.splitlines()
            self.assertIn("apps/haunt-preview/src/assets.json", files)
            self.assertNotIn("apps/haunt-preview/src/main.ts", files)
            self.assertNotIn("apps/haunt-preview/.env", files)

    def test_context_export_ignores_the_agents_private_umask(self):
        script = (ROOT / "scripts/deploy/web.sh").read_text()
        export = re.search(r"^export_context\(\) \{\n.*?^\}", script, re.M | re.S).group()
        with tempfile.TemporaryDirectory(dir=os.environ["OPENAGENTS_SCRATCH"]) as temporary:
            root = Path(temporary)
            repo, out = root / "repo", root / "out"
            repo.mkdir()
            out.mkdir()
            font = repo / "fonts/Geist.ttf"
            font.parent.mkdir()
            font.write_bytes(b"font")
            start = repo / "start.sh"
            start.write_text("#!/bin/sh\n")
            start.chmod(0o755)
            for args in (["init", "-q"], ["add", "."],
                         ["-c", "user.name=Test", "-c", "user.email=test@example.com",
                          "-c", "core.hooksPath=/dev/null", "commit", "-qm", "Fixture"]):
                subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True)
            font.chmod(0o600)
            subprocess.run(["sh", "-c", 'umask 077\n' + export + '\nexport_context HEAD "$1"',
                            "export-test", str(out)], check=True, env={**os.environ, "ROOT": str(repo)})
            self.assertEqual(stat.S_IMODE((out / "fonts/Geist.ttf").stat().st_mode), 0o644)
            self.assertEqual(stat.S_IMODE((out / "fonts").stat().st_mode), 0o755)
            self.assertTrue((out / "start.sh").stat().st_mode & stat.S_IXOTH)

    def test_runtime_assets_are_readable_and_executables_still_run(self):
        for name in ("Dockerfile", "Dockerfile.native-overlay", "Dockerfile.components"):
            dockerfile = (ROOT / "crates/openagents-web" / name).read_text()
            normalize = re.search(r"^RUN (chmod .* /srv)$", dockerfile, re.M).group(1)
            # The check must run with the same non-root identity as the server.
            self.assertIn('USER 10001:10001\nRUN unreadable="$(find /srv ! -readable -print -quit)" && test -z "$unreadable"', dockerfile)
            self.assertLess(dockerfile.index("RUN " + normalize), dockerfile.index("USER 10001:10001"))
        with tempfile.TemporaryDirectory(dir=os.environ["OPENAGENTS_SCRATCH"]) as temporary:
            assets = Path(temporary) / "srv"
            fonts = assets / "gpui-fonts"
            fonts.mkdir(parents=True)
            files = [fonts / ("Geist.ttf" + suffix) for suffix in ("", ".br", ".gz")]
            for file in files:
                file.write_bytes(b"font")
                file.chmod(0o600)
            executable = assets / "first-frame-capture"
            executable.write_text("#!/bin/sh\nexit 0\n")
            executable.chmod(0o700)
            fonts.chmod(0o700)
            subprocess.run(["sh", "-c", normalize.replace("/srv", shlex.quote(str(assets)))], check=True)
            for file in files:
                self.assertTrue(file.stat().st_mode & stat.S_IROTH)
            self.assertTrue(fonts.stat().st_mode & stat.S_IXOTH)
            self.assertEqual(subprocess.run([str(executable)], check=False).returncode, 0)


if __name__ == "__main__":
    unittest.main()
