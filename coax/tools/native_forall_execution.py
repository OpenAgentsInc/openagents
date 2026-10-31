"""Optional bounded ABI2 execution; Coax owns domains, progress and verdicts."""
import copy
import hashlib
import os
from pathlib import Path
import shlex
import shutil
import tempfile
import time

import native_check_execution as common
import native_execution_identities as identities
from native_execution_policy import ROOT, REPO, OUTCOMES, PolicyUnavailable, digest
from native_forall_policy import ForallPolicy

SCHEMA = 'coax.native-checks.execution.v2'
OBSERVER = ROOT / 'tools/native-forall-observer.c'
_POLICIES = {}


def get_policy(compiler=None, directory=None, deadline=None, _runner=None):
    compiler = common._compiler(compiler)
    deadline = deadline if deadline is not None else time.monotonic() + 840
    inputs = [ROOT / 'lib/compiler' / name for name in ForallPolicy.input_names]
    key = (common._sha(compiler), tuple(common._sha(path) for path in inputs),
           os.environ.get('COAX_EXECUTION_HOST_CC'), os.environ.get('CC'), os.environ.get('SDKROOT'))
    cached = _POLICIES.get(key)
    if cached is None or not all(path.exists() for path in (cached.c, cached.header, cached.library)):
        target = Path(tempfile.mkdtemp(prefix='coax-quantified-policy-')) / 'policy'
        cached = ForallPolicy(compiler, target, deadline, runner=_runner or common.Session(deadline).run)
        _POLICIES[key] = cached
    if directory is None:
        return cached
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=False)
    result = copy.copy(cached)
    result.directory = directory
    result.identity = dict(cached.identity)
    for name in ('c', 'header', 'library'):
        destination = directory / getattr(cached, name).name
        shutil.copyfile(getattr(cached, name), destination)
        setattr(result, name, destination)
        result.identity[name] = str(destination)
    result.identity['retained_bootstrap'] = dict(cached.identity)
    return result


def _observe(binary, checks, session, policy, timeout_s, output_bytes, directory):
    observed = []
    binary_sha = common._sha(binary)
    for check in checks:
        progress = directory / ('progress-' + str(check['ordinal']) + '.bin')
        initial = policy.initial_region()
        reserved = len(initial) + 1
        admission = policy.capture(session.remaining_bytes, reserved)
        if admission:
            identity = common._identity(check)
            observed.append({'outcome': OUTCOMES[admission], 'outcome_code': admission,
                'valid': False, 'identity': identity, 'source': identity['source'],
                'case_count': check['cases'], 'completed_cases': 0, 'failing_case': None,
                'values': [], 'witness': '', 'process': None,
                'binding_names': [binding['name'].decode('utf-8', 'strict') for binding in check['bindings']],
                'progress_capture_admission': {'outcome': OUTCOMES[admission],
                    'remaining_bytes': session.remaining_bytes, 'requested_bytes': reserved},
                'scope': 'Progress capture was refused before launching this property; no process observation is invented.'})
            continue
        with progress.open('xb') as stream:
            stream.write(initial)
        session.remaining_bytes -= reserved
        process = session.run([str(binary), 'one', str(check['ordinal']), str(progress)],
                              timeout_s=timeout_s, max_output_bytes=output_bytes)
        with progress.open('rb') as stream:
            region = stream.read(reserved)
        session.remaining_bytes += reserved - len(region)
        entry = policy.observe(check, region, process)
        code = policy.aggregate([entry['outcome_code'], policy.integrity(binary_sha, common._sha(binary))])
        entry['outcome'], entry['outcome_code'] = OUTCOMES[code], code
        identity = common._identity(check)
        entry.update({'identity': identity, 'source': identity['source'],
                      'binding_names': [binding['name'].decode('utf-8', 'strict') for binding in check['bindings']],
                      'process': common._record(process), 'progress_path': str(progress),
                      'progress': common._record(region), 'recognized_trap': False})
        entry['witness'] = policy.witness(check, entry['values']) if entry['valid'] else ''
        observed.append(entry)
    return {'outcome': OUTCOMES[policy.aggregate([item['outcome'] for item in observed])],
            'checks': observed, 'scope': 'Artifact observations only; no original baseline or qualification is invented.'}


