#!/usr/bin/env python3
"""Check incomplete, duplicate, failed, and passing summary records without a GPU."""
import copy
import json
from summarize import analyze, SIZES, TIMED, SCOPES

receipt = {'status': 'complete', 'exit': 0, 'mode': 'bench', 'memory': {'owned_group_stopped': True},
           'max_owned_group_bytes': 2*1024**3, 'minimum_available_bytes': 12*1024**3,
           'minimum_start_bytes': 14*1024**3, 'deadline_seconds': 60}
rows = [{'kind': 'device'}, {'kind': 'weights', 'q4_conversion_executed': False}]
for n in SIZES:
    rows += [{'kind': 'correctness', 'n': n, 'variant': 'serial', 'all_outputs_finite': True,
              'outputs_sha256': {name: 'a'*64 for name in ['gate','up','activation','residual']}},
             {'kind': 'correctness', 'n': n, 'variant': 'overlap', 'all_outputs_bitwise_equal': True, 'output_bytes': n*(12288*10+4096*4)},
             {'kind': 'case_complete', 'n': n, 'immutable_weights_and_expectations': True, 'redzones': True,
              'explicit_live_bytes': 100, 'owned_cpu_bytes': 100, 'device_current_allocated_bytes': 100}]
for n in TIMED:
    for r in range(5):
        for scope in SCOPES:
            for p in range(4):
                rows.append({'kind':'timing','n':n,'round':r,'scope':scope,'position':p,
                             'variant':'serial' if p in (0,3) else 'overlap',
                             'gpu_seconds':1 if p in (0,3) else .8,'host_seconds':1.1,'exact_output_check_passed':True})
rows.append({'kind':'complete','commands':136,'dispatches':560,'timing_rows':120,
             'owned_cpu_bytes_after_cases':0,'q4_conversion_executed':False,'explicit_peak_bytes':100,'combined_peak_bytes':100})
assert analyze(rows, receipt)['speed_qualified']
checks = 1

def refused(data, rec=receipt):
    global checks
    try:
        analyze(data, rec)
    except (ValueError, TypeError):
        checks += 1; return
    raise AssertionError('invalid receipt accepted')

refused(rows[:-1])
refused(rows + [rows[-2]])
refused([row for row in rows if not (row['kind']=='timing' and row['n']==512 and row['round']==0 and row['scope']=='full_ffn' and row['position']==0)])
for field, value in [('gpu_seconds', 0), ('host_seconds', float('nan')), ('exact_output_check_passed', False), ('variant', 'other')]:
    changed = copy.deepcopy(rows)
    next(row for row in changed if row['kind']=='timing')[field] = value
    refused(changed)
for key in ['cleanup_error','monitor_failure','error']:
    rec = copy.deepcopy(receipt);rec[key]='failure';refused(rows,rec)
for key in ['memory_rejection','sampling_error','cleanup_error']:
    rec=copy.deepcopy(receipt);rec['memory'][key]='failure';refused(rows,rec)
rec=copy.deepcopy(receipt);rec['status']='failed';refused(rows,rec)
rec=copy.deepcopy(receipt);rec['memory']['owned_group_stopped']=False;refused(rows,rec)
for kind, field, value in [('complete','dispatches',559),('complete','combined_peak_bytes',2*1024**3),('correctness','all_outputs_finite',False),('case_complete','redzones',False)]:
    changed=copy.deepcopy(rows);next(row for row in changed if row['kind']==kind)[field]=value;refused(changed)
changed=copy.deepcopy(rows)
for row in changed:
    if row['kind']=='timing' and row['variant']=='overlap': row['gpu_seconds']=.91
answer=analyze(changed,receipt)
assert answer['correctness_passed'] and not answer['speed_qualified'];checks+=1
print(json.dumps({'passed':True,'checks':checks,'gpu_executed':False}))
