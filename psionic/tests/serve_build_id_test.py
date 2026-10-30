#!/usr/bin/env python3
"""Build tooling check: relocation is stable; changing inventory bytes is not."""
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

source, coax, cc = sys.argv[1:]
with tempfile.TemporaryDirectory(prefix='psionic-build-id-') as temporary:
    base = Path(temporary)
    ids = []
    for name in ('first', 'relocated'):
        out = base / name
        out.mkdir()
        for file in ('serve.c', 'serve.h', 'elephant-inventory.h'):
            shutil.copyfile(Path(source) / file, out / file)
        subprocess.run([sys.executable, '-I', 'psionic/tests/serve_build_id.py', str(out), coax, cc,
                        'release', 'plain', 'cpu'], check=True)
        ids.append(json.loads((out / 'serve-build.json').read_text())['build'])
    assert ids[0] == ids[1]
    with (out / 'elephant-inventory.h').open('a') as stream:
        stream.write('\n/* changed inventory input */\n')
    subprocess.run([sys.executable, '-I', 'psionic/tests/serve_build_id.py', str(out), coax, cc,
                    'release', 'plain', 'cpu'], check=True)
    assert json.loads((out / 'serve-build.json').read_text())['build'] != ids[0]
print('build identity: path relocation stable; generated inventory changes identity')
