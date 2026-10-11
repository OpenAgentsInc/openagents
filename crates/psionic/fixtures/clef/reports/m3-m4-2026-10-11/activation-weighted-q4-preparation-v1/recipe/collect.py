#!/usr/bin/env python3
"""Collect FFN input moments on a frozen independent corpus; no decision head."""
import argparse
from dataclasses import asdict
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import time

import numpy as np
from common import BASE,CHECKPOINT,HERE,canonical,check_checkpoint,checkpoint_identity,load_corpus,load_official,require,sha,write
from moments import Moments

GIB=1024**3


def immutable_copy(torch,tensor):
    require(tensor.device.type=='mps','expected an MPS tensor')
    torch.mps.synchronize()
    copied=tensor.detach().to(device='cpu',copy=True).contiguous()
    torch.mps.synchronize()
    # Materialize immutable bytes before releasing the transfer, avoiding a
    # NumPy view tied to a later MPS allocation or reused tensor storage.
    raw=copied.view(torch.uint8).numpy().tobytes()
    del copied
    return raw


def transport_self_test(torch):
    cpu=((torch.arange(2*4*128,dtype=torch.float32).reshape(2,4,128)%31)-15).div(16).to(torch.bfloat16)
    expected=cpu.contiguous().view(torch.uint8).numpy().tobytes()
    device=cpu.to('mps',copy=True)
    require(immutable_copy(torch,device)==expected,'contiguous BF16 transfer differs')
    view=device[:,:,::2]
    expected_view=cpu[:,:,::2].contiguous().view(torch.uint8).numpy().tobytes()
    require(not view.is_contiguous(),'noncontiguous fixture lost its stride')
    copied=immutable_copy(torch,view)
    require(copied==expected_view,'noncontiguous BF16 transfer differs')
    temporary=torch.full((2,4,128),-3,dtype=torch.bfloat16,device='mps')
    torch.mps.synchronize()
    require(immutable_copy(torch,view)==expected_view and immutable_copy(torch,device)==expected,'later allocation changed input storage')
    require(copied==expected_view,'immutable snapshot changed')
    del temporary,view,device
    torch.mps.synchronize()
    return {'passed':True,'contiguous_bytes':len(expected),'noncontiguous_bytes':len(expected_view),
            'contiguous_sha256':hashlib.sha256(expected).hexdigest(),'noncontiguous_sha256':hashlib.sha256(expected_view).hexdigest(),
            'model_loaded':False}


