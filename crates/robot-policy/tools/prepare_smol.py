#!/usr/bin/env python3
"""Prepare local SmolVLA data files for the checked Rust loader.

Inputs must already be downloaded and reviewed. This helper reads safetensors
and JSON only. It never fetches a model or executes checkpoint repository code.
"""
import argparse, hashlib, json, shutil
from pathlib import Path
parser=argparse.ArgumentParser()
for name in ('weights','architecture','binding','normalization','tokenizer','source','output'):
    parser.add_argument('--'+name,type=Path,required=True)
parser.add_argument('--empty-cameras',type=int,default=0)
args=parser.parse_args()
root=args.output
assert root.is_absolute() and not root.exists()
assert not any((p/'.git').exists() for p in (root.parent.resolve(),*root.parent.resolve().parents))
root.mkdir(mode=0o700)
architecture=json.loads(args.architecture.read_text())
binding=json.loads(args.binding.read_text())
normalization=json.loads(args.normalization.read_text())
source=json.loads(args.source.read_text())
assert source['revision'] and source['license'] and source['purpose']
assert 0 <= args.empty_cameras <= 4
assert not binding['environment']
assert normalization['environment'] is None
assert all(n['mode']=='identity' for n in normalization['cameras'].values())
# Verify the tensor container before copying it. No tensors execute code.
from safetensors import safe_open
with safe_open(str(args.weights),framework='pt',device='cpu') as weights:
    assert 'model.state_proj.weight' in weights.keys()
    assert 'model.vlm_with_expert.lm_expert.norm.weight' in weights.keys()
from tokenizers import Tokenizer
tokenizer=Tokenizer.from_file(str(args.tokenizer))
encoded=tokenizer.encode(binding['instruction']+'\n',add_special_tokens=True)
assert 0 < len(encoded.ids) <= 128
assert max(encoded.ids) < architecture['vocabulary']
files={}
for source_path,name in [(args.weights,'model.safetensors'),(args.normalization,'normalization.json'),(args.tokenizer,'tokenizer.json')]:
    assert source_path.is_file() and not source_path.is_symlink()
    assert source_path.stat().st_size <= (2<<30)
    shutil.copyfile(source_path,root/name)
    (root/name).chmod(0o600)
    hasher=hashlib.sha256()
    with (root/name).open('rb') as stream:
        while data:=stream.read(1<<20):hasher.update(data)
    files[name]={'sha256':hasher.hexdigest(),'bytes':(root/name).stat().st_size}
pin={'schema':'robot-smolvla.v1','source':source,'architecture':architecture,'binding':binding,'weights':'model.safetensors','normalization':'normalization.json','tokenizer':'tokenizer.json','language_tokens':encoded.ids,'language_valid':[bool(n) for n in encoded.attention_mask],'empty_cameras':args.empty_cameras,'files':files}
(root/'policy.json').write_text(json.dumps(pin,indent=2)+'\n')
(root/'policy.json').chmod(0o600)
print(hashlib.sha256((root/'policy.json').read_bytes()).hexdigest())
