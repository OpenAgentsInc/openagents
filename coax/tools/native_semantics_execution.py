"""Optional semantic experiments over original checked verification artifacts.

Coax owns admission, structured traps, seed bytes and tuple reduction. Python
performs bounded process, file, compiler and ABI transport effects only.
"""
import argparse
import base64
import copy
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import tempfile
import time

import native_check_execution as common
import native_forall_execution as quantified
import native_execution_identities as identities
from native_execution_policy import ROOT, REPO, OUTCOMES, PolicyUnavailable
from native_semantics_policy import SemanticsPolicy

SCHEMA = 'coax.native-checks.semantics.v1'
OBSERVER = ROOT / 'tools/native-semantics-observer.c'
_POLICIES = {}


def _transport():
    return [Path(__file__), ROOT / 'tools/native_semantics_policy.py',
        ROOT / 'tools/native-forall-observer.c', ROOT / 'tools/native-forall-observer.h',
        ROOT / 'tools/native_forall_policy.py', ROOT / 'tools/native_forall_execution.py',
        ROOT / 'tools/native_execution_policy.py', ROOT / 'tools/native_check_execution.py',
        ROOT / 'tools/native_execution_identities.py', ROOT / 'tools/bounded_process.py',
        ROOT / 'tests/quiet_crash.py', ROOT / 'tools/native-forall-progress.h',
        OBSERVER, OBSERVER.with_suffix('.h')]


def get_policy(compiler=None, directory=None, deadline=None, _runner=None):
    compiler = common._compiler(compiler)
    deadline = deadline if deadline is not None else time.monotonic() + 840
    key = (common._sha(compiler), *(common._sha(ROOT / 'lib/compiler' / name)
        for name in SemanticsPolicy.input_names), os.environ.get('COAX_EXECUTION_HOST_CC'),
        os.environ.get('CC'), os.environ.get('SDKROOT'))
    cached = _POLICIES.get(key)
    if cached is None:
        target = Path(tempfile.mkdtemp(prefix='coax-semantics-policy-')) / 'policy'
        cached = SemanticsPolicy(compiler, target, deadline,
            runner=_runner or common.Session(deadline).run)
        _POLICIES[key] = cached
    return common._retained_policy(cached, directory)


def _raw(value):
    if isinstance(value, dict):
        if {'base64', 'bytes', 'sha256'} <= set(value):
            raw = base64.b64decode(value['base64'], validate=True)
            if len(raw) != value['bytes'] or hashlib.sha256(raw).hexdigest() != value['sha256']:
                raise ValueError('Retained byte observation is corrupt')
            return raw
        return {key: _raw(child) for key, child in value.items()}
    if isinstance(value, list):
        return [_raw(child) for child in value]
    return value


def run(files, *, keep=None, compiler=None, steps=None, only=None,
        timeout_s=10.0, max_output_bytes=1048576, cc=None):
    result = quantified._run_profile(files, keep=keep, compiler=compiler, steps=steps,
        only=only, timeout_s=timeout_s, max_output_bytes=max_output_bytes, cc=cc, abi=2,
        schema=SCHEMA, observer=OBSERVER, policy_factory=get_policy,
        observe=quantified._observe, extra_transport=_transport())
    result.update(artifact_only=False, source_minimization='not-supported')
    try:
        policy = get_policy(compiler)
        result['baseline_qualified'] = policy.qualified(result['outcome'],
            result['outcome'] == 'passed', False)
    except PolicyUnavailable:
        result['baseline_qualified'] = False
    return result


def _artifact_codes(policy, artifact, chain):
    codes = [policy.integrity(expected, common._sha(path))
             for path, expected in artifact.items()]
    codes += common._toolchain_integrity(policy, chain)
    return codes


def _compile(policy, source_c, session, native_cc, optimization, binary, cap):
    process = session.run([*native_cc, '-std=c11', '-O' + str(optimization),
        '-ffp-contract=off', '-fno-fast-math', '-I', str(policy.directory),
        str(source_c), str(policy.c), str(OBSERVER), '-lm', '-o', str(binary)],
        timeout_s=policy.limits.compile_ms / 1000, max_output_bytes=cap)
    return process


