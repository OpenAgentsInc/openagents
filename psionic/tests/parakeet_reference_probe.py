#!/usr/bin/env python3
# Firm reason: 3 (actual pinned NeMo physical/valid-length and tokenizer behavior probes).
from pathlib import Path
import hashlib,json,os,sys
os.environ.setdefault('CUBLAS_WORKSPACE_CONFIG',':4096:8');os.environ.setdefault('WANDB_MODE','disabled')
from parakeet_reference import clean,mapping,sha,NEMO_REV
import torch,numpy as np,yaml
from safetensors import safe_open
from nemo.collections.asr.modules import ConformerEncoder,AudioToMelSpectrogramPreprocessor
from nemo.collections.common.tokenizers.sentencepiece_tokenizer import SentencePieceTokenizer
model=Path(sys.argv[1]);fixtures=Path(sys.argv[2]);english=Path(sys.argv[3]);out=Path(sys.argv[4]);out.mkdir(exist_ok=True)
torch.set_num_threads(4);torch.backends.cuda.matmul.allow_tf32=False;torch.backends.cudnn.allow_tf32=False;torch.use_deterministic_algorithms(True)
cfg=yaml.safe_load((fixtures/'nemo-model-config.yaml').read_text());prep=clean(cfg['preprocessor']);prep.update(dither=0.0,pad_to=0);preprocessor=AudioToMelSpectrogramPreprocessor(**prep).eval().cuda();encoder=ConformerEncoder(**clean(cfg['encoder'])).eval();state={}
with safe_open(model/'model.safetensors',framework='pt',device='cpu') as weights:
 for name in weights.keys():
  group,key=mapping(name)
  if group=='encoder':state[key]=weights.get_tensor(name)
encoder.load_state_dict(state,strict=True);encoder.cuda();del state
record={'schema':'psionic.parakeet-nemo-boundaries.v1','source_revision':NEMO_REV,'physical_vs_compact':[],'length_probes':[]}
for ident,path in [('english-nonmultiple',english/'6930-75918-0000.pcm'),('fleurs-en-multiple-eight',fixtures/'en.pcm')]:
 raw=path.read_bytes();signal=torch.from_numpy(np.frombuffer(raw,dtype='<i2').astype(np.float32)/32768).unsqueeze(0).cuda();length=torch.tensor([signal.shape[1]],device='cuda');seen=[]
 def capture(module,args,kwargs,result):seen.append({'physical':list(result[0].shape),'valid':result[1].tolist(),'values':result[0].detach().cpu()})
 hook=encoder.pre_encode.register_forward_hook(capture,with_kwargs=True)
 with torch.inference_mode():
  features,valid=preprocessor(input_signal=signal,length=length);full,flen=encoder(audio_signal=features,length=valid);compact,clen=encoder(audio_signal=features[:,:,:int(valid[0])].contiguous(),length=valid)
 hook.remove();n=int(flen[0]);delta=(full[:,:,:n]-compact[:,:,:n]).abs();subdelta=(seen[0]['values'][:,:n,:]-seen[1]['values'][:,:n,:]).abs()
 assert n==int(clen[0]);row={'id':ident,'samples':signal.shape[1],'pcm_sha256':hashlib.sha256(raw).hexdigest(),'feature_physical':list(features.shape),'feature_valid':int(valid[0]),'feature_tail_abs_max':float(features[:,:,int(valid[0]):].abs().max()),'encoder_full_physical':list(full.shape),'encoder_compact_physical':list(compact.shape),'encoded_valid':n,'subsample_full':{k:v for k,v in seen[0].items() if k!='values'},'subsample_compact':{k:v for k,v in seen[1].items() if k!='values'},'subsample_valid_max_abs_gap':float(subdelta.max()),'encoder_valid_max_abs_gap':float(delta.max()),'within_frozen_abs0_001':bool((delta<=0.001).all())};record['physical_vs_compact'].append(row);print(json.dumps(row),flush=True)
for n in [0,1,159,160,161,319,320,321,479,480,481,1279,1280,1281,16000,16001]:
 samples=max(1,n);signal=torch.tensor([(((i*37)%101)-50)/64 for i in range(samples)],dtype=torch.float32,device='cuda').unsqueeze(0);length=torch.tensor([n],device='cuda')
 try:
  with torch.inference_mode():features,valid=preprocessor(input_signal=signal,length=length)
  got={'samples':n,'valid':int(valid[0]),'physical':features.shape[-1],'finite':bool(torch.isfinite(features).all()),'tail_zero':bool((features[:,:,int(valid[0]):]==0).all())}
 except Exception as e:got={'samples':n,'exception':type(e).__name__,'message':str(e)}
 record['length_probes'].append(got)
tokenizer=SentencePieceTokenizer(model_path=str(fixtures/'nemo-tokenizer.model'),legacy=False);cases=[[0],[0,0],[0,284],[284,0],[284,0,300],[284,0,284],[0,1],[1,0],[0,284,0],[300],[300,0],[0,300],[300,300]];record['unknown_cases']=[{'ids':ids,'pieces':tokenizer.ids_to_tokens(ids),'text':tokenizer.ids_to_text(ids),'utf8_hex':tokenizer.ids_to_text(ids).encode().hex()} for ids in cases]
record['passed']=all(x['within_frozen_abs0_001'] for x in record['physical_vs_compact']);(out/'record.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n');print('PROBE_DONE',record['passed'],flush=True)
