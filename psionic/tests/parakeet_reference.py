#!/usr/bin/env python3
# Firm reason: 3 (actual pinned NVIDIA NeMo reference inference, independent of shipped Coax arithmetic).
"""Record actual NeMo stages/tokens on safe safetensors weights; no pickle/trust_remote_code.

Run only in the isolated reference environment on the owned Linux GPU. The
configuration is data extracted by parakeet_reference_acquire.py. The exact
NeMo source pin is checked and installed module source hashes are retained.
"""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,time,unicodedata
os.environ.setdefault('CUBLAS_WORKSPACE_CONFIG',':4096:8')
os.environ.setdefault('WANDB_MODE','disabled')
NEMO_REV='5f15875acd0885ed92eadfee874ae87b0529ffa2'
MODEL_SHA='3a2026366188c8c68598edbbff92f8d11590a08e0ae2e6775544e7b07d6a5e11'
def sha(p):
 h=hashlib.sha256()
 with Path(p).open('rb') as f:
  for b in iter(lambda:f.read(1<<24),b''):h.update(b)
 return h.hexdigest()
def clean(d):return {k:v for k,v in d.items() if k!='_target_'}
def normalize(s):
 out=[];gap=False
 for c in unicodedata.normalize('NFKC',s).casefold():
  if unicodedata.category(c)[0] in 'LNM' or c=="'":
   if gap and out:out.append(' ')
   out.append(c);gap=False
  else:gap=True
 return ''.join(out)
def distance(a,b):
 row=list(range(len(b)+1))
 for i,x in enumerate(a,1):
  prev=row[0];row[0]=i
  for j,y in enumerate(b,1):old=row[j];row[j]=min(row[j]+1,row[j-1]+1,prev+(x!=y));prev=old
 return row[-1]
def score(ref,hyp):
 a=normalize(ref);b=normalize(hyp);w=a.split();return {'words':len(w),'word_errors':distance(w,b.split()),'characters':len(a),'character_errors':distance(a,b)}
def mapping(name):
 if name.startswith('encoder.subsampling.layers.'):return 'encoder','pre_encode.conv.'+name.split('encoder.subsampling.layers.',1)[1]
 if name.startswith('encoder.subsampling.linear.'):return 'encoder','pre_encode.out.'+name.split('encoder.subsampling.linear.',1)[1]
 if name.startswith('encoder.layers.'):
  n=name[len('encoder.'):].replace('.conv.norm.','.conv.batch_norm.').replace('.self_attn.bias_u','.self_attn.pos_bias_u').replace('.self_attn.bias_v','.self_attn.pos_bias_v')
  for old,new in [('q_proj','linear_q'),('k_proj','linear_k'),('v_proj','linear_v'),('o_proj','linear_out'),('relative_k_proj','linear_pos')]:n=n.replace('.self_attn.'+old+'.','.self_attn.'+new+'.')
  return 'encoder',n
 if name=='decoder.embedding.weight':return 'decoder','prediction.embed.weight'
 if name.startswith('decoder.lstm.'):return 'decoder','prediction.dec_rnn.lstm.'+name[len('decoder.lstm.'):]
 if name.startswith('decoder.decoder_projector.'):return 'joint','pred.'+name[len('decoder.decoder_projector.'):]
 if name.startswith('encoder_projector.'):return 'joint','enc.'+name[len('encoder_projector.'):]
 if name.startswith('joint.head.'):return 'joint','joint_net.2.'+name[len('joint.head.'):]
 raise ValueError('unexpected source tensor '+name)
