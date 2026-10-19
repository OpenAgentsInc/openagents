#!/usr/bin/env python3
"""Assemble scrubbed #11816 raw case evidence into docs/sdk/evidence/space-qualification-2026-10-18/raw."""
import json, shutil, sys, hashlib
from pathlib import Path
S = Path(sys.argv[1]); D = Path(sys.argv[2]); C = S / 'raw' / 'control'
raw = D / 'raw'; (raw / 'cases').mkdir(parents=True, exist_ok=True); (raw / 'trials').mkdir(exist_ok=True)
def j(p): return json.loads(Path(p).read_text())
def jl(p): return [json.loads(l) for l in Path(p).read_text().splitlines() if l.strip()]
def put(name, value):
    value = {'schema': 'openagents.computers.space-case-evidence.v1', 'qualification': 11816, 'deployment': 'space-private-11814 (private candidate oa-space-service-computers)',
             'owners': 'synthetic account digests only; no owner account or host', 'contains_credentials': False, **value}
    (raw / 'cases' / f'{name}.json').write_text(json.dumps(value, indent=1, sort_keys=True) + '\n')
for f in ['trials-fix-cold-warm-c1.jsonl', 'trials-fix-cold-warm-c2.jsonl', 'trials-fix-wake-warm-c1.jsonl', 'trials-fix-wake-warm-c2.jsonl', 'trials-cold-warm-c1.jsonl', 'trials-cold-warm-c2.jsonl', 'trials-snap-warm-c1.jsonl', 'trials-wake-warm-c1.jsonl', 'trials-wake-warm-c2.jsonl',
          'trials-pilot2.jsonl', 'pre-tuning-trials-pilot.jsonl', 'pre-tuning-trials-cold-warm-c1.jsonl']:
    shutil.copy(C / f, raw / 'trials' / f)
shutil.copy(S / 'raw' / 'worker-guest-observations-pre-reset.json', raw / 'worker-guest-observations.json')
guests = j(S / 'raw' / 'worker-guest-observations-pre-reset.json')
from collections import Counter
modes = Counter(m for v in guests.values() for m in v['boot_modes_by_process'].values())
shapes = Counter(json.dumps(v['machine_config'], sort_keys=True) for v in guests.values())
snap = jl(C / 'trials-snap-warm-c1.jsonl'); cold = jl(C / 'trials-cold-warm-c1.jsonl') + jl(C / 'trials-cold-warm-c2.jsonl'); wake = jl(C / 'trials-wake-warm-c1.jsonl') + jl(C / 'trials-wake-warm-c2.jsonl')
uncertain = j(S / 'raw' / 'uncertain-turns.json')
pm = j(S / 'raw' / 'private-management.json'); pm['external_ingress_probe_from_operator_machine'] = (S / 'raw' / 'external-ingress-probe.txt').read_text().splitlines()
pm['alert_policies'] = (S / 'raw' / 'alert-policies.txt').read_text().splitlines()
(raw / 'private-management.json').write_text(json.dumps(pm, indent=1) + '\n')
put('public_sdk', {'status': 'failed', 'issue': 12146,
    'finding': 'The public SDK computers workflow cannot run on fast-sandbox: no public deployment offers the placement, and SPACE placements refuse the workflow file upload and command execution.',
    'public_capabilities': j(S / 'raw' / 'public-capabilities.json'), 'private_space_refusals': j(C / 'public-sdk-refusals.json'),
    'example_run': 'not started: the example accepts only the staging or production origin, neither offers fast-sandbox, and no staging test credential is available'})
put('actual_boot_and_shape', {'status': 'passed', 'scope': 'Private Computers path; actual backend observed on the worker, not via the public SDK.',
    'bound_backend': {'driver': 'space', 'deployment': 'space-private-11814', 'placement': 'fast-sandbox', 'template': 'code-runtime@sha256:e2db8e6f2e96c6c61bea066c03a1d675a2afa1915dd6f6a1e2bd298ab2344149'},
    'observed_firecracker_processes': dict(modes), 'observed_machine_configs': dict(shapes), 'distinct_sandboxes_sampled': len(guests),
    'boot_modes': 'Every create is a cold boot (--config-file); every wake is a snapshot load. Creates with a ready clean base snapshot still cold boot (#12145).',
    'guest_view': 'nproc 2 and MemTotal 1008900 kB in every trial turn', 'runtime': 'Firecracker v1.15.1, kernel 6.1.155, firecracker-snapshot 9.0.0, volume_encryption true'})
