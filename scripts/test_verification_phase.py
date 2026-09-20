#!/usr/bin/env python3
"""Check progress, exit status, and interrupt forwarding without running the gate."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import unittest

RUNNER = Path(__file__).with_name('run-verification-phase.py')


class PhaseTests(unittest.TestCase):
    def test_live_output_heartbeat_and_failure_status(self):
        process = subprocess.run(
            [sys.executable, str(RUNNER), '--heartbeat-seconds', '0.05', 'fake', '--',
             sys.executable, '-u', '-c',
             'import time,sys; print("child output"); time.sleep(.16); sys.exit(7)'],
            capture_output=True, text=True, timeout=5)
        self.assertEqual(process.returncode, 7)
        self.assertIn('PHASE START: fake', process.stdout)
        self.assertIn('child output', process.stdout)
        self.assertIn('PHASE RUNNING: fake', process.stdout)
        self.assertIn('exit 7', process.stdout)

    def test_termination_reaches_child_group(self):
        process = subprocess.Popen(
            [sys.executable, str(RUNNER), 'interrupt', '--', sys.executable, '-u', '-c',
             'import os,time; print("ready",os.getpid(),flush=True); time.sleep(30)'],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        child_pid = None
        try:
            for line in process.stdout:
                if line.startswith('ready '):
                    child_pid = int(line.split()[1])
                    break
            self.assertIsNotNone(child_pid)
            process.send_signal(signal.SIGTERM)
            output, _ = process.communicate(timeout=5)
            self.assertEqual(process.returncode, 143)
            self.assertIn('forwarding SIGTERM', output)
            self.assertIn('exit 143', output)
            with self.assertRaises(ProcessLookupError):
                os.kill(child_pid, 0)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            process.stdout.close()


if __name__ == '__main__':
    unittest.main()
