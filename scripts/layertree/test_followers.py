"""Exercise false-acceptance cases; all week-long inputs here are synthetic."""
import argparse
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('followers', Path(__file__).with_name('followers.py'))
followers = importlib.util.module_from_spec(spec)
spec.loader.exec_module(followers)


class Evidence(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.operator = [dict(seq=0, kind='tree_open', network='signet', tree_id='public', height=100),
                         dict(seq=1, kind='transition', network='signet', height=101, txid='first'),
                         dict(seq=2, kind='transition', network='signet', height=102, txid='last')]
        self.save('operator.jsonl', self.operator)
        self.plan = dict(schema='layertree.followers.evidence.v1', operator='operator.jsonl',
                         notifications='received.jsonl', honest=[], attacks={})
        self.logs = {}
        received = []
        for member in range(3):
            state = f'honest-{member}'
            self.plan['honest'].append(state)
            checked = []
            for elapsed in range(0, followers.WEEK + 1, followers.MAX_GAP):
                status = dict(network='signet', tree_id='public', member=member, pid=member + 10,
                              checked_at=1000 + elapsed, scanned=101 if elapsed == 0 else 102,
                              observed_tip=101 if elapsed == 0 else 102, stopped=None,
                              exited=False, unroll_held=True, outpoint='last:0')
                checked.append(dict(kind='checked', status=status))
            self.logs[state] = checked + [dict(kind='light_client', result='ok', txid=txid)
                                          for txid in ['first', 'last']]
        for scenario, result in [('theft', 'OutputMismatch'), ('withheld', 'RecordMissing'), ('mismatch', 'RecordNotAnchored')]:
            self.plan['attacks'][scenario] = []
            for member in range(3):
                state = f'{scenario}-{member}'
                self.plan['attacks'][scenario].append(state)
                status = dict(network='signet', tree_id='broken', member=member)
                alarm = dict(kind='light_client', result=result, txid=scenario, id=state,
                             network='signet', height=200, observed_tip=200)
                self.logs[state] = [dict(kind='checked', status=status), alarm,
                                    dict(kind='webhook_ack', id=state)]
                received.append(alarm)
        self.save('received.jsonl', received)

    def save(self, name, rows):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(''.join(json.dumps(row) + '\n' for row in rows))

    def check(self):
        for state, rows in self.logs.items():
            self.save(state + '/observations.jsonl', rows)
        (self.root / 'evidence.json').write_text(json.dumps(self.plan))
        return followers.check(self.root / 'evidence.json')

    def test_consistent_synthetic_week_passes(self):
        result = self.check()
        self.assertEqual(result['common_seconds'], followers.WEEK)
        self.assertEqual(len(result['sha256']), 14)

    def test_short_regtest_gap_late_theft_missing_transition_and_undelivered_alarm_fail(self):
        original = copy.deepcopy(self.logs)
        for change in ['short', 'regtest', 'gap', 'late', 'transition', 'notification', 'different_spend']:
            with self.subTest(change=change):
                self.logs = copy.deepcopy(original)
                honest = self.logs['honest-0']
                theft = self.logs['theft-0']
                if change == 'short':
                    del honest[-3]
                elif change == 'regtest':
                    for row in honest:
                        if row['kind'] == 'checked':
                            row['status']['network'] = 'regtest'
                elif change == 'gap':
                    del honest[5]
                elif change == 'late':
                    theft[1]['observed_tip'] = 202
                elif change == 'transition':
                    honest.pop()
                elif change == 'notification':
                    theft.pop()
                else:
                    theft[1]['txid'] = 'unrelated'
                with self.assertRaises(ValueError):
                    self.check()

    def test_merge_preserves_operator_order_and_rejects_another_tree(self):
        for member in range(3):
            self.save(f'merge-{member}/observations.jsonl', [dict(kind='light_client', id=str(member),
                network='signet', tree_id='public', height=101, observed_at=2000,
                member=member, result='ok')])
        args = argparse.Namespace(operator=str(self.root / 'operator.jsonl'),
            states=[str(self.root / f'merge-{m}') for m in range(3)], output=str(self.root / 'merged.jsonl'))
        followers.merge(args)
        merged = followers.rows(args.output)
        self.assertEqual([r['seq'] for r in merged], list(range(6)))
        self.assertEqual([r['txid'] for r in merged if r['kind'] == 'transition'], ['first', 'last'])
        self.assertEqual([r['kind'] for r in merged][2:5], ['light_client'] * 3)
        self.save('merge-0/observations.jsonl', [dict(kind='light_client', id='bad', network='signet', tree_id='other')])
        with self.assertRaises(ValueError):
            followers.merge(args)


if __name__ == '__main__':
    unittest.main()
