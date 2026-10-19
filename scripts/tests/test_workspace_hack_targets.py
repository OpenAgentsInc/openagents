"""Check that generated dependency normalization preserves Wasm builds."""

from contextlib import redirect_stderr, redirect_stdout
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import tomllib
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "workspace_hack_targets",
    Path(__file__).resolve().parents[1] / "dev/workspace-hack-targets.py",
)
targets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(targets)


class TargetDependencyTests(unittest.TestCase):
    def test_missing_dependency_requires_hakari(self):
        with self.assertRaisesRegex(ValueError, "cargo hakari manage-deps"):
            targets.rewrite_manifest('[package]\nname = "example"\n')

    def test_new_unconditional_edge_moves_without_other_changes(self):
        text = '[dependencies]\nserde = "1"\n' + targets.DEPENDENCY_LINE
        updated = targets.rewrite_manifest(text)
        self.assertEqual(
            updated,
            '[dependencies]\nserde = "1"\n\n'
            + targets.TARGET_HEADER + targets.DEPENDENCY_LINE,
        )
        self.assertEqual(targets.rewrite_manifest(updated), updated)

    def test_existing_target_dependencies_and_comments_are_preserved(self):
        text = ('[dependencies]\n' + targets.DEPENDENCY_LINE + '\n'
                + targets.TARGET_HEADER + '# Existing native library.\nlibc = "0.2"\n'
                + '\n[dev-dependencies]\ntempfile = "3"\n')
        updated = targets.rewrite_manifest(text)
        self.assertEqual(updated.count(targets.TARGET_HEADER), 1)
        self.assertIn('# Existing native library.\nlibc = "0.2"\n', updated)
        self.assertTrue(updated.endswith('[dev-dependencies]\ntempfile = "3"\n'))
        self.assertEqual(targets.rewrite_manifest(updated), updated)

    def test_native_edge_prevents_wasm_from_enabling_the_hack(self):
        updated = targets.rewrite_manifest('[dependencies]\n' + targets.DEPENDENCY_LINE)
        manifest = tomllib.loads(updated)
        self.assertNotIn(targets.HACK, manifest['dependencies'])
        self.assertEqual(list(manifest['target']), [targets.NATIVE_TARGET])
        self.assertEqual(
            manifest['target'][targets.NATIVE_TARGET]['dependencies'][targets.HACK],
            targets.GENERATED_DEPENDENCY,
        )

    def test_a_wasm_edge_is_rejected(self):
        text = '[target.\'cfg(target_arch = "wasm32")\'.dependencies]\n' + targets.DEPENDENCY_LINE
        with self.assertRaisesRegex(ValueError, "must use"):
            targets.rewrite_manifest(text)

    def test_existing_native_edge_deduplicates_new_hakari_edge(self):
        text = ('[dependencies]\n' + targets.DEPENDENCY_LINE + '\n'
                + targets.TARGET_HEADER + targets.DEPENDENCY_LINE)
        updated = targets.rewrite_manifest(text)
        self.assertEqual(updated.count(targets.DEPENDENCY_LINE), 1)

    def test_custom_dependency_is_not_silently_rewritten(self):
        text = '[dependencies]\nopenagents-workspace-hack = { version = "0.1", optional = true }\n'
        with self.assertRaisesRegex(ValueError, "unexpected"):
            targets.rewrite_manifest(text)

    def test_root_native_edge_moves_to_existing_desktop_table(self):
        desktop_header = f"[target.'{targets.DESKTOP_TARGET}'.dependencies]\n"
        text = (targets.TARGET_HEADER + targets.DEPENDENCY_LINE + 'libc = "0.2"\n\n'
                + desktop_header + '# Desktop settings.\nwinit = "0.30"\n')
        updated = targets.rewrite_manifest(text, targets.DESKTOP_TARGET)
        self.assertIn(targets.TARGET_HEADER + 'libc = "0.2"\n', updated)
        self.assertIn('# Desktop settings.\nwinit = "0.30"\n', updated)
        self.assertEqual(updated.count(desktop_header), 1)
        manifest = tomllib.loads(updated)
        self.assertNotIn(targets.HACK, manifest['target'][targets.NATIVE_TARGET]['dependencies'])
        self.assertEqual(
            manifest['target'][targets.DESKTOP_TARGET]['dependencies'][targets.HACK],
            targets.GENERATED_DEPENDENCY,
        )
        self.assertEqual(targets.rewrite_manifest(updated, targets.DESKTOP_TARGET), updated)

    def test_root_migration_removes_empty_old_native_table(self):
        text = targets.TARGET_HEADER + targets.DEPENDENCY_LINE
        updated = targets.rewrite_manifest(text, targets.DESKTOP_TARGET)
        self.assertNotIn(targets.TARGET_HEADER, updated)
        self.assertEqual(list(tomllib.loads(updated)['target']), [targets.DESKTOP_TARGET])
        self.assertEqual(targets.rewrite_manifest(updated, targets.DESKTOP_TARGET), updated)

    def test_existing_empty_old_native_table_is_removed(self):
        desktop_header = f"[target.'{targets.DESKTOP_TARGET}'.dependencies]\n"
        text = targets.TARGET_HEADER + '\n' + desktop_header + targets.DEPENDENCY_LINE
        self.assertEqual(
            targets.rewrite_manifest(text, targets.DESKTOP_TARGET),
            desktop_header + targets.DEPENDENCY_LINE,
        )

    def test_empty_old_native_table_with_comments_is_preserved(self):
        for comment in ('# Keep this native table.\n', ''):
            header = targets.TARGET_HEADER if comment else targets.TARGET_HEADER.rstrip() + ' # Keep.\n'
            text = header + comment + targets.DEPENDENCY_LINE
            updated = targets.rewrite_manifest(text, targets.DESKTOP_TARGET)
            self.assertIn(header + comment, updated)


