#!/usr/bin/env python3
"""Range-contained checked casts on the native target: the large profile v2
admits a secret operand for exactly these, so their emitted code must not
branch on the value. Checked and release C, compiled at -O2 by the host cc:
each widening function has no conditional branch and no trap call, the
narrowing control keeps both, and the widened values are exact."""
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
SOURCE = HERE / 'secret_contained_cast_fixture.coax'
COMPILER = ROOT / 'coax/bin/coax'
WIDE = ['widen_u8_u32', 'widen_u8_u64', 'widen_u32_u64']
# x86-64 and AArch64 conditional branches and conditional selects.
CONDITIONAL = re.compile(r'\s(j(?!mp)[a-z]{1,3}|b\.[a-z]{2}|cbn?z|tbn?z|cmov[a-z]*|csel)\s')

def run(*args):
    r = subprocess.run([str(x) for x in args], capture_output=True, text=True, timeout=120)
    if r.returncode: raise AssertionError((args, r.returncode, r.stdout, r.stderr))
    return r.stdout

def bodies(disassembly):
    out, name = {}, None
    for line in disassembly.splitlines():
        m = re.match(r'^[0-9a-f]+ <(\w+)>:', line)
        if m: name = m.group(1); out[name] = []; continue
        if name and line.strip(): out[name].append(line)
    return out

def main():
    sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
    results = []
    with tempfile.TemporaryDirectory(prefix='coax-contained-casts-') as td:
        d = Path(td)
        for mode, flags in (('checked', []), ('release', ['--release'])):
            c, h = d / 'fixture.c', d / 'fixture.h'
            c.write_text(run(COMPILER, 'c', SOURCE, '--library', '--header', h, *flags))
            obj = d / 'fixture.o'
            run('cc', '-std=c11', '-O2', '-c', '-I' + str(d), c, '-o', obj)
            fns = bodies(run('objdump', '-d', '--no-show-raw-insn', obj))
            for f in WIDE:
                body = ' ' + ' '.join(fns[f]) + ' '
                if CONDITIONAL.search(body) or 'trap' in body or 'call' in body:
                    raise AssertionError((mode, f, fns[f]))
            control = ' ' + ' '.join(fns['narrow_u64_u32']) + ' '
            if not CONDITIONAL.search(control):
                raise AssertionError((mode, 'narrowing control lost its check', fns['narrow_u64_u32']))
            native = d / 'native'
            run('cc', '-std=c11', '-O2', '-I' + str(d), c, HERE / 'secret_contained_cast_fixture.c', '-lm', '-o', native)
            if run(native).strip() != 'ok': raise AssertionError((mode, 'values'))
            results.append({'mode': mode, 'c_sha256': sha(c), 'object_sha256': sha(obj),
                            'branch_free': WIDE, 'control_branches': True, 'values': 'exact'})
    print(json.dumps({'source_sha256': sha(SOURCE), 'compiler_sha256': sha(COMPILER),
                      'host_cc': shutil.which('cc'), 'host_cc_sha256': sha(shutil.which('cc')),
                      'cc_version': run('cc', '--version').splitlines()[0], 'machine': run('uname', '-m').strip(),
                      'results': results}, indent=2))

if __name__ == '__main__':
    main()