put('stage_timings', {'status': 'pending', 'reason': 'Needs the public SDK and web flows (#12146). Private-path stages are kept in the trial files: create response, readiness, first verified output. Reconcile, attach, repository, and model stages were not measured and stay null.'})
put('web_task_and_followup', {'status': 'pending', 'reason': 'No staging environment binds a dedicated test account to SPACE (#12146), and SPACE placements lack the file and command paths the web task uses.'})
put('account_isolation', {'status': 'passed', **j(C / 'isolation.json'), 'note': 'Owner B tried every route on owner A computer and turn; all refused or empty, and A kept its computer. Credential references are not offered on this placement (credential proxy off).'})
put('memory_and_workspace', {'status': 'passed', 'wake_trials_with_memory_check': len(wake), 'memory_preserved': sum(1 for r in wake if r.get('memory_preserved')),
    'checked': 'boot ID, canary PID, /proc start ticks, SHA-256 of the canary environment value, and SHA-256 of a 1 MiB random workspace file, before suspension and after each wake',
    'canaries': {p.name: j(p) for p in sorted(C.glob('canary-*.json'))},
    'credential_references': 'Not offered: command_sessions false and credential proxy off; credential_ref requests are refused (see unsupported_controls).'})
put('cancellation_and_children', {'status': 'failed', 'issue': 12141,
    'finding': 'Stopping a running turn never confirmed cancellation. The turn ended "We couldn\'t confirm whether the command stopped" and the computer stayed in phase turn until deleted, so the delayed-child check could not run.',
    'stopped_turn': next(t for t in uncertain if t['id'] == 'trn_80d5d09fc9d7c6922a08'),
    'uncertain_turns_in_run': [{k: t[k] for k in ('id', 'computer', 'state', 'exit_code', 'error', 'created_at', 'finished_at')} | {'stdout_bytes': len(t['stdout'] or '')} for t in uncertain if (t['error'] or '').startswith('We couldn')]})
put('attach_interruption', {'status': 'passed', 'scope': 'Turn read path (SPACE has no command sessions).', **j(C / 'case-attach.json')})
put('lost_command_reply', {'status': 'failed', 'issue': 12142, **j(C / 'case-lost_turn_reply.json')})
d = j(C / 'case-duplicates.json')
put('lost_create_reply', {'status': 'passed', 'request_key': d['request_key'], 'computer': d['computer'], 'drop': d['lost_create_drop'], 'retry': d['lost_create_retry'],
    'natives_after_create': d['natives_after_create'], 'natives_after_lifecycle': d['natives_after_lifecycle'], 'natives_after_delete': d['natives_after_delete']})
put('duplicate_delivery', {'status': 'failed', 'issue': 12142, 'finding': 'Lifecycle duplicates converge on one computer and sandbox; two identical turn submissions ran twice.', **d})
put('api_restart', {'status': 'passed', **j(C / 'fault-api_restart.json')})
put('reconciler_restart', {'status': 'passed', 'native_cluster_controller_restart_during_stop': j(C / 'fault-controller_restart.json'),
    'computers_revision_rollout_during_stop_wake_cycles': j(C / 'fault-lifecycle_loop.json')})
put('delayed_lifecycle', {'status': 'passed', **j(C / 'fault-delayed.json')})
put('provider_outage', {'status': 'failed', 'issue': 12143, **j(C / 'fault-provider_outage.json'),
    'operation_left_requested': {'operation': '40bae79628aafb07c0a6811ef037fe4b0fbea2040032184ec5bef13ee42a144c', 'action': 'wake', 'disposition': 'requested', 'provider_operation': None},
    'after_15_minutes': 'state unknown, 9 attempts, last_error "no progress on Inspect"; native sandbox suspended; deletion at 21:13 UTC succeeded'})