def _observe_one(binary, check, expected, session, policy, directory,
                 timeout_s, cap, index=None):
    directory.mkdir(parents=True, exist_ok=False)
    progress = directory / 'progress.bin'
    initial = policy.initial_region()
    reserved = len(initial) + 1
    admitted = policy.capture(session.remaining_bytes, reserved)
    if admitted:
        return {'outcome': OUTCOMES[admitted], 'recognized_trap': False,
            'native_outcome': OUTCOMES[admitted], 'native_outcome_code': admitted,
            'valid': False, 'trap': None,
            'baseline_qualified': False, 'process': None,
            'progress_capture_admission': {'outcome': OUTCOMES[admitted],
                'remaining_bytes': session.remaining_bytes, 'requested_bytes': reserved}}
    progress.write_bytes(initial)
    session.remaining_bytes -= reserved
    argv = [str(binary), 'one', str(check['ordinal']), str(progress)] if index is None else [
        str(binary), 'case', str(check['ordinal']), str(index), str(progress)]
    process = session.run(argv, timeout_s=timeout_s, max_output_bytes=cap)
    with progress.open('rb') as stream:
        region = stream.read(reserved)
    session.remaining_bytes += reserved - len(region)
    classified = policy.classify(check, expected, region, process, replay_index=index)
    classified.update(identity=common._identity(check), process=common._record(process),
        progress=common._record(region), progress_path=str(progress),
        witness=policy.witness(check, classified['values']) if classified['valid'] else '')
    return common._record(classified)


def observe_artifact(binary, checks, expected, *, keep, policy=None, compiler=None,
                     timeout_s=10.0, max_output_bytes=1048576, deadline=None):
    deadline = deadline if deadline is not None else time.monotonic() + 840
    policy = policy or get_policy(compiler, deadline=deadline)
    if not policy.budget(timeout_s, max_output_bytes):
        raise ValueError('Coax refused the semantic observation bounds')
    session = common.Session(deadline)
    root = Path(keep)
    root.mkdir(parents=True, exist_ok=False)
    binary = Path(binary).resolve()
    before = common._sha(binary)
    observations = [_observe_one(binary, check, expected, session, policy,
        root / str(check['ordinal']), timeout_s, max_output_bytes) for check in checks]
    outcome = OUTCOMES[policy.aggregate([*[item['outcome'] for item in observations],
        policy.integrity(before, common._sha(binary))])]
    return {'schema': SCHEMA, 'outcome': outcome, 'artifact_only': True,
        'baseline_qualified': policy.qualified(outcome, False, True), 'checks': observations,
        'source_minimization': 'not-supported', 'binary': str(binary), 'binary_sha256': before}


def _diagnostics(process):
    values = []
    for line in process['stderr'].splitlines():
        try:
            value = json.loads(line, parse_constant=lambda text: (_ for _ in ()).throw(ValueError(text)))
        except (ValueError, UnicodeError):
            continue
        if isinstance(value, dict) and 'code' in value:
            values.append(value)
    return values


