#!/usr/bin/env python3
"""Runs a command in a real pseudo-terminal of COLS x ROWS and relays raw
bytes: this process's stdin goes to the terminal, the terminal's output
comes out on stdout. The launch film's terminal recorder (record-terminal.mjs)
drives it.

Usage: ptyrun.py COLS ROWS -- COMMAND [ARGS...]
"""
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios


def main() -> int:
    cols, rows = int(sys.argv[1]), int(sys.argv[2])
    command = sys.argv[sys.argv.index("--") + 1 :]
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.environ["COLORTERM"] = "truecolor"
        os.execvp(command[0], command)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    os.kill(pid, signal.SIGWINCH)
    stdin = sys.stdin.fileno()
    stdout = sys.stdout.fileno()
    open_in = True
    while True:
        watch = [fd] + ([stdin] if open_in else [])
        ready, _, _ = select.select(watch, [], [], 0.5)
        if fd in ready:
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if not data:
                break
            os.write(stdout, data)
        if open_in and stdin in ready:
            data = os.read(stdin, 65536)
            if not data:
                open_in = False
            else:
                os.write(fd, data)
        done, status = os.waitpid(pid, os.WNOHANG)
        if done:
            # Drain what is left.
            while True:
                r, _, _ = select.select([fd], [], [], 0.2)
                if not r:
                    break
                try:
                    data = os.read(fd, 65536)
                except OSError:
                    break
                if not data:
                    break
                os.write(stdout, data)
            return os.waitstatus_to_exitcode(status)
    _, status = os.waitpid(pid, 0)
    return os.waitstatus_to_exitcode(status)


if __name__ == "__main__":
    sys.exit(main())
