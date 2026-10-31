#!/usr/bin/env python3
"""Firm reason2: NEW bounded SDK metadata oracle, no learned forward pass.

Instantiate installed Transformers on meta only; get_rope_index performs small
integer metadata operations on CPU. Original gated configs/tokenizer stay private.
"""
import argparse
import ctypes as ct
import hashlib
import importlib.util
import json
from pathlib import Path
import numpy as np
import torch
import transformers
from tokenizers import Tokenizer
from transformers import Qwen3VLConfig,Qwen3VLModel

ROOT=Path(__file__).resolve().parents[3]
COSMOS=Path.home()/".cache/huggingface/hub/models--nvidia--Cosmos-Reason2-2B/snapshots/9ce19a195e423419c349abfc86fd07178b230561"

def main():
    parser=argparse.ArgumentParser();parser.add_argument("library",type=Path);parser.add_argument("--write-metadata",type=Path);args=parser.parse_args()
    assert transformers.__version__=="4.57.3" and torch.__version__.split("+")[0]=="2.9.0"
    config_bytes=(COSMOS/"config.json").read_bytes()
    assert hashlib.sha256(config_bytes).hexdigest()=="bec4b3d446efa05807365c9e1cec03ac590836879d02f3a6da879971154bdd3b"
    config=Qwen3VLConfig.from_dict(json.loads(config_bytes));config.text_config.num_hidden_layers=16
    with torch.device("meta"):
        model=Qwen3VLModel(config)
    assert all(p.device.type=="meta" for p in model.parameters())
    spec=importlib.util.spec_from_file_location("owned_request_reference",ROOT/"psionic/tests/robotics/groot_request_reference.py")
    owned=importlib.util.module_from_spec(spec);spec.loader.exec_module(owned)
    lib=ct.CDLL(str(args.library.resolve()))
    lib.gr_plan.argtypes=[owned.I64,ct.c_int64,owned.I64,ct.c_bool];lib.gr_plan.restype=owned.Read
    tokenizer=Tokenizer.from_file(str(COSMOS/"tokenizer.json"))
    grid=torch.tensor([[1,16,16],[1,16,16]],dtype=torch.int64)
    records=[];words=0;cases=0
    for instruction in ["","Open the drawer!","PICK UP the red cube.","  keep__underscores\t123\n","1"*375]:
        rendered=owned.canonical(instruction)
        ids=tokenizer.encode(rendered.decode(),add_special_tokens=False).ids
        for pad in sorted({0,len(ids),min(512,len(ids)+1),512}):
            expected,delta=owned.reference_plan(ids,pad)
            count=expected.shape[1]
            sdk_ids=torch.from_numpy(expected[0].copy()).view(1,-1)
            sdk_attention=torch.from_numpy(expected[1].copy()).view(1,-1)
            positions,deltas=model.get_rope_index(sdk_ids,image_grid_thw=grid,attention_mask=sdk_attention)
            actual=positions[:,0].numpy()
            assert np.array_equal(actual,expected[3:]) and int(deltas.item())==delta
            out=np.full(3073,-7,dtype=np.int64);tokens=np.asarray(ids,dtype=np.int64)
            result=lib.gr_plan(owned.i64(tokens),pad,owned.i64(out),False)
            assert (result.status,result.written,result.content,result.count,result.rope_delta)==(0,6*count,len(ids),count,delta)
            assert np.array_equal(out[:6*count].reshape(6,count),expected) and np.all(out[6*count:]==-7)
            assert np.array_equal(out[3*count:6*count].reshape(3,count),actual)
            assert np.array_equal(expected[2],(sdk_ids.numpy()[0]==151655).astype(np.int64))
            words+=6*count+1;cases+=1
            records.append({"instruction_utf8_sha256":hashlib.sha256(instruction.encode()).hexdigest(),"content":len(ids),"count":count,"rope_delta":delta,"sdk_positions_le_i64_sha256":hashlib.sha256(actual.astype('<i8').tobytes()).hexdigest()})
    metadata={"schema":"psionic-groot-request-sdk-qualification-v1","date":"2026-10-31","source_file_sha256":hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),"transformers":transformers.__version__,"torch":torch.__version__,"model_initialization":"Qwen3VLModel(meta),sourceCosmosconfig,selected16textlayers; no learned tensors allocated/loaded","operation":"get_rope_index only; small CPUinteger metadata operations, no forward pass or CPUmodel fallback","source_config_sha256":hashlib.sha256(config_bytes).hexdigest(),"coax_request_source_sha256":"3222cd09e9b4cf12fe0fc4dfccb3d6318bd5a6e2ed76882e6f072f69d6c50ecd","cases":cases,"exact_integer_words":words,"records":records,"scope":"SDK positions/delta match Coax and independent general-grid integer oracle; padding metadata parity does not imply padded/unpadded learned-feature equivalence."}
    if args.write_metadata:args.write_metadata.write_text(json.dumps(metadata,indent=2)+"\n")
    print(f"GR00T request SDKTransformers{transformers.__version__}: {cases}metadata cases,{words}exact integerwords passed; all modelparameters meta")

if __name__=="__main__":main()