def adversaries(torch,RNNTDecoder,GreedyTDTInfer):
 class Joint(torch.nn.Module):
  def __init__(self,token,durations):super().__init__();self.token=token;self.ds=durations;self.calls=0
  def joint(self,enc,pred):
   logits=enc.new_full((1,1,1,8),-10);logits[0,0,0,self.token]=10;logits[0,0,0,3+self.ds[self.calls%len(self.ds)]]=10;self.calls+=1;return logits
 rows=[]
 for name,token,ds,frames,caps in [('constant-skip',1,[2],10,[1,2,5]),('cap-with-skip',1,[0,2],10,[2]),('zero-duration',1,[0],4,[1,2,5]),('zero-blank',2,[0],4,[1,2,5]),('skipping-blank',2,[2],10,[1,2,5])]:
  for cap in caps:
   dec=RNNTDecoder(prednet={'pred_hidden':1,'pred_rnn_layers':1,'dropout':0.0},vocab_size=2,blank_as_pad=True).eval();joint=Joint(token,ds)
   infer=GreedyTDTInfer(dec,joint,blank_index=2,durations=[0,1,2,3,4],max_symbols_per_step=cap,include_duration=True)
   hyp=infer(encoder_output=torch.zeros((1,1,frames)),encoded_lengths=torch.tensor([frames]))[0][0];tokens=hyp.y_sequence.tolist();at=list(map(int,hyp.timestamp));dur=list(map(int,hyp.token_duration))
   expected_at=[] if token==2 else ([t for t in range(0,frames,2) for _ in range(2)] if name=='cap-with-skip' else [t for t in range(frames) for _ in range(cap)] if name=='zero-duration' else list(range(0,frames,2)))
   assert at==expected_at and tokens==[token]*len(expected_at),(name,cap,tokens,at,expected_at)
   rows.append({'case':name,'max_symbols':cap,'tokens':tokens,'frames':at,'durations':dur,'joint_calls':joint.calls,'passed':True})
 return rows
