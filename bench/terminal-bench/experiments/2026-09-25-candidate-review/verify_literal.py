#!/usr/bin/env python3
"""Exercise literal CLI controls and compare unchanged public contract plans."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace

from archive_checks import make_plan
from reproduce import sha, write


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'runtime', 'cohort', 'preflight', 'original', 'out'):
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    if a.out.exists():
        raise ValueError('Use a new verification directory')
    a.out.mkdir(parents=True)
    checks = []
    with tempfile.TemporaryDirectory(prefix='literal-controls-') as name:
        root = Path(name)
        work = root / 'work'
        work.mkdir()
        instruction = root / 'instruction.md'
        instruction.write_text('Write result.bin. result.bin must be at most 4 bytes.\n')
        plan = root / 'plan.json'
        command = [str(a.binary), 'checks', 'contract']

        def invoke(args, dest):
            proc = subprocess.run(command + args, capture_output=True, text=True, timeout=10)
            write(a.out / dest, {'args': args, 'exit': proc.returncode,
                                 'stdout': proc.stdout, 'stderr': proc.stderr})
            if proc.returncode:
                raise ValueError('Literal CLI control failed')
            return json.loads(proc.stdout)

        value = invoke(['literal-plan', '--instruction', str(instruction), '--workdir', str(work)],
                       'plan-process.json')
        plan.write_text(json.dumps(value))
        for case, content, expected in [('missing', None, 'fail'), ('exact-boundary', b'1234', None),
                                         ('too-large', b'12345', 'fail'), ('empty', b'', None)]:
            output = work / 'result.bin'
            if content is not None:
                output.write_bytes(content)
            before = sha(output) if output.exists() else None
            result = invoke(['literal-run', '--plan', str(plan)], case + '.json')
            after = sha(output) if output.exists() else None
            if result['call'] != expected or before != after:
                raise ValueError('CLI changed a candidate or contradicted its fixture')
            checks.append({'case': case, 'call': result['call'], 'candidate_unchanged': True})
        instruction.write_text('Write either first.bin or second.bin.\n')
        value = invoke(['literal-plan', '--instruction', str(instruction), '--workdir', str(work)],
                       'alternatives.json')
        if value['obligations']:
            raise ValueError('An alternative output became a mandatory single path')
        checks.append({'case': 'alternative-output', 'obligations': 0})

    plans = []
    for item in json.loads((a.preflight / 'preflight.json').read_text()):
        config = SimpleNamespace(out=a.out / 'compatibility', cohort=a.cohort,
                                 binary=a.binary, runtime=a.runtime)
        make_plan(item['task'], item['image'], config)
        new = config.out / 'plans' / item['task'] / 'original-plan.json'
        old = a.original / 'plans' / item['task'] / 'original-plan.json'
        if new.read_bytes() != old.read_bytes():
            raise ValueError('Ordinary contract extraction changed on ' + item['task'])
        plans.append({'task': item['task'], 'sha256': sha(new), 'byte_identical': True})
    write(a.out / 'result.json', {'checks': checks, 'original_plans': plans,
                                'binary_sha256': sha(a.binary), 'model_calls': 0})
    print(json.dumps({'controls': len(checks), 'unchanged_original_plans': len(plans), 'model_calls': 0}))


if __name__ == '__main__':
    main()
