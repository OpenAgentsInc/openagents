#!/usr/bin/env python3
"""Generate small SmolVLA conformance cases with the official offline model.

Requires LeRobot 0.3.3 and Transformers 4.51.3. The tiny configuration uses
original random weights and supplied noise; it is not a trained checkpoint.
All weights remain in the explicit private artifact directory.
"""
import argparse, hashlib, importlib.metadata, json
from pathlib import Path
from types import SimpleNamespace
parser=argparse.ArgumentParser()
parser.add_argument('--artifacts',type=Path,required=True)
parser.add_argument('--fixtures',type=Path,required=True)
args=parser.parse_args()
assert args.artifacts.is_absolute() and not any((p/'.git').exists() for p in (args.artifacts,*args.artifacts.parents))
args.artifacts.mkdir(parents=True,exist_ok=True)
args.fixtures.mkdir(parents=True,exist_ok=True)
assert importlib.metadata.version('lerobot')=='0.3.3'
assert importlib.metadata.version('transformers')=='4.51.3'
import torch
from transformers import SmolVLMConfig, SmolVLMVisionConfig, LlamaConfig
from lerobot.policies.smolvla import smolvlm_with_expert as reference
from lerobot.policies.smolvla.modeling_smolvla import VLAFlowMatching
from lerobot.policies.smolvla.configuration_smolvla import SmolVLAConfig
from safetensors.torch import save_file

torch.set_num_threads(2)
torch.use_deterministic_algorithms(True)
vision=SmolVLMVisionConfig(hidden_size=16,intermediate_size=32,num_hidden_layers=2,num_attention_heads=4,image_size=32,patch_size=8,hidden_act='gelu_pytorch_tanh')
text=LlamaConfig(vocab_size=64,hidden_size=32,intermediate_size=64,num_hidden_layers=4,num_attention_heads=4,num_key_value_heads=2,head_dim=8,max_position_embeddings=256,pad_token_id=0,attention_bias=False,rms_norm_eps=1e-5)
config=SmolVLMConfig(vision_config=vision,text_config=text,scale_factor=2,image_token_id=63,pad_token_id=0)
# Replace only offline configuration/processor lookup. Model code and forward
# operations are the unmodified official reference implementations.
reference.AutoConfig.from_pretrained=lambda *a,**k:config
reference.AutoProcessor.from_pretrained=lambda *a,**k:SimpleNamespace(tokenizer=SimpleNamespace(fake_image_token_id=62,global_image_token_id=63))
for name,mode,every,layers,special in [('hybrid','cross_attn',2,4,False),('cross','cross_attn',0,4,True),('reduced','cross_attn',0,2,False),('self','self_attn',0,4,False)]:
    torch.manual_seed(113)
    cfg=SmolVLAConfig(device='cpu',max_state_dim=8,max_action_dim=8,chunk_size=4,n_action_steps=4,num_steps=3,num_vlm_layers=4,num_expert_layers=layers,expert_width_multiplier=.75,attention_mode=mode,self_attn_every_n_layers=every,add_image_special_tokens=special,resize_imgs_with_padding=(32,32),prefix_length=20)
    model=VLAFlowMatching(cfg).eval()
    # Nontrivial original inputs, a masked second camera, and right padding
    # exercise cached attention, camera masks, token positions, and flow.
    state=torch.tensor([[.2,-.3,.1,.7,-.2,.05,0.,0.]])
    images=[torch.linspace(-1,1,3*32*32).reshape(1,3,32,32),torch.zeros(1,3,32,32)]
    image_valid=[torch.tensor([True]),torch.tensor([False])]
    tokens=torch.tensor([[3,8,11,0]])
    token_mask=torch.tensor([[True,True,True,False]])
    noise=torch.linspace(-1.2,1.4,4*8).reshape(1,4,8)
    with torch.no_grad():
        output=model.sample_actions(images,image_valid,tokens,token_mask,state,noise=noise.clone())
        embedded=model.vlm_with_expert.embed_image(images[0])
    weights=args.artifacts/f'smol-{name}.safetensors'
    save_file({f'model.{k}':v.contiguous() for k,v in model.state_dict().items()},str(weights))
    architecture={'vision':{'hidden':16,'intermediate':32,'layers':2,'heads':4,'image_size':32,'patch_size':8,'shuffle':2,'norm_epsilon':1e-6},'vocabulary':64,'hidden':32,'intermediate':64,'layers':4,'expert_hidden':24,'expert_intermediate':256,'expert_layers':layers,'heads':4,'kv_heads':2,'head_dim':8,'norm_epsilon':1e-5,'attention_bias':False,'cross_attention':mode=='cross_attn','self_attention_every':every,'max_state_dim':8,'max_action_dim':8,'chunk_size':4,'denoise_steps':3,'min_period':cfg.min_period,'max_period':cfg.max_period,'prefix_length':20,'image_start_tokens':[62,63] if special else [],'image_end_tokens':[62] if special else []}
    def tensor(t):return {'shape':list(t.shape),'values':t.flatten().tolist()}
    fixture={'reference':'lerobot-0.3.3/transformers-4.51.3','configuration':architecture,'weights':{'bytes':weights.stat().st_size,'sha256':hashlib.sha256(weights.read_bytes()).hexdigest()},'state':tensor(state),'images':[tensor(i) for i in images],'image_valid':[True,False],'tokens':[3,8,11,0],'language_valid':[True,True,True,False],'noise':tensor(noise),'output':tensor(output),'first_image_embedding':tensor(embedded),'absolute_tolerance':.0002}
    (args.fixtures/f'smol-{name}.json').write_text(json.dumps(fixture,separators=(',',':'))+'\n')
    print(name,weights.stat().st_size,output.flatten()[:4].tolist(),flush=True)

# The same original RGB pattern checks aspect-preserving resize and the
# reference's top/left padding, independently of model attention.
from PIL import Image
import numpy as np
from lerobot.policies.smolvla.modeling_smolvla import resize_with_pad
path=args.fixtures/'camera-pattern.png'
pixels=np.array(Image.open(path).convert('RGB'))
input_image=torch.from_numpy(pixels.copy()).permute(2,0,1).float().unsqueeze(0)/255.
prepared=resize_with_pad(input_image,32,32,pad_value=0)*2.-1.
(args.fixtures/'smol-preprocess.json').write_text(json.dumps({'image':path.name,'source':[pixels.shape[1],pixels.shape[0]],'size':32,'output':prepared.flatten().tolist(),'absolute_tolerance':2e-6},separators=(',',':'))+'\n')
