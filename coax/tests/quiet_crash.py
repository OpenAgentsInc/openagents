#!/usr/bin/env python3
"""Launch an expected-fault subprocess without macOS crash dialogs.

Use run(argv, **subprocess_run_options), or python3 quiet_crash.py -- COMMAND.
The launcher execs the command: real signal termination, stdout/stderr, stdin,
timeouts and ordinary exit codes keep their original meaning. No signal
handler, trap implementation or machine-wide CrashReporter setting changes.
"""
import ctypes
import os
from pathlib import Path
import resource
import subprocess
import sys


def command(argv):
    argv = [os.fspath(arg) for arg in argv]
    if sys.platform == 'darwin':
        return [sys.executable, str(Path(__file__).resolve()), '--', *argv]
    return argv


def run(argv, **kwargs):
    """subprocess.run for an explicitly expected fault; assertions stay with callers."""
    return subprocess.run(command(argv), **kwargs)


def _prepare():
    if sys.platform != 'darwin':
        return
    # Only this child and its descendants lose the inherited crash-report
    # port. The default BSD signal delivery/termination stays intact.
    # Same process-local mechanism used by LLVM's PrintStackTraceOnErrorSignal:
    # https://llvm.org/doxygen/Unix_2Signals_8inc_source.html
    lib = ctypes.CDLL('/usr/lib/libSystem.B.dylib')
    task = ctypes.c_uint.in_dll(lib, 'mach_task_self_').value
    setter = lib.task_set_exception_ports
    setter.argtypes = [ctypes.c_uint, ctypes.c_uint, ctypes.c_uint,
                       ctypes.c_int, ctypes.c_int]
    setter.restype = ctypes.c_int
    # mach/exception_types.h: EXC_MASK_CRASH=1<<10, EXCEPTION_DEFAULT=1.
    # A null port needs no thread-state flavor. EXC_MASK_ALL omits EXC_CRASH.
    status = setter(task, 1 << 10, 0, 1, 0)
    if status != 0:
        raise RuntimeError(f'task_set_exception_ports failed ({status})')
    # Core files and crash dialogs are separate. Keep the existing hard limit.
    _, hard = resource.getrlimit(resource.RLIMIT_CORE)
    resource.setrlimit(resource.RLIMIT_CORE, (0, hard))


def main():
    if len(sys.argv) < 3 or sys.argv[1] != '--':
        print('usage: python3 quiet_crash.py -- COMMAND [ARG ...]', file=sys.stderr)
        return 2
    try:
        _prepare()
    except (OSError, RuntimeError, AttributeError, ValueError) as error:
        print(f'quiet_crash: {error}', file=sys.stderr)
        return 125
    try:
        os.execvp(sys.argv[2], sys.argv[2:])
    except OSError as error:
        print(f'quiet_crash: {error}', file=sys.stderr)
        return 127


if __name__ == '__main__':
    sys.exit(main())
