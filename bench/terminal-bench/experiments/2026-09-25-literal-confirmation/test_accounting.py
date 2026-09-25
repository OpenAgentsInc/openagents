"""Keep missing grades, unknown calls, and unpriced attempts in the accounting."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from breakdown import describe

PREVIOUS = Path(__file__).resolve().parent.parent / '2026-09-25-candidate-review'


class AccountingTests(unittest.TestCase):
    def test_unknown_predictions_count_as_misses_but_unknown_grades_stay_separate(self):
        rows = [{'trial': str(i), 'task': 'synthetic', 'reward': reward,
                 'calls': {'signal': call}, 'reproduced_call': 'unknown'}
                for i, (reward, call) in enumerate([(0, 'fail'), (0, None), (1, 'fail'),
                                                   (1, None), (None, None), (0, 'fail')])]
        result = describe(rows)
        self.assertEqual((result['attempts'], result['graded']), (6, 5))
        self.assertEqual(result['unknown_outcomes'], ['4'])
        self.assertEqual(result['false_alarms']['signal'], ['2'])
        self.assertEqual(result['missed_failures']['signal'], ['1'])
        for key in ('fail_precision', 'failure_recall'):
            self.assertEqual(result['signals']['signal'][key]['correct'], 2)
            self.assertEqual(result['signals']['signal'][key]['total'], 3)

    def test_custom_population_keeps_failed_costs_missing_usage_and_undefined_cost_per_pass(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            tasks = root / 'tasks.txt'
            tasks.write_text('synthetic\n')
            rows = [{'job': arm, 'trial': f'{arm}-{i}', 'task': 'synthetic', 'executor': arm}
                    for arm in ('luna', 'astra') for i in range(3)]
            manifest = root / 'manifest.json'
            manifest.write_text(json.dumps(rows))
            for row in rows:
                if row['trial'] == 'luna-2':
                    continue
                usage = root / 'jobs' / row['job'] / row['trial'] / 'agent/episode/evaluation/usage.json'
                usage.parent.mkdir(parents=True)
                usage.write_text(json.dumps({'cost': {'lower_bound_usd': 2, 'unknown_calls': 1},
                                             'components': {'native': {'cost_lower_bound_usd': 2}}}))
            costs = root / 'costs.json'
            base = [sys.executable, str(PREVIOUS / 'archive_executor_costs.py'), '--manifest', str(manifest),
                    '--jobs', str(root / 'jobs'), '--out', str(costs), '--tasks-file', str(tasks)]
            subprocess.run(base, capture_output=True, text=True, check=True)
            value = json.loads(costs.read_text())['by_executor']['luna']
            self.assertEqual(value['known_list_price_lower_bound_usd'], 4)
            self.assertEqual(value['unknown_calls'], 2)
            self.assertEqual(value['missing_usage_trials'], ['luna-2'])
            labels = root / 'labels.json'
            labels.write_text(json.dumps({'labels': [r | {'reward': 0, 'seconds': {}} for r in rows]}))
            result = root / 'resources.json'
            command = [sys.executable, str(PREVIOUS / 'archive_resources.py'), '--labels', str(labels),
                       '--executor-costs', str(costs), '--out', str(result), '--tasks-file', str(tasks)]
            subprocess.run(command, capture_output=True, text=True, check=True)
            self.assertIsNone(json.loads(result.read_text())['by_executor']['luna']
                              ['known_list_price_usd_per_official_pass_lower_bound'])
            tasks.write_text('synthetic\nsynthetic\n')
            proc = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(proc.returncode, 0)
            self.assertIn('unique and nonempty', proc.stderr)


if __name__ == '__main__':
    unittest.main()
