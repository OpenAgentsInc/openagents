#!/usr/bin/env python3
"""Offline app-server peer; any realtime start is a test failure."""
import json
import pathlib
import sys
import os
import uuid
thread = str(uuid.uuid4())
pathlib.Path("voice-environment.json").write_text(json.dumps({key: os.environ.get(key) for key in ("CODEX_HOME", "HOME", "BROWSER")}))
for line in sys.stdin:
    frame = json.loads(line)
    method = frame.get('method')
    with pathlib.Path('voice-wire.jsonl').open('a') as log:
        log.write(json.dumps(frame) + '\n')
    if 'id' not in frame:
        continue
    result = {}
    if method == 'config/read':
        result = {'config': {}}
    elif method in ('thread/start', 'thread/resume'):
        result = {'thread': {'id': thread, 'turns': []}}
    elif method == 'turn/start':
        result = {'turn': {'id': 'text-turn'}}
    elif method == 'thread/realtime/start':
        sys.exit(98)
    print(json.dumps({'id': frame['id'], 'result': result}), flush=True)
    if method == 'turn/start':
        print(json.dumps({'method': 'turn/completed', 'params': {'threadId': thread, 'turn': {'id':'text-turn', 'status':'completed'}}}), flush=True)
