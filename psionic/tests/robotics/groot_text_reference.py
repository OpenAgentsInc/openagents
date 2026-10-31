#!/usr/bin/env python3
"""Firm reason2: installed Transformers MPS mRoPE oracle, not a runtime."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import numpy as np
import torch
import transformers
from transformers import Qwen3VLTextConfig
from transformers.models.qwen3_vl.modeling_qwen3_vl import Qwen3VLTextRotaryEmbedding

# Frozen before native or SDK comparison. CPU table has explicit F32
# reciprocal/phase boundaries and wide transcendental evaluation.
ATOL, RTOL = 5e-5, 2e-6
p = argparse.ArgumentParser(description=__doc__)
p.add_argument("outputs", nargs="+", type=Path)
p.add_argument("--record", required=True, type=Path)
a = p.parse_args()
assert not a.record.exists()
assert torch.__version__.split("+")[0] == "2.9.0" and transformers.__version__ == "4.57.3"
assert torch.backends.mps.is_available() and os.environ.get("PYTORCH_ENABLE_MPS_FALLBACK", "0") == "0"
config = Qwen3VLTextConfig(head_dim=128, rope_theta=5000000,
                         rope_scaling={"rope_type":"default", "mrope_section":[24,20,20], "mrope_interleaved":True})
rotary = Qwen3VLTextRotaryEmbedding(config, device="mps")
positions = torch.tensor([[(axis*137+t*17)%512 for t in range(512)] for axis in range(3)], device="mps")[:, None]
with torch.inference_mode():
    co, sn = rotary(torch.zeros((1,512,128), device="mps"), positions)
    expected = torch.stack((co[0,:,:64], sn[0,:,:64]), dim=-1).reshape(512,128).cpu().numpy()
records=[]
for path in a.outputs:
    raw=path.read_bytes();actual=np.frombuffer(raw,dtype="<f4").reshape(512,128)
    error=np.abs(actual.astype(np.float64)-expected.astype(np.float64))
    limit=ATOL+RTOL*np.abs(expected.astype(np.float64))
    assert np.isfinite(actual).all() and np.all(error<=limit), (path,float(error.max()),float((error/limit).max()))
    records.append({"file":path.name,"sha256":hashlib.sha256(raw).hexdigest(),"values":65536,
                    "max_abs_error":float(error.max()),"max_tolerance_fraction":float((error/limit).max())})
record={"schema":"psionic-groot-text-mrope-reference-v1","device":"mps","torch":torch.__version__,
        "transformers":transformers.__version__,"atol":ATOL,"rtol":RTOL,"passed":True,"results":records,
        "scope":"Bounded Cosmos24/20/20 interleaved rotary table; no learned text model or whole VLA comparison."}
a.record.write_text(json.dumps(record,indent=2,sort_keys=True)+"\n")
print("GR00T text mRoPE:65536values/mode matched original SDK MPS at frozen tolerance")
