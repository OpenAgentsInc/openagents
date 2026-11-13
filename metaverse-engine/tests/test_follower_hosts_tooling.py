"""Failure handling for the follower qualification tool; no engine reruns."""
import copy
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_follower_hosts as runner


class FollowerToolingTests(unittest.TestCase):
    def test_recorded_cadences_really_close64_steps_without_losing_fractional_time(self):
        for fps in [30, 60, 120, 144]:
            with self.subTest(fps=fps):
                frames = runner.cadence(fps)
                total = sum(map(runner.frame_ns, frames))
                self.assertEqual(total, 1_000_000_000)
                self.assertEqual(divmod(total, 15_625_000), (64, 0))
                self.assertTrue(all(0 < frame <= 1 / fps + .000001 for frame in frames))
                self.assertIn(len(frames), [fps, fps + 1])

    def test_admission_requires_every_success_and_exact_normal_profile(self):
        admitted = {'exit_code': 0, 'source_changes': [], 'compiler_unchanged': True,
            'wrapper_unchanged': True, 'clang_wrapper_unchanged': True, 'actual_clang_unchanged': True,
            'compiler_sha256': 'compiler', 'normal_coax_c_flags': ['--library', '--header', '<unit>.h']}
        self.assertTrue(runner.admitted_page_ok(admitted, 'compiler'))
        for name, bad in {'exit_code': 1, 'source_changes': ['changed'], 'compiler_unchanged': False,
                'wrapper_unchanged': False, 'clang_wrapper_unchanged': False, 'actual_clang_unchanged': False,
                'compiler_sha256': 'different', 'normal_coax_c_flags': ['--release']}.items():
            with self.subTest(name=name):
                altered = copy.deepcopy(admitted); altered[name] = bad
                self.assertFalse(runner.admitted_page_ok(altered, 'compiler'))
                altered = copy.deepcopy(admitted); del altered[name]
                self.assertFalse(runner.admitted_page_ok(altered, 'compiler'))

    def test_cache_requires_success_command_inputs_and_unchanged_output(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / 'artifact'; artifact.write_bytes(b'admitted')
            identity = {'command': ['cc', '-O2'], 'c': 'source', 'headers': 'headers', 'clang': 'compiler'}
            metadata = {'identity': identity, 'sha256': runner.gate.digest(artifact.read_bytes()), 'exit_code': 0}
            self.assertTrue(runner.reusable_artifact(metadata, identity, artifact))
            for name, different in {'command': ['cc', '-O0'], 'c': 'changed', 'headers': 'changed', 'clang': 'changed'}.items():
                altered = copy.deepcopy(identity); altered[name] = different
                self.assertFalse(runner.reusable_artifact(metadata, altered, artifact))
            for status in [None, 1, 124]:
                altered = copy.deepcopy(metadata); altered['exit_code'] = status
                self.assertFalse(runner.reusable_artifact(altered, identity, artifact))
            artifact.write_bytes(b'changed')
            self.assertFalse(runner.reusable_artifact(metadata, identity, artifact))

    def test_toolchain_stdout_cannot_capture_the_process_environment(self):
        # Inspect the actual subprocess request while the qualifier runs,
        # stopping before any C/Wasm/native command or host test executes.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); cc = root / 'cc'; cc.write_bytes(b'compiler fixture')
            original = runner.gate.run_process_group
            seen = []

            def intercepted(command, **kwargs):
                seen.append(command)
                if command[:2] == ['bash', '-c']:
                    script = command[2]
                    self.assertNotIn('env -0', script)
                    self.assertIn('printf "%s\\0" "$COAXIAL_CLANG" "$COAXIAL_SYSROOT" "$COAXIAL_RESOURCES" "$PATH"', script)
                    raise StopIteration('toolchain request inspected')
                # No compiler is run: write a harmless artifact so the
                # qualifier can reach the actual toolchain request.
                if '-o' in command:
                    Path(command[command.index('-o') + 1]).write_bytes(b'not executable')
                elif '--header' in command:
                    Path(command[command.index('--header') + 1]).write_bytes(b'header fixture')
                return 0

            args = type('Args', (), {'work': str(root / 'work'), 'record': str(root / 'record.json'),
                'coax': str(cc), 'realm': 'garden', 'mode': 'debug', 'admission': None,
                'cc': str(cc), 'node': 'node'})()
            with patch.object(runner.gate, 'run_process_group', side_effect=intercepted):
                with self.assertRaisesRegex(StopIteration, 'inspected'):
                    runner.qualify(args)
            saved = json.loads((root / 'record.json').read_text())
            self.assertEqual(saved['status'], 'failed')
            self.assertTrue(any(command[:2] == ['bash', '-c'] for command in seen))


if __name__ == '__main__': unittest.main()
