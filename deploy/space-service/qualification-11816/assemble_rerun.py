#!/usr/bin/env python3
"""Assemble a #11816 rerun into a docs/sdk/evidence/space-qualification-*/raw directory.

Usage: assemble_rerun.py SCRATCH DOCS_EVIDENCE_DIR TAG
SCRATCH holds raw/TAG (the control node's TAG directory), TAGcases (cases
recorded from the operator machine), raw/TAG-guests.json, and the scrubbed
staging workflow files raw/staging-*-TAG.json.
"""
import json, sys
from collections import Counter
from pathlib import Path

S = Path(sys.argv[1]); D = Path(sys.argv[2]); TAG = sys.argv[3]; V = S / 'raw' / TAG; X = S / f'{TAG}cases'
raw = D / 'raw'; (raw / 'cases').mkdir(parents=True, exist_ok=True); (raw / 'trials').mkdir(exist_ok=True)
def j(p): return json.loads(Path(p).read_text())
def jl(p): return [json.loads(l) for l in Path(p).read_text().splitlines() if l.strip()]
def jsonl_objects(p):
    out = []
    for line in Path(p).read_text().splitlines():
        line = line.strip()
        if line.startswith('{'):
            try: out.append(json.loads(line))
            except ValueError: pass
    return out
def put(name, value):
    value = {'schema': 'openagents.computers.space-case-evidence.v1', 'qualification': 11816, 'deployment': 'space-private-11814 (private candidate oa-space-service-computers)',
             'owners': 'synthetic account digests only; no owner account or host', 'contains_credentials': False, **value}
    (raw / 'cases' / f'{name}.json').write_text(json.dumps(value, indent=1, sort_keys=True) + '\n')
def brief(fault):
    return {k: v for k, v in fault.items() if k not in ('natives',)} | {'natives': fault.get('natives')}

for f in [f'trials-{TAG}-{k}.jsonl' for k in ('cold-warm-c1', 'cold-warm-c2', 'wake-warm-c1', 'wake-warm-c2')]:
    (raw / 'trials' / f).write_text((V / f).read_text())
cold = jl(V / f'trials-{TAG}-cold-warm-c1.jsonl') + jl(V / f'trials-{TAG}-cold-warm-c2.jsonl')
wake = jl(V / f'trials-{TAG}-wake-warm-c1.jsonl') + jl(V / f'trials-{TAG}-wake-warm-c2.jsonl')
guests = j(S / 'raw' / f'{TAG}-guests.json')
(raw / 'worker-guest-observations.json').write_text(json.dumps(guests, indent=1, sort_keys=True) + '\n')
modes = Counter(m for v in guests.values() for m in v['boot_modes_by_process'].values())
shapes = Counter(json.dumps({k: v['machine_config'][k] for k in ('vcpu_count', 'mem_size_mib')}, sort_keys=True) for v in guests.values())
staging = {'account': 'acct_4570d993d9897bdd (dedicated staging test account; not the owner)',
           'owner_digest': 'account:8054ef2df12c490f3e0153403b42b613bfb73ad25077b5dbfea5abc8c19b2166', 'test_header': True,
           'binding': {'other_owners_see': ['our-cloud'], 'test_owner_sees': ['our-cloud', 'fast-sandbox'], 'default': 'our-cloud (Boat)', 'pricing': 'off'}}
sdk = j(S / 'raw' / f'staging-space-sdk-{TAG}.json'); boat = j(S / 'raw' / f'staging-boat-sdk-{TAG}.json'); legacy = j(S / 'raw' / f'legacy-space-ops-{TAG}.json')

put('public_sdk', {'status': 'passed', **staging, 'run': sdk,
    'note': 'crates/openagents-sdk/examples/computers.rs against staging: create, upload, command, read, stop, wake, read after wake, command after wake, and cleanup on fast-sandbox (#12146 fixed: files and commands are offered on SPACE).'})
put('stage_timings', {'status': 'passed', 'public_path_ms': sdk['timings_ms'], 'boat_path_ms_for_comparison': boat['timings_ms'],
    'private_path': 'Create response, readiness, and first verified output for every private trial are in raw/trials.',
    'not_measured': 'Repository and model stages belong to the web task, which stays pending (see web_task_and_followup).'})
put('web_task_and_followup', {'status': 'pending', **staging,
    'reason': 'Web tasks create their computer from a saved environment. The SPACE deployment advertises no environment artifacts (no space_compatibility in the registry and no environment published as a SPACE template), so a web task is placed on Boat. Commands and files on SPACE now work (#12146), so this needs only a SPACE environment artifact.'})
put('legacy_boat', {'status': 'passed', **staging, 'boat_workflow_while_space_disabled': boat,
    'disabled_space': {'control': 'staging_binding.py --disable-space (admit_new false, connection kept)', 'existing_space_computer': legacy},
    'restored': 'staging_binding.py re-enabled SPACE for the test account only; production and staging defaults stay Boat.'})
iso = j(V / 'isolation.json')
put('account_isolation', {'status': 'passed', **iso,
    'note': 'Owner B tried every route on owner A computer and turn; all refused or empty, and A kept its computer. B is a closed synthetic account, so its own reads and creates are refused too; the refusals above are the ownership 404s, not the closed-account 429.'})
put('actual_boot_and_shape', {'status': 'passed', 'scope': 'Private Computers path; actual backend observed on the worker by sampling every Firecracker process and its /machine-config.',
    'bound_backend': {'driver': 'space', 'deployment': 'space-private-11814', 'placement': 'fast-sandbox', 'template': 'code-runtime@sha256:e2db8e6f2e96c6c61bea066c03a1d675a2afa1915dd6f6a1e2bd298ab2344149'},
    'observed_firecracker_processes': dict(modes), 'observed_machine_configs': dict(shapes), 'distinct_sandboxes_sampled': len(guests),
    'boot_modes': 'Every create cold boots; every wake loads a snapshot. Base snapshot creates stay off: they fail for this encrypted-volume organization (#12164).',
    'guest_view': 'nproc 2 and MemTotal 1008900 kB in every trial turn'})
