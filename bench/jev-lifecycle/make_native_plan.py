#!/usr/bin/env python3
"""Freeze twelve exploratory pilot configs before any native task execution."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import uuid

import native_pilot as pilot
from seed_manifest import digest, validate_seed

BINARY = '/usr/local/lib/node_modules/@anthropic-ai/claude-code/bin/claude.exe'
BINARY_HASH = '0298068b686e7fdbaf9402a7a587bb7f49c0b0e084de09f69145a0719207640c'
BINARY_VERSION = '2.1.288 (Claude Code)'
TASKS = {
    'alternative-beta': ('7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6', 'ee58a16d8975ee63b1f7a6911ee63e990a1e0355d0456c9b83da7febfea1ef01'),
    'alternative-gamma': ('c427943a5c84ba5938a3549f24b27de551812a37', '575a283ba8d4e09005b75ad8d81977eb532cc6c0bf2d47eaf80bca0bb62fbb9f'),
}
RATES = {
    'claude-sonnet-5-5': dict(input=2, output=10, cache_write_5m=2.5, cache_write_1h=4, cache_read=.2),
    'claude-haiku-4-5': dict(input=1, output=5, cache_write_5m=1.25, cache_write_1h=2, cache_read=.1),
    'claude-haiku-4-5-20251001': dict(input=1, output=5, cache_write_5m=1.25, cache_write_1h=2, cache_read=.1),
}
# Exposed development cases, fixed order; this is not a held-out efficacy panel.
ORDER = [
    ('alternative-beta', 1, ('bare', 'deterministic', 'jev')),
    ('alternative-gamma', 1, ('jev', 'bare', 'deterministic')),
    ('alternative-beta', 2, ('deterministic', 'jev', 'bare')),
    ('alternative-gamma', 2, ('bare', 'deterministic', 'jev')),
]


def ref(path):
    return {'path': str(path), 'sha256': digest(path)}


def make(root, output):
    root, output = Path(root).resolve(), Path(output).resolve()
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    binary = Path(BINARY)
    if digest(binary) != BINARY_HASH or subprocess.check_output([str(binary), '--version'], text=True, timeout=20).strip() != BINARY_VERSION:
        raise ValueError('The pinned native CLI changed')
    probe_path = root / 'probe/result.json'
    probe = pilot.read_json(probe_path)
    if (probe.get('model_completed') is not True or probe.get('execution_closed') is not True
            or probe.get('provider_accounting_complete') is not True
            or not pilot.finite(probe.get('provider_cost_usd'))
            or probe.get('cli_sha256') != BINARY_HASH or probe.get('cli_hash_after') != BINARY_HASH
            or probe.get('cli_version') != BINARY_VERSION or probe.get('model') != 'claude-sonnet-5-5'
            or 'claude-sonnet-5-5' not in probe.get('served_models', [])
            or any(model not in RATES for model in probe.get('served_models', []))):
        raise ValueError('The bound capability probe is incomplete or has different identities')
    modules = pilot.module_hashes()
    templates = {}
    for alias, (commit, checker_sha) in TASKS.items():
        inputs = root / 'inputs' / alias
        public = pilot.read_json(inputs / 'task.json')
        if len(public['tasks']) != 1:
            raise ValueError('Expected one safe public task per input')
        task = public['tasks'][0]
        if task['source_commit'] != commit or task['id'] != alias:
            raise ValueError('The selected task changed')
        seed_root = root / 'seeds' / alias
        seed = seed_root / 'seed'
        seed_value = pilot.read_json(seed / 'seed-manifest.json')
        seed_config = pilot.read_json(inputs / 'seed-config.json')
        archive = seed_root / 'source.tar'
        features = task.get('cargo_features', [])
        validate_seed(seed, digest(seed / 'seed-manifest.json'), commit, digest(archive),
                      seed_config['toolchain']['environment'], features=features)
        if seed_value['packages'] != task['packages'] or seed_config['packages'] != task['packages']:
            raise ValueError('Seed packages differ from the public task')
        checker = inputs / 'checker.rs'
        if digest(checker) != checker_sha:
            raise ValueError('The qualified checker changed')
        if pilot.read_json(inputs / 'index.json', 128 * 1024 * 1024).get('commit') != commit:
            raise ValueError('The syntax index changed source')
        package = task['packages'][-1]
        templates[alias] = {
            'schema': pilot.SCHEMA, 'protocol': ref(root / 'inputs/protocol.md'), 'task_id': alias, 'task_manifest': ref(inputs / 'task.json'),
            'index': ref(inputs / 'index.json'), 'source_repo': '/home/user/openagents',
            'lean_system': ref(root / 'inputs/lean-system.txt'), 'module_hashes': modules,
            'slot_lock': '/home/user/work/openagents-target-agent3/.ds4-calibration.lock',
            'gateway_key_file': str(root / 'private/gateway-key'),
            'credential_file': str(root / 'private/claude-access-token'),
            'native_template': {
                'source_archive': str(archive), 'source_archive_sha256': digest(archive),
                'target_seed': str(seed), 'target_seed_manifest_sha256': digest(seed / 'seed-manifest.json'),
                'binary': BINARY, 'binary_sha256': BINARY_HASH, 'binary_version': BINARY_VERSION,
                'model': 'claude-sonnet-5-5', 'effort': 'medium', 'timeout_s': 600, 'cli_budget_usd': 2,
                'initialize_git': True, 'cargo_features': features, 'toolchain': seed_config['toolchain'],
                'provider_meter': {'admission_target_usd': 8, 'max_requests': 100, 'max_concurrent': 4,
                    'max_output_tokens': 128000, 'models': {model: {'max_requests': 100, 'usd_per_million': rates}
                                                          for model, rates in RATES.items()}}},
            'acceptance_template': {
                'packages': task['packages'], 'allowed_paths': task['allowed_paths'],
                'cargo_features': features, 'toolchain': seed_config['toolchain'], 'total_timeout_s': 240,
                'checker': {'path': str(checker), 'sha256': checker_sha, 'package': package,
                            'target': 'ds4_independent', 'injection': f'crates/{package}/tests/ds4_independent.rs'}}}
    schedule = []
    for alias, repetition, arms in ORDER:
        for arm in arms:
            run_id = str(uuid.uuid4())
            config = dict(templates[alias], run_id=run_id, arm=arm)
            pilot.validate(config)
            path = output / (run_id + '.json')
            pilot.trial.durable(path, config)
            schedule.append({'position': len(schedule) + 1, 'run_id': run_id, 'task_id': alias,
                             'repetition': repetition, 'arm': arm, 'config': ref(path),
                             'output': str(root / 'runs' / run_id)})
    plan = {'schema': 'openagents.jev-lifecycle.native-pilot-plan.v1', 'created_at': pilot.utc(),
            'status': 'frozen_before_execution', 'capability_probe': ref(probe_path), 'kind': 'exploratory_exposed_tasks', 'schedule': schedule,
            'module_hashes': modules, 'protocol': ref(root / 'inputs/protocol.md'), 'prices': RATES, 'source_inputs': {alias: {k: templates[alias][k] for k in ('task_manifest', 'index', 'lean_system')}
                                                                   for alias in TASKS},
            'policy': {'attempts_per_cell': 1, 'external_repair': False, 'jev_is_correctness_gate': False,
                       'max_gateway_http_calls_per_jev_trial': 1, 'max_gateway_request_bytes': 128 * 1024,
                       'gateway_socket_timeout_s': 30, 'preparation_wall_timeout_s': 90, 'source_pack_bytes': 16384,
                       'native_nominal_budget_usd': 2, 'broker_admission_target_usd': 8,
                       'panel_observed_cost_admission_stop_usd': 24,
                       'unknown_accounting_or_unconfirmed_closure': 'stop_before_next_trial',
                       'native_timeout_s': 600, 'native_wrapper_timeout_s': 1200,
                       'acceptance_timeout_s': 240, 'acceptance_wrapper_timeout_s': 260,
                       'note': 'The external serial driver enforces prior safe_to_continue and observed cost admission. In-flight calls can overshoot; this is not a hard spending ceiling.'}}
    pilot.trial.durable(output / 'plan.json', plan)
    return plan


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    plan = make(args.root, args.output)
    print(json.dumps({'trials': len(plan['schedule']), 'plan_sha256': digest(args.output / 'plan.json')}))


if __name__ == '__main__':
    main()
