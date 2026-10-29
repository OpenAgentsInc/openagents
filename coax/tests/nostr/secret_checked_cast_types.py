#!/usr/bin/env python3
"""Float and enum casts are outside the aggregate secret-audit domain."""
import json
import hashlib
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SOURCE = Path(__file__).with_suffix('.coax')

with tempfile.TemporaryDirectory(prefix='coax-cast-types-') as td:
    report = Path(td)/'report.json'
    compiler = ROOT/'coax/bin/coax'
    digest = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
    compilation = subprocess.run([str(compiler),'c',str(SOURCE),'--library'],capture_output=True,text=True,timeout=120)
    if compilation.returncode: raise AssertionError(('source did not compile', compilation.stderr))
    expected = {'float_source':'type','float_target':'unsupported',
                'enum_source':'type','enum_target':'unsupported'}
    for profile in ('secret-check','public-view-check','large-source-audit'):
        for entry, error in expected.items():
            cmd = [sys.executable, str(ROOT/'tools/coax-nostr'/profile/'aggregate.py'), str(SOURCE),
                   '--entry', entry, '--report', str(report)]
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
            data = json.loads(report.read_text())
            if (result.returncode != 1 or data['accepted'] or data['error_name'] != error or
                data['compiler_sha256'] != digest(compiler) or
                data['source_sha256'] != {str(SOURCE.resolve()): digest(SOURCE)}):
                raise AssertionError((profile, entry, data))
            print(profile, entry, data['error_name'], flush=True)