def run_trap(source, expected, *, keep, compiler=None, steps=None,
             timeout_s=10.0, max_output_bytes=1048576, cc=None):
    start = time.monotonic()
    deadline = start + 840
    compiler = common._compiler(compiler)
    work = Path(keep).resolve()
    work.mkdir(parents=True, exist_ok=False)
    source = Path(source).resolve()
    result = {'schema': SCHEMA, 'outcome': 'infrastructure-error', 'artifact_only': True,
        'baseline_qualified': False, 'source_minimization': 'not-supported',
        'compiler': str(compiler), 'compiler_sha256': common._sha(compiler),
        'file': str(source), 'source_sha256': common._sha(source), 'expected': common._record(expected),
        'profiles': {}, 'transport_sources': {str(path): common._sha(path) for path in _transport()}}
    session = common.Session(deadline)
    try:
        policy = get_policy(compiler, work / 'policy', deadline, session.run)
        result.update(policy=policy.identity, policy_bootstrap=common._record(policy.observations))
        requested = policy.limits.steps if steps is None else steps
        if not policy.budget(timeout_s, max_output_bytes) or not policy.step_request(requested):
            result['why'] = 'Coax refused the execution bounds'
            return result
        version = session.run([str(compiler), 'version'])
        result['compiler_version_execution'] = common._record(version)
        if policy.phase(version):
            return result
        version_text = version['stdout'].decode('utf-8', 'strict').strip()
        baseline_process = session.run([str(compiler), 'test', str(source), '--library', '--json',
            '--arithmetic', 'checked', '--evidence', '/dev/stderr', '--profile-report', '/dev/stdout',
            '--steps', str(requested)], timeout_s=policy.limits.baseline_ms / 1000,
            max_output_bytes=policy.limits.baseline_output_bytes)
        baseline, inputs = common._baseline_records(baseline_process)
        validation = policy.baseline(baseline, baseline_process, source, result['source_sha256'],
            result['compiler_sha256'], version_text, requested)
        failed = policy.failed_baseline(validation)
        result.update(baseline=baseline, baseline_input_manifest=inputs,
            baseline_execution=common._record(baseline_process), baseline_validation=validation,
            artifact_parity_admission=failed)
        codes = [failed['outcome'], policy.failed_inputs(validation, baseline, inputs, baseline_process, version_text)]
        selection = policy.select(baseline, None)
        result['failed_claim_selection'] = common._record(selection)
        # Failed evaluator claims point at the fault, which may be inside an
        # imported helper. They are not declaration-location observations.
        # Named negative experiments therefore require an independent oracle;
        # the raw evidence and its selected identities remain untouched.
        declaration = expected['declaration']
        native_expected = copy.deepcopy(selection['expected'])
        for check in native_expected:
            check['identity'].update(source=str(Path(declaration['source']).resolve()),
                line=declaration['line'], column=declaration['column'])
        result['authored_declaration_oracle'] = common._record(declaration)
        result['native_expected'] = common._record(native_expected)
        diagnostics = _diagnostics(baseline_process)
        result['evaluator_diagnostics'] = diagnostics
        # Coax owns both failed-original admission and exact diagnostic matching.
        diagnostic_codes = [0 if policy.diagnostic(item['identity'], expected, diagnostics) else 3
                            for item in selection['expected']]
        codes += [selection['outcome'], *diagnostic_codes]
        if policy.aggregate(codes):
            result['outcome'] = OUTCOMES[policy.aggregate(codes)]
            return result
        emitted = session.run([str(compiler), 'checks-c', str(source), '--verification-abi', '2',
            '--arithmetic', 'checked', '--profile-report', '/dev/stderr'],
            timeout_s=policy.limits.compile_ms / 1000, max_output_bytes=policy.limits.baseline_output_bytes)
        manifest = common._last_json(emitted)
        source_c = work / 'checks.c'
        source_c.write_bytes(emitted['stdout'])
        result.update(emission=common._record(emitted), emission_manifest=manifest,
                      c=str(source_c), c_sha256=common._sha(source_c))
        codes += [policy.emission(manifest, baseline, emitted, version_text, common._sha(source_c)),
                  policy.input_descriptors(inputs, manifest)]
        native_cc = list(cc) if isinstance(cc, (list, tuple)) else shlex.split(cc or os.environ.get('COAX_NATIVE_CC', 'cc'))
        chain = identities.inspect(native_cc, session.run, cwd=REPO, environment=session.environment)
        codes += common._toolchain_outcomes(policy, chain)
        result.update(native_cc=native_cc, toolchain=common._record(chain))
        if policy.aggregate(codes):
            result['outcome'] = OUTCOMES[policy.aggregate(codes)]
            return result
        if chain['sdk'].get('environment'):
            session.environment = {**os.environ, **chain['sdk']['environment']}
        protected = {**result['transport_sources'], **policy.identity['sources'],
            str(source_c): common._sha(source_c), str(policy.c): policy.identity['c_sha256'],
            str(policy.header): policy.identity['header_sha256'], str(policy.library): policy.identity['library_sha256']}
        result['artifact_inputs'] = protected
        for optimization in ('0', '2'):
            binary = work / ('native-O' + optimization)
            process = _compile(policy, source_c, session, native_cc, optimization, binary, max_output_bytes)
            profile = {'compile': common._record(process), 'binary': str(binary), 'binary_sha256': common._sha(binary)}
            result['profiles']['-O' + optimization] = profile
            phase = policy.phase(process)
            if not phase:
                metadata = session.run([str(binary), 'metadata'], timeout_s=policy.limits.metadata_ms / 1000,
                                       max_output_bytes=policy.limits.metadata_bytes)
                profile['metadata'] = common._record(metadata)
                phase = policy.metadata(native_expected, metadata['stdout'], metadata)
                if not phase:
                    catalog = policy.decode_catalog(metadata['stdout'])
                    profile['catalog'] = common._record(catalog)
                    observations = [_observe_one(binary, check, expected, session, policy,
                        work / ('O' + optimization + '-progress-' + str(check['ordinal'])),
                        timeout_s, max_output_bytes) for check in catalog]
                    profile['checks'] = observations
                    phase = policy.aggregate([item['outcome'] for item in observations])
            phase = policy.aggregate([phase, *_artifact_codes(policy, protected, chain),
                policy.integrity(profile['binary_sha256'], common._sha(binary))])
            profile['outcome'] = OUTCOMES[phase]
            codes.append(phase)
        final = policy.baseline(baseline, baseline_process, source, result['source_sha256'],
            result['compiler_sha256'], version_text, requested)
        codes += [policy.failed_baseline(final)['outcome'],
            policy.failed_inputs(final, baseline, inputs, baseline_process, version_text),
            policy.input_descriptors(inputs, manifest), *_artifact_codes(policy, protected, chain),
            policy.integrity(result['compiler_sha256'], common._sha(compiler))]
        result['outcome'] = OUTCOMES[policy.aggregate(codes)]
    except PolicyUnavailable as error:
        result.update(why=str(error), policy_loaded=False, policy_bootstrap=common._record(error.observations))
    except (OSError, ValueError, UnicodeError, KeyError, TypeError) as error:
        result.update(why=str(error), startup_or_transport_error=type(error).__name__)
    finally:
        result['elapsed_s'] = time.monotonic() - start
        result['compiler_sha256_after'] = common._sha(compiler)
    return result