class WorkspaceCheckTests(unittest.TestCase):
    def metadata(self, command, *, cwd, env, text):
        self.assertEqual(command, ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'])
        self.assertFalse(any(key in env for key in ('RUSTC', 'CARGO_BUILD_RUSTC', 'RUSTUP_TOOLCHAIN', 'CARGO')))
        return json.dumps({
            'workspace_members': ['example'],
            'packages': [{'id': 'example', 'name': 'example',
                          'manifest_path': str(cwd / 'crates/example/Cargo.toml')}],
        })

    def test_check_is_read_only_and_fix_covers_both_workspaces(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get('OPENAGENTS_SCRATCH')) as directory, patch.object(
            targets.subprocess, 'check_output', side_effect=self.metadata,
        ):
            root = Path(directory) / 'checkout'
            root.mkdir()
            alias = Path(directory) / 'checkout-alias'
            alias.symlink_to(root, target_is_directory=True)
            paths = []
            text = ('[package]\nname = "example"\n\n[dependencies]\n'
                    + targets.DEPENDENCY_LINE)
            for workspace in (root, root / 'apps/openagents'):
                (workspace / '.config').mkdir(parents=True)
                (workspace / '.config/hakari.toml').write_text(
                    f'hakari-package = "{targets.HACK}"\n')
                (workspace / 'Cargo.toml').write_text(
                    '[workspace]\nmembers = ["crates/*"]\n')
                path = workspace / 'crates/example/Cargo.toml'
                path.parent.mkdir(parents=True)
                path.write_text(text)
                paths.append(path)
            errors = io.StringIO()
            with redirect_stderr(errors):
                self.assertEqual(targets.main(['--check'], alias), 1)
            self.assertEqual(errors.getvalue().count('workspace-hack needs'), 2)
            self.assertTrue(all(path.read_text() == text for path in paths))
            with redirect_stdout(io.StringIO()):
                self.assertEqual(targets.main([], alias), 0)
            self.assertTrue(all(path.read_text() != text for path in paths))
            self.assertIn(targets.DESKTOP_TARGET, paths[0].read_text())
            self.assertIn(targets.NATIVE_TARGET, paths[1].read_text())
            self.assertEqual(targets.main(['--check'], alias), 0)

    def test_automatic_path_members_are_checked(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get('OPENAGENTS_SCRATCH')) as directory:
            root = Path(directory)
            (root / '.config').mkdir()
            (root / '.config/hakari.toml').write_text(
                f'hakari-package = "{targets.HACK}"\n')
            path = root / 'helpers/implicit/Cargo.toml'
            metadata = json.dumps({
                'workspace_members': ['implicit'],
                'packages': [
                    {'id': 'implicit', 'name': 'implicit', 'manifest_path': str(path)},
                    {'id': 'external', 'name': 'external', 'manifest_path': '/external/Cargo.toml'},
                ],
            })
            with patch.object(targets.subprocess, 'check_output', return_value=metadata):
                self.assertEqual(list(targets.workspace_manifests(root)), [path.resolve()])


if __name__ == '__main__':
    unittest.main()
