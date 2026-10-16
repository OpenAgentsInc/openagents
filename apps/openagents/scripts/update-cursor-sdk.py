#!/usr/bin/env python3
"""Update only the SDK pin, accepting a strictly newer stable npm version."""
import json
import pathlib
import re
import subprocess

source = pathlib.Path('crates/harness/src/cursor/mod.rs')
text = source.read_text()
pattern = r'(const CURSOR_SDK_PIN: &str = "@cursor/sdk@)([0-9]+\.[0-9]+\.[0-9]+)(";)'
match = re.search(pattern, text)
if not match:
    raise SystemExit('Cursor SDK pin format changed; manual review required')
latest = json.loads(subprocess.check_output(['npm', 'view', '@cursor/sdk', 'version', '--json'], text=True))
if not isinstance(latest, str) or not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+', latest):
    raise SystemExit('Expected a stable numeric npm version')
if tuple(map(int, latest.split('.'))) > tuple(map(int, match[2].split('.'))):
    source.write_text(text[:match.start(2)] + latest + text[match.end(2):])
    print(f'Updated Cursor SDK {match[2]} -> {latest}')
else:
    print(f'Cursor SDK {match[2]} is current; no downgrade')