def check_inputs(torch,batch,ids):
    observed=np.frombuffer(immutable_copy(torch,batch['input_ids']),dtype='<i8').tolist()
    mask=np.frombuffer(immutable_copy(torch,batch['attention_mask']),dtype='<i8').tolist()
    require(observed==ids and mask==[1]*len(ids),'MPS IDs or attention mask changed')
    return {'input_ids_sha256':hashlib.sha256(np.array(observed,dtype='<i8').tobytes()).hexdigest(),
            'attention_mask_sha256':hashlib.sha256(np.array(mask,dtype='<i8').tobytes()).hexdigest()}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus',type=Path,required=True);parser.add_argument('--encoded',type=Path,required=True)
    parser.add_argument('--mode',choices=['pilot','full'],required=True);parser.add_argument('--output-dir',type=Path,required=True)
    args=parser.parse_args()
    leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','))
    require({'gpu','memory','disk'}<=leases and 'quiet' not in leases,'requires GPU, memory, and disk leases; quiet is forbidden')
    require(os.environ.get('CLEF_ACTIVATION_GUARDED')=='1','requires task-owned guard')
    require(not args.output_dir.exists(),'preserve previous collection results')
    args.output_dir.mkdir(parents=True)
    identity=checkpoint_identity();rows,manifest=load_corpus(args.corpus)
    encoded=json.loads(args.encoded.read_text())
    require(encoded['status']=='prepared_not_model_executed' and encoded['corpus_sha256']==manifest['corpus_sha256'],'encoded corpus identity differs')
    require(encoded['corpus_manifest_sha256']==sha(args.corpus/'manifest.json'),'corpus manifest changed after encoding')
    require(encoded['checkpoint']==identity,'checkpoint identity changed since encoding')
    require(encoded['exact_acceptance_request_or_state_overlaps']==0,'corpus overlap validation missing')
    by_id={r['id']:r for r in encoded['records']}
    require(set(by_id)=={r['id'] for r in rows},'encoded record set differs')
    calibration=[r for r in rows if r['partition']=='calibration']
    if args.mode=='pilot':
        # Selection uses frozen lengths only, never model predictions or errors.
        ordered=sorted(calibration,key=lambda r:(by_id[r['id']]['tokens'],r['id']))
        calibration=[ordered[0],ordered[-1]]
    report={'schema':'clef.activation-moments.v1','status':'loading','mode':args.mode,'completed':False,
            'corpus_sha256':manifest['corpus_sha256'],'encoded_sha256':sha(args.encoded),'checkpoint':identity,
            'expected_records':[r['id'] for r in calibration],'records':[],'module_calls':192,'head_executed':False,
            'acceptance_outputs_read':False,'development_activations_used':False,'max_tokens':1024,
            'memory_gib':72,'mps_memory_gib':60,'cpu_threads':2,'raw_activation_tensors_retained':False,
            'source_sha256':{p.name:sha(p) for p in (HERE/'collect.py',HERE/'moments.py',HERE/'common.py')},
            'statistics':'Per-input-column sum(x*x), float64 CPU accumulation, one count per real token row; export mean to f32.',
            'conditions':'Shared machine; statistics collection, not latency evidence.'}
    path=args.output_dir/'receipt.json';write(path,report)
    hooks=[];started=time.monotonic()
    try:
        for name in ('OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','VECLIB_MAXIMUM_THREADS','RAYON_NUM_THREADS'):
            require(os.environ.get(name)=='2','CPU thread bounds missing')
        import torch,transformers
        torch.set_num_threads(2);torch.set_num_interop_threads(1);torch.set_default_device('cpu')
        require(str(torch.__version__)=='2.9.1' and transformers.__version__=='5.10.2','runtime versions differ')
        require(torch.backends.mps.is_available(),'MPS is unavailable')
        torch.mps.set_per_process_memory_fraction(min(1.0,60*GIB/torch.mps.recommended_max_memory()))
        report['transport_self_test']=transport_self_test(torch);write(path,report)
        official=load_official();processor=transformers.AutoProcessor.from_pretrained(CHECKPOINT,local_files_only=True)
        prepared={}
        for row in calibration:
            value=official.encode_record(processor.tokenizer,row['request'],processor=processor)
            require(value.media is None and 0<len(value.input_ids)<=1024,'input bounds differ')
            require(canonical(asdict(value))==canonical(by_id[row['id']]['record']),'fresh encoding differs from frozen IDs/spans')
            prepared[row['id']]=value
        load_started=time.monotonic()
        backbone=transformers.Qwen3_5ForConditionalGeneration.from_pretrained(
            CHECKPOINT,dtype=torch.bfloat16,device_map={'':'mps'},local_files_only=True,
            attn_implementation='eager',low_cpu_mem_usage=True)
        backbone.config.use_cache=False;backbone.eval()
        base=backbone.get_base_model() if hasattr(backbone,'get_base_model') else backbone
        text=base.model
        if hasattr(text,'language_model'):text=text.language_model
        require(len(text.layers)==64,'unexpected backbone layer count')
        widths={};modules={}
        for i,layer in enumerate(text.layers):
            for attribute,operation,width in [('gate_proj','ffn_gate',5120),('up_proj','ffn_up',5120),('down_proj','ffn_down',17408)]:
                name=f'blk.{i}.{operation}.weight';module=getattr(layer.mlp,attribute)
                expected=[17408,5120] if attribute!='down_proj' else [5120,17408]
                require(list(module.weight.shape)==expected and module.weight.dtype==torch.bfloat16 and module.weight.device.type=='mps','FFN weight layout differs')
                widths[name]=width;modules[name]=module
        moments=Moments(widths);hook_seconds={};active={'tokens':None}
        def hook_for(name,width):
            def observe(module,inputs):
                began=time.monotonic()
                require(len(inputs)==1 and isinstance(inputs[0],torch.Tensor),'unexpected FFN pre-hook arguments')
                value=inputs[0]
                require(value.dtype==torch.bfloat16 and list(value.shape)==[1,active['tokens'],width],'FFN activation layout differs')
                raw=immutable_copy(torch,value)
                moments.observe_bf16(name,raw,active['tokens'],width)
                hook_seconds[name]=hook_seconds.get(name,0.0)+(time.monotonic()-began)
            return observe
        for name,module in modules.items():hooks.append(module.register_forward_pre_hook(hook_for(name,widths[name])))
        report.update(status='running',load_seconds=time.monotonic()-load_started,module_widths=widths,
                      runtime={'torch':str(torch.__version__),'transformers':transformers.__version__,'accelerate':importlib.metadata.version('accelerate')})
        write(path,report)
        for row in calibration:
            check_checkpoint(identity)
            record=prepared[row['id']];ids=list(record.input_ids);active['tokens']=len(ids)
            batch=official.collate_records([record],processor.tokenizer.pad_token_id,torch.device('mps'))
            require(not batch.get('media'),'unexpected media')
            before=check_inputs(torch,batch,ids)
            moments.begin(row['request_sha256'],len(ids));before_seconds=sum(hook_seconds.values());began=time.monotonic()
            try:
                with torch.inference_mode():
                    result=text(input_ids=batch['input_ids'],attention_mask=batch['attention_mask'],use_cache=False,return_dict=True)
                torch.mps.synchronize()
                require(list(result.last_hidden_state.shape)==[1,len(ids),5120] and result.last_hidden_state.dtype==torch.bfloat16,'final hidden shape/dtype differs')
                raw=immutable_copy(torch,result.last_hidden_state)
                words=np.frombuffer(raw,dtype='<u2')
                require(np.all((words & 0x7f80)!=0x7f80),'final hidden contains nonfinite values')
                after=check_inputs(torch,batch,ids);require(before==after,'input identity changed')
                require(canonical(asdict(record))==canonical(by_id[row['id']]['record']),'record changed during forward')
                check_checkpoint(identity);moments.commit()
                report['records'].append({'id':row['id'],'request_sha256':row['request_sha256'],'tokens':len(ids),
                                           'seconds':time.monotonic()-began,'hook_seconds':sum(hook_seconds.values())-before_seconds,
                                           'input_immutable':True,'input_identity':before,'final_hidden_sha256':hashlib.sha256(raw).hexdigest(),
                                           'mps_driver_allocated_bytes':torch.mps.driver_allocated_memory()})
                write(path,report);print(json.dumps(report['records'][-1]),flush=True)
            except BaseException:
                moments.abort();raise
            finally:
                # A failed request cannot publish pending statistics.
                if 'result' in locals():del result
                del batch
        require(len(report['records'])==len(calibration),'collection incomplete')
        exported=moments.exported();files=[]
        for name in sorted(widths):
            raw=moments.sums[name].astype('<f8').tobytes();mean=exported[name].astype('<f4').tobytes()
            sum_path=args.output_dir/(name+'.sum2.f64');mean_path=args.output_dir/(name+'.mean2.f32')
            sum_path.write_bytes(raw);mean_path.write_bytes(mean)
            files.append({'tensor':name,'columns':widths[name],'rows':moments.counts[name],
                          'sum2_file':sum_path.name,'sum2_sha256':hashlib.sha256(raw).hexdigest(),
                          'mean2_file':mean_path.name,'mean2_sha256':hashlib.sha256(mean).hexdigest(),
                          'zero_columns':int(np.count_nonzero(exported[name]==0)),'minimum':float(exported[name].min()),'maximum':float(exported[name].max())})
        report.update(status='complete',completed=True,statistics=files,total_tokens=sum(r['tokens'] for r in report['records']),
                      hook_seconds_total=sum(hook_seconds.values()),elapsed_seconds=time.monotonic()-started)
    except BaseException as error:
        report.update(status='failed',error_type=type(error).__name__,error=str(error),elapsed_seconds=time.monotonic()-started)
        raise
    finally:
        for hook in hooks:hook.remove()
        write(path,report)


if __name__=='__main__':main()
