"""Exercise transport retries with synthetic responses and no live services."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import urllib.error

import replay_resilient as replay


class TransportRetryTests(unittest.TestCase):
    def failure(self, code):
        return urllib.error.HTTPError('https://invalid.test/PRIVATE_PATH', code,
                                      'PRIVATE_RESPONSE', {'Authorization': 'PRIVATE_HEADER'}, None)

    def call(self, method, responses):
        records = []
        clock = [10.0]
        def delay(seconds):
            clock[0] += seconds
        with mock.patch.dict(replay.os.environ, {'BOAT_API_KEY': 'synthetic-key'}), \
             mock.patch.object(replay.urllib.request, 'urlopen', side_effect=responses) as opened, \
             mock.patch.object(replay.time, 'monotonic', side_effect=lambda: clock[0]), \
             mock.patch.object(replay.time, 'sleep', side_effect=delay) as slept:
            error = None
            try:
                replay.api(method, 'sandboxes/PRIVATE_SANDBOX/commands/PRIVATE_PROCESS',
                           {'command': 'PRIVATE_COMMAND'} if method != 'GET' else None,
                           retry_records=records)
            except Exception as failure:
                error = failure
        return records, opened.call_count, [call.args[0] for call in slept.call_args_list], error

    def test_get_retries_transient_failures_and_records_success_and_elapsed_backoff(self):
        records, calls, delays, error = self.call('GET', [self.failure(502), self.failure(503), io.BytesIO(b'{}')])
        self.assertIsNone(error)
        self.assertEqual(calls, 3)
        self.assertEqual(delays, [1, 2])
        self.assertEqual(records, [{'method': 'GET', 'operation': 'process_status', 'attempts': 3,
                                    'http_statuses': [502, 503], 'scheduled_backoff_s': [1, 2],
                                    'outcome': 'succeeded', 'elapsed_s': 3.0}])
        self.assertNotIn('PRIVATE', json.dumps(records))

    def test_get_exhaustion_is_bounded_and_retained(self):
        records, calls, delays, error = self.call('GET', [self.failure(504) for _ in range(4)])
        self.assertIsInstance(error, urllib.error.HTTPError)
        self.assertEqual(calls, 3)
        self.assertEqual(delays, [1, 2])
        self.assertEqual(records[0]['http_statuses'], [504, 504, 504])
        self.assertEqual(records[0]['outcome'], 'exhausted')
        self.assertEqual(records[0]['elapsed_s'], 3.0)

    def test_mutations_and_other_failures_are_never_retried(self):
        for method in ['POST', 'PUT', 'PATCH', 'DELETE']:
            for code in [502, 503, 504]:
                with self.subTest(method=method, code=code):
                    records, calls, delays, error = self.call(method, [self.failure(code)])
                    self.assertIsInstance(error, urllib.error.HTTPError)
                    self.assertEqual((calls, delays, records), (1, [], []))
        for failure in [self.failure(429), self.failure(500), urllib.error.URLError('synthetic'),
                        io.BytesIO(b'not json')]:
            with self.subTest(failure=type(failure).__name__):
                records, calls, delays, error = self.call('GET', [failure])
                self.assertIsNotNone(error)
                self.assertEqual((calls, delays, records), (1, [], []))

    def test_non_transient_error_after_retry_is_retained_without_another_retry(self):
        records, calls, delays, error = self.call('GET', [self.failure(502), self.failure(401)])
        self.assertIsInstance(error, urllib.error.HTTPError)
        self.assertEqual((calls, delays), (2, [1]))
        self.assertEqual(records[0]['http_statuses'], [502, 401])
        self.assertEqual(records[0]['outcome'], 'failed')

    def test_run_metadata_keeps_success_and_exhaustion_and_charges_existing_timers(self):
        for exhaust in [False, True]:
            with self.subTest(exhaust=exhaust), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / 'work').mkdir()
                config = {'workspace_parent': str(root / 'work'), 'source_commit': 'a' * 40,
                          'source_repo': str(root / 'source'), 'instruction_manifest': 'manifest',
                          'instruction_block': 'block', 'instructions_file': 'instructions',
                          'task_file': 'task', 'allowed_roots': ['src']}
                inputs = {'manifest': json.dumps({'declared_source_revision': config['source_commit']}).encode(),
                          'block': b'block', 'instructions': b'instructions', 'task': b'task'}
                archive = mock.Mock(stdout=io.BytesIO(), wait=mock.Mock(return_value=0))
                process = mock.Mock(stdin=io.BytesIO(), stdout=mock.Mock(), poll=mock.Mock(return_value=0), returncode=0)
                selector = mock.Mock(select=mock.Mock(return_value=[True]))
                events = [{'type': 'system', 'subtype': 'init', 'tools': replay.TOOLS, 'model': replay.MODEL},
                          {'type': 'result', 'subtype': 'success', 'is_error': False, 'total_cost_usd': 0.1}]
                stream = ''.join(json.dumps(e) + '\n' for e in events).encode()
                clock = [100.0]
                def delay(seconds):
                    clock[0] += seconds
                def verify(*args, retry_records):
                    replay.api('GET', 'sandboxes/synthetic/files?path=PRIVATE_PATH', retry_records=retry_records)
                    return {'passed': True, 'checks': []}
                responses = [self.failure(502), self.failure(503), self.failure(504) if exhaust else io.BytesIO(b'{}')]
                with contextlib.ExitStack() as stack:
                    stack.enter_context(mock.patch.dict(replay.os.environ, {'BOAT_API_KEY': 'synthetic-key'}))
                    stack.enter_context(mock.patch.object(replay.subprocess, 'Popen', side_effect=[archive, process]))
                    stack.enter_context(mock.patch.object(replay.subprocess, 'run', return_value=mock.Mock(returncode=0)))
                    stack.enter_context(mock.patch.object(replay, 'baseline_map', return_value={}))
                    stack.enter_context(mock.patch.object(replay.instruction_guard, 'bounded', side_effect=lambda path, limit: inputs[path]))
                    stack.enter_context(mock.patch.object(replay.instruction_guard, 'verify', return_value={}))
                    stack.enter_context(mock.patch.object(replay.instruction_guard, 'verify_pair'))
                    stack.enter_context(mock.patch.object(replay.selectors, 'DefaultSelector', return_value=selector))
                    stack.enter_context(mock.patch.object(replay.os, 'read', return_value=stream))
                    stack.enter_context(mock.patch.object(replay, 'verify', side_effect=verify))
                    stack.enter_context(mock.patch.object(replay.urllib.request, 'urlopen', side_effect=responses))
                    stack.enter_context(mock.patch.object(replay.time, 'monotonic', side_effect=lambda: clock[0]))
                    stack.enter_context(mock.patch.object(replay.time, 'sleep', side_effect=delay))
                    stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
                    result = replay.run(config, 'control', root / 'run')
                saved = json.loads((root / 'run/result.json').read_text())
                self.assertEqual(saved['transport_retries'], result['transport_retries'])
                self.assertEqual(saved['transport_retries'][0]['outcome'], 'exhausted' if exhaust else 'succeeded')
                self.assertEqual(saved['transport_retries'][0]['operation'], 'file_read')
                self.assertEqual(saved['wall_s'], 3.0)
                self.assertEqual(saved['checks_wall_s'], 3.0)
                self.assertEqual(saved['accepted'], not exhaust)
                self.assertNotIn('PRIVATE', json.dumps(saved['transport_retries']))


if __name__ == '__main__':
    unittest.main()
