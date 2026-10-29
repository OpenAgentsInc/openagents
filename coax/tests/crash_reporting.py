#!/usr/bin/env python3
"""Expected faults retain their real wait status, output and process isolation."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from quiet_crash import command, run
import quiet_crash


class CrashReporting(unittest.TestCase):
    def test_exit_output_arguments_environment_and_working_directory(self):
        with tempfile.TemporaryDirectory(prefix='coax quiet crash ') as directory:
            result = run([sys.executable, '-c',
                          'import os,sys; print(sys.argv[1]); print(os.getcwd()); '
                          'print(os.environ["COAX_CRASH_TEST"],file=sys.stderr); sys.exit(23)',
                          'literal $HOME ; spaces'], cwd=directory,
                         env={**os.environ, 'COAX_CRASH_TEST': 'diagnostic'},
                         capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 23)
            self.assertEqual(result.stdout.splitlines(),
                             ['literal $HOME ; spaces', str(Path(directory).resolve())])
            self.assertEqual(result.stderr, 'diagnostic\n')

    def test_native_abort_is_a_signal_and_preserves_the_diagnostic(self):
        result = run([sys.executable, '-c',
                      'import ctypes,sys; print("expected guard diagnostic",file=sys.stderr,flush=True); '
                      'ctypes.CDLL(None).abort()'], capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, -signal.SIGABRT)
        self.assertEqual(result.stderr, 'expected guard diagnostic\n')

    def test_normal_exit_cannot_impersonate_a_signal(self):
        result = run([sys.executable, '-c', 'raise SystemExit(134)'], timeout=10)
        self.assertEqual(result.returncode, 134)
        self.assertNotEqual(result.returncode, -signal.SIGABRT)

    def test_a_different_signal_cannot_impersonate_abort(self):
        result = run([sys.executable, '-c',
                      'import os,signal; os.kill(os.getpid(),signal.SIGTERM)'], timeout=10)
        self.assertEqual(result.returncode, -signal.SIGTERM)

    def test_check_and_timeout_are_still_enforced(self):
        with self.assertRaises(subprocess.CalledProcessError) as caught:
            run([sys.executable, '-c', 'raise SystemExit(23)'], check=True, timeout=10)
        self.assertEqual(caught.exception.returncode, 23)
        with self.assertRaises(subprocess.TimeoutExpired):
            run([sys.executable, '-c', 'import time; time.sleep(60)'], timeout=0.2)

    def test_other_platforms_launch_the_original_command(self):
        with patch('quiet_crash.sys.platform', 'linux'):
            self.assertEqual(command(['/some/program', 'argument']), ['/some/program', 'argument'])

    def test_setup_failure_does_not_execute_the_test_or_mimic_abort(self):
        with patch.object(sys, 'argv', ['quiet_crash.py', '--', '/some/program']), \
             patch('quiet_crash._prepare', side_effect=RuntimeError('setup failed')), \
             patch('quiet_crash.os.execvp') as execute, \
             patch('sys.stderr'):
            self.assertEqual(quiet_crash.main(), 125)
            execute.assert_not_called()

    @unittest.skipUnless(sys.platform == 'darwin', 'Mach exception ports are macOS-only')
    def test_native_port_oracle_and_parent_isolation_across_exec(self):
        # Independent SDK constants/ABI. The child must clear its inherited
        # crash port without clearing the parent's or changing signal actions.
        with tempfile.TemporaryDirectory(prefix='coax-crash-ports-') as directory:
            source = Path(directory)/'ports.c'
            binary = Path(directory)/'ports'
            source.write_text('''#include <mach/mach.h>
#include <stdio.h>
int main(void) {
  exception_mask_t masks[EXC_TYPES_COUNT];
  mach_port_t ports[EXC_TYPES_COUNT];
  exception_behavior_t behaviors[EXC_TYPES_COUNT];
  thread_state_flavor_t flavors[EXC_TYPES_COUNT];
  mach_msg_type_number_t count = EXC_TYPES_COUNT;
  kern_return_t kr = task_get_exception_ports(mach_task_self(), EXC_MASK_CRASH,
      masks, &count, ports, behaviors, flavors);
  if (kr != KERN_SUCCESS) return 2;
  unsigned int active = 0;
  for (unsigned int i = 0; i < count; ++i) {
    if (ports[i] != MACH_PORT_NULL) {
      ++active;
      mach_port_deallocate(mach_task_self(), ports[i]);
    }
  }
  printf("%u\\n", active);
  return 0;
}
''')
            subprocess.run([os.environ.get('CC', 'cc'), '-std=c11', '-Wall', '-Wextra',
                            '-Werror', str(source), '-o', str(binary)], check=True, capture_output=True)
            before = subprocess.run([binary], check=True, capture_output=True, text=True).stdout
            result = run([binary], check=True, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.stdout, '0\n')
            # A further exec/spawn (coax run, or a Python ctypes witness) must
            # inherit the same suppression; no injected libraries are needed.
            nested = run([sys.executable, '-c',
                          'import os,sys; os.execv(sys.argv[1],[sys.argv[1]])', str(binary)],
                         check=True, capture_output=True, text=True, timeout=10)
            self.assertEqual(nested.stdout, '0\n')
            spawned = run([sys.executable, '-c',
                           'import subprocess,sys; subprocess.run(sys.argv[1:],check=True)', str(binary)],
                          check=True, capture_output=True, text=True, timeout=10)
            self.assertEqual(spawned.stdout, '0\n')
            after = subprocess.run([binary], check=True, capture_output=True, text=True).stdout
            self.assertEqual(after, before)

    def test_missing_executable_fails_without_becoming_an_expected_fault(self):
        if sys.platform == 'darwin':
            result = run(['/nonexistent/coax-crash-test'], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 127)
            self.assertIn('quiet_crash:', result.stderr)
        else:
            with self.assertRaises(FileNotFoundError):
                run(['/nonexistent/coax-crash-test'])


if __name__ == '__main__':
    unittest.main()
