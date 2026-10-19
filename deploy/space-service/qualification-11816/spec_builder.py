import json, sys, time
S = sys.argv[1]; observed = int(sys.argv[2])
identity = json.load(open(S + '/identity.json'))
E = 'evidence/space-qualification-2026-10-18'
statuses = {}
import glob, os
for f in glob.glob(os.path.expanduser('~/work/oa-wt-space-qual/docs/sdk/') + E + '/raw/cases/*.json'):
    v = json.load(open(f)); statuses[os.path.basename(f)[:-5]] = v['status']
cases = {}
for case in ['public_sdk','web_task_and_followup','account_isolation','actual_boot_and_shape','stage_timings','memory_and_workspace','cancellation_and_children','api_restart','reconciler_restart','lost_create_reply','lost_command_reply','attach_interruption','duplicate_delivery','delayed_lifecycle','worker_loss_and_single_writer','cross_host_memory_restore','checkpoint_recovery','database_restore','storage_outage','provider_outage','credential_revocation','egress_bypass','capacity_exhaustion','concurrent_wake','quotas_and_accounting','retention','account_deletion','orphan_cleanup','upgrade_and_rollback','disabled_placement','legacy_boat','unsupported_controls','monitoring','fixture_cleanup']:
    st = statuses.get(case, 'pending')
    if st == 'not_required': st = 'pending'
    cases[case] = {'status': st, 'source': f'{E}/raw/cases/{case}.json' if case in statuses and st in ('passed', 'failed') else None}
# Packet trials are the post-#12144 series; the earlier series stay in raw/trials as diagnostics.
trial_files = [f'{E}/raw/trials/{n}' for n in ['trials-fix-cold-warm-c1.jsonl','trials-fix-cold-warm-c2.jsonl','trials-snap-warm-c1.jsonl','trials-fix-wake-warm-c1.jsonl','trials-fix-wake-warm-c2.jsonl']]
spec = {'evidence_dir': E, 'identity': identity, 'observed_unix': observed,
  'advertised': {'fork': False, 'terminal': False, 'preview': False, 'cross_host_memory_restore': False},
  'accounts': ['account:f9b110d34613b1783bcac550f001f45c76f98fd84d76856b928f172a7ec42d75', 'account:352cd3cc4183ce192b8efac43c85a20d1750c133f125f3a74560e40957a7ee0d'],
  'private_management': True, 'rollback_ready': True,
  'limits': {'minimum_samples': 20, 'required_concurrency': 2, 'snapshot_p50_ms': 1500, 'snapshot_p90_ms': 2000, 'wake_p90_ms': 5000,
             'maximum_error_permille': 10, 'maximum_service_cost_per_useful_task_micros': 50000, 'currency': 'USD', 'maximum_age_seconds': 86400},
  'economics': json.load(open(S + '/economics.json')), 'cases': cases, 'trial_files': trial_files, 'snapshot_boot_mode_observed': 'cold'}
json.dump(spec, open(S + '/spec.json', 'w'), indent=1)
print({k: sum(1 for c in cases.values() if c['status'] == k) for k in ('passed', 'failed', 'pending')})
