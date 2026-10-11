#!/usr/bin/env python3
"""Verify bounded import fixtures and stored dense transforms without inference."""
import json
import os
from pathlib import Path
import time
for name in ('OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','VECLIB_MAXIMUM_THREADS'):
    os.environ[name]='2'
import numpy as np
from gguf_import import BASE,StreamingGguf,fixture_checks,ordinary_norm_to_hf,head_permutation,value_rows_to_hf,require,sha_bytes
HERE=Path(__file__).resolve().parent
HF=BASE/'artifacts/clef-bf16'
CLEAN=BASE/'reference27b/cpu-head-stage1/results'


def main():
    output=HERE/'fixtures.json'
    require(not output.exists(),'preserve previous fixture results')
    reader=StreamingGguf()
    began=time.monotonic()
    result={'kind':'streamed_gguf_import_fixtures','model_execution':False,'gpu_execution':False,'synthetic':fixture_checks(),'stored_dense':[], 'sampled_quantized':[]}
    index=json.loads((HF/'model.safetensors.index.json').read_text())['weight_map']
    headers={};read_bytes=0
    def hf_flat(name,first=0,count=None):
        nonlocal read_bytes
        path=HF/index[name]
        if path not in headers:
            with path.open('rb') as f:
                length=int.from_bytes(f.read(8),'little');require(length<=16*1024**2,'header too large')
                headers[path]=(8+length,json.loads(f.read(length)))
        base,header=headers[path];item=header[name]
        require(item['dtype']=='BF16','expected BF16 original')
        total=int(np.prod(item['shape']));count=total-first if count is None else count
        require(0<=first and 0<count<=total-first,'invalid slice')
        read_bytes+=2*count;require(read_bytes<=16*1024**2,'fixture reads exceed bound')
        with path.open('rb') as f:f.seek(base+item['data_offsets'][0]+2*first);raw=f.read(2*count)
        require(len(raw)==2*count,'short BF16 read')
        return (np.frombuffer(raw,dtype='<u2').astype(np.uint32)<<16).view(np.float32)
    for layer in (0,3,63):
        for gg,hf in [('attn_norm.weight','input_layernorm.weight'),('post_attention_norm.weight','post_attention_layernorm.weight')]+([('attn_q_norm.weight','self_attn.q_norm.weight'),('attn_k_norm.weight','self_attn.k_norm.weight')] if layer%4==3 else []):
            native=reader.full(f'blk.{layer}.{gg}');parameter=ordinary_norm_to_hf(native)
            original=hf_flat(f'model.language_model.layers.{layer}.{hf}')
            require(np.array_equal(parameter,original),'norm offset differs from official checkpoint')
            result['stored_dense'].append({'tensor':f'blk.{layer}.{gg}','values':int(native.size),'parameter_exact':True,'reconstruction_exact':True})
    final=ordinary_norm_to_hf(reader.full('output_norm.weight'))
    require(np.array_equal(final,hf_flat('model.language_model.norm.weight')),'final norm differs')
    for gg,hf,width,offset in [('ssm_conv1d.weight','conv1d.weight',128,4096),('ssm_dt.bias','dt_bias',1,0),('ssm_norm.weight','norm.weight',0,0)]:
        native=reader.full('blk.0.'+gg)
        restored=value_rows_to_hf(native,width=width,offset=offset) if width else native
        original=hf_flat('model.language_model.layers.0.linear_attn.'+hf).reshape(restored.shape)
        require(np.array_equal(restored,original),'dense head permutation differs: '+gg)
        result['stored_dense'].append({'tensor':'blk.0.'+gg,'values':int(native.size),'inverse_permutation_exact':True})
    # The row selected for HF head h must be GGUF inverse[h]. Test distant
    # head groups against their original BF16 rows, retaining both error sizes.
    _,inverse=head_permutation()
    for gg,hf,width,offset in [('ssm_alpha.weight','in_proj_a.weight',1,0),('ssm_beta.weight','in_proj_b.weight',1,0),('attn_gate.weight','in_proj_z.weight',128,0),('attn_qkv.weight','in_proj_qkv.weight',128,4096)]:
        for head in (0,1,15,16,47):
            native_row=offset+int(inverse[head])*width
            original_row=offset+head*width
            native=reader.rows('blk.0.'+gg,[native_row])[0]
            original=hf_flat('model.language_model.layers.0.linear_attn.'+hf,original_row*5120,5120)
            relative=float(np.linalg.norm((native-original).astype(np.float64))/np.linalg.norm(original.astype(np.float64)))
            require(relative<0.03,'Q8 row mapping inconsistent with original')
            result['sampled_quantized'].append({'tensor':'blk.0.'+gg,'hf_row':original_row,'gguf_row':native_row,'relative_l2_error':relative})
    reader.check_state();result.update(gguf_payload_bytes=reader.payload_read_bytes,bf16_payload_bytes=read_bytes,gguf_metadata={k:v for k,v in reader.metadata.items() if 'head' in k or 'architecture' in k},seconds=time.monotonic()-began)
    reader.close()
    case=json.loads((CLEAN/'text-014.json').read_text());ids=case['input_ids'];require(len(ids)==321,'wrong token count')
    token_bytes=(' '.join(str(token) for token in ids)+'\n').encode()
    (HERE/'text-014-tokens.txt').write_bytes(token_bytes)
    (HERE/'native-layer-dump-metadata.json').write_text(json.dumps({'token_file':'text-014-tokens.txt','token_file_sha256':sha_bytes(token_bytes),'tokens':321,'hidden_size':5120,'layers':64,'expected_bytes_per_output':321*5120*4,'chunk':321,'gguf_sha256_from_prior_verification':'9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822','source_case_sha256':sha_bytes((CLEAN/'text-014.json').read_bytes())},indent=2)+'\n')
    output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print(json.dumps({'passed':True,'stored_dense':len(result['stored_dense']),'sampled_quantized':len(result['sampled_quantized']),'seconds':result['seconds']}))

if __name__=='__main__':main()