put('storage_outage', {'status': 'passed', 'block': (S / 'raw' / 'storage-block.log').read_text().replace('WARNING: \n', '').splitlines(),
    'stop_during_outage': j(C / 'fault-storage-stop.json'), 'recovery': j(C / 'fault-storage-finish.json'),
    'earlier_attempts': {'host_output_chain_only': j(C / 'attempt2-outputonly-fault-storage-stop.json'), 'partial_google_ranges': j(C / 'attempt3-partial-ranges-fault-storage-stop.json'),
                         'note': 'Both earlier blocks missed the volume manager path, so the upload finished; they are kept and not counted. The counted block refused all non-private HTTPS from host and pods.'},
    'boundary': 'Stop stayed stopping for the full 180 s outage, reached stopped 127 s after storage returned, and the memory canary survived. No data was lost.'})
w = j(C / 'fault-worker-watch.json')
put('worker_loss_and_single_writer', {'status': 'passed', 'fault': 'gcloud compute instances reset oa-space-service-worker (hard reset, disks kept)', 'reset_unix': (S / 'raw' / 'worker-reset-unix.txt').read_text().split(),
    'prepare': j(C / 'fault-worker-prepare.json'), 'watch': w,
    'single_writer': 'Each computer kept exactly one native sandbox throughout; no second active copy appeared.',
    'data_loss_boundary': 'A computer that was awake at the loss is not recoverable: its turn becomes uncertain and the product requires deletion (#12141), so its last durable checkpoint is not offered for restore. A suspended computer loses nothing.',
    'recovery_time': 'The checkpointed computer woke 29.4 s after its wake request once the worker was back.'})
put('checkpoint_recovery', {'status': 'passed', 'computer': w['stopped_computer'], 'wake_after_worker_reset_s': w.get('w_wake_s'), 'memory': w.get('w_memory'),
    'natives': w.get('natives_w_final'), 'note': 'Same boot ID, PID, start ticks, canary, and workspace hash after the hard reset; restored from the durable local and uploaded checkpoint.'})
put('cross_host_memory_restore', {'status': 'not_required', 'reason': 'Not advertised: the deployment has one worker and its capabilities advertise no cross-host restore.'})
put('database_restore', {'status': 'passed', 'method': 'database-recovery.py clone to the separate oa-space-service-restore-11814 instance; the maintained instance was not changed.',
    'clone_operation': {'id': 'c0a059d9-d070-4cbe-bb82-237a00000032', 'requested': '2026-10-18T20:28:45Z', 'started': '2026-10-18T20:28:56Z', 'done': '2026-10-18T20:34:06Z'},
    'native_check': {'private_tls': True, 'migrations': 13, 'published_seed': True, 'qualification_org': True, 'instance_ca_sha256': 'b689ce5da4362ce8d352f41dc222d2ea926410493f22014cf4e4674f07a3a09f'},
    'computers_check': {'computers': 48, 'computer_A_present': True, 'latest_usage_event': '2026-10-18T20:28:41Z', 'latest_turn': '2026-10-18T20:28:22Z', 'migrations': 16, 'tls': True},
    'recovery_time_s': 321, 'data_loss_boundary': 'Point in time of the clone request (20:28:45Z); the latest usage event in the copy is 4 s earlier.',
    'deleted': 'copy deleted after the check (operation 064b5c15-db92-4255-bd2d-461400000032)'})
put('credential_revocation', {'status': 'pending', 'reason': 'The credential adapter (deploy/space-credentials) is not deployed on the maintained service: no broker, gateway, or Google CA signer, and the registry sets no network profile or credential_proxy.'})
put('egress_bypass', {'status': 'passed', 'scope': 'Guest denial under the deny-all template profile; the credential gateway is not deployed, so header-injection redirects are out of scope until credential_revocation runs.',
    'probes': j(C / 'egress.json'), 'tls_and_http_by_ip': j(C / 'egress-deep.json'),
    'finding': 'Metadata (IP and name), DNS bypass, IPv6, management, database, and Private Google Access addresses are denied. TCP connects to public and Google VIP addresses are accepted locally by the gateway and reset on the first byte; no TLS session or HTTP reply is possible.'})
put('capacity_exhaustion', {'status': 'passed', 'third_create_small': {'status': 429, 'error': 'quota_exhausted', 'message': 'No computers are free right now. Try again later.'},
    'third_create_medium': {'status': 429, 'error': 'quota_exhausted'}, 'bound': 'deployment capacity 2 and service limit 2 computers / 4 vCPUs / 8 GiB'})
