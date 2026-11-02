#!/usr/bin/env python3
"""B1 actual generated-header/native observation gate; maintenance tooling only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = Path(__file__).with_name('retained-borrows')
ORACLE = FIXTURES / 'native-expectations.json'
DRIVER = Path(__file__).with_name('retained-borrows-native.c')
sys.path.insert(0, str(ROOT / 'tools'))
import bounded_process as process
import quiet_crash


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def clean(observation):
    return (observation['started'] and observation['stdout_eof'] and observation['stderr_eof']
            and observation['returncode'] == 0
            and all(not observation[key] for key in (
                'timeout', 'deadline_exhausted', 'stdout_limit', 'stderr_limit',
                'output_limit', 'group_killed', 'launch_errno', 'launch_error',
                'kill_errno', 'read_errno')))


def source(name):
    path = (FIXTURES / name).resolve()
    if not path.is_file() or not path.is_relative_to(FIXTURES.resolve()):
        raise ValueError('oracle source outside fixture inventory')
    return path


def compiler_alias(program, cwd, environment):
    """Find argv0 in the exact child cwd/PATH while preserving alias dispatch."""
    if os.path.dirname(program):
        candidate = Path(program)
        alias = candidate if candidate.is_absolute() else cwd/candidate
    else:
        search = [str(Path(entry) if Path(entry).is_absolute() else cwd/entry)
                  for entry in os.get_exec_path(environment)]
        found = shutil.which(program, path=os.pathsep.join(search))
        if found is None:
            raise ValueError('native compiler is unavailable')
        alias = Path(found)
    alias = Path(os.path.abspath(alias))
    if not alias.is_file() or not os.access(alias, os.X_OK):
        raise ValueError('native compiler alias is not executable')
    return alias


def generate_vectors(path, oracle):
    """Copy literal programs and expected facts, never compute borrow policy."""
    lines = ['/* Independently authored literal source/report oracles. */']
    fixture_rows = []
    for index, row in enumerate(oracle['cases']):
        text = source(row['source']).read_text()
        if not text.isascii():
            raise ValueError('this mechanical C string transport admits ASCII fixtures only')
        if digest(source(row['source'])) != row['authored_source_sha256']:
            raise ValueError('oracle source changed')
        reports = row['reports']
        if reports:
            lines.append('static const ExpectedReport expected_' + str(index) + '[]={')
            for expected in reports:
                lines.append('{' + ','.join([
                    str(expected['kind']), json.dumps(expected['owner_name']),
                    str(expected['site_line']), str(expected['site_col']),
                    '1' if expected['facts_complete'] else '0',
                    '1' if expected['relation_complete'] else '0',
                    '1' if expected['forbidden'] else '0', str(expected['pending'])]) + '},')
            lines.append('};')
        errors = row['diagnostics']
        if len(errors) > 1:
            raise ValueError('native source oracle expects one original diagnostic at most')
        diagnostic = errors[0] if errors else {'code': '', 'line': 0, 'col': 0}
        fixture_rows.append('{' + ','.join([
            json.dumps(row['id']), json.dumps(str(source(row['source']))),
            json.dumps(text), '1' if row['expected_success'] else '0',
            json.dumps(diagnostic['code']), str(diagnostic['line']), str(diagnostic['col']),
            'expected_' + str(index) if reports else 'NULL', str(len(reports))]) + '},')
    lines += ['static const Fixture fixtures[]={', *fixture_rows, '};']
    path.write_text('\n'.join(lines) + '\n')


def run(compiler, directory, expected_compiler_sha256=None):
    oracle = json.loads(ORACLE.read_text())
    if oracle['schema'] != 'coax.retained-borrows.native-oracles.v1':
        raise ValueError('unknown native oracle')
    if len(oracle['cases']) != 19 or len({row['id'] for row in oracle['cases']}) != 19:
        raise ValueError('native oracle inventory changed')
    # This inventory is deliberately broad. Emission profiles below additionally
    # pin each actual consumed source and reject a compiler-read/host-read race.
    inputs = sorted(set([
        ORACLE, FIXTURES/'fixtures.json', DRIVER, Path(__file__).resolve(),
        Path(process.__file__).resolve(), Path(quiet_crash.__file__).resolve(),
        *[source(row['source']) for row in oracle['cases']],
        *(ROOT/'lib/compiler').rglob('*.coax'),
    ]))
    before = {str(path): digest(path) for path in inputs}
    compiler_sha = digest(compiler)
    if expected_compiler_sha256 and compiler_sha != expected_compiler_sha256:
        raise ValueError('wrong selected compiler SHA-256')
    launch_environment = dict(os.environ)
    cc = shlex.split(launch_environment.get('CC', 'cc'))
    if not cc:
        raise ValueError('empty native compiler command')
    selected_alias = compiler_alias(cc[0], ROOT.parent, launch_environment)
    resolved_cc = selected_alias.resolve(strict=True)
    cc_sha = digest(resolved_cc)
    record = {
        'schema': 'coax.retained-borrows.native-qualification.v1',
        'scope': 'Actual single-source lexer/parser/CK_run and public committed effect observations; generated-header LP64 guards, O0/O2/sanitizers. No module resolver, private effect-cache inference, B2 returned-field composition or host-lifetime qualification.',
        'compiler': str(compiler), 'compiler_sha256': compiler_sha,
        'producer_provenance': 'Actual selected producer; matching normal/source qualification is separate.',
        'cc': cc, 'cc_alias': str(selected_alias), 'resolved_cc': str(resolved_cc),
        'resolved_cc_sha256': cc_sha, 'cc_resolution_cwd': str(ROOT.parent),
        'cc_resolution_path': launch_environment.get('PATH'),
        'source_before': before, 'commands': [], 'profiles': {},
        'consumed_sources': {}, 'artifacts': {}, 'outcome': 'not-checked',
        'oracle_sha256': digest(ORACLE), 'selection': [row['id'] for row in oracle['cases']],
    }
    started = time.monotonic()
    deadline = started + 840

    def guards():
        after = {str(path): digest(path) for path in inputs}
        return (before == after and compiler_sha == digest(compiler)
                and compiler_alias(cc[0], ROOT.parent, launch_environment) == selected_alias
                and selected_alias.resolve(strict=True) == resolved_cc and cc_sha == digest(resolved_cc)
                and all(digest(path) == sha for path, sha in record['consumed_sources'].items()))

    def save():
        record['source_after'] = {str(path): digest(path) for path in inputs}
        record['compiler_sha256_after'] = digest(compiler)
        record['inputs_unchanged'] = guards()
        record['artifacts_unchanged'] = all(Path(path).is_file() and digest(path) == sha
                                            for path, sha in record['artifacts'].items())
        (directory/'qualification.json').write_text(json.dumps(record, indent=2) + '\n')

    def execute(label, argv, limit, env=None):
        save()
        if not record['inputs_unchanged'] or not record['artifacts_unchanged']:
            raise RuntimeError('input/artifact drift before ' + label)
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError('native gate total deadline exhausted')
        observation = process.run(list(map(str, argv)), cwd=ROOT.parent,
                                  env=launch_environment if env is None else env,
                                  timeout_s=min(limit, remaining), max_output_bytes=33554432,
                                  deadline=deadline, quiet_faults=True)
        raw = dict(observation)
        streams = {}
        for name in ('stdout', 'stderr'):
            output = directory/(label + '.' + name)
            output.write_bytes(raw.pop(name))
            streams[name] = {'path': str(output), 'sha256': digest(output), 'bytes': output.stat().st_size}
        passed = clean(observation)
        record['commands'].append({'label': label, 'argv': list(map(str, argv)),
                                   'observation': raw, 'raw': streams, 'passed': passed})
        save()
        if not passed:
            raise RuntimeError('native command failed: ' + label)
        if not record['inputs_unchanged'] or not record['artifacts_unchanged']:
            raise RuntimeError('input/artifact drift during ' + label)
        return observation['stdout']

    try:
        include = directory/'retained-borrow-native-vectors.inc'
        generate_vectors(include, oracle)
        record['artifacts'][str(include)] = digest(include)
        c_sources = []
        for name in ('lexer', 'parser', 'checker'):
            module = ROOT/'lib/compiler'/(name + '.coax')
            c = directory/(name + '.c')
            header = directory/(name + '.h')
            profile = directory/(name + '.profile.json')
            c.write_bytes(execute('emit-' + name, [compiler, 'c', module, '--library',
                '--arithmetic', 'checked', '--header', header, '--profile-report', profile], 180))
            manifest = json.loads(profile.read_text())
            if (manifest.get('schema_version') != 1 or manifest.get('command') != 'c'
                    or manifest.get('backend') != 'c11' or manifest.get('runtime_arithmetic') != 'checked'
                    or manifest.get('interpreter_arithmetic') != 'checked'
                    or manifest.get('success') is not True or manifest.get('exit_status') != 0
                    or manifest.get('emitted_sha256') != digest(c)
                    or manifest.get('artifact_sha256') != digest(c)
                    or not isinstance(manifest.get('sources'), list)
                    or manifest.get('descriptors') != []):
                raise ValueError('missing actual emission input manifest')
            for item in manifest['sources']:
                path = Path(item['path'])
                if not path.is_absolute():
                    path = (ROOT/path).resolve()
                path = path.resolve(strict=True)
                observed = digest(path)
                if observed != item['sha256']:
                    raise RuntimeError('consumed emission source drift')
                prior = record['consumed_sources'].get(str(path))
                if prior is not None and prior != observed:
                    raise RuntimeError('inconsistent consumed source identity')
                record['consumed_sources'][str(path)] = observed
            for path in (c, header, profile):
                record['artifacts'][str(path)] = digest(path)
            c_sources.append(c)
            save()
        expected_reports = sum(len(row['reports']) for row in oracle['cases'])
        negative_cases = sum(not row['expected_success'] for row in oracle['cases'])
        profiles = [('O0', ['-O0']), ('O2', ['-O2']),
                    ('sanitized', ['-O1', '-g', '-fsanitize=address,undefined', '-fno-omit-frame-pointer'])]
        for name, flags in profiles:
            binary = directory/name
            execute(name + '-compile', [*cc, '-std=c11', *flags, '-Wall', '-Wextra',
                    '-Werror', '-Wno-unused-function', '-Wno-unused-variable',
                    '-Wno-unused-but-set-variable', '-Wno-unused-parameter', '-Wno-misleading-indentation',
                    '-ffp-contract=off', '-fno-fast-math', '-I', directory,
                    *c_sources, DRIVER, '-lm', '-o', binary], 180)
            record['artifacts'][str(binary)] = digest(binary)
            output = execute(name + '-run', [binary], 60,
                             env={**launch_environment, 'ASAN_OPTIONS': 'detect_leaks=0',
                                  'UBSAN_OPTIONS': 'halt_on_error=1'})
            observed = json.loads(output)
            expected_counts = {'cases': 21, 'source_cases': 19, 'negative_cases': negative_cases,
                               'matched_reports': expected_reports, 'disabled_controls': 1,
                               'short_controls': 1, 'effect_size': 880,
                               'effect_source_storage_offset': 872,
                               'observation_size': 40, 'caps_size': 24}
            if any(type(observed.get(key)) is not int or observed[key] != value
                   for key, value in expected_counts.items()):
                raise AssertionError('native literal-oracle count/layout mismatch')
            actual = observed.get('observations')
            expected_id_modes = {(row['id'], 0) for row in oracle['cases']}
            expected_id_modes |= {(oracle['cases'][1]['id'], 1), (oracle['cases'][1]['id'], 2)}
            if (not isinstance(actual, list) or len(actual) != 21
                    or {(row['fixture_id'], row['mode']) for row in actual} != expected_id_modes):
                raise AssertionError('per-case native observation inventory mismatch')
            record['profiles'][name] = {'outcome': 'passed', 'binary': str(binary),
                                       'binary_sha256': digest(binary), 'observed': observed,
                                       'fresh_generated_header_guards_compiled': True}
            save()
        record['outcome'] = 'passed'
    except BaseException as error:
        record['outcome'] = 'failed'
        record['error'] = type(error).__name__ + ': ' + str(error)
        raise
    finally:
        record['elapsed_s'] = time.monotonic() - started
        save()
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', default=os.environ.get('COAX_TEST_COMPILER', str(ROOT/'bin/coax')))
    parser.add_argument('--expected-compiler-sha256')
    parser.add_argument('--record-dir')
    args = parser.parse_args()
    compiler = Path(args.compiler).resolve(strict=True)
    directory = (Path(args.record_dir).resolve() if args.record_dir
                 else Path(tempfile.mkdtemp(prefix='coax-retained-borrows-native-')))
    if directory.is_relative_to(FIXTURES.resolve()):
        raise ValueError('records must not replace fixture inputs')
    if args.record_dir:
        directory.mkdir(parents=True, exist_ok=False)
    record = run(compiler, directory, args.expected_compiler_sha256)
    print(json.dumps({'outcome': record['outcome'], 'profiles': list(record['profiles']),
                      'record': str(directory/'qualification.json')}))


if __name__ == '__main__':
    main()
