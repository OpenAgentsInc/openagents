import os
import pathlib
import subprocess
import tempfile
import unittest


class AgentHostIdle(unittest.TestCase):
    def test_idle_clock_resets_after_boot(self):
        source = pathlib.Path(__file__).with_name('agent-host-idle.sh').read_text()
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            state = root / 'run'
            old = root / 'persistent'
            old.mkdir()
            (old / 'busy').touch()
            os.utime(old / 'busy', (1, 1))
            script = root / 'idle'
            script.write_text(source.replace('/run/oa-agent-host', str(state))
                              .replace('/home/coder/.openagents/keep-awake', str(root / 'keep-awake')))
            commands = {
                'pgrep': 'exit 1',
                'curl': 'echo 60',
                'date': 'echo 10000',
                'stat': 'echo 6400',
                'logger': 'exit 0',
                'systemctl': 'echo "$@" >> "$POWER_LOG"',
            }
            for name, body in commands.items():
                command = root / name
                command.write_text('#!/bin/sh\n' + body + '\n')
                command.chmod(0o755)
            log = root / 'power'
            env = dict(os.environ, PATH=str(root) + ':' + os.environ['PATH'],
                       POWER_LOG=str(log))
            def tick():
                subprocess.run(['sh', str(script)], env=env, check=True)
            # A stale marker on persistent disk cannot trigger a stop after boot.
            tick()
            self.assertTrue((state / 'busy').exists())
            self.assertFalse(log.exists())
            # A full idle interval within this boot still powers off.
            tick()
            self.assertEqual(log.read_text(), 'poweroff\n')
            log.unlink()
            # tmpfs disappears across boots.
            (state / 'busy').unlink()
            tick()
            self.assertFalse(log.exists())
            (root / 'keep-awake').touch()
            tick()
            self.assertFalse(log.exists())


if __name__ == '__main__':
    unittest.main()