def _qualified_original(policy, record):
    """Revalidate retained facts and raw observations without rerunning bodies."""
    item = record['files'][0]
    baseline, process = item['baseline'], item['baseline_execution']
    version = record['compiler_version']
    steps = baseline['budgets']['check_steps']
    validation = policy.baseline(baseline, process, item['file'], item['source_sha256'],
        record['compiler_sha256'], version, steps)
    codes = [validation['outcome'], policy.baseline_inputs(baseline,
        item['baseline_input_manifest'], process, version),
        policy.emission(item['emission_manifest'], baseline, item['emission_execution'],
            version, item['artifact']['c_sha256']),
        policy.input_descriptors(item['baseline_input_manifest'], item['emission_manifest'])]
    selection = policy.select(baseline, record['selection']['only'])
    codes.append(selection['outcome'])
    catalogs = {}
    for level in ('-O0', '-O2'):
        profile = item['profiles'][level]
        metadata = profile['metadata']
        code = policy.metadata(selection['expected'], metadata['stdout'], metadata)
        codes.append(code)
        if code:
            continue
        catalog = policy.decode_catalog(metadata['stdout'])
        catalogs[level] = catalog
        observations = profile['execution']['checks']
        if len(catalog) != len(observations):
            raise ValueError('Retained property observations have the wrong extent')
        for check, observation in zip(catalog, observations):
            codes.append(policy.observe(check, observation['progress'], observation['process'])['outcome'])
    return item, selection, catalogs, validation, codes


