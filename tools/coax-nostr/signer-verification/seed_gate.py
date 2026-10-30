#!/usr/bin/env python3
"""Negative-test a signer shell gate with one extra branch on the raw key.

Runs copies in a new temporary repository layout. A compiler-output shim supplies
the given generated C/header; this tests the shell gate, not Coax compilation.
The default nonzero exit means the gate failed to reject the seeded branch.
"""
import argparse
import json
import os
import re
import subprocess
from pathlib import Path

from compare import executable, sha

INJECTION = '\n  static volatile uint32_t review_leak_sink;\n  if (l_key.data[0] & 1) review_leak_sink++;\n'


def seed(source):
    # The existing shell parser expects release.c locations. Remove only debug
    # line directives; production arithmetic and native build flags are intact.
    base = re.sub(r'^#line[^\n]*\n', '', source, flags=re.MULTILINE)
    pattern = r'(_Bool sm_equation\(coax_v_u8 l_key,[^;\n]*\) \{)'
    if len(re.findall(pattern, base)) != 1:
        raise ValueError('expected exactly one sm_equation definition')
    return base, re.sub(pattern, lambda m: m[0]+INJECTION, base)


def rejects_seed(runs):
    # A build failure or crashed probe is not detection of the planted leak.
    if len(runs) != 2 or any('calls 6 status 2 sig e907..' not in r['stdout'] for r in runs):
        raise ValueError('the baseline and seeded signer must both complete')
    return runs[0]['exit_code']==0 and runs[1]['exit_code']!=0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--generated-c', type=Path, required=True)
    parser.add_argument('--header', type=Path, required=True)
    parser.add_argument('--gate-script', type=Path, required=True)
    parser.add_argument('--gate-probe', type=Path, required=True)
    parser.add_argument('--cc', default='gcc')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    out = args.out.resolve(); out.mkdir(parents=True, exist_ok=False)
    original = args.generated_c.read_text(); baseline, mutant = seed(original)
    identities = {str(p.resolve()): sha(p) for p in [args.generated_c, args.header, args.gate_script, args.gate_probe, Path(__file__)]}
    record = dict(schema='coax.signer-gate-negative.v1', input_sha256=identities,
                  scope='unchanged shell gate with substituted generated C; no product source changed',
                  injection=INJECTION, gate_rejects_seed=False, runs=[])
    for case, source in [('baseline', baseline), ('seeded-key-branch', mutant)]:
        directory = out/case
        (directory/'coax/bin').mkdir(parents=True)
        test = directory/'coax/tests/nostr'; test.mkdir(parents=True)
        script = test/'schnorr_sign_secret_probe.sh'; script.write_bytes(args.gate_script.read_bytes())
        (test/'schnorr_sign_secret_probe.c').write_bytes(args.gate_probe.read_bytes())
        c = directory/'injected.c'; c.write_text(source)
        header = directory/'injected.h'; header.write_bytes(args.header.read_bytes())
        wrapper = directory/'coax/bin/coax'
        wrapper.write_text('#!/usr/bin/env python3\nimport sys,shutil\na=sys.argv[1:]\n'
                           'assert a[0]=="c"\nshutil.copyfile('+repr(str(header))+',a[a.index("--header")+1])\n'
                           'sys.stdout.buffer.write(open('+repr(str(c))+',"rb").read())\n')
        wrapper.chmod(0o755)
        result = subprocess.run(['bash', str(script)], capture_output=True, text=True,
                                env={**os.environ, 'CC':str(executable(args.cc))}, timeout=120)
        (directory/'stdout.log').write_text(result.stdout); (directory/'stderr.log').write_text(result.stderr)
        record['runs'].append(dict(case=case, exit_code=result.returncode, stdout=result.stdout,
                                   stderr=result.stderr, generated_c_sha256=sha(c)))
        print(case, 'exit', result.returncode, flush=True)
    if any(sha(path) != expected for path, expected in identities.items()):
        raise ValueError('gate-test input changed during measurement')
    record['gate_rejects_seed'] = rejects_seed(record['runs'])
    (out/'gate-negative.json').write_text(json.dumps(record, indent=2)+'\n')
    return 0 if record['gate_rejects_seed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