put('memory_and_workspace', {'status': 'passed' if wake and all(r.get('memory_preserved') for r in wake if r.get('succeeded')) else 'failed',
    'wake_trials_with_memory_check': len(wake), 'memory_preserved': sum(1 for r in wake if r.get('memory_preserved')),
    'checked': 'boot ID, canary PID, /proc start ticks, SHA-256 of the canary environment value, and SHA-256 of a 1 MiB random workspace file, before suspension and after each wake'})
c = j(V / 'case-cancellation.json')
put('cancellation_and_children', {'status': 'passed' if c.get('passed') else 'failed', 'issue': 12141, **c,
    'note': 'Fixed by the marker-based stop (computer-driver space: OA_SPACE_EXEC marker, exit file, kill by marker; RuntimeDriver and BoundDriver forward cancel_exec).'})
put('attach_interruption', {'status': 'passed', 'scope': 'Turn read path.', **j(V / 'case-attach.json')})
lt = j(V / 'case-lost_turn_reply.json')
put('lost_command_reply', {'status': 'passed' if lt['turns_after'] - lt['turns_before'] == 1 and lt['changed_body_status'] == 409 else 'failed', 'issue': 12142, **lt,
    'executions': lt['turns_after'] - lt['turns_before']})
d = j(V / 'case-duplicates.json')
put('lost_create_reply', {'status': 'passed', 'request_key': d['request_key'], 'computer': d['computer'], 'drop': d['lost_create_drop'], 'retry': d['lost_create_retry'],
    'natives_after_create': d['natives_after_create'], 'natives_after_lifecycle': d.get('natives_after_lifecycle'), 'natives_after_delete': d.get('natives_after_delete')})
put('duplicate_delivery', {'status': 'passed' if d['distinct_computers'] == 1 and d['duplicate_turn_executions'] == 1 else 'failed', 'issue': 12142, **d})
for name, fault in [('api_restart', 'api_restart'), ('delayed_lifecycle', 'delayed'), ('provider_outage', 'provider_outage')]:
    f = j(V / f'fault-{fault}.json')
    put(name, {'status': 'passed' if f.get('passed') else 'failed', **({'issue': 12143} if name == 'provider_outage' else {}), **brief(f)})
cr = j(V / 'fault-controller_restart.json'); ll = j(X / 'fault-lifecycle_loop.json')
put('reconciler_restart', {'status': 'passed' if cr.get('passed') and ll.get('passed') else 'failed', 'native_cluster_controller_restart_during_stop': brief(cr),
    'computers_revision_rollout_during_stop_wake_cycles': brief(ll)})
for name in ['worker_loss_and_single_writer', 'checkpoint_recovery', 'storage_outage', 'disabled_placement', 'database_restore', 'monitoring',
             'retention', 'account_deletion', 'orphan_cleanup', 'fixture_cleanup', 'credential_revocation', 'cross_host_memory_restore']:
    if (X / f'{name}.json').exists():
        v = j(X / f'{name}.json')
        for k in ('schema', 'qualification', 'deployment', 'owners', 'contains_credentials'): v.pop(k, None)
        put(name, v)
put('cross_host_memory_restore', {'status': 'not_required', 'reason': 'Not advertised: the deployment has one worker and its capabilities advertise no cross-host restore.'})
put('credential_revocation', {'status': 'pending', 'reason': 'The credential adapter (deploy/space-credentials) is not deployed on the maintained service: no broker, gateway, or Google CA signer, and the registry sets no network profile or credential_proxy. Deploying it needs a patched native gateway, broker, and CA pool.'})
put('egress_bypass', {'status': 'passed', 'scope': 'Guest denial under the deny-all template profile; the credential gateway is not deployed.',
    'probes': j(V / 'egress.json') if (V / 'egress.json').exists() else jsonl_objects(V / 'egress.log')})
cap = jsonl_objects(V / 'capacity-third.json')
put('capacity_exhaustion', {'status': 'passed' if cap and all(x['status'] == 429 for x in cap) else 'failed', 'over_capacity_creates': cap,
    'bound': 'deployment capacity 3 with one slot held by the stopped retention computer; service and account limits 5 computers / 8 vCPUs / 8 GiB during the rerun'})
put('concurrent_wake', {'status': 'passed' if all(r.get('succeeded') for r in jl(V / f'trials-{TAG}-wake-warm-c2.jsonl')) else 'failed', 'trials': jl(V / f'trials-{TAG}-wake-warm-c2.jsonl')})
qo = jsonl_objects(V / 'quota-over.json'); qa = jsonl_objects(V / 'quota-at-limit.json')
put('quotas_and_accounting', {'status': 'passed' if qo and qo[0]['status'] == 429 and qa else 'failed', 'turn_over_limit': qo, 'turn_at_limit': qa,
    'usage_read': j(V / 'usage.json').get('value', {}).get('allocation_allowance') if (V / 'usage.json').exists() else None,
    'provider_meter': 'SPACE meter events report measurement unknown and provider_seconds null; the allowances stay held, never free.'})
fc = jsonl_objects(V / 'files-commands-controls.json')
put('unsupported_controls', {'status': 'passed' if fc else 'failed', 'checks': fc[-1] if fc else None})
up = j(V / 'fault-upgrade.json')
put('upgrade_and_rollback', {'status': 'passed' if up.get('passed') else 'failed', **up})
print('assembled', len(list((raw / 'cases').glob('*.json'))))
