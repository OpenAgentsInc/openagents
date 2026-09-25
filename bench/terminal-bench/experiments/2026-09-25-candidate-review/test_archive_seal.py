"""Regression checks for cohort attribution and failure-rate denominators."""
from copy import deepcopy
from pathlib import Path
import tempfile
import unittest

from archive_preflight import TASKS
from measure_archive import paired_rows, separation
from measure_prospective import intervals
from reproduce import write
from seal_archive import baseline, population, prediction


def rows():
    return [{'job': arm, 'trial': task + str(i) + arm, 'task': task, 'executor': arm}
            for task in TASKS for arm in ('luna', 'astra') for i in range(3)]


class ArchiveSealTests(unittest.TestCase):
    def test_population_rejects_missing_extra_and_duplicate_candidates(self):
        original = rows()
        population(original)
        for bad in (original[:-1], original + [original[0]], original[:-1] + [original[0]]):
            with self.assertRaises(ValueError):
                population(bad)

    def test_join_rejects_duplicate_and_wrong_attribution(self):
        original = rows()
        labels = [r | {'reward': 0} for r in original]
        self.assertEqual(len(paired_rows(original, labels)), 72)
        for bad in (labels + [labels[0]], [labels[0] | {'task': 'wrong'}] + labels[1:]):
            with self.assertRaises(ValueError):
                paired_rows(original, bad)

    def test_missing_signal_is_unknown_and_failed_checks_dominate(self):
        self.assertEqual(baseline({}), {'checks.final': None, 'verdict.combined': None})
        value = {'final_checks': {'verdicts': {'failed': 1, 'passed': 9, 'inconclusive': 1}}}
        self.assertEqual(baseline(value)['checks.final'], 'fail')

    def test_unknown_review_does_not_remove_real_failure(self):
        sample = [{'task': 'one', 'reward': 0, 'calls': {'s': 'fail'}},
                  {'task': 'one', 'reward': 0, 'calls': {'s': None}},
                  {'task': 'one', 'reward': 1, 'calls': {'s': None}}]
        result = intervals.stats(sample, 's')
        self.assertEqual(result['fail_precision']['value'], 1)
        self.assertEqual(result['failure_recall']['value'], .5)
        self.assertEqual(separation(sample, 's')['concordance'], .75)
        sample[0]['calls']['s'] = None
        self.assertIsNone(intervals.stats(sample, 's')['fail_precision']['value'])
        self.assertEqual(separation(sample, 's')['concordance'], .5)

    def test_seal_does_not_parse_official_result(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            row = rows()[0]
            trial = root / 'jobs' / row['job'] / row['trial']
            checks = root / 'checks'
            write(trial / 'config.json', {'task': {'path': '/tasks/' + row['task']}})
            (trial / 'result.json').write_text('deliberately not JSON; never open before seal')
            write(checks / row['trial'] / 'combined.json', {
                **{k: row[k] for k in ('job', 'trial', 'task')},
                'call': 'unknown', 'contract_call': 'unknown', 'reproduced_call': 'unknown'})
            sealed = prediction(row, checks, root / 'jobs')
            self.assertTrue(all(v is None for v in sealed['calls'].values()))
            bad = deepcopy(row)
            bad['task'] = 'different'
            with self.assertRaises(ValueError):
                prediction(bad, checks, root / 'jobs')


if __name__ == '__main__':
    unittest.main()
