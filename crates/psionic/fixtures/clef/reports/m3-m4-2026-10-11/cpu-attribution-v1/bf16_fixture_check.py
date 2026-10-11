#!/usr/bin/env python3
"""Validate streamed BF16 loader headers and saved lexical rows without inference."""
import os,json,time
from pathlib import Path
for name in ('OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','VECLIB_MAXIMUM_THREADS'):os.environ[name]='2'
import numpy as np
from bf16_import import StreamingBf16,HF,CLEAN
from gguf_import import require,sha_bytes
from compare_final import tensor
HERE=Path(__file__).resolve().parent;output=HERE/'bf16-fixtures-with-history.json';require(not output.exists(),'preserve prior fixture')
reader=StreamingBf16();case=json.loads((CLEAN/'text-014.json').read_text())
tokens=sorted({token for q in case['records'][0]['questions'] for first,end in q['option_spans'] for token in case['input_ids'][first:end]})
loaded=reader.rows('lm_head.weight',tokens);capture=CLEAN/case['head_inputs']['file'];require(sha_bytes(capture.read_bytes())==case['head_inputs']['sha256'],'clean capture changed')
saved=tensor(capture,'lexical');require(np.array_equal(loaded.view(np.uint32),saved.view(np.uint32)),'BF16 lexical loader differs')
import torch
from transformers.models.qwen3_5 import modeling_qwen3_5 as modeling
torch.set_num_threads(2);torch.set_num_interop_threads(1)
config=modeling.Qwen3_5TextConfig.from_dict(json.loads((HF/'config.json').read_text())['text_config']);config._attn_implementation='eager'
checked=0
for i in range(64):
    with torch.device('meta'):layer=modeling.Qwen3_5DecoderLayer(config,i)
    prefix=f'model.language_model.layers.{i}.'
    shapes={n[len(prefix):]:t['shape'] for n,t in reader.tensors.items() if n.startswith(prefix)}
    require(shapes=={k:list(v.shape) for k,v in layer.state_dict().items()},'BF16 layer shape/key mismatch')
    require(all(v.device.type=='meta' for v in layer.parameters()),'fixture allocated layer')
    checked+=len(shapes);del layer
reader.check_state();reader.close()
r={'kind':'original_bf16_stream_import_fixtures','passed':True,'model_execution':False,'gpu_execution':False,
   'layers_shape_checked':64,'parameters_shape_checked':checked,'lexical_values_bitwise_checked':int(loaded.size),'payload_read_bytes':reader.payload_read_bytes,
   'maximum_layer_f32_bytes':reader.plan(321)['maximum_layer_f32_bytes'],'original_checkpoint_index_sha256':sha_bytes((HF/'model.safetensors.index.json').read_bytes()),
   'saved_capture_sha256':case['head_inputs']['sha256'],'importer_sha256':sha_bytes((HERE/'bf16_import.py').read_bytes())}
output.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
