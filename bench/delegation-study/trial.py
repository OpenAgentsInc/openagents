#!/usr/bin/env python3
"""Coordinate one registered trial on one host; existing attempts are inspect-only."""
from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import signal
import shutil
import stat
import subprocess
import sys
import time
import uuid

import candidate
import report
import schedule
from seed_manifest import cargo_features, feature_check_command

SCHEMA = "openagents.delegation.trial.v1"
NATIVE_COMMON = {"provider_meter", "toolchain", "timeout_s", "cli_budget_usd", "capture_limits", "initialize_git"}
DERIVED_ACCEPTANCE = {"run_id", "source_commit", "source_archive", "source_archive_sha256", "candidate_dir",
                      "candidate_manifest_sha256", "target_seed", "target_seed_manifest_sha256", "expected_snapshot_commit"}


def durable(path, value):
    """Replace one small receipt atomically; the launch journal remains append-only."""
    path = Path(path)
    temporary = path.with_name(path.name + ".new")
    with temporary.open("x") as output:
        json.dump(value, output, sort_keys=True, indent=2, allow_nan=False)
        output.write("\n")
        output.flush()
        os.fsync(output.fileno())
    os.replace(temporary, path)
    descriptor = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def event(output, value):
    with (output / "launches.jsonl").open("a") as stream:
        stream.write(json.dumps(value, sort_keys=True, allow_nan=False) + "\n")
        stream.flush()
        os.fsync(stream.fileno())


def reference(root, path):
    path = Path(path).resolve(strict=True)
    relative = path.relative_to(root)
    hasher = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            hasher.update(chunk)
        os.fsync(source.fileno())
    return {"path": relative.as_posix(), "sha256": hasher.hexdigest()}


def process_phase(name, argv, output, timeout_s):
    """One subprocess launch; a timeout never implies that a paid call was absent."""
    with (output / (name + ".stdout.log")).open("xb") as stdout, (output / (name + ".stderr.log")).open("xb") as stderr:
        process = subprocess.Popen(argv, cwd=output, stdout=stdout, stderr=stderr, start_new_session=True)
        try:
            return {"exit_code": process.wait(timeout=timeout_s), "timed_out": False}
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            return {"exit_code": process.returncode, "timed_out": True}
        except BaseException:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            raise


def inspect(output, run_id=None):
    """Never continue or infer no charge from an incomplete retained attempt."""
    output = Path(output)
    path = output / "trial.json"
    if path.is_file():
        value = json.loads(path.read_text())
        if run_id is not None and value.get("run_id") != run_id:
            raise ValueError("The existing attempt has a different run identity")
        return {**value, "inspection_only": True, "replayed": False}
    return {"schema": SCHEMA, "run_id": run_id, "status": "incomplete", "inspection_only": True,
            "replayed": False, "accounting_complete": False, "cost_upper_usd": None,
            "reason": "Retained directory has no final receipt; inspect launch records without relaunching"}


def native_arguments(config):
    tail = ['-p', '--output-format', 'stream-json', '--verbose', '--dangerously-skip-permissions',
            '--no-session-persistence', '--model', config['model'], '--effort', config['effort'],
            '--max-budget-usd', str(config.get('cli_budget_usd', 2))]
    if config.get('system_file'):
        tail += ['--system-prompt-file', '/opt/run/system.txt']
    if config.get('tools') is not None:
        tail += ['--tools', config['tools']]
    return tail


def cleanup_targets(output, native_closed, acceptance_closed, reconstruction_ready=False):
    """Remove owned copies only after closure and durable reconstruction evidence."""
    def removal_error(_operation, path, error_info):
        if Path(path) == target or not isinstance(error_info[1], FileNotFoundError):
            raise error_info[1]

    rows = []
    for name, closed in (('native',native_closed),('acceptance',acceptance_closed)):
        if not closed:
            continue
        paths = ['home/target'] + (['workspace'] if reconstruction_ready else [])
        for relative in paths:
            target = output/name/relative
            if not target.exists() and not target.is_symlink():
                continue
            row = {'path':name+'/'+relative,'removed':False}
            try:
                parents = [output/name] + ([output/name/'home'] if relative == 'home/target' else [])
                if target.is_symlink() or any(p.is_symlink() for p in parents):
                    raise ValueError('An owned cleanup path became a symlink')
                if target.resolve() != target or output not in target.parents or not target.is_dir():
                    raise ValueError('The cleanup directory is outside the attempt or invalid')
                if not shutil.rmtree.avoids_symlink_attacks:
                    raise ValueError('This host lacks protected directory removal')
                shutil.rmtree(target, onerror=removal_error)
                row['removed'] = True
            except (OSError,ValueError) as error:
                row['error_type'] = type(error).__name__
            rows.append(row)
    return rows


