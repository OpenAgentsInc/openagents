#!/usr/bin/env python3
"""Concurrent Wasm builds must not share generated intermediates."""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / 'coax/web/wasm.sh'

class WasmIsolation(unittest.TestCase):
    def test_concurrent_shared_library_and_output(self):
        with tempfile.TemporaryDirectory(prefix='coax-wasm-isolation-') as d:
            root = Path(d); repo = root / 'repo'
            (repo / 'coax/web').mkdir(parents=True); (repo / 'coax/bin').mkdir()
            shutil.copy2(SCRIPT, repo / 'coax/web/wasm.sh')
            shutil.copy2(SCRIPT.parent / 'toolchain.sh', repo / 'coax/web/toolchain.sh')
            shutil.copy2(SCRIPT.parent / 'defaults.py', repo / 'coax/web/defaults.py')
            compiler = repo / 'coax/bin/coax'
            compiler.write_text('''#!/usr/bin/env python3
import os, pathlib, sys
source=pathlib.Path(sys.argv[2]); header=pathlib.Path(sys.argv[sys.argv.index('--header')+1])
header.write_text('int shared(void);\\n')
if source.stem == 'shared':
 with open(os.environ['HEADER_LOG'],'a') as log: log.write(str(header.parent)+'\\n')
 print('int shared(void) { return 7; }')
else: print('int generated(void) { return 7; }')
''')
            compiler.chmod(0o755)
            for name in ('alpha','beta','shared'): (root / (name+'.coax')).write_text('// fixture\n')
            face = root / 'face.c'; face.write_text('#include "Shared.h"\nint face(void) { return shared(); }\n')
            output = root / 'out'; output.mkdir()
            env = dict(os.environ, HEADER_LOG=str(root/'headers.log'), COAXIAL_MODE='dev')
            procs = []
            for name in ('alpha','beta','alpha'):
                procs.append(subprocess.Popen([str(repo/'coax/web/wasm.sh'),str(root/(name+'.coax')),
                                              str(face),str(output/(name+'.wasm')),str(root/'shared.coax')],
                                             stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env,text=True))
            for proc in procs:
                stdout,stderr=proc.communicate(timeout=120)
                self.assertEqual(proc.returncode,0,stdout+stderr)
            paths=(root/'headers.log').read_text().splitlines()
            self.assertEqual(len(paths),3)
            self.assertEqual(len(set(paths)),3,paths)
            for name in ('alpha','beta'):
                self.assertEqual((output/(name+'.wasm')).read_bytes()[:4],b'\x00asm')

if __name__ == '__main__': unittest.main()
