#!/usr/bin/env python3
"""B1 original-source retention fixtures; maintenance tooling, never runtime."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = Path(__file__).with_name('retained-borrows')
MANIFEST = FIXTURES / 'fixtures.json'
sys.path.insert(0, str(ROOT / 'tools'))
import bounded_process as process
import quiet_crash


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def fixture(name):
    path = (FIXTURES / name).resolve()
    if not path.is_relative_to(FIXTURES.resolve()) or not path.is_file():
        raise ValueError('fixture outside closed inventory: ' + name)
    return path


def inventory():
    manifest = json.loads(MANIFEST.read_text())
    if manifest['schema'] != 'coax.retained-borrows.fixtures.v1':
        raise ValueError('unknown manifest schema')
    cases = manifest['cases']
    if not cases or len({row['id'] for row in cases}) != len(cases):
        raise ValueError('empty or ambiguous case inventory')
    names = [row['source'] for row in cases]
    if len(set(names)) != len(names):
        raise ValueError('duplicate source entry')
    expected = set(names) | set(manifest['support'])
    actual = {str(p.relative_to(FIXTURES)) for p in FIXTURES.rglob('*.coax')}
    if actual != expected:
        raise ValueError('source/support inventory is not closed')
    for name in expected:
        fixture(name)
    for row in cases:
        if row['outcome'] not in ('accept', 'accept-manual', 'reject'):
            raise ValueError('unsupported expectation')
        if bool(row['diagnostics']) != (row['outcome'] == 'reject'):
            raise ValueError('diagnostic/outcome mismatch')
        if digest(fixture(row['source'])) != row['authored_source_sha256']:
            raise ValueError('authored source changed: ' + row['source'])
        for diag in row['diagnostics']:
            lines = fixture(diag['file']).read_text().splitlines()
            if type(diag['line']) is not int or not 0 < diag['line'] <= len(lines):
                raise ValueError('invalid original witness line')
            if type(diag['col']) is not int or not 0 < diag['col'] <= len(lines[diag['line']-1]):
                raise ValueError('invalid original witness column')
            if diag['code'] not in ('E0383', 'E0366'):
                raise ValueError('unsupported source diagnostic')
            witness = row['witness']['text']
            if not lines[diag['line']-1][diag['col']-1:].startswith(witness):
                raise ValueError('witness does not match literal source')
    return cases, [fixture(name) for name in sorted(expected)], manifest['deferred']


def clean(observation):
    return (observation['started'] and observation['stdout_eof'] and observation['stderr_eof']
            and all(not observation[key] for key in (
                'timeout', 'deadline_exhausted', 'stdout_limit', 'stderr_limit',
                'output_limit', 'group_killed', 'launch_errno', 'launch_error',
                'kill_errno', 'read_errno')))


def parse_diagnostics(data):
    raw = [json.loads(line) for line in data.decode('utf-8').splitlines()]
    normalized = []
    for item in raw:
        if (not isinstance(item, dict) or type(item.get('line')) is not int
                or type(item.get('col')) is not int
                or not isinstance(item.get('file'), str)
                or not isinstance(item.get('code'), str)):
            raise ValueError('invalid diagnostic transport')
        normalized.append({key: item[key] for key in ('line', 'col', 'code')}
                          | {'file': str(Path(item['file']).resolve())})
    return raw, normalized


def run(compiler, directory, selected=None, preflight=False, expected_sha256=None):
    cases, sources, deferred = inventory()
    if selected:
        unknown = set(selected) - {row['id'] for row in cases}
        if unknown:
            raise ValueError('unknown fixture IDs: ' + repr(sorted(unknown)))
        cases = [row for row in cases if row['id'] in selected]
    inputs = [*sources, MANIFEST, Path(__file__).resolve(),
              Path(process.__file__).resolve(), Path(quiet_crash.__file__).resolve()]
    before = {str(p): digest(p) for p in inputs}
    compiler_before = digest(compiler)
    if expected_sha256 is not None and compiler_before != expected_sha256:
        raise ValueError('selected compiler image does not match requested SHA-256')
    record = {
        'schema': 'coax.retained-borrows.source-qualification.v1',
        'scope': 'Original-source RHS/CALL physical witnesses and source acceptance only. Effect completeness/pending reports require a separate native gate; acceptance is not host lifetime safety.',
        'mode': 'preflight-check-first' if preflight else 'qualification',
        'compiler': str(compiler), 'compiler_sha256': compiler_before,
        'producer_provenance': 'Actual selected image. Matching compiler/source qualification is separately retained; this harness does not create a compiler seal.',
        'source_before': before, 'selection': [row['id'] for row in cases],
        'deferred': deferred, 'cases': [], 'semantic_red_cases': [],
        'outcome': 'not-checked',
    }
    start = time.monotonic()
    deadline = start + 600

    def save():
        record['source_after'] = {str(p): digest(p) for p in inputs}
        record['compiler_sha256_after'] = digest(compiler)
        record['inputs_unchanged'] = (before == record['source_after']
                                     and compiler_before == record['compiler_sha256_after'])
        if not record['inputs_unchanged']:
            record['outcome'] = 'source-changed'
        (directory / 'qualification.json').write_text(json.dumps(record, indent=2) + '\n')

    try:
        for case in cases:
            save()
            if not record['inputs_unchanged']:
                raise RuntimeError('source/compiler changed before invocation')
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RuntimeError('ten-minute source gate deadline exhausted')
            argv = [str(compiler), 'check', '--json', str(fixture(case['source']))]
            observation = process.run(argv, cwd=ROOT.parent, env=None, timeout_s=min(30.0, remaining),
                                      max_output_bytes=1048576, deadline=deadline,
                                      quiet_faults=True)
            raw_observation = dict(observation)
            output = {}
            for stream in ('stdout', 'stderr'):
                data = raw_observation.pop(stream)
                path = directory / (case['id'] + '.' + stream)
                path.write_bytes(data)
                output[stream] = {'path': str(path), 'sha256': digest(path), 'bytes': len(data)}
            parse_error = None
            try:
                raw, observed = parse_diagnostics(observation['stderr'])
            except (ValueError, TypeError, UnicodeError) as error:
                raw, observed = [], []
                parse_error = str(error)
            expected = [{**item, 'file': str(fixture(item['file']))}
                        for item in case['diagnostics']]
            accepted = (clean(observation) and observation['returncode'] == 0
                        and not observation['stdout'] and not observation['stderr'])
            passed = (clean(observation) and parse_error is None
                      and observation['returncode'] == (1 if expected else 0)
                      and not observation['stdout'] and observed == expected)
            semantic_red = bool(expected and all(d['code'] == 'E0383' for d in expected) and accepted)
            if semantic_red:
                classification = 'forbidden-store-accepted-semantic-red'
                record['semantic_red_cases'].append(case['id'])
            elif passed:
                classification = 'expected-original-diagnostic' if expected else 'source-accepted-only'
            elif not clean(observation):
                classification = 'infrastructure-or-budget-failure'
            elif observed:
                classification = 'different-original-diagnostic-or-setup-refusal'
            else:
                classification = 'unparsed-or-malformed-compiler-observation'
            record['cases'].append({
                'id': case['id'], 'source': str(fixture(case['source'])),
                'expected_scope': case['outcome'], 'expected_diagnostics': expected,
                'observed_diagnostics': raw, 'normalized_diagnostics': observed,
                'diagnostic_transport_error': parse_error, 'process': raw_observation,
                'raw': output, 'classification': classification,
                'semantic_red': semantic_red, 'outcome': 'passed' if passed else 'failed',
            })
            save()
            if not record['inputs_unchanged']:
                raise RuntimeError('source/compiler changed during invocation')
            if not clean(observation):
                raise RuntimeError('bounded compiler transport did not complete')
        record['outcome'] = ('failed' if any(row['outcome'] != 'passed' for row in record['cases'])
                             else 'passed')
        if preflight:
            record['qualification_claim'] = False
            record['syntax_or_existing_diagnostic_failures'] = [
                row['id'] for row in record['cases']
                if row['outcome'] != 'passed' and not row['semantic_red']]
    except BaseException as error:
        record['outcome'] = 'failed'
        record['error'] = type(error).__name__ + ': ' + str(error)
        raise
    finally:
        record['elapsed_s'] = time.monotonic() - start
        save()
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', default=os.environ.get('COAX_TEST_COMPILER', str(ROOT/'bin/coax')))
    parser.add_argument('--record-dir')
    parser.add_argument('--only', action='append')
    parser.add_argument('--preflight', action='store_true')
    parser.add_argument('--expected-compiler-sha256')
    args = parser.parse_args()
    compiler = Path(args.compiler).resolve(strict=True)
    directory = (Path(args.record_dir).resolve() if args.record_dir
                 else Path(tempfile.mkdtemp(prefix='coax-retained-borrow-sources-')))
    if directory.is_relative_to(FIXTURES.resolve()):
        raise ValueError('records must not replace fixture sources')
    if args.record_dir:
        directory.mkdir(parents=True, exist_ok=False)
    record = run(compiler, directory, args.only, args.preflight, args.expected_compiler_sha256)
    print(json.dumps({'outcome': record['outcome'], 'fixtures': len(record['cases']),
                      'semantic_red_cases': record['semantic_red_cases'],
                      'record': str(directory/'qualification.json')}))
    return 0 if record['outcome'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
