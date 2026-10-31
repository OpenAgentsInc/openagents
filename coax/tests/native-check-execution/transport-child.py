#!/usr/bin/env python3
"""Deliberately broken EP1 transport fixture; never a Coax qualification."""
import json
import os
from pathlib import Path
import sys
import time


vectors = json.loads(Path(__file__).with_name('ep1-vectors.json').read_text())
mode = os.environ['COAX_EXECUTION_TEST_TRANSPORT_MODE']
if sys.argv[1:] != ['one', '0']:
    raise SystemExit(64)
if mode == 'sleep':
    os.write(1, bytes.fromhex(vectors['execution']['begun_only']))
    time.sleep(10)
elif mode == 'output':
    os.write(1, bytes.fromhex(vectors['execution']['begun_only']) + b'x' * 65536)
elif mode == 'truncated-success':
    os.write(1, bytes.fromhex(vectors['execution']['truncated_end']))
elif mode == 'malformed-success':
    os.write(1, b'not an EP1 frame')
elif mode == 'complete-nonzero':
    os.write(1, bytes.fromhex(vectors['execution']['valid']))
    raise SystemExit(7)
else:
    raise SystemExit(65)
