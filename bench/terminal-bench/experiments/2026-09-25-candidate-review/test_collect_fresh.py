"""Keep shared job files unique while retaining every candidate's path."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class TraceCollectionTests(unittest.TestCase):
    def test_shared_config_has_one_path_and_identical_traces_share_a_blob(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            jobs = root / 'jobs'
            job = jobs / 'job'
            rows = []
            for name in ('first', 'second'):
                trial = job / name
                (trial / 'agent').mkdir(parents=True)
                (trial / 'agent/trace.json').write_text('{"same": true}\n')
                (trial / 'config.json').write_text(json.dumps({'trial': name}))
                rows.append({'job': 'job', 'trial': name})
            (job / 'config.json').write_text('{"shared": true}\n')
            manifest = root / 'inputs.json'
            manifest.write_text(json.dumps(rows))
            result = subprocess.run([
                sys.executable, str(Path(__file__).with_name('collect_fresh.py')),
                '--records', str(root), '--jobs', str(jobs), '--manifest', str(manifest),
            ], capture_output=True, text=True, check=True)
            stats = json.loads(result.stdout)
            files = json.loads((root / 'prospective-trace-files.json').read_text())['files']
            self.assertEqual(stats['files'], 5)
            self.assertEqual(stats['blobs'], 4)
            self.assertEqual(len({r['path'] for r in files}), 5)
            self.assertEqual(sum(r['path'] == 'job/config.json' for r in files), 1)
            self.assertTrue((root / 'prospective-traces.tar.gz').is_file())


if __name__ == '__main__':
    unittest.main()
