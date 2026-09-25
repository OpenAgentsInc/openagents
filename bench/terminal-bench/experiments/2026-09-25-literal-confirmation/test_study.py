"""The reserved study must not drop candidates or read outcomes while sealing."""
import json
from pathlib import Path
import tempfile
import unittest

from study import population, prediction, sha, write


class StudyTests(unittest.TestCase):
    def test_reserved_population_rejects_missing_and_duplicate_attempts(self):
        tasks = ['new-task', 'another-task']
        rows = [{'job': arm, 'trial': f'{task}-{arm}-{i}', 'task': task, 'executor': arm}
                for task in tasks for arm in ('luna', 'astra') for i in range(3)]
        population(rows, tasks)
        for invalid in (rows[:-1], rows + [rows[0]], rows[:-1] + [rows[0]]):
            with self.assertRaises(ValueError):
                population(invalid, tasks)

    def fixture(self, root):
        row = {'job': 'job', 'trial': 'trial', 'task': 'new-task', 'executor': 'luna'}
        jobs, checks = root / 'jobs', root / 'checks'
        trial = jobs / 'job/trial'
        write(trial / 'config.json', {'task': {'path': '/tasks/new-task'}})
        (trial / 'result.json').write_text('NOT JSON: a seal must not parse this')
        combined = {k: row[k] for k in ('job', 'trial', 'task')}
        combined.update(call='unknown', contract_call='unknown', literal_call='unknown', reproduced_call='unknown')
        write(checks / 'trial/combined.json', combined)
        return row, jobs, checks, combined

    def test_unknown_candidate_stays_in_seal_without_grade_read(self):
        with tempfile.TemporaryDirectory() as name:
            row, jobs, checks, _ = self.fixture(Path(name))
            result = prediction(row, checks, jobs)
            self.assertTrue(all(v is None for v in result['calls'].values()))

    def test_literal_failure_requires_difference_identity_and_skipped_review(self):
        with tempfile.TemporaryDirectory() as name:
            row, jobs, checks, combined = self.fixture(Path(name))
            archive = jobs / 'job/trial/agent/episode/snapshot/workspace.tar.gz'
            archive.parent.mkdir(parents=True)
            archive.write_bytes(b'sealed synthetic candidate')
            combined.update(call='fail', literal_call='fail', reproduced_call='not_requested',
                            candidate_unchanged=True, candidate_identity=sha(archive))
            write(checks / 'trial/combined.json', combined)
            report = {'call': 'fail', 'items': [{'outcome': {'outcome': 'differed'}}]}
            write(checks / 'trial/literal.json', report)
            result = prediction(row, checks, jobs)
            self.assertEqual(result['calls']['checks.literal-artifacts'], 'fail')
            self.assertEqual(result['calls']['verdict.literal-executed'], 'fail')
            for key, value in [('candidate_unchanged', False), ('candidate_identity', None),
                               ('contract_error', 'unavailable'), ('reproduced_call', 'unknown'),
                               ('call', 'unknown'), ('task', 'wrong-task')]:
                write(checks / 'trial/combined.json', combined | {key: value})
                with self.assertRaises(ValueError):
                    prediction(row, checks, jobs)
            write(checks / 'trial/combined.json', combined)
            write(checks / 'trial/literal.json', {'call': 'fail', 'items': []})
            with self.assertRaises(ValueError):
                prediction(row, checks, jobs)
            write(checks / 'trial/literal.json', report)
            archive.write_bytes(b'changed candidate')
            with self.assertRaises(ValueError):
                prediction(row, checks, jobs)


if __name__ == '__main__':
    unittest.main()
