#!/usr/bin/env python3
"""Tests for scripts/check-coder-delegation-run.py: a real-schema pass and
each refusal the checker is built to give."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'check-coder-delegation-run.py'
PROGRAM = 'delegate-fan-out'
EXPECTED = 2


def program_call(choice=PROGRAM, call_id='call-1'):
    return {
        'id': call_id,
        'name': 'program',
        'arguments': {
            'state': {'request': 'delegate the work items'},
            'model': 'jev-latest',
            'questions': {'program': {'type': 'choice'}},
        },
        'output': '{}',
        'outcome': 'Completed',
        'milliseconds': 10,
        'extra': {
            'schema': 'openagents.decision-call.v1',
            'door': 'https://api.typesafe.ai',
            'model': 'jev-1.13.0',
            'answers': {
                'program': {
                    'type': 'choice',
                    'choice': choice,
                    'confidence': 0.9,
                    'probabilities': {choice: 0.9},
                },
            },
        },
    }


def delegate_call(number, output='5', outcome='Completed', status='answered',
                  extra=None, boundary='local'):
    record = {
        'schema': 'openagents.delegate-call.v1',
        'capability': 'devin-local',
        'status': status,
        'executor_path': '/opt/bin/devin',
        'workdir': f'/repo/.coder/worktrees/t{number}',
        'concurrent_max': 6,
        'wrote': None,
        'expected': '5',
        'correct': True,
    }
    if boundary == 'local':
        record['boundary'] = {
            'backend': '/usr/bin/sandbox-exec',
            'checkout': None,
            'writable': ['/tmp/scratch'],
            'protected': ['/repo'],
            'sealed': ['/repo/.git'],
        }
    elif boundary == 'relay':
        record['relayed'] = {
            'relay': 'wss://relay.openagents.com',
            'worker': 'ab' * 32,
            'request': 'cd' * 32,
            'model': None,
            'feedback': 0,
        }
    if extra:
        record.update(extra)
    return {
        'id': f'call-{number + 2}',
        'name': 'delegate',
        'arguments': {
            'agent': 'devin-local',
            'isolation': 'worktree',
            'prompt': f'question {number}?',
            'bounds': {'minutes': 60},
        },
        'output': output,
        'outcome': outcome,
        'milliseconds': 100,
        'purpose': 'Do one item of the work the request lists.',
        'extra': record,
    }


def write_trace(path, calls):
    records = [
        {'record': 'session', 'schema_version': 'ATIF-v1.7', 'at': 1,
         'session': {'id': 's-1', 'model': 'stub', 'door': 'stub',
                     'repository': '/repo', 'version': '0.1.0'}},
        {'record': 'step',
         'step': {'at': 2, 'source': 'User', 'message': 'delegate the items'}},
    ]
    records += [
        {'record': 'step',
         'step': {'at': 3 + i, 'source': 'Agent', 'message': '', 'call': call}}
        for i, call in enumerate(calls)
    ]
    records.append({'record': 'end', 'at': 9, 'state': 'ended'})
    path.write_text(''.join(json.dumps(r) + '\n' for r in records),
                    encoding='utf-8')


def write_result(path, trace_path, **overrides):
    result = {
        'reply': f'{PROGRAM} ran its 5 steps: {EXPECTED} delegations, '
                 f'{EXPECTED} answered, 0 passed, 0 failed, 2 unverifiable.',
        'trace': str(trace_path),
        'outcome': 'answered',
        'route': None,
        'program': PROGRAM,
        'usage': None,
        'error': None,
        'cause': None,
        'refusal': None,
    }
    result.update(overrides)
    path.write_text(json.dumps(result) + '\n', encoding='utf-8')


class CheckerTests(unittest.TestCase):
    def setUp(self):
        self.work = tempfile.TemporaryDirectory()
        self.addCleanup(self.work.cleanup)
        self.dir = Path(self.work.name)
        self.result = self.dir / 'result.json'
        self.trace = self.dir / 'run.atif.jsonl'

    def run_check(self, program=PROGRAM, expected=EXPECTED):
        return subprocess.run(
            [sys.executable, str(SCRIPT),
             '--result', str(self.result), '--trace', str(self.trace),
             '--program', program, '--expected', str(expected)],
            capture_output=True, text=True, timeout=30)

    def good(self, calls=None, **overrides):
        if calls is None:
            calls = [program_call()] + [
                delegate_call(n) for n in range(EXPECTED)]
        write_trace(self.trace, calls)
        write_result(self.result, self.trace, **overrides)

    def test_accepts_a_completed_run(self):
        kept = '/repo/.coder/worktrees/t9'
        calls = [program_call(), delegate_call(0),
                 delegate_call(1, extra={'retained': kept})]
        self.good(calls)
        process = self.run_check()
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertIn('outcome=answered', process.stdout)
        self.assertIn(f'delegates={EXPECTED}', process.stdout)
        self.assertIn(f'retained worktree: {kept}', process.stdout)
        self.assertIn('execution evidence', process.stdout)

    def test_accepts_a_relayed_delegation(self):
        calls = [program_call(), delegate_call(0),
                 delegate_call(1, boundary='relay')]
        self.good(calls)
        self.assertEqual(self.run_check().returncode, 0)

    def test_refuses_malformed_result(self):
        self.good()
        self.result.write_text('not json\n', encoding='utf-8')
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('not one JSON object', process.stderr)

    def test_refuses_result_missing_keys(self):
        self.good()
        self.result.write_text('{"outcome": "answered"}\n', encoding='utf-8')
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('missing keys', process.stderr)

    def test_refuses_malformed_trace(self):
        self.good()
        self.trace.write_text('{"record":"session"}\nnot json\n',
                              encoding='utf-8')
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('not a JSON object', process.stderr)

    def test_refuses_interrupted_trace(self):
        self.good()
        lines = self.trace.read_text(encoding='utf-8').splitlines()
        self.trace.write_text('\n'.join(lines[:-1]) + '\n', encoding='utf-8')
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('interrupted', process.stderr)

    def test_refuses_declined_outcome(self):
        self.good(outcome='declined')
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn("'declined'", process.stderr)

    def test_refuses_failed_outcome(self):
        self.good(outcome='failed', reply=None, error='the door failed',
                  cause='door')
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn("'failed'", process.stderr)

    def test_refuses_when_every_delegate_cancelled(self):
        # Top-level answered, exit 0 behind it — the trace still refuses.
        calls = [program_call()] + [
            delegate_call(n, output='refused: untrusted_workspace',
                          outcome='Cancelled',
                          status='refused: untrusted_workspace',
                          extra={'refusal': 'untrusted_workspace'},
                          boundary=None)
            for n in range(EXPECTED)]
        self.good(calls)
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('Cancelled', process.stderr)

    def test_refuses_partial_fanout(self):
        calls = [program_call(), delegate_call(0)]
        self.good(calls)
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('trace holds 1', process.stderr)

    def test_refuses_wrong_count(self):
        calls = [program_call()] + [
            delegate_call(n) for n in range(EXPECTED + 1)]
        self.good(calls)
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn(f'trace holds {EXPECTED + 1}', process.stderr)

    def test_refuses_wrong_program_in_result(self):
        self.good(program='answer-question')
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn("'answer-question'", process.stderr)

    def test_refuses_wrong_program_in_trace(self):
        calls = [program_call(choice='answer-question')] + [
            delegate_call(n) for n in range(EXPECTED)]
        self.good(calls)
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('selected', process.stderr)

    def test_refuses_failed_verdict(self):
        calls = [program_call(), delegate_call(0),
                 delegate_call(1, extra={'expected': '5', 'correct': False})]
        self.good(calls)
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('verdict failed', process.stderr)

    def test_refuses_unverifiable_delegation(self):
        calls = [program_call(), delegate_call(0),
                 delegate_call(1, boundary=None)]
        self.good(calls)
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('unverifiable', process.stderr)

    def test_refuses_empty_output(self):
        calls = [program_call(), delegate_call(0),
                 delegate_call(1, output='  ')]
        self.good(calls)
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('output is empty', process.stderr)

    def test_refuses_a_delegate_call_without_the_schema(self):
        fake = delegate_call(1)
        del fake['extra']['schema']
        calls = [program_call(), delegate_call(0), fake]
        self.good(calls)
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('openagents.delegate-call.v1', process.stderr)

    def test_refuses_when_result_names_another_trace(self):
        self.good()
        write_result(self.result, '/elsewhere/run.atif.jsonl')
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('not', process.stderr)

    def test_refuses_missing_expectations(self):
        call = delegate_call(1)
        del call['extra']['expected']
        del call['extra']['correct']
        self.good([program_call(), delegate_call(0), call])
        self.assertIn('no nonempty expectation', self.run_check().stderr)
        self.assertEqual(self.run_check().returncode, 1)

    def test_recomputes_text_match_instead_of_trusting_correct(self):
        self.good([program_call(), delegate_call(0), delegate_call(1, output='wrong')])
        process = self.run_check()
        self.assertEqual(process.returncode, 1)
        self.assertIn('does not match', process.stderr)

    def test_refuses_duplicated_delegation(self):
        self.good([program_call(), delegate_call(0), delegate_call(0)])
        self.assertEqual(self.run_check().returncode, 1)

    def test_refuses_cancelled_end_record(self):
        self.good()
        records = [json.loads(line) for line in self.trace.read_text().splitlines()]
        records[-1]['state'] = 'cancelled'
        self.trace.write_text(''.join(json.dumps(r) + '\n' for r in records))
        self.assertEqual(self.run_check().returncode, 1)


if __name__ == '__main__':
    unittest.main()