def main():
 p=argparse.ArgumentParser();p.add_argument('model',type=Path);p.add_argument('fixtures',type=Path);p.add_argument('english',type=Path);p.add_argument('out',type=Path);p.add_argument('--nemo-source',type=Path,required=True);p.add_argument('--inspect',action='store_true');a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 before=sha(a.fixtures/'manifest.json');manifest=json.loads((a.fixtures/'manifest.json').read_text());assert manifest['nemo_source_revision']==NEMO_REV
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=a.nemo_source,text=True).strip()==NEMO_REV
 import torch,numpy as np,yaml,nemo
 from nemo.collections.common.tokenizers.sentencepiece_tokenizer import SentencePieceTokenizer
 from safetensors import safe_open
 from nemo.collections.asr.modules import ConformerEncoder,RNNTDecoder,RNNTJoint,AudioToMelSpectrogramPreprocessor
 from nemo.collections.asr.parts.submodules.rnnt_greedy_decoding import GreedyTDTInfer
 import nemo.collections.asr.modules.rnnt as rnntmod
 import nemo.collections.asr.modules.conformer_encoder as encmod
 import nemo.collections.asr.parts.submodules.rnnt_greedy_decoding as greedymod
 torch.set_num_threads(4);torch.backends.cuda.matmul.allow_tf32=False;torch.backends.cudnn.allow_tf32=False;torch.use_deterministic_algorithms(True)
 modelsha=sha(a.model/'model.safetensors');assert modelsha==MODEL_SHA
 cfg=yaml.safe_load((a.fixtures/'nemo-model-config.yaml').read_text());assert sha(a.fixtures/'nemo-model-config.yaml')==manifest['nemo_config']['sha256']
 prep=clean(cfg['preprocessor']);prep.update(dither=0.0,pad_to=0)
 modules={'encoder':ConformerEncoder(**clean(cfg['encoder'])),'decoder':RNNTDecoder(**clean(cfg['decoder'])),'joint':RNNTJoint(**{**clean(cfg['joint']),'fuse_loss_wer':False})}
 states={k:{} for k in modules};bindings=[]
 with safe_open(a.model/'model.safetensors',framework='pt',device='cpu') as weights:
  for name in weights.keys():
   group,key=mapping(name);v=weights.get_tensor(name);assert key not in states[group];states[group][key]=v;bindings.append({'source':name,'destination':group+'.'+key,'shape':list(v.shape),'dtype':str(v.dtype)})
 for key,module in modules.items():module.load_state_dict(states[key],strict=True);module.eval().cuda()
 del states
 preprocessor=AudioToMelSpectrogramPreprocessor(**prep).eval().cuda()
 infer=GreedyTDTInfer(modules['decoder'],modules['joint'],blank_index=8192,durations=[0,1,2,3,4],max_symbols_per_step=10,include_duration=True)
 vocab=json.loads((a.model/'tokenizer.json').read_text())['model']['vocab'];pieces=['']*8192
 for piece,index in vocab.items():
  if index<8192:pieces[index]=piece
 assert all(pieces)
 tokenizer=SentencePieceTokenizer(model_path=str(a.fixtures/'nemo-tokenizer.model'),legacy=False)
 tokenizer_cases=[]
 for ids in [[i] for i in range(32)]+[[0,284,300],[1,284,300],[2,284,300],[3,284,300],[4,284,300]]:
  tokenizer_cases.append({'ids':ids,'nemo_text':tokenizer.ids_to_text(ids),'piece_text':''.join(pieces[i] for i in ids).replace('▁',' ').lstrip()})
 installed={str(Path(m.__file__)):sha(m.__file__) for m in [rnntmod,encmod,greedymod]}
 assert sha(greedymod.__file__)==sha(a.nemo_source/'nemo/collections/asr/parts/submodules/rnnt_greedy_decoding.py')
 record={'schema':'psionic.parakeet-nemo-reference.v1','model_sha256':modelsha,'nemo_source_revision':NEMO_REV,'installed_sources':installed,'torch':torch.__version__,'nemo':nemo.__version__,'device':torch.cuda.get_device_name(0),'profile':manifest['profile'],'fixture_manifest_sha256':before,'bindings':bindings,'pickle_loaded':False,'tokenizer_cases':tokenizer_cases,'adversaries':adversaries(torch,RNNTDecoder,GreedyTDTInfer),'clips':[]}
 (a.out/'bindings.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n');print('MODELS_LOADED_STRICT',len(bindings),flush=True)
 if a.inspect:return
 clips=[]
 for line in (a.english/'clips.txt').read_text().splitlines():
  if line and not line.startswith('#'):
   name,ref=line.split(' ',1);clips.append({'language':'en','corpus':'parakeet-clips-v1','id':name,'pcm':a.english/(name+'.pcm'),'reference':ref})
 for c in manifest['clips']:clips.append({'language':c['language'],'corpus':'fleurs-smoke-v1','id':c['language'],'pcm':a.fixtures/c['pcm_file'],'reference':c['reference'],'pcm_sha256':c['pcm_sha256']})
 for clip in clips:
  raw=clip['pcm'].read_bytes();digest=hashlib.sha256(raw).hexdigest()
  if 'pcm_sha256' in clip:assert digest==clip['pcm_sha256']
  signal=torch.from_numpy(np.frombuffer(raw,dtype='<i2').astype(np.float32)/32768).unsqueeze(0).cuda();length=torch.tensor([signal.shape[1]],device='cuda',dtype=torch.int64)
  torch.cuda.synchronize();start=time.monotonic()
  with torch.inference_mode():
   features,flen=preprocessor(input_signal=signal,length=length);encoded,elen=modules['encoder'](audio_signal=features,length=flen);hyp=infer(encoder_output=encoded,encoded_lengths=elen)[0][0]
  torch.cuda.synchronize();elapsed=time.monotonic()-start
  tokens=hyp.y_sequence.tolist();text=tokenizer.ids_to_text(tokens);folder=a.out/clip['id'];folder.mkdir(exist_ok=True)
  arrays={'features':features[0,:,:int(flen[0])].cpu().contiguous().numpy().astype('<f4'),'encoded':encoded[0,:,:int(elen[0])].transpose(0,1).cpu().contiguous().numpy().astype('<f4')}
  artifacts={}
  for name,array in arrays.items():path=folder/(name+'.f32');array.tofile(path);artifacts[name]={'file':str(path.relative_to(a.out)),'shape':list(array.shape),'sha256':sha(path)}
  row={k:v for k,v in clip.items() if k not in ['pcm']};row.update(samples=signal.shape[1],pcm_sha256=digest,feature_frames=int(flen[0]),encoded_frames=int(elen[0]),tokens=tokens,frames=list(map(int,hyp.timestamp)),durations=list(map(int,hyp.token_duration)),text=text,score=score(clip['reference'],text),seconds=elapsed,artifacts=artifacts)
  (folder/'tokens.json').write_text(json.dumps(row,ensure_ascii=False,indent=2)+'\n');record['clips'].append(row);(a.out/'record.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n');print('CLIP',clip['id'],int(flen[0]),int(elen[0]),len(tokens),text,flush=True)
 assert sha(a.fixtures/'manifest.json')==before
 record['passed']=True;record['manifest_unchanged']=True;record['python']=os.sys.version;record['cuda_runtime']=torch.version.cuda;record['cudnn']=torch.backends.cudnn.version()
 (a.out/'record.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n')
 print('REFERENCE_DONE',len(record['clips']),flush=True)
if __name__=='__main__':main()