def retain_logs(root, output):
    """Hash and sync logs without copying their private contents into receipts."""
    refs = []
    for directory in (output,output/'native',output/'acceptance'):
        if not directory.exists():
            continue
        for path in sorted(directory.iterdir()):
            if path.suffix not in ('.log','.jsonl','.stdout','.stderr'):
                continue
            if path.is_symlink() or not path.is_file():
                raise ValueError('A private process log is not a regular file')
            refs.append(reference(root,path))
    return refs


def release_native_scratch(root, output, native, candidate_identity, clock=time.monotonic_ns):
    """Retain reconstruction evidence before freeing closed native scratch."""
    if native.get('execution_closed') is not True:
        raise ValueError('Native execution closure is unconfirmed')
    artifacts = {}
    for key, name in (('native','result.json'), ('provider_calls','provider-calls.jsonl'),
                      ('candidate_manifest','candidate-manifest.json'),
                      ('candidate_payload','candidate.tar.gz'), ('candidate_changes','changes.json')):
        artifacts[key] = reference(root, output/'native'/name)
    if artifacts['candidate_manifest']['sha256'] != candidate_identity:
        raise ValueError('The candidate changed before scratch removal')
    logs = retain_logs(root, output)
    descriptor = os.open(output/'native', os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    retained = {'schema':'openagents.delegation.native-retention.v1',
                'candidate_manifest_sha256':candidate_identity, 'execution_closed':True,
                'artifacts':artifacts, 'private_logs':logs}
    durable(output/'native-retention.json', retained)
    started = clock()
    receipt = {'schema':'openagents.delegation.scratch-release.v1', 'status':'incomplete',
               'inside_primary_endpoint':True, 'start_monotonic_ns':started,
               'retention':reference(root, output/'native-retention.json'),
               'free_bytes_before':shutil.disk_usage(output).free, 'targets':[]}
    durable(output/'native-scratch-release.json', receipt)
    try:
        receipt['targets'] = cleanup_targets(output, True, False, True)
        receipt['status'] = 'complete' if all(row['removed'] for row in receipt['targets']) else 'failed'
    finally:
        receipt['end_monotonic_ns'] = clock()
        receipt['wall_s'] = (receipt['end_monotonic_ns']-started)/1e9
        receipt['free_bytes_after'] = shutil.disk_usage(output).free
        durable(output/'native-scratch-release.json', receipt)
    return receipt


def eligibility(root, registration, registration_sha, run_id):
    """Admit complete blocks in fixed order using retained, uniquely charged work."""
    cells = registration['schedule']
    position = next(i for i, cell in enumerate(cells) if cell['run_id'] == run_id)
    ledger = list(registration.get('accounting', []))
    prior = []
    for cell in cells[:position]:
        path = root/'runs'/cell['run_id']/'trial.json'
        if not path.is_file():
            raise ValueError('An earlier assigned trial has not finished')
        row = json.loads(path.read_text())
        if (row.get('registration_sha256') != registration_sha or row.get('run_id') != cell['run_id']
                or row.get('cell') != cell or row.get('status') not in ('complete', 'failed')
                or row.get('execution_closed') is not True or row.get('accounting_complete') is not True
                or not report.number(row.get('cost_upper_usd'))):
            raise ValueError('An earlier trial has unresolved execution or accounting')
        if (row.get('failed_stage') == 'native_scratch_release'
                or any(not item.get('removed') for item in row.get('cleanup', {}).get('targets', []))):
            raise ValueError('An earlier trial has unresolved scratch cleanup')
        ledger.append({'run_id':cell['run_id'],'cost_upper_usd':row['cost_upper_usd']})
        prior.append(reference(root, path))
    block_start = position - position % 6
    block_cells = cells[block_start:block_start+6]
    if len(block_cells) != 6 or len({c['block'] for c in block_cells}) != 1:
        raise ValueError('A complete six-arm block is required')
    admission_root = root/'block-admissions'
    admission_root.mkdir(exist_ok=True, mode=0o700)
    path = admission_root/(block_cells[0]['block']+'.json')
    if position % 6 == 0:
        if path.exists():
            raise ValueError('A prior block admission exists; inspect it instead of repeating admission')
        budget = schedule.admission(ledger, registration['next_block_preparation_reserve_usd'])
        if not budget['admissible']:
            raise ValueError('The full next block exceeds the remaining accounting ceiling')
        value = {'schema':'openagents.delegation.block-admission.v1','registration_sha256':registration_sha,
                 'cells':block_cells,'prior_trial_receipts':prior,'budget':budget}
        durable(path, value)
    else:
        value = json.loads(path.read_text())
        if value.get('registration_sha256') != registration_sha or value.get('cells') != block_cells:
            raise ValueError('The block lacks its original full-block admission')
        # Overshoots and unknown liabilities stop later requests, never vanish.
        reduced = schedule.admission(ledger, registration['next_block_preparation_reserve_usd'])
        known = reduced['accounted_and_reserved_upper_usd']
        remaining = (6-position % 6)*8 + registration['next_block_preparation_reserve_usd']
        if known is None or known+remaining > 120:
            raise ValueError('Observed spend no longer covers the remaining block reservation')
    return reference(root, path)


def run(registration_path, run_id, output, credential_file, **options):
    """One local coordinator owns the slot; existing UUID directories never replay."""
    root = Path(registration_path).resolve().parent
    output = Path(output).resolve()
    if str(uuid.UUID(run_id)) != run_id or output != root/'runs'/run_id:
        raise ValueError('Use the canonical registration/runs/RUN_UUID output')
    if output.exists():
        return inspect(output, run_id)
    with (root/'trial-coordinator.lock').open('a') as lock:
        try:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError('Another trial coordinator owns the execution slot') from None
        return _run(registration_path, run_id, output, credential_file, **options)


def _run(registration_path, run_id, output, credential_file, *, phase_runner=process_phase,
        validator=schedule.validate, clock=time.monotonic_ns):
    """Run once; injected phases are only for offline tests, never CLI options."""
    root = Path(registration_path).resolve().parent
    output = Path(output).resolve()
    if root not in output.parents or str(uuid.UUID(run_id)) != run_id:
        raise ValueError("Use a canonical run UUID and an output below the registration directory")
    try:
        output.mkdir(mode=0o700, parents=True, exist_ok=False)
    except FileExistsError:
        return inspect(output, run_id)
    os.chmod(output, 0o700)
    started = clock()
    state = {"schema": SCHEMA, "run_id": run_id, "status": "incomplete", "accepted": None,
             "execution_closed": False, "accounting_complete": False, "cost_upper_usd": None,
             "cost_lower_usd": 0.0, "phases": [], "errors": [], "artifacts": {},
             "registration_sha256": None}
    durable(output / "trial.json", state)
    prep = native = checks = None
    cell = task = arm = protocol = None
    native_started = native_closed = preparation_started = False
    acceptance_closed = False
    acceptance_started = candidate_validated = False
    native_release_started = False
    candidate_identity = None
    validation = None
    stage = 'registration'

    def phase(name, argv, timeout, paid_possible):
        if not report.number(timeout) or timeout <= 0:
            raise ValueError("A positive frozen phase deadline is required")
        role = {'preparation':'preparer','native':'native_runner','acceptance':'acceptance_coordinator'}[name]
        schedule.validate_harness(root,registration)
        report.artifact(root,registration['artifacts'][role],'binary')
        begin = clock()
        # This write happens before Popen or any injected phase function.
        event(output, {"phase": name, "event": "launch_intent", "at_monotonic_ns": begin,
                       "paid_call_possible": paid_possible, "run_id": run_id})
        result = phase_runner(name, argv, output, timeout)
        completed = {"phase": name, "event": "returned", "start_monotonic_ns": begin,
                     "end_monotonic_ns": clock(), **result}
        event(output, completed)
        state["phases"].append(completed)
        durable(output / "trial.json", state)
        return result

    try:
        state['registration_sha256'] = reference(root, registration_path)['sha256']
        validation_started = clock()
        validation = validator(Path(registration_path))
        state['registration_validation_s'] = (clock()-validation_started)/1e9
        durable(output / "registration-validation.json", validation)
        if validation.get("ready_for_external_dispatch") is not True:
            raise ValueError("Registration or full-block admission is not ready")
        registration = json.loads(Path(registration_path).read_text())
        if reference(root,registration_path)['sha256'] != state['registration_sha256']:
            raise ValueError('Registration changed during validation')
        if phase_runner is process_phase and (root/registration['artifacts']['dispatch_coordinator']['path']).resolve() != Path(__file__).resolve():
            raise ValueError('Run the registered coordinator executable')
        protocol = report.artifact(root, registration["protocol"])
        bindings = protocol["registration"]["report_bindings"]
        cell = next(c for c in registration["schedule"] if c["run_id"] == run_id)
        task = next(t for t in bindings["tasks"] if t["task_id"] == cell["task_id"])
        arm = bindings["arms"][cell["arm"]]
        state["cell"] = cell
        state['block_admission'] = eligibility(root, registration, state['registration_sha256'], run_id)
        stage = 'configuration'
        runtime = report.artifact(root, registration["artifacts"]["trial_config"])
        if runtime.get("schema") != "openagents.delegation.trial-config.v1":
            raise ValueError("Unexpected trial runtime schema")
        task_runtime = runtime["tasks"][cell["task_id"]]
        features = cargo_features(task.get('cargo_features',[]))
        template = report.artifact(root, task_runtime['acceptance_template'])
        if cargo_features(template.get('cargo_features',[]),template['packages']) != features:
            raise ValueError('Native and acceptance Cargo features differ')
        source_refs = registration["task_artifacts"][cell["task_id"]]
        common = dict(runtime["native_common"])
        if (set(common) - NATIVE_COMMON or common.get("cli_budget_usd") != 2
                or common.get('initialize_git', True) is not True):
            raise ValueError("Native common settings differ from the registered contract")
        build_env = common.get('toolchain',{}).get('environment',{})
        if any(build_env.get(key) != '0' for key in ('CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG')):
            raise ValueError('The common registered build profile is required')
        meter = dict(common["provider_meter"])
        if meter["admission_target_usd"] != 8 or not set(arm["allowed_models"]).issubset(meter["models"]):
            raise ValueError("Native provider admission differs from the frozen arm")
        meter['models'] = {model: meter['models'][model] for model in arm['allowed_models']}
        common['provider_meter'] = meter
        for model, policy in meter["models"].items():
            if policy["usd_per_million"] != bindings["prices"][model]:
                raise ValueError("Native metering prices differ")
        credential = Path(credential_file).resolve(strict=True)
        if not credential.is_file() or stat.S_IMODE(credential.stat().st_mode) & 0o077:
            raise ValueError("Use a private broker credential file")
        base = report.artifact(root, task["base_prompt"], "bytes")
        if features and feature_check_command(features).encode() not in base:
            raise ValueError('The common task prompt lacks its bound Cargo feature check command')
        prompt = base
        if arm["preparation"]:
            stage = 'preparation'
            preparation_started = True
            mode = "jev" if arm["system_one"] else "deterministic"
            preparer = root / registration["artifacts"]["preparer"]["path"]
            result = phase("preparation", [sys.executable, str(preparer), '--repo', task_runtime['source_repo'],
                            '--rev', task['source_commit'], '--index', str(root / source_refs['index']['path']),
                            '--issue', str(root / source_refs['issue']['path']), '--mode', mode,
                            '--output', str(output / 'preparation')], runtime['preparation_timeout_s'], arm['system_one'])
            prep = json.loads((output / 'preparation' / 'preparation.json').read_text())
            if result['timed_out'] or result['exit_code'] != 0:
                raise ValueError("Preparation process did not complete")
            expected = task['preparation']
            if any(prep.get(k) != expected[k] for k in report.PREPARATION_BINDINGS):
                raise ValueError("Prepared input or policy changed")
            if prep.get('coverage', {}).get('candidate_units') != expected['candidate_units']:
                raise ValueError("Prepared candidate count changed")
            pool_ref = reference(root, output / 'preparation' / 'candidates.json')
            if pool_ref['sha256'] != expected['candidate_sha256']:
                raise ValueError("Prepared candidate pool changed")
            pack = (output / 'preparation' / 'briefing.md').read_bytes()
            if len(pack) > 16384 or report.sha(pack) != prep['briefing_sha256'] or len(pack) != prep['briefing_bytes']:
                raise ValueError("Prepared source pack changed")
            prompt += b'\n\n' + pack
            state['artifacts'].update(preparation=reference(root, output/'preparation'/'preparation.json'),
                                      briefing=reference(root, output/'preparation'/'briefing.md'), candidate_pool=pool_ref)
        prompt_path = output / 'prompt.txt'
        with prompt_path.open('xb') as handle:
            handle.write(prompt); handle.flush(); os.fsync(handle.fileno())
        state['artifacts']['prompt'] = reference(root, prompt_path)
        archive = root / source_refs['source_archive']['path']
        config = dict(common, run_id=run_id, source_commit=task['source_commit'], source_archive=str(archive),
                      source_archive_sha256=task['source_archive_sha256'], binary=str(root/registration['artifacts']['native_cli']['path']),
                      binary_sha256=bindings['cli']['sha256'], binary_version=bindings['cli']['version'],
                      model=arm['primary_model'], effort=arm['effort'], prompt_file=str(prompt_path),
                      prompt_sha256=report.sha(prompt), credential_file=str(credential), target_seed=task_runtime['target_seed'],
                      target_seed_manifest_sha256=source_refs['target_seed_manifest']['sha256'])
        if features:config['cargo_features']=features
        arm_runtime = runtime['arms'][cell['arm']]
        if set(arm_runtime) - {'tools', 'system_prompt'}:
            raise ValueError("Unexpected native arm setting")
        config['tools'] = arm_runtime.get('tools')
        if arm['preparation']:
            system = report.artifact(root, arm_runtime['system_prompt'], 'bytes')
            if report.sha(system) != arm['system_prompt_sha256']:
                raise ValueError("Lean system prompt changed")
            config.update(system_file=str(root/arm_runtime['system_prompt']['path']), system_sha256=report.sha(system))
            state['artifacts']['system_prompt'] = arm_runtime['system_prompt']
        elif arm_runtime.get('system_prompt') is not None:
            raise ValueError("Native control has a custom system prompt")
        if native_arguments(config) != arm['argv_tail']:
            raise ValueError("Native arguments differ from sealed prompt and tools")
        # This private config contains a credential-file path, never its value.
        durable(output / 'native-config.private.json', config)
        stage = 'native'
        native_started = True
        result = phase('native', [sys.executable, str(root/registration['artifacts']['native_runner']['path']),
                                 str(output/'native-config.private.json'), str(output/'native')], runtime['native_timeout_s'], True)
        native = json.loads((output/'native'/'result.json').read_text())
        native_closed = (not result['timed_out'] and native.get('execution_closed') is True)
        if not native_closed:
            raise ValueError("Native execution closure is unconfirmed")
        report.verify_input_bindings(root,state['artifacts'],{'native':native,'preparation':prep} if prep else {'native':native},cell,arm,bindings)
        candidate_identity = native.get('candidate_manifest_sha256')
        candidate.validate(output/'native', candidate_identity, task['source_commit'], task['source_archive_sha256'])
        candidate_validated = True
        if not isinstance(native.get('snapshot_commit'), str) or len(native['snapshot_commit']) != 40:
            raise ValueError('Native base Git snapshot identity is missing')
        template = report.artifact(root, task_runtime['acceptance_template'])
        if set(template) & DERIVED_ACCEPTANCE:
            raise ValueError("Acceptance template overrides derived identities")
        if template.get('toolchain') != common.get('toolchain'):
            raise ValueError("Native and acceptance toolchains differ")
        if cargo_features(template.get('cargo_features',[]),template['packages']) != features:
            raise ValueError('Acceptance Cargo features changed')
        if template['checker']['sha256'] != source_refs['checker']['sha256']:
            raise ValueError("Acceptance checker differs from frozen task")
        acceptance = dict(template, run_id=run_id, source_commit=task['source_commit'], source_archive=str(archive),
                          source_archive_sha256=task['source_archive_sha256'], candidate_dir=str(output/'native'),
                          candidate_manifest_sha256=candidate_identity, target_seed=task_runtime['target_seed'],
                          expected_snapshot_commit=native['snapshot_commit'],
                          target_seed_manifest_sha256=source_refs['target_seed_manifest']['sha256'])
        durable(output/'acceptance-config.private.json', acceptance)
        # Acceptance reconstructs solely from retained source and candidate data.
        # Free the closed executor's copies before creating another full tree.
        stage = 'native_scratch_release'
        native_release_started = True
        released = release_native_scratch(root, output, native, candidate_identity, clock)
        state['native_scratch_release'] = released
        state['artifacts']['native_retention'] = reference(root, output/'native-retention.json')
        state['artifacts']['native_scratch_release'] = reference(root, output/'native-scratch-release.json')
        durable(output/'trial.json', state)
        if released['status'] != 'complete':
            raise ValueError('Native scratch cleanup failed; acceptance was not launched')
        stage = 'acceptance'
        acceptance_started = True
        checked = phase('acceptance', [sys.executable, str(root/registration['artifacts']['acceptance_coordinator']['path']),
                            str(output/'acceptance-config.private.json'), str(output/'acceptance')], runtime['acceptance_timeout_s'], False)
        checks = json.loads((output/'acceptance'/'checks.json').read_text())
        acceptance_closed = (not checked['timed_out'] and checked['exit_code'] >= 0
                             and checks.get('execution_closed') is True)
        if (checked['timed_out'] or checks.get('schema') != 'openagents.delegation.final-checks.v1'
                or checks.get('run_id') != run_id or checks.get('candidate_manifest_sha256') != candidate_identity
                or cargo_features(checks.get('cargo_features',[])) != features
                or checks.get('completed') is not True or not acceptance_closed):
            raise ValueError("Independent final checks are incomplete or name another candidate")
        values = [checks.get(k, {}).get('passed') for k in ('scope','format','ordinary','independent')]
        if any(type(v) is not bool for v in values):
            raise ValueError("Final acceptance outcomes are missing")
        state['accepted'] = all(values)
        state['status'] = 'complete'
    except BaseException as error:
        state['status'] = 'incomplete' if (native_started and not native_closed) or (acceptance_started and not acceptance_closed) else 'failed'
        state['failed_stage'] = stage
        state['errors'].append(type(error).__name__)
    finally:
        state['execution_closed'] = (not native_started or native_closed) and (not acceptance_started or acceptance_closed)
        for key, relative in (('native','native/result.json'), ('provider_calls','native/provider-calls.jsonl'),
                              ('candidate_manifest','native/candidate-manifest.json'), ('candidate_payload','native/candidate.tar.gz'),
                              ('candidate_changes','native/changes.json'), ('checks','acceptance/checks.json'),
                              ('native_retention','native-retention.json'), ('native_scratch_release','native-scratch-release.json'),
                              ('preparation','preparation/preparation.json'), ('system_one_call','preparation/jev-call.json')):
            path = output/relative
            if path.is_file():
                try:
                    state['artifacts'][key] = reference(root, path)
                except (OSError, ValueError):
                    state['errors'].append('artifact_retention_failed_'+key)
                    state['status'] = 'incomplete'
        # Missing paid-phase receipts remain unbounded, even if process launch failed.
        provider = None
        if native_started and 'provider_calls' in state['artifacts'] and arm and protocol:
            try:
                text = report.artifact(root, state['artifacts']['provider_calls'], 'text')
                provider = report.provider_cost(text, run_id, arm['allowed_models'], protocol['registration']['report_bindings']['prices'])
            except (ValueError, OSError, TypeError, KeyError):
                state['errors'].append('provider_accounting_unavailable')
        native_low = provider['lower_usd'] if provider else 0.0
        native_high = provider['upper_usd'] if provider and native_closed else 0.0 if not native_started else None
        if isinstance(native, dict) and report.number(native.get('cost_usd')) and native_high is not None and native['cost_usd'] > native_high + 1e-9:
            native_high = None
            state['errors'].append('native_cost_exceeds_provider_upper')
        jev_low, jev_high = 0.0, 0.0
        if preparation_started and arm and arm['system_one']:
            try:
                if prep is None:
                    prep_path = output/'preparation'/'preparation.json'
                    if prep_path.is_file():
                        prep = json.loads(prep_path.read_text())
                    else:
                        call = json.loads((output/'preparation'/'jev-call.json').read_text())
                        prep = {'mode':'jev','system_one':call}
                jev_low, jev_high, _ = report.preparation_cost(prep, protocol, True)
            except (ValueError, OSError, TypeError, KeyError, AttributeError):
                jev_high = None
        state['cost_lower_usd'] = native_low + jev_low
        state['cost_upper_usd'] = native_high + jev_high if native_high is not None and jev_high is not None else None
        state['accounting_complete'] = state['cost_upper_usd'] is not None
        if checks is None:
            checks = {'schema':'openagents.delegation.final-checks.v1','run_id':run_id,
                      'candidate_manifest_sha256':candidate_identity,'completed':False,
                      **{k:{'passed':None,'status':'unavailable'} for k in ('scope','format','ordinary','independent')}}
            durable(output/'checks-unavailable.json', checks)
            state['artifacts']['checks'] = reference(root, output/'checks-unavailable.json')
        logs_retained = False
        try:
            state['private_logs'] = retain_logs(root,output)
            logs_retained = True
        except (ValueError,OSError):
            state['errors'].append('private_log_retention_failed')
            state['status'] = 'incomplete'
        entry = {'run_id':run_id,'arm':cell['arm'] if cell else None,'category':'scored',
                 'bindings':{'native':{key:native.get(key) for key in report.NATIVE_BINDINGS} if isinstance(native,dict) else {},
                             'preparation':{}},'artifacts':dict(state['artifacts'])}
        durable(output/'report-entry.json',entry)
        endpoint = {'schema':'openagents.delegation.endpoint.v1','run_id':run_id,
                    'candidate_manifest_sha256':candidate_identity,'clock_id':'single-host-monotonic-'+run_id,
                    'start_monotonic_ns':started,'end_monotonic_ns':clock(),'execution_closed':state['execution_closed']}
        durable(output/'endpoint.json', endpoint)
        state['artifacts']['endpoint'] = reference(root, output/'endpoint.json')
        entry['artifacts']['endpoint'] = state['artifacts']['endpoint']
        durable(output/'report-entry.json',entry)
        state['report_entry'] = reference(root,output/'report-entry.json')
        state['endpoint_wall_s'] = (endpoint['end_monotonic_ns']-started)/1e9
        durable(output/'trial.json', state)
        cleanup_started = clock()
        reconstruction_ready = (candidate_validated and logs_retained
                               and all(key in state['artifacts'] for key in ('candidate_manifest','candidate_payload','candidate_changes','checks','endpoint')))
        # A failed pre-acceptance release is retained, never silently retried.
        state['cleanup'] = {'outside_primary_endpoint':True,'targets':cleanup_targets(output,native_closed and not native_release_started,acceptance_closed,reconstruction_ready),
                            'wall_s':(clock()-cleanup_started)/1e9}
        durable(output/'trial.json',state)
    return state


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--registration', type=Path)
    parser.add_argument('--run-id')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--credential-file', type=Path)
    parser.add_argument('--inspect', action='store_true', help='Inspect only; never resume an incomplete paid phase')
    args = parser.parse_args()
    def interrupted(signum, frame):
        raise InterruptedError('Trial interrupted')
    signal.signal(signal.SIGTERM, interrupted)
    if args.inspect:
        result = inspect(args.output, args.run_id)
    else:
        if not all((args.registration, args.run_id, args.credential_file)):
            parser.error('Execution requires registration, run ID, and a private credential file')
        result = run(args.registration, args.run_id, args.output, args.credential_file)
    print(json.dumps({key:result.get(key) for key in ('run_id','status','accepted','accounting_complete','cost_lower_usd','cost_upper_usd','inspection_only')}))
    raise SystemExit(0 if result.get('status') == 'complete' else 1)


if __name__ == '__main__':
    main()
