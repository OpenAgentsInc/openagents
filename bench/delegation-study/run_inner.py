#!/usr/bin/env python3
"""Run one native CLI with a loopback provider bridge inside the namespace."""
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import time


def main():
    request=json.loads(Path('/opt/run/request.json').read_text())
    bridge=subprocess.Popen(['/usr/bin/python3','/opt/study/bridge.py','--socket','/run/provider.sock'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    try:
        for _ in range(100):
            try:
                with socket.create_connection(('127.0.0.1',18080),timeout=.1):
                    break
            except OSError:
                if bridge.poll() is not None:
                    raise RuntimeError('The provider bridge exited')
                time.sleep(.02)
        else:
            raise RuntimeError('The provider bridge did not start')
        env=os.environ.copy()
        env.update(ANTHROPIC_BASE_URL='http://127.0.0.1:18080',CLAUDE_CODE_OAUTH_TOKEN='sk-ant-oat01-benchmark-placeholder',DISABLE_AUTOUPDATER='1',CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC='1',IS_SANDBOX='1')
        with Path('/opt/run/prompt.txt').open('rb') as prompt:
            process=subprocess.Popen(request['argv'],cwd='/workspace',env=env,stdin=prompt)
            return process.wait()
    finally:
        bridge.terminate()
        try:bridge.wait(timeout=3)
        except subprocess.TimeoutExpired:
            bridge.kill();bridge.wait()


if __name__=='__main__':
    sys.exit(main())
