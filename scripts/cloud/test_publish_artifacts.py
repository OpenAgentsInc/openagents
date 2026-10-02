import importlib.util
import json
import pathlib
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('publisher', pathlib.Path(__file__).with_name('publish-artifacts.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class Publication(unittest.TestCase):
    def test_upload_and_expiring_comment(self):
        with tempfile.TemporaryDirectory(prefix='run-') as directory:
            for name in ('stdout.log', 'stderr.log', 'change.patch', 'evidence.json'):
                pathlib.Path(directory, name).write_text('failed run evidence')
            def output(*args):
                return json.dumps([{'signed_url': 'https://storage.example/artifact?signature=test'}]) if 'sign-url' in args else ''
            with patch.object(module, 'run', side_effect=output) as calls, patch.object(module.subprocess, 'run') as comment:
                module.publish(directory, 'gs://test-artifacts', 10227, 'OpenAgentsInc/openagents')
                self.assertEqual(calls.call_count, 8)
                self.assertIn('24 hours', comment.call_args.kwargs['input'])
                self.assertIn('stderr.log', comment.call_args.kwargs['input'])

    def test_upload_failure_never_comments_success(self):
        with tempfile.TemporaryDirectory(prefix='run-') as directory:
            pathlib.Path(directory, 'stdout.log').write_text('error')
            with patch.object(module, 'run', side_effect=RuntimeError('upload failed')), patch.object(module.subprocess, 'run') as comment:
                with self.assertRaises(RuntimeError):
                    module.publish(directory, 'gs://test-artifacts', 10227, 'OpenAgentsInc/openagents')
                comment.assert_not_called()

    def test_bucket_path_refused(self):
        with self.assertRaises(ValueError):
            module.publish('/tmp/run-test', 'gs://bucket/other', 1, 'owner/repo')


if __name__ == '__main__':
    unittest.main()
