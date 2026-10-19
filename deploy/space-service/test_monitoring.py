#!/usr/bin/env python3
"""Check the Computers-layer monitor query and alert wiring (#12147).

The query test starts a disposable PostgreSQL cluster when `initdb` is on PATH
and skips otherwise. It never connects to a deployed database.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import types
import unittest

# monitor.py runs on the control node beside these helpers; stub them here.
for name in ('cluster_secrets', 'database_check', 'storage_inventory'):
    module = types.ModuleType(name)
    module.identity = module.secret = module.credential = lambda *a: None
    module.protocol = module.unfinished_uploads = lambda *a: None
    sys.modules.setdefault(name, module)

import monitor  # noqa: E402
import monitoring  # noqa: E402

SCHEMA = """
CREATE SCHEMA computers;
CREATE TABLE computers.computers (id text PRIMARY KEY, phase text, desired text, record jsonb);
CREATE TABLE computers.turns (id text PRIMARY KEY, computer text, error text, finished_at timestamptz);
CREATE TABLE computers.lifecycle_operations (operation text PRIMARY KEY, resource text, disposition text, created_at timestamptz);
INSERT INTO computers.computers VALUES
 ('stranded','turn','running','{}'), ('busy','turn','running','{}'),
 ('lost','unknown','running','{"boots":[{"resource":"sbx_lost"}]}'),
 ('gone','deleted','deleted','{"boots":[{"resource":"sbx_gone"}]}'), ('fine','stopped','stopped','{}');
INSERT INTO computers.turns VALUES
 ('t1','stranded','We couldn''t confirm whether the command stopped. Delete this computer to end it.', now()-interval '20 minutes'),
 ('t2','busy',NULL,NULL),
 ('t3','fine','We couldn''t confirm whether the command stopped. Delete this computer to end it.', now()-interval '2 hours');
INSERT INTO computers.lifecycle_operations VALUES
 ('o1','sbx_lost','requested',now()-interval '15 minutes'), ('o2','sbx_lost','accepted',now()-interval '3 hours'),
 ('o3','sbx_gone','requested',now()-interval '5 hours');
"""


class AlertWiring(unittest.TestCase):
    def test_every_computers_measurement_has_a_descriptor(self):
        names = {'computers_stranded', 'oldest_stranded_turn_seconds', 'computers_unknown',
                 'oldest_requested_lifecycle_seconds'}
        for name in names:
            self.assertIn(f"'{name}'", monitor.COMPUTERS_SQL)
            self.assertIn(name, monitoring.METRIC_NAMES)
        alerted = {spec[0] for spec in monitoring.ALERT_SPECS}
        self.assertTrue({'computers_stranded', 'computers_unknown', 'oldest_requested_lifecycle_seconds'} <= alerted)
        self.assertTrue(alerted <= set(monitoring.METRIC_NAMES))

    def test_channels_are_preserved_and_the_owner_choice_is_added(self):
        existing = ['projects/openagentsgemini/notificationChannels/1']
        self.assertEqual(monitoring.notification_channels(existing, None), existing)
        self.assertEqual(monitoring.notification_channels(existing, existing[0]), existing)
        self.assertEqual(monitoring.notification_channels([], 'projects/openagentsgemini/notificationChannels/2'),
                         ['projects/openagentsgemini/notificationChannels/2'])


@unittest.skipUnless(shutil.which('initdb') and shutil.which('pg_ctl'), 'PostgreSQL is not installed')
class ComputersQuery(unittest.TestCase):
    def test_stuck_states_are_counted(self):
        # Unix socket paths are limited to about 100 bytes; keep the cluster short-pathed.
        with tempfile.TemporaryDirectory(dir='/tmp') as root:
            data = Path(root) / 'data'
            subprocess.run(['initdb', '-D', str(data), '-A', 'trust', '-U', 'test'], check=True, capture_output=True)
            socket_dir = Path(root)
            subprocess.run(['pg_ctl', '-D', str(data), '-l', str(Path(root) / 'log'), '-o', f'-k {socket_dir} -c listen_addresses=', '-w', '-t', '30', 'start'],
                           check=True, capture_output=True, timeout=60)
            try:
                env = {**os.environ, 'PGHOST': str(socket_dir), 'PGUSER': 'test', 'PGDATABASE': 'postgres'}
                subprocess.run(['psql', '-X', '-q', '--set=ON_ERROR_STOP=1', '-c', SCHEMA], env=env, check=True, capture_output=True)
                out = subprocess.run(['psql', '-X', '-A', '-t', '--set=ON_ERROR_STOP=1', '-c', monitor.COMPUTERS_SQL],
                                     env=env, check=True, capture_output=True).stdout
                value = json.loads(out)
            finally:
                subprocess.run(['pg_ctl', '-D', str(data), '-m', 'immediate', 'stop'], capture_output=True)
        self.assertEqual(value['computers_stranded'], 1)
        self.assertEqual(value['computers_unknown'], 1)
        self.assertGreater(value['oldest_stranded_turn_seconds'], 19 * 60)
        self.assertLess(value['oldest_stranded_turn_seconds'], 21 * 60)
        self.assertGreater(value['oldest_requested_lifecycle_seconds'], 14 * 60)
        self.assertLess(value['oldest_requested_lifecycle_seconds'], 16 * 60)


class AlertMail(unittest.TestCase):
    def test_alert_table_matches_the_cloud_policies(self):
        kinds = {'COMPARISON_LT': 'lt', 'COMPARISON_GT': 'gt'}
        self.assertEqual([(m, kinds[c], t, d) for m, c, t, d in monitoring.ALERT_SPECS], monitor.ALERTS)

    def test_a_condition_opens_after_its_duration_and_clears_once(self):
        state = {}
        self.assertEqual(monitor.transitions(state, {'computers_stranded': 1}, 1000), [])
        self.assertEqual(monitor.transitions(state, {'computers_stranded': 1}, 1200), [])
        self.assertEqual(monitor.transitions(state, {'computers_stranded': 1}, 1300), [('computers_stranded', 'open', 1)])
        self.assertEqual(monitor.transitions(state, {'computers_stranded': 2}, 1360), [], 'an open alert is not resent')
        self.assertEqual(monitor.transitions(state, {'computers_stranded': 0}, 1420), [('computers_stranded', 'clear', 0)])
        self.assertEqual(monitor.transitions(state, {'computers_stranded': 0}, 1480), [])

    def test_a_short_breach_never_opens(self):
        state = {}
        monitor.transitions(state, {'services_ready_ratio': 0.5}, 0)
        self.assertEqual(monitor.transitions(state, {'services_ready_ratio': 1.0}, 60), [])

    def test_mail_is_rate_limited_per_hour(self):
        state = {'_sent': [100.0 * i for i in range(monitor.MAIL_PER_HOUR)]}
        self.assertEqual(monitor.admit_mail(state, 600), 0)
        self.assertEqual(monitor.admit_mail(state, 3700), 1)


if __name__ == '__main__':
    unittest.main()
