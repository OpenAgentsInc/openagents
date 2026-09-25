#!/usr/bin/env python3
"""Retain old and corrected literal calls on synthetic artifact lifecycle cases."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

from reproduce import sha, write


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('old', 'binary', 'out'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        raise ValueError('Use a new control directory')
    args.out.mkdir(parents=True)
    cases = [
        ('intermediate', 'Write scratch.txt as an intermediate file, then delete it before finishing.', []),
        ('later-cleanup', 'Write scratch.txt. Delete scratch.txt when finished.', []),
        ('pronoun', 'Write scratch.txt. Remove it before finishing.', []),
        ('parent', 'Write {work}/nested/scratch.txt. Remove {work}/nested when finished.', []),
        ('rename', 'Write scratch.txt. Rename scratch.txt to result.txt.', []),
        ('fenced', 'Write scratch.txt. Clean up with:\n```sh\nrm scratch.txt\n```', []),
        ('deferred', 'Write a program to cache later.csv.', []),
        ('gerund', 'Write a program generating later.csv.', []),
        ('separate-output', 'Write scratch.txt. Write result.txt. Delete scratch.txt.', ['result.txt']),
        ('outside-cleanup', 'Write result.txt. Delete /tmp/old.txt.', ['result.txt']),
        ('destination', 'Write a script to result.txt.', ['result.txt']),
    ]
    rows = []
    with tempfile.TemporaryDirectory(prefix='literal-lifecycle-') as temp:
        work = Path(temp) / 'work'
        work.mkdir()
        (work / 'result.txt').write_text('Retained final output.\n')
        before = sha(work / 'result.txt')
        for name, instruction, outputs in cases:
            instruction = instruction.replace("{work}", str(work))
            dest = args.out / name
            write(dest / 'expected.json', {'instruction': instruction, 'final_outputs': outputs,
                                         'corrected_call': None})
            source = dest / 'instruction.md'
            source.write_text(instruction + '\n')
            row = {'case': name}
            for version, binary in [('old', args.old), ('corrected', args.binary)]:
                base = [str(binary), 'checks', 'contract']
                command = base + ['literal-plan', '--instruction', str(source), '--workdir', str(work)]
                proc = subprocess.run(command, text=True, capture_output=True, timeout=10)
                write(dest / (version + '-plan-process.json'),
                      {'command': command, 'exit': proc.returncode, 'stdout': proc.stdout, 'stderr': proc.stderr})
                proc.check_returncode()
                plan = json.loads(proc.stdout)
                path = dest / (version + '-plan.json')
                write(path, plan)
                command = base + ['literal-run', '--plan', str(path)]
                proc = subprocess.run(command, text=True, capture_output=True, timeout=10)
                write(dest / (version + '-run-process.json'),
                      {'command': command, 'exit': proc.returncode, 'stdout': proc.stdout, 'stderr': proc.stderr})
                proc.check_returncode()
                result = json.loads(proc.stdout)
                write(dest / (version + '-report.json'), result)
                row[version] = {'call': result['call'], 'obligations': len(plan['obligations'])}
                if version == 'corrected':
                    actual = [str(Path(item['path']).relative_to(work)) for item in plan['obligations']]
                    if actual != outputs or result['call'] is not None:
                        raise ValueError('Corrected checker contradicts control ' + name)
            rows.append(row)
        if sha(work / 'result.txt') != before or sorted(p.name for p in work.iterdir()) != ['result.txt']:
            raise ValueError('A checker changed the candidate')
    if rows[0]['old']['call'] != 'fail':
        raise ValueError('The original temporary-file false alarm was not reproduced')
    result = {'old_sha256': sha(args.old), 'corrected_sha256': sha(args.binary),
              'model_calls': 0, 'candidate_unchanged': True, 'cases': rows}
    write(args.out / 'result.json', result)
    print(json.dumps({'cases': len(rows), 'old_false_alarms': sum(r['old']['call'] == 'fail' for r in rows),
                      'corrected_false_alarms': 0, 'model_calls': 0}))


if __name__ == '__main__':
    main()
