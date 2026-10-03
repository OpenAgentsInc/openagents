"""Check the reconciliation timer's exit-status policy without deploying units."""

import configparser
from pathlib import Path
import shlex
import unittest


ROOT = Path(__file__).resolve().parents[2]


def unit(name):
    config = configparser.ConfigParser(interpolation=None)
    config.read(ROOT / 'deploy' / 'pay' / name)
    return config


class ReconcileUnitTests(unittest.TestCase):
    def test_timer_accepts_unreadable_wallet_exit(self):
        service = unit('openagents-pay-reconcile.service')['Service']
        self.assertEqual(service['Type'], 'oneshot')
        # Accept exactly exit 1 in addition to systemd's default success code.
        # Do not accept other exit codes or signal failures.
        self.assertEqual(service['SuccessExitStatus'].split(), ['1'])

    def test_timer_still_records_and_resolves_reports_every_ten_minutes(self):
        timer = unit('openagents-pay-reconcile.timer')['Timer']
        self.assertEqual(timer['Unit'], 'openagents-pay-reconcile.service')
        self.assertEqual(timer['OnUnitActiveSec'], '10m')
        command = shlex.split(unit(timer['Unit'])['Service']['ExecStart'])
        self.assertEqual(command[:4], [
            '/opt/openagents-pay/current/openagents', '--json', 'pay', 'reconcile',
        ])
        self.assertIn('--resolve', command)
        self.assertEqual(command[command.index('--report-dir') + 1],
                         '/var/lib/openagents-pay/reconcile')
        self.assertEqual(command[command.index('--spark-home') + 1],
                         '/var/lib/openagents-pay/spark')
        self.assertEqual(command[command.index('--ledger') + 1],
                         '/var/lib/openagents-pay/ledger/ledger.sqlite')


if __name__ == '__main__':
    unittest.main()
