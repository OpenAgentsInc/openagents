#!/usr/bin/env python3
"""Audit a public circuit specification after sealing, without changing labels."""
import argparse
import ast
import json
from pathlib import Path
import subprocess
import tempfile

from reproduce import container_command, execution, restore, sha, tree, write

SOURCE_SHA = '700645ac6156cdc88ec1e210f7c7d8d9969bb42aec969c410597e8efca796173'
EXTRA = [0, 65535, 65536, 99999, 100000, 100001, 1000000, 16777216, 4294967295]


def integer(node):
    if isinstance(node, ast.Constant) and type(node.value) is int:
        return node.value
    if isinstance(node, ast.BinOp):
        left, right = integer(node.left), integer(node.right)
        if isinstance(node.op, ast.Add):
            return left + right
        if isinstance(node.op, ast.Sub):
            return left - right
        if isinstance(node.op, ast.Pow) and 0 <= right <= 10:
            return left ** right
    raise ValueError('Unsupported official case expression; never execute the grader to extract it')


def cases(path):
    assignments = [n for n in ast.walk(ast.parse(path.read_text())) if isinstance(n, ast.Assign)
                   and any(isinstance(t, ast.Name) and t.id == 'test_cases' for t in n.targets)]
    if len(assignments) != 1 or not isinstance(assignments[0].value, ast.List):
        raise ValueError('Expected one explicit official case list')
    return [integer(n) for n in assignments[0].value.elts]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('predictions', 'jobs', 'task', 'out'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--image', required=True)
    a = p.parse_args()
    a.out.mkdir(parents=True, exist_ok=False)
    rows = [r for r in json.loads(a.predictions.read_text())['predictions'] if r['task'] == 'circuit-fibsqrt']
    if len(rows) != 6 or sha(a.task / 'tests/sim.c') != SOURCE_SHA:
        raise ValueError('Unexpected cohort or simulator source')
    original = cases(a.task / 'tests/test_outputs.py')
    if len(original) != 28 or max(original) != 48401:
        raise ValueError('The original grader case list changed')
    values = list(dict.fromkeys(original + EXTRA))
    # Independent algorithms: a bounded recurrence and logarithmic fast doubling.
    script = '''import json,math,subprocess
values = VALUES
mask=(1<<32)-1
f=[0,1]
for i in range(2,65536): f.append((f[-1]+f[-2]) & mask)
def doubling(n):
 if n==0:return (0,1)
 a,b=doubling(n//2)
 c=(a*(2*b-a)) & mask; d=(a*a+b*b) & mask
 return (d,(c+d)&mask) if n%2 else (c,d)
rows=[]
for n in values:
 k=math.isqrt(n); expected=f[k]
 assert expected==doubling(k)[0]
 p=subprocess.run(['/tmp/audit-sim',str(n)],capture_output=True,text=True,timeout=10)
 try:actual=int(p.stdout.strip())
 except ValueError:actual=None
 rows.append(dict(input=n,isqrt=k,expected=expected,actual=actual,exit=p.returncode,
                  stdout=p.stdout,stderr=p.stderr,matched=p.returncode==0 and actual==expected))
print(json.dumps(rows,indent=2))
'''.replace('VALUES', json.dumps(values))
    (a.out / 'probe.py').write_text(script)
    write(a.out / 'protocol.json', {
        'post_label_audit': True, 'official_labels_changed': False, 'model_calls': 0,
        'prediction_sha256': sha(a.predictions), 'source_sha256': SOURCE_SHA,
        'official_tests_sha256': sha(a.task / 'tests/test_outputs.py'),
        'image': a.image, 'original_cases': original, 'additional_cases': EXTRA,
        'profile': 'owner-exec', 'scope': 'Reproduce counterexamples, not estimate generalization',
    })
    results = []
    for row in rows:
        archive = a.jobs / row['job'] / row['trial'] / 'agent/episode/snapshot/workspace.tar.gz'
        if sha(archive) != row['candidate_identity']:
            raise ValueError('Candidate changed after prediction sealing')
        record = {k: row[k] for k in ('job', 'trial', 'executor', 'candidate_identity')}
        container = None
        with tempfile.TemporaryDirectory(prefix='truth9645-') as temp:
            scratch = Path(temp)
            restore(archive, scratch)
            app = scratch / 'app'
            if sha(app / 'sim.c') != SOURCE_SHA:
                raise ValueError('Candidate simulator differs from the public original')
            before = tree(app)
            settings = execution('owner-exec', app)
            command = container_command(a.image, app, row['candidate_identity'], settings)
            record.update(settings=settings, container_command=command, gates_sha256=sha(app / 'gates.txt'))
            try:
                container = subprocess.check_output(command, text=True, timeout=30).strip()
                compile_command = ['cc', '-O3', '/app/sim.c', '-o', '/tmp/audit-sim']
                compiled = subprocess.run(['docker', 'exec', container, *compile_command],
                                          capture_output=True, text=True, timeout=30)
                record['compile'] = dict(command=compile_command, exit=compiled.returncode,
                                         stdout=compiled.stdout, stderr=compiled.stderr)
                if compiled.returncode:
                    raise ValueError('Original simulator compilation failed')
                run = subprocess.run(['docker', 'exec', '-i', container, 'python3', '-'], input=script,
                                     capture_output=True, text=True, timeout=180)
                record['run'] = dict(command=['python3', '-'], script_sha256=sha(a.out / 'probe.py'),
                                     exit=run.returncode, stdout=run.stdout, stderr=run.stderr)
                if run.returncode:
                    raise ValueError('Independent audit execution failed')
                record['cases'] = json.loads(run.stdout)
                record['original_mismatches'] = [c for c in record['cases'] if c['input'] in original and not c['matched']]
                record['additional_mismatches'] = [c for c in record['cases'] if c['input'] in EXTRA and not c['matched']]
                if tree(app) != before:
                    raise ValueError('Candidate bytes changed during read-only audit')
                record['candidate_unchanged'] = True
            finally:
                if container:
                    cleanup = subprocess.run(['docker', 'rm', '-f', container], capture_output=True, text=True, timeout=30)
                    record['cleanup_exit'] = cleanup.returncode
                write(a.out / (row['trial'] + '.json'), record)
        results.append({k: record[k] for k in ('trial', 'executor', 'candidate_identity',
                                              'original_mismatches', 'additional_mismatches', 'candidate_unchanged')})
    write(a.out / 'result.json', results)
    print(json.dumps([{k: (len(v) if isinstance(v, list) else v) for k, v in r.items()} for r in results], indent=2))


if __name__ == '__main__':
    main()
