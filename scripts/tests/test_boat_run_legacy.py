"""Regression tests for the legacy Boat runner without network access."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'boat-run-legacy.sh'


class BoatRunLegacyTests(unittest.TestCase):
    def run_boat(self, responses):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bin_dir = root / 'bin'
            bin_dir.mkdir()
            state = root / '.openagents' / 'boat'
            state.mkdir(parents=True)
            (state / 'test').write_text('sandbox-123')
            (root / 'responses.json').write_text(json.dumps(responses))
            mocks = {
                'git': '#!/usr/bin/env bash\nif [ "$1" = rev-parse ]; then echo "$HOME"; fi\n',
                'sleep': '#!/usr/bin/env bash\necho sleep >> "$HOME/sleeps"\n',
                'curl': '''#!/usr/bin/env python3
import json, os, pathlib, sys
root = pathlib.Path(os.environ['HOME'])
url = next(a for a in sys.argv if a.startswith('https://'))
if url.endswith('/commands/process-456'):
    counter = root / 'polls'
    count = int(counter.read_text()) if counter.exists() else 0
    responses = json.loads((root / 'responses.json').read_text())
    counter.write_text(str(count + 1))
    if count >= len(responses):
        sys.exit(90)
    print(json.dumps(responses[count]))
elif url.endswith('/commands'):
    print(json.dumps({'processId': 'process-456'}))
elif url.endswith('/files'):
    print(json.dumps({'type': 'file.written'}))
else:
    print(json.dumps({'state': 'ready'}))
''',
            }
            for name, text in mocks.items():
                path = bin_dir / name
                path.write_text(text)
                path.chmod(0o755)
            env = dict(os.environ, HOME=str(root),
                       PATH=str(bin_dir) + os.pathsep + os.environ['PATH'],
                       BOAT_API_KEY='test-placeholder')
            env.pop('OA_ARTIFACT_BUCKET', None)
            result = subprocess.run(['bash', str(SCRIPT), 'test', '--', 'true'],
                                    env=env, capture_output=True, text=True, timeout=10)
            polls = int((root / 'polls').read_text())
            sleeps = (root / 'sleeps').read_text().count('sleep') if (root / 'sleeps').exists() else 0
            return result, polls, sleeps

    def test_non_exited_status_fails_without_polling_again(self):
        for status in ('lost', 'failed', 'cancelled', 'unknown', '', None):
            with self.subTest(status=status):
                response = {'stdout': 'partial output\n', 'stderr': 'partial error\n',
                            'exitCode': 0, 'running': False}
                if status is not None:
                    response['status'] = status
                result, polls, sleeps = self.run_boat([response])
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertEqual(polls, 1)
                self.assertEqual(sleeps, 0)
                self.assertEqual(result.stdout, 'partial output\n')
                self.assertIn('partial error\n', result.stderr)
                self.assertIn('command process-456 on sandbox sandbox-123 failed with status', result.stderr)
                self.assertIn(status or '""', result.stderr)

    def test_running_then_exited_preserves_output_and_exit_code(self):
        for code in (0, 7, None):
            with self.subTest(code=code):
                result, polls, sleeps = self.run_boat([
                    {'status': 'running', 'running': True},
                    {'status': 'exited', 'exitCode': code,
                     'stdout': 'done\n', 'stderr': 'warning\n'},
                ])
                self.assertEqual(result.returncode, 1 if code is None else code)
                self.assertEqual(polls, 2)
                self.assertEqual(sleeps, 1)
                self.assertEqual(result.stdout, 'done\n')
                self.assertIn('warning\n', result.stderr)
                self.assertNotIn('failed with status', result.stderr)

    def test_running_then_lost_fails_even_with_best_effort_running_flag(self):
        result, polls, sleeps = self.run_boat([
            {'status': 'running'},
            {'status': 'lost', 'running': True, 'exitCode': 0},
        ])
        self.assertEqual(result.returncode, 1)
        self.assertEqual(polls, 2)
        self.assertEqual(sleeps, 1)
        self.assertIn('failed with status lost', result.stderr)


if __name__ == '__main__':
    unittest.main()
