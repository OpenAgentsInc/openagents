import pathlib
import subprocess
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).with_name('wait-job-logs.py')


class WaitLogsTests(unittest.TestCase):
    def test_success_and_large_logs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / 'build.log').write_text('x' * 70000 + '\nlast line\n')
            (root / 'build.exit').write_text('0')
            result = subprocess.run(['python3', str(SCRIPT), directory, 'build'], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0)
            self.assertIn('last line', result.stdout)
            self.assertIn('finished: exit 0', result.stdout)

    def test_failure_and_timeout(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / 'tests.exit').write_text('2')
            result = subprocess.run(['python3', str(SCRIPT), directory, 'tests'], capture_output=True)
            self.assertEqual(result.returncode, 1)
            result = subprocess.run(['python3', str(SCRIPT), directory, 'build', '--timeout', '.05', '--interval', '.01'], capture_output=True)
            self.assertEqual(result.returncode, 124)
            self.assertIn(b'build running', result.stdout)


if __name__ == '__main__':
    unittest.main()
