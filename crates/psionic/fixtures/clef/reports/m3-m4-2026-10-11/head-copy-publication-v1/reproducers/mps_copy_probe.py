#!/usr/bin/env python3
"""Small offset/cast probe; requires a guarded GPU+memory lease, no model."""
import json
import os
from pathlib import Path
import sys
import time

for name in ['OMP_NUM_THREADS','MKL_NUM_THREADS','OPENBLAS_NUM_THREADS','VECLIB_MAXIMUM_THREADS']:
    os.environ[name]='2'
os.environ['PYTORCH_ENABLE_MPS_FALLBACK']='0'
if not {'gpu','memory'} <= set(os.environ.get('OPENAGENTS_LEASES','').split(',')):
    raise SystemExit('requires GPU and memory leases plus an owned-process guard')
import torch
torch.set_num_threads(2)
torch.set_num_interop_threads(1)
torch.mps.set_per_process_memory_fraction(min(1.0,1024**3/torch.mps.recommended_max_memory()))
output=Path(sys.argv[1])
if output.exists():
    raise SystemExit('output exists; preserve the previous probe')
report={'kind':'mps_offset_copy_probe','torch':str(torch.__version__),
        'model_loaded':False,'maximum_base_elements':8192,'copy_elements':64,'cases':[],
        'sources':['https://github.com/pytorch/pytorch/pull/189572',
                   'https://github.com/pytorch/pytorch/issues/194344']}
began=time.monotonic()
for source_dtype,destination_dtype in [(torch.bfloat16,torch.float32),(torch.float16,torch.float32),
                                     (torch.float32,torch.bfloat16),(torch.float32,torch.int32),
                                     (torch.int64,torch.int32)]:
    for offset in [0,1,64,256,4096]:
        for path in ['fused_to_cpu','cpu_clone_then_cast','mps_cast_then_cpu','destination_offset_copy']:
            cpu=((torch.arange(8192,dtype=torch.float32)%97)-48).to(source_dtype)
            base=cpu.to('mps')
            mask=torch.ones((1,1031),dtype=torch.long,device='mps')
            view=base[offset:offset+64]
            expected=cpu[offset:offset+64].to(destination_dtype)
            row={'source_dtype':str(source_dtype),'destination_dtype':str(destination_dtype),
                 'source_offset':offset,'path':path}
            try:
                if path=='fused_to_cpu':
                    actual=view.to(device='cpu',dtype=destination_dtype)
                elif path=='cpu_clone_then_cast':
                    actual=view.cpu().clone().to(destination_dtype)
                elif path=='mps_cast_then_cpu':
                    actual=view.to(destination_dtype).cpu().clone()
                else:
                    destination=torch.full((192,),-7,dtype=destination_dtype)
                    destination[64:128].copy_(view)
                    actual=destination[64:128].clone()
                    row['destination_guards_intact']=bool((destination[:64]==-7).all() and (destination[128:]==-7).all())
                after=base.cpu().clone()
                mask_after=mask.cpu().clone()
                row.update(output_exact=torch.equal(actual,expected),source_unchanged=torch.equal(after,cpu),
                           mask_unchanged=bool((mask_after==1).all()),
                           maximum_absolute_difference=float((actual.float()-expected.float()).abs().max()))
                if not row['output_exact']:
                    row['first_output_values']=actual[:8].tolist()
                    row['first_expected_values']=expected[:8].tolist()
                if not row['source_unchanged']:
                    changed=(after!=cpu).nonzero().flatten()
                    row['changed_source_indices']=changed[:16].tolist()
                    row['changed_source_count']=changed.numel()
            except Exception as error:
                row.update(error_type=type(error).__name__,error=str(error))
            report['cases'].append(row)
report['elapsed_seconds']=time.monotonic()-began
report['completed']=True
report['failures']=[row for row in report['cases'] if row.get('error') or not all(
    row.get(key,True) for key in ['output_exact','source_unchanged','mask_unchanged','destination_guards_intact'])]
output.write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
print(json.dumps({'cases':len(report['cases']),'failures':len(report['failures']),
                  'elapsed_seconds':report['elapsed_seconds']}),flush=True)