def observe_artifact(binary, checks, *, timeout_s=10.0, max_output_bytes=1048576,
                     deadline=None, policy=None):
    deadline = deadline if deadline is not None else time.monotonic() + 840
    policy = policy or get_policy(deadline=deadline)
    if not policy.budget(timeout_s, max_output_bytes):
        raise ValueError('execution bounds were refused by Coax policy')
    directory = Path(tempfile.mkdtemp(prefix='coax-quantified-progress-'))
    return _observe(Path(binary).resolve(), checks, common.Session(deadline), policy,
                    timeout_s, max_output_bytes, directory)


def run(files, *, keep=None, steps=None, only=None, timeout_s=10.0,
        max_output_bytes=1048576, cc=None, compiler=None, abi=2):
    return _run_profile(files, keep=keep, steps=steps, only=only, timeout_s=timeout_s,
        max_output_bytes=max_output_bytes, cc=cc, compiler=compiler, abi=abi,
        schema=SCHEMA, observer=OBSERVER, policy_factory=get_policy, observe=_observe)


def _run_profile(files, *, keep, steps, only, timeout_s, max_output_bytes, cc,
                 compiler, abi, schema, observer, policy_factory, observe,
                 extra_transport=()):
    # Shared host effects for explicitly versioned verification companions.
    # Each caller supplies its Coax policy and fixed observer; baseline gates
    # and the default ABI2 contract are identical for every strict profile.
    started = time.monotonic()
    deadline = started + 840
    compiler = common._compiler(compiler)
    work = Path(keep) if keep is not None else Path(tempfile.mkdtemp(prefix='coax-native-forall-'))
    if keep is not None:
        work.mkdir(parents=True, exist_ok=False)
    work = work.resolve()
    session = common.Session(deadline)
    result = {'schema': schema, 'scope': 'Complete original evaluator baseline and selected ABI2 native execution',
              'selection': {'only': only, 'baseline_only': None, 'baseline_list': False},
              'verification_abi': abi, 'compiler': str(compiler), 'compiler_sha256': common._sha(compiler),
              'artifacts': str(work), 'files': [], 'outcome': 'infrastructure-error'}
    transport = [Path(__file__), ROOT / 'tools/native_forall_policy.py', ROOT / 'tools/native_check_execution.py',
                 ROOT / 'tools/native_execution_policy.py', ROOT / 'tools/bounded_process.py',
                 ROOT / 'tools/native_execution_identities.py', ROOT / 'tests/quiet_crash.py',
                 observer, observer.with_suffix('.h'), ROOT / 'tools/native-forall-progress.h', *extra_transport]
    result['transport_sources'] = {str(path): common._sha(path) for path in transport}
    try:
        policy = policy_factory(compiler, work / 'policy', deadline, _runner=session.run)
        result['policy'], result['policy_bootstrap'] = policy.identity, common._record(policy.observations)
        if not policy.abi(abi) or not policy.budget(timeout_s, max_output_bytes):
            result['why'] = 'ABI or execution bounds were refused by Coax policy before original-source execution'
            return result
        requested_steps = policy.limits.steps if steps is None else steps
        if not policy.step_request(requested_steps):
            result['why'] = 'original evaluator step budget was refused by Coax policy before execution'
            return result
        version = session.run([str(compiler), 'version'])
        result['compiler_version_execution'] = common._record(version)
        version_code = policy.phase(version)
        if version_code:
            result['outcome'] = OUTCOMES[version_code]
            return result
        compiler_version = version['stdout'].decode('utf-8', 'strict').strip()
        result['compiler_version'] = compiler_version
        native_cc = list(cc) if isinstance(cc, (list, tuple)) else shlex.split(cc or os.environ.get('COAX_NATIVE_CC', 'cc'))
        result['native_cc'] = native_cc
        chain, chain_codes = None, []
        for file_index, filename in enumerate(files):
            source = Path(filename).resolve()
            directory = work / ('file-' + str(file_index))
            directory.mkdir()
            source_sha = common._sha(source)
            item = {'file': str(source), 'source_sha256': source_sha, 'baseline': None,
                    'native_coverage': [], 'profiles': {}, 'outcome': 'not-checked'}
            result['files'].append(item)
            baseline_argv = [str(compiler), 'test', str(source), '--library', '--arithmetic', 'checked',
                             '--evidence', '/dev/stderr', '--profile-report', '/dev/stdout',
                             '--steps', str(requested_steps)]
            baseline_process = session.run(baseline_argv, timeout_s=policy.limits.baseline_ms / 1000,
                                           max_output_bytes=policy.limits.baseline_output_bytes)
            baseline, baseline_manifest = common._baseline_records(baseline_process)
            item.update({'baseline': baseline, 'baseline_input_manifest': baseline_manifest,
                         'baseline_execution': common._record(baseline_process)})
            validation = policy.baseline(baseline, baseline_process, source, source_sha,
                                         result['compiler_sha256'], compiler_version, requested_steps)
            item['baseline_validation'] = validation
            if validation['outcome'] != 'passed':
                item['outcome'] = validation['outcome']
                continue
            input_code = policy.baseline_inputs(baseline, baseline_manifest, baseline_process, compiler_version)
            item['baseline_input_validation'] = OUTCOMES[input_code]
            if input_code:
                item['outcome'] = OUTCOMES[input_code]
                continue
            selection = policy.select(baseline, only)
            item['native_coverage'] = common._coverage(baseline, selection)
            item['native_selection'] = {name: value for name, value in selection.items() if name not in ('identities', 'expected')}
            item['checks'] = [common._identity(value) for value in selection['identities']]
            if selection['outcome'] != 'passed':
                item['outcome'] = selection['outcome']
                continue
            emission_argv = [str(compiler), 'checks-c', str(source), '--verification-abi', '2',
                             '--arithmetic', 'checked', '--profile-report', '/dev/stderr']
            if only is not None:
                emission_argv += ['--only', only]
            emission = session.run(emission_argv, timeout_s=policy.limits.compile_ms / 1000,
                                   max_output_bytes=policy.limits.baseline_output_bytes)
            manifest = common._last_json(emission)
            c_file = directory / 'checks.c'
            c_file.write_bytes(emission['stdout'])
            c_sha = digest(c_file)
            codes = [policy.emission(manifest, baseline, emission, compiler_version, c_sha),
                     policy.input_descriptors(baseline_manifest, manifest)]
            item.update({'emission_execution': common._record(emission), 'emission_manifest': manifest,
                         'artifact': {'c': str(c_file), 'c_sha256': c_sha,
                             'observer': str(observer), 'observer_sha256': digest(observer),
                             'observer_header': str(observer.with_suffix('.h')), 'observer_header_sha256': digest(observer.with_suffix('.h')),
                             'policy_c': str(policy.c), 'policy_c_sha256': digest(policy.c),
                             'policy_header': str(policy.header), 'policy_header_sha256': digest(policy.header)}})
            if chain is None:
                chain = identities.inspect(native_cc, session.run, cwd=REPO, environment=session.environment)
                chain_codes = common._toolchain_outcomes(policy, chain)
                result['toolchain'] = common._record(chain)
                if policy.aggregate(chain_codes) == 0 and chain['sdk'].get('environment'):
                    session.environment = {**os.environ, **chain['sdk']['environment']}
            codes += chain_codes
            if policy.aggregate(codes):
                item['outcome'] = OUTCOMES[policy.aggregate(codes)]
                continue
            for optimization in ('-O0', '-O2'):
                binary = directory / ('native-' + optimization[1:])
                profile = {'binary': str(binary), 'binary_sha256': '', 'outcome': 'not-checked'}
                item['profiles'][optimization] = profile
                compiled = session.run([*native_cc, '-std=c11', optimization, '-ffp-contract=off', '-fno-fast-math',
                    '-I', str(policy.directory), str(c_file), str(policy.c), str(observer), '-lm', '-o', str(binary)],
                    timeout_s=policy.limits.compile_ms / 1000, max_output_bytes=max_output_bytes)
                profile['compile'] = common._record(compiled)
                profile['binary_sha256'] = common._sha(binary)
                profile_codes = [policy.phase(compiled), policy.integrity(c_sha, common._sha(c_file)),
                    policy.integrity(item['artifact']['observer_sha256'], common._sha(observer)),
                    policy.integrity(item['artifact']['observer_header_sha256'], common._sha(observer.with_suffix('.h'))),
                    policy.integrity(item['artifact']['policy_c_sha256'], common._sha(policy.c)),
                    policy.integrity(item['artifact']['policy_header_sha256'], common._sha(policy.header)),
                    policy.integrity(profile['binary_sha256'], common._sha(binary))]
                if not policy.aggregate(profile_codes):
                    metadata = session.run([str(binary), 'metadata'], timeout_s=policy.limits.metadata_ms / 1000,
                                           max_output_bytes=policy.limits.metadata_bytes)
                    profile['metadata'] = common._record(metadata)
                    metadata_code = policy.metadata(selection['expected'], metadata['stdout'], metadata)
                    profile_codes.append(metadata_code)
                    if metadata_code == 0:
                        catalog = policy.decode_catalog(metadata['stdout'])
                        profile['catalog'] = common._record(catalog)
                        progress_dir = directory / (optimization[1:] + '-progress')
                        progress_dir.mkdir()
                        observed = observe(binary, catalog, session, policy, timeout_s, max_output_bytes, progress_dir)
                        profile['execution'] = observed
                        profile_codes.append(OUTCOMES.index(observed['outcome']))
                profile['outcome'] = OUTCOMES[policy.aggregate(profile_codes)]
                codes.append(OUTCOMES.index(profile['outcome']))
            final = policy.baseline(baseline, baseline_process, source, source_sha,
                                    result['compiler_sha256'], compiler_version, requested_steps)
            item['final_source_validation'] = final
            codes += [OUTCOMES.index(final['outcome']),
                policy.baseline_inputs(baseline, baseline_manifest, baseline_process, compiler_version),
                policy.input_descriptors(baseline_manifest, manifest),
                policy.integrity(result['compiler_sha256'], common._sha(compiler))]
            codes += [policy.integrity(expected, common._sha(path)) for path, expected in policy.identity['sources'].items()]
            codes.append(policy.integrity(policy.identity['library_sha256'], common._sha(policy.library)))
            codes += [policy.integrity(expected, common._sha(path)) for path, expected in result['transport_sources'].items()]
            codes.append(policy.integrity(chain.get('program_sha256') or '', common._sha(chain.get('resolved_program') or '')))
            item['outcome'] = OUTCOMES[policy.aggregate(codes)]
        result['outcome'] = OUTCOMES[policy.aggregate([item['outcome'] for item in result['files']])]
    except PolicyUnavailable as error:
        result['why'], result['policy_bootstrap'], result['policy_loaded'] = str(error), common._record(error.observations), False
    except (OSError, ValueError, UnicodeError) as error:
        result['why'], result['startup_or_transport_error'] = str(error), type(error).__name__
    finally:
        result['elapsed_s'] = time.monotonic() - started
        result['compiler_sha256_after'] = common._sha(compiler)
    return result
