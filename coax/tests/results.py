#!/usr/bin/env python3
# Firm reason: 2 (independent compiler, ABI, diagnostic and native-effect oracles; no product implementation).
"""R2 original-source diagnostics and evaluator/native regression oracles.

Tooling only. Each child has bounded output/time; --only splits the fixed
inventory. Native programs execute authored literal assertions and effect traces
under checked arithmetic, independently at O0, O2 and ASan/UBSan.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = Path(__file__).with_name('results')
sys.path.insert(0, str(ROOT / 'tools'))
import bounded_process


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def inventory():
    manifest = json.loads((FIXTURES / 'fixtures.json').read_text())
    assert manifest['schema'] == 'coax.results.fixtures.v1'
    cases = manifest['cases']
    assert len(cases) == 41 and sum(c['expected'] == 'accept' for c in cases) == 13
    assert len({c['id'] for c in cases}) == len(cases)
    assert {c['source'] for c in cases} == {p.name for p in FIXTURES.glob('*.coax')}
    for case in cases:
        path = FIXTURES / case['source']
        assert path.parent == FIXTURES and digest(path) == case['sha256']
        assert case['expected'] in ('accept', 'reject')
        if case['expected'] == 'reject':
            assert case['code'].startswith('E0')
            assert 0 < case['line'] <= len(path.read_text().splitlines())
    return cases


def clean(result):
    return (result['started'] and result['stdout_eof'] and result['stderr_eof']
            and not any(result[k] for k in (
                'timeout', 'deadline_exhausted', 'stdout_limit', 'stderr_limit',
                'output_limit', 'group_killed', 'launch_errno', 'launch_error',
                'kill_errno', 'read_errno')))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, default=ROOT / 'bin/coax')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--only', action='append', default=[])
    parser.add_argument('--mode', choices=('source', 'native', 'all'), default='all')
    parser.add_argument('--preflight', action='store_true')
    parser.add_argument('--budget-s', type=int, default=840)
    args = parser.parse_args()
    cases = inventory()
    if args.only:
        selected = set(args.only)
        assert selected <= {c['id'] for c in cases}, 'unknown case selection'
        cases = [c for c in cases if c['id'] in selected]
    assert 0 < args.budget_s <= 840
    compiler = args.compiler.resolve(strict=True)
    compiler_hash = digest(compiler)
    assert not args.output.exists(), 'retain prior observations in separate directories'
    args.output.mkdir(parents=True)
    deadline = time.monotonic() + args.budget_s
    record = {'schema': 'coax.results.observations.v1', 'outcome': 'running',
              'compiler': str(compiler), 'compiler_sha256': compiler_hash,
              'mode': args.mode, 'selected_cases': [c['id'] for c in cases],
              'manifest_sha256': digest(FIXTURES / 'fixtures.json'), 'cases': []}

    def save():
        (args.output / 'record.json').write_text(json.dumps(record, indent=2) + '\n')

    def observe(argv, directory, name, timeout=120):
        result = bounded_process.run(argv, ROOT.parent, os.environ.copy(),
                                     timeout, 8 * 1024 * 1024, deadline,
                                     quiet_faults=True)
        for pipe in ('stdout', 'stderr'):
            raw = result.pop(pipe)
            (directory / (name + '.' + pipe)).write_bytes(raw)
            result[pipe + '_sha256'] = hashlib.sha256(raw).hexdigest()
        (directory / (name + '.json')).write_text(json.dumps(result, indent=2) + '\n')
        return result

    failures = []
    for case in cases:
        directory = args.output / case['id']
        directory.mkdir()
        source = FIXTURES / case['source']
        row = {'id': case['id'], 'source_sha256': digest(source), 'passed': True}
        record['cases'].append(row)
        try:
            if not args.preflight and (args.mode != 'native' or case['expected'] == 'reject'):
                command = 'test' if case['expected'] == 'accept' else 'check'
                result = observe([str(compiler), command, str(source),
                                  '--diagnostics', 'json'], directory, 'source')
                assert clean(result), 'unclean compiler process'
                if case['expected'] == 'accept':
                    assert result['returncode'] == 0, 'authored evaluator checks failed'
                else:
                    assert result['returncode'] == 1, 'negative source was not refused'
                    diagnostics = [json.loads(line) for line in
                                   (directory / 'source.stderr').read_text().splitlines()]
                    assert diagnostics, 'missing structured diagnostic'
                    for diagnostic in diagnostics:
                        assert diagnostic['schema'] == 'coax.diagnostic.v1'
                        span = diagnostic['span']
                        assert Path(span['file']).resolve() == source.resolve()
                        raw = source.read_bytes()
                        assert 0 <= span['offset'] <= len(raw)
                        assert 0 <= span['length'] <= len(raw) - span['offset']
                        prefix = raw[:span['offset']]
                        assert span['line'] == prefix.count(b'\n') + 1
                        assert span['column'] == len(prefix) - prefix.rfind(b'\n')
                    first = diagnostics[0]
                    assert (first['code'], first['span']['line']) == (case['code'], case['line']), 'wrong primary code/source witness'
            if not args.preflight and args.mode != 'source' and case['expected'] == 'accept':
                result = observe([str(compiler), 'c', str(source)], directory, 'emit')
                assert clean(result) and result['returncode'] == 0, 'C emission failed'
                emitted = directory / 'program.c'
                emitted.write_bytes((directory / 'emit.stdout').read_bytes())
                for profile, flags in (
                    ('O0', ['-O0']), ('O2', ['-O2']),
                    ('sanitized', ['-O1', '-g', '-fsanitize=address,undefined',
                                   '-fno-sanitize-recover=all', '-fno-omit-frame-pointer'])):
                    binary = directory / profile
                    result = observe([os.environ.get('CC', 'cc'), '-std=c11', *flags,
                                      str(emitted), '-lm', '-o', str(binary)],
                                     directory, profile + '-build', 180)
                    assert clean(result) and result['returncode'] == 0, profile + ' compile failed'
                    result = observe([str(binary)], directory, profile + '-run')
                    assert clean(result) and result['returncode'] == 0, profile + ' execution failed'
                    assert (directory / (profile + '-run.stdout')).read_bytes() == case['native_stdout'].encode(), profile + ' stdout differs from literal oracle'
                    assert not (directory / (profile + '-run.stderr')).read_bytes(), profile + ' stderr/sanitizer output'
            assert digest(source) == case['sha256'], 'source changed during observation'
        except (AssertionError, ValueError, KeyError) as error:
            row['passed'] = False
            row['failure'] = str(error)
            failures.append(case['id'])
        save()
        print(case['id'], 'PASS' if row['passed'] else 'FAIL: ' + row['failure'], flush=True)
    assert digest(compiler) == compiler_hash, 'compiler changed during observation'
    record['outcome'] = 'preflight' if args.preflight else 'failed' if failures else 'passed'
    record['failures'] = failures
    save()
    return bool(failures)


if __name__ == '__main__':
    sys.exit(main())