def seed_and_reduce(qualified_record, *, expected, keep, optimization='2', seed_id='mul-add',
                    timeout_s=10.0, max_output_bytes=1048576):
    start = time.monotonic()
    deadline = start + 840
    record = _raw(qualified_record)
    work = Path(keep).resolve()
    work.mkdir(parents=True, exist_ok=False)
    result = {'schema': SCHEMA, 'outcome': 'infrastructure-error', 'artifact_only': True,
        'baseline_qualified': False, 'source_minimization': 'not-supported',
        'seed_id': seed_id, 'optimization': optimization, 'replays': [],
        'qualified_record_sha256': hashlib.sha256(json.dumps(common._record(qualified_record),
            sort_keys=True, ensure_ascii=False, separators=(',', ':')).encode()).hexdigest()}
    session = common.Session(deadline)
    try:
        policy = get_policy(record['compiler'], work / 'policy', deadline, session.run)
        result.update(policy=policy.identity, policy_bootstrap=common._record(policy.observations))
        if not policy.qualified(record['outcome'], record.get('baseline_qualified', False), record.get('artifact_only', True)):
            result['why'] = 'The seed requires a qualified original positive record'
            return result
        if not policy.seed_id(seed_id) or not policy.budget(timeout_s, max_output_bytes):
            result['why'] = 'Coax refused the seed identity or execution bounds'
            return result
        item, selection, original_catalogs, validation, admission_codes = _qualified_original(policy, record)
        result['retained_original_validation'] = validation
        result['retained_original_admission'] = OUTCOMES[policy.aggregate(admission_codes)]
        if policy.aggregate(admission_codes):
            result['outcome'] = OUTCOMES[policy.aggregate(admission_codes)]
            result['why'] = 'The retained complete original baseline or native observations were refused'
            return result
        profile = item['profiles']['-O' + optimization]
        artifact = item['artifact']
        protected = {**record['transport_sources'], **record['policy']['sources'],
            artifact['c']: artifact['c_sha256'], record['compiler']: record['compiler_sha256'],
            record['policy']['c']: record['policy']['c_sha256'],
            record['policy']['header']: record['policy']['header_sha256'],
            record['policy']['library']: record['policy']['library_sha256'],
            **{item['profiles'][level]['binary']: item['profiles'][level]['binary_sha256']
               for level in ('-O0', '-O2')}}
        protected.update({fact['path']: fact['sha256'] for fact in [
            *item['baseline']['sources'], *item['baseline_input_manifest']['descriptors']]})
        chain = record['toolchain']
        if policy.aggregate(_artifact_codes(policy, protected, chain)):
            result['why'] = 'Qualified artifact inputs changed before seeding'
            return result
        original = Path(artifact['c']).read_bytes()
        original_path = work / 'checks-original.c'
        original_path.write_bytes(original)
        seeded = policy.seed(original)
        seeded_path = work / 'checks-seeded.c'
        seeded_path.write_bytes(seeded)
        result.update(original_c=str(original_path), original_c_sha256=common._sha(original_path),
            seeded_c=str(seeded_path), seeded_c_sha256=common._sha(seeded_path),
            original_record_compiler_sha256=record['compiler_sha256'],
            original_baseline=item['baseline'], original_baseline_validation=item['baseline_validation'])
        protected.update({**policy.identity['sources'],
            str(original_path): result['original_c_sha256'], str(seeded_path): result['seeded_c_sha256'],
            str(policy.c): policy.identity['c_sha256'], str(policy.header): policy.identity['header_sha256'],
            str(policy.library): policy.identity['library_sha256']})
        if chain['sdk'].get('environment'):
            session.environment = {**os.environ, **chain['sdk']['environment']}
        # Use the executable whose bytes were qualified, regardless of later
        # PATH or relative-command resolution. Preserve its recorded arguments.
        native_cc = [chain['resolved_program'], *record['native_cc'][1:]]
        result['native_cc'] = native_cc
        result['toolchain'] = common._record(chain)
        codes = []
        catalogs = {}
        for level in ('0', '2'):
            binary = work / ('seeded-O' + level)
            compiled = _compile(policy, seeded_path, session, native_cc, level, binary, max_output_bytes)
            current = {'compile': common._record(compiled), 'binary': str(binary), 'binary_sha256': common._sha(binary)}
            result.setdefault('profiles', {})['-O' + level] = current
            phase = policy.phase(compiled)
            if not phase:
                metadata = session.run([str(binary), 'metadata'], timeout_s=policy.limits.metadata_ms / 1000,
                    max_output_bytes=policy.limits.metadata_bytes)
                current['metadata'] = common._record(metadata)
                phase = policy.metadata(selection['expected'], metadata['stdout'], metadata)
                if not phase:
                    catalog = policy.decode_catalog(metadata['stdout'])
                    catalogs[level] = catalog
                    current['catalog'] = common._record(catalog)
                    current['checks'] = [_observe_one(binary, check, expected, session, policy,
                        work / ('seeded-O' + level + '-' + str(check['ordinal'])), timeout_s, max_output_bytes)
                        for check in catalog]
                    phase = policy.aggregate([observation['outcome'] for observation in current['checks']])
            current['outcome'] = OUTCOMES[phase]
            codes.append(phase)
        # Every offered tuple and acceptance decision comes from Coax. Replays
        # call the same retained body in a fresh process with honest SP1 progress.
        if not policy.aggregate(codes):
            check = catalogs[optimization][0]
            first = _raw(result['profiles']['-O' + optimization]['checks'][0])
            values = first['values']
            signature = {'optimization': int(optimization), 'identity': check, 'kind': expected['kind'],
                'line': expected['line'], 'source': str(expected['source'])
                    if not isinstance(expected['source'], bytes) else expected['source'], 'seed_id': seed_id}
            state = policy.reduce_start(check, values, 32)
            result['initial_values'] = values
            binary = Path(result['profiles']['-O' + optimization]['binary'])
            accepted_observation = first
            accepted_reference = {'profile': '-O' + optimization, 'check': 0, 'replay': None}
            accepted_index = None
            while not state['done'] and not state['reason']:
                candidate = policy.reduce_next(state, check, values)
                state = candidate['state']
                if not candidate['offered']:
                    continue
                replay = _observe_one(binary, check, expected, session, policy,
                    work / ('replay-' + str(state['attempts'])), timeout_s, max_output_bytes,
                    index=candidate['ordinal'])
                probe_code = policy.probe(replay['native_outcome'])
                codes.append(probe_code)
                if probe_code or replay['process'] is None:
                    result['replays'].append({'candidate': candidate, 'observation': replay, 'accepted': False})
                    result['reduction_interruption'] = replay
                    break
                trap = _raw(replay['trap'])
                actual = {**signature, 'kind': trap['kind'], 'line': trap['line'], 'source': trap['source']}
                accepted = policy.accept(signature, actual, replay)
                result['replays'].append({'candidate': candidate, 'observation': replay, 'accepted': accepted})
                if accepted:
                    values = candidate['values']
                    accepted_observation = _raw(replay)
                    accepted_reference = {'profile': '-O' + optimization, 'check': 0,
                                          'replay': len(result['replays']) - 1}
                    accepted_index = candidate['ordinal']
            # Revalidate the retained accepted observation; never promote a
            # proposed tuple that was not observed in the bound executable.
            verified = policy.classify(check, expected, accepted_observation['progress'],
                accepted_observation['process'], replay_index=accepted_index)
            actual = {**signature, 'kind': verified['trap']['kind'],
                      'line': verified['trap']['line'], 'source': verified['trap']['source']}
            accepted_final = policy.accept(signature, actual, verified)
            codes += [policy.reduce_status(state, len(check['bindings'])),
                      0 if accepted_final else 3]
            values = verified['values']
            result.update(reduced_values=values, reduction_state=state, signature=common._record(signature))
            result['reduced_witness'] = {'observation': accepted_reference,
                'coax_signature_accepted': accepted_final, 'case_index': verified['case_index'],
                'witness': policy.witness(check, values), 'values': values,
                'classification': common._record(verified)}
            result['seeded_native_outcome'] = first['native_outcome']
        codes += _artifact_codes(policy, protected, chain)
        codes += [policy.integrity(profile['binary_sha256'], common._sha(profile['binary']))
                  for profile in result.get('profiles', {}).values()]
        result['artifact_inputs'] = protected
        result['outcome'] = OUTCOMES[policy.aggregate(codes)]
    except PolicyUnavailable as error:
        result.update(why=str(error), policy_loaded=False, policy_bootstrap=common._record(error.observations))
    except (OSError, ValueError, UnicodeError, KeyError, IndexError, TypeError) as error:
        result.update(why=str(error), startup_or_transport_error=type(error).__name__)
    finally:
        result['elapsed_s'] = time.monotonic() - start
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('files', nargs='+')
    parser.add_argument('--keep')
    parser.add_argument('--compiler')
    parser.add_argument('--only')
    parser.add_argument('--steps', type=int)
    parser.add_argument('--out')
    args = parser.parse_args(argv)
    record = run(args.files, keep=args.keep, compiler=args.compiler, only=args.only, steps=args.steps)
    if args.out:
        common.write_report(args.out, record, compiler=args.compiler)
    print(record['outcome'])
    return 0 if record['outcome'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
