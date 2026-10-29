#!/usr/bin/env python3
"""Linux-only independent Transformers 5.3.0 layer-zero comparison.

The inputs are a byte-for-byte extract from the authenticated checkpoint.
PyTorch executes the official Qwen3.5 layer in float32; this is a numerical
reference, not shipped Psionic code and not a bit-parity claim.
"""
import json,sys
from pathlib import Path
import numpy as np
import torch
from safetensors.torch import load_file
from transformers import Qwen3_5Config
from transformers.models.qwen3_5.modeling_qwen3_5 import Qwen3_5DecoderLayer

def main():
 if len(sys.argv)!=3:raise SystemExit('usage: elephant_layer0_reference.py EXTRACT_DIR PSIONIC_F32')
 directory=Path(sys.argv[1]);torch.set_num_threads(4)
 config=Qwen3_5Config.from_dict(json.loads((directory/'config.json').read_text())).text_config
 model=Qwen3_5DecoderLayer(config,0).eval()
 original=load_file(str(directory/'layer0.safetensors'),device='cpu')
 model.load_state_dict(original,strict=True)
 row=torch.frombuffer(bytearray((directory/'token32.bf16').read_bytes()),dtype=torch.bfloat16).float().reshape(1,1,4096)
 with torch.inference_mode(): result=model(row,position_embeddings=None).reshape(-1).numpy()
 actual=np.fromfile(sys.argv[2],dtype='<f4')
 if actual.shape!=result.shape or not np.isfinite(actual).all() or not np.isfinite(result).all():raise ValueError('invalid result')
 delta=np.abs(actual.astype(np.float64)-result.astype(np.float64))
 print('transformers layer zero first four:',*[f'{v:.9g}' for v in result[:4]])
 print('psionic layer zero first four:',*[f'{v:.9g}' for v in actual[:4]])
 print(f'absolute difference: max={delta.max():.9g}, rms={np.sqrt(np.mean(delta*delta)):.9g}')
 (directory/'transformers.f32').write_bytes(result.astype('<f4').tobytes())
if __name__=='__main__':main()