put('concurrent_wake', {'status': 'passed', 'trials': jl(C / 'trials-wake-warm-c2.jsonl'), 'note': 'Two computers owned by different accounts woke together 4 times; all 8 restored memory.'})
put('quotas_and_accounting', {'status': 'passed', 'turn_over_limit': {'request': {'timeout_seconds': 120}, 'status': 429, 'error': 'quota_exhausted'},
    'turn_at_limit': {'id': 'trn_fa0077075fa68a7f8cf8', 'command': 'sleep 75; echo over-limit', 'timeout_seconds': 60, 'ran_s': 60, 'exit_code': 124, 'stdout': ''},
    'computer_count_limit': 'third create refused 429 (capacity_exhaustion); a closed account create refused 429',
    'usage_read': {'allocation_allowance': {'held_cpu_ms': 7516800000, 'settled_cpu_ms': 0, 'unknown_allowances': 0, 'unresolved_allowances': 87},
                   'storage_allowance': {'held_bytes': None, 'held_allowances': 87, 'held_checkpoints': None, 'unknown_allowances': 87, 'released_allowances': 0}, 'customer_charge': None, 'pricing': 'not_enabled'},
    'provider_meter': 'SPACE meter events report measurement unknown and provider_seconds null; the allowances stay held, never free.',
    'copy_note': 'The over-limit turn refusal says "The computer limit has been reached.", which names the wrong limit.'})
put('disabled_placement', {'status': 'passed', 'control': 'computers.py --disable-creates (admit_new false, enabled true, same image, template, shape, and registry deployment ID)',
    'creates_refused': [{'owner': 'a', 'status': 403}, {'owner': 'b', 'status': 403}], 'existing_computer_operations': j(C / 'fault-lifecycle_loop.json'),
    'restored': 'admit_new true redeployed at 22:3x UTC; creates admitted again'})
put('upgrade_and_rollback', {'status': 'failed', 'issue': 12141, 'attempts': {'1': 'Candidate images applied; the existing computer was stranded by an uncertain turn (#12141); rolled back by hand with kubectl rollout undo at 22:44 UTC.',
    '2': j(C / 'attempt2-fault-upgrade.json'), '3': j(C / 'fault-upgrade.json')},
    'finding': 'Upgrading to the checked cleanup API and controller images and rolling back restored the exact previous images each time, and the existing computer kept its memory under the candidate. After rollback the existing computer was stranded by #12141, so continued operation across rollback is not shown.'})
put('legacy_boat', {'status': 'pending', 'reason': 'The private candidate registry has no Boat deployment, and no staging registry mixes Boat and SPACE (#12146). Production and staging still default to Boat (our-cloud).'})
put('unsupported_controls', {'status': 'passed', 'advertised': {'profiles': ['core', 'suspend'], 'fork': False, 'terminal': False, 'preview': False, 'command_sessions': False, 'workspace_file_bytes': None},
    'refusals': {'fork': 404, 'snapshots': 404, 'terminal': 404, 'preview': 404, 'commands': 422, 'files': 501}, 'private_ingress': 'see raw/private-management.json'})
put('monitoring', {'status': 'pending', 'issue': 12147, 'reason': 'The #12147 measurements and policies are live; alerts still reach nobody until the owner picks a notification channel (NEEDS_OWNER.md).',
    'after_fix': {'measurements': 15, 'policies': 12, 'new': ['computers_stranded', 'oldest_stranded_turn_seconds', 'computers_unknown', 'oldest_requested_lifecycle_seconds'], 'commit': '1668c41a3c'},
    'before_fix': 'failed', 'collector': 'oa-space-monitor.timer active; 11 measurements every minute; collector_errors 0',
    'policies': (S / 'raw' / 'alert-policies.txt').read_text().splitlines(),
    'finding': 'No policy has a notification channel; outages under 120 s never open an incident; Computers-layer stuck states (unknown for 15+ minutes, stranded turns) are not measured.'})
EXTRA = json.loads((S / 'extra-cases.json').read_text())
for name, value in EXTRA.items(): put(name, value)
(raw / 'uncertain-turns.json').write_text(json.dumps([{k: t[k] for k in ('id', 'computer', 'state', 'exit_code', 'error', 'created_at', 'started_at', 'finished_at')} | {'stdout_bytes': len(t['stdout'] or '')} for t in uncertain], indent=1) + '\n')
print('assembled', len(list((raw / 'cases').glob('*.json'))))
