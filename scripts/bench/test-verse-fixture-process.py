#!/usr/bin/env python3
"""Check natural completion and controller-disconnect cleanup for scratch processes."""
import pathlib
import subprocess
import sys
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).with_name('verse-fixture-process.py')


class SupervisionTests(unittest.TestCase):
    def launch(self, home, source):
        return subprocess.Popen([sys.executable, str(SCRIPT), '--home', home, '--',
                                 sys.executable, '-u', '-c', source],
                                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE)

    def test_remote_load_requires_absolute_path_and_ssh_fixture(self):
        script=SCRIPT.with_name('verse-battle-capture.py')
        with tempfile.TemporaryDirectory() as home:
            base=[sys.executable,str(script),'--asset-dir',home,'--binaries',home,'--compiled-revision','test']
            missing_ssh=subprocess.run([*base,'--remote-load-binary','/tmp/load'],capture_output=True,text=True,timeout=5)
            self.assertEqual(missing_ssh.returncode,2)
            relative=subprocess.run([*base,'--ssh-host','unused.invalid','--remote-binary','/tmp/host','--remote-load-binary','relative'],capture_output=True,text=True,timeout=5)
            self.assertEqual(relative.returncode,2)
            self.assertNotIn('Scratch artifacts',missing_ssh.stdout+relative.stdout)

    def test_natural_exit_does_not_wait_for_controller_eof(self):
        with tempfile.TemporaryDirectory() as home:
            process = self.launch(home, 'import os,sys; print(os.environ["HOME"]); sys.exit(17)')
            try:
                self.assertEqual(process.wait(timeout=5), 17)
                self.assertEqual(process.stdout.read().decode().strip(), home)
            finally:
                process.stdin.close()
                process.stdout.close()
                process.stderr.close()
                if process.poll() is None:
                    process.kill(); process.wait()

    def test_controller_eof_terminates_running_child(self):
        with tempfile.TemporaryDirectory() as home:
            process = self.launch(home, 'import time; print("ready"); time.sleep(120)')
            try:
                self.assertEqual(process.stdout.readline().decode().strip(), 'ready')
                process.stdin.close()
                self.assertEqual(process.wait(timeout=5), 143)
            finally:
                if not process.stdin.closed:
                    process.stdin.close()
                process.stdout.close()
                process.stderr.close()
                if process.poll() is None:
                    process.kill(); process.wait()


    def test_disconnect_cleans_descendant_that_ignores_term(self):
        descendant = 'import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); print("descendant",flush=True); time.sleep(120)'
        source = 'import subprocess,sys,time; subprocess.Popen([sys.executable,"-u","-c",'+repr(descendant)+']); time.sleep(120)'
        with tempfile.TemporaryDirectory() as home:
            process = self.launch(home, source)
            try:
                self.assertEqual(process.stdout.readline().decode().strip(), 'descendant')
                process.stdin.close()
                process.stdin = None
                # communicate also waits for inherited output pipes to close.
                process.communicate(timeout=5)
                self.assertEqual(process.returncode, 143)
            finally:
                if process.stdin is not None and not process.stdin.closed:
                    process.stdin.close()
                if process.poll() is None:
                    process.kill(); process.wait()
                process.stdout.close()
                process.stderr.close()


if __name__ == '__main__':
    unittest.main()
