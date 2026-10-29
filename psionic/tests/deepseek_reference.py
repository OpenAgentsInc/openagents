#!/usr/bin/env python3
"""Owned cache-free full-prefix oracle. No Transformers or upstream code.

Opt-in original qualification only; routine checks use the retained capture.
NumPy is authoring tooling and never shipped or imported by Psionic.
"""
import argparse,hashlib,json,os,struct,time
from pathlib import Path
import numpy as np
ROOT=Path(__file__).resolve().parents[2]
def digest(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  while b:=f.read(1024*1024):h.update(b)
 return h.hexdigest()
def reference(path,tokens):
 with path.open('rb') as f:n=struct.unpack('<Q',f.read(8))[0];header=json.loads(f.read(n))
 raw=np.memmap(path,mode='r',dtype=np.uint8);begin=8+n
 def tensor(name):
  h=header[name];a,b=h['data_offsets'];assert h['dtype']=='BF16'
  u=np.frombuffer(raw[begin+a:begin+b],dtype='<u2').astype(np.uint32)<<16
  return u.view(np.float32).reshape(h['shape'])
 def projection(x,name,bias=None):
  w=tensor(name);out=(x.astype(np.float64)@w.astype(np.float64).T).astype(np.float32)
  if bias is not None:out=(out.astype(np.float64)+tensor(bias).astype(np.float64)).astype(np.float32)
  return out
 def norm(x,name):
  xd=x.astype(np.float64);return (xd/np.sqrt(np.sum(xd*xd,axis=-1,keepdims=True)/1536+1e-6)*tensor(name).astype(np.float64)).astype(np.float32)
 # Gather only requested original embedding rows.
 h=header['model.embed_tokens.weight'];offset=begin+h['data_offsets'][0];embedding=np.frombuffer(raw[offset:offset+2*151936*1536],dtype='<u2').reshape(151936,1536)
 x=(embedding[tokens].astype(np.uint32)<<16).view(np.float32);positions=np.arange(len(tokens),dtype=np.float64)[:,None];angles=positions*np.power(10000.,-np.arange(0,128,2,dtype=np.float64)[None,:]/128)
 cs=np.cos(angles).astype(np.float32).astype(np.float64)[:,None,:];sn=np.sin(angles).astype(np.float32).astype(np.float64)[:,None,:]
 def rotate(a):
  a=a.astype(np.float64);lo,hi=a[:,:,:64],a[:,:,64:];return np.concatenate([lo*cs-hi*sn,hi*cs+lo*sn],axis=-1).astype(np.float32)
 trace=[]
 for layer in range(28):
  p=f'model.layers.{layer}.';z=norm(x,p+'input_layernorm.weight');q=rotate(projection(z,p+'self_attn.q_proj.weight',p+'self_attn.q_proj.bias').reshape(-1,12,128));k=rotate(projection(z,p+'self_attn.k_proj.weight',p+'self_attn.k_proj.bias').reshape(-1,2,128));v=projection(z,p+'self_attn.v_proj.weight',p+'self_attn.v_proj.bias').reshape(-1,2,128);a=np.zeros((len(tokens),12,128),dtype=np.float32)
  for pos in range(len(tokens)):
   for head in range(12):
    kh=head//6;score=k[:pos+1,kh].astype(np.float64)@q[pos,head].astype(np.float64)/np.sqrt(128.)
    e=np.exp(score-np.max(score));a[pos,head]=((e@v[:pos+1,kh].astype(np.float64))/np.sum(e)).astype(np.float32)
  x=(x.astype(np.float64)+projection(a.reshape(-1,1536),p+'self_attn.o_proj.weight').astype(np.float64)).astype(np.float32);trace.append(x[-1].copy());z=norm(x,p+'post_attention_layernorm.weight');g=projection(z,p+'mlp.gate_proj.weight').astype(np.float64);u=projection(z,p+'mlp.up_proj.weight').astype(np.float64);act=(g/(1+np.exp(-g))*u).astype(np.float32);x=(x.astype(np.float64)+projection(act,p+'mlp.down_proj.weight').astype(np.float64)).astype(np.float32);trace.append(x[-1].copy())
  print('reference layer',layer,flush=True)
 z=norm(x[-1:], 'model.norm.weight');h=header['lm_head.weight'];offset=begin+h['data_offsets'][0];w=np.frombuffer(raw[offset:offset+2*151936*1536],dtype='<u2').reshape(151936,1536);logits=np.empty(151936,dtype=np.float32)
 for first in range(0,151936,2048):
  b=(w[first:first+2048].astype(np.uint32)<<16).view(np.float32).astype(np.float64);logits[first:first+len(b)]=(b@z[0].astype(np.float64)).astype(np.float32)
 return logits,np.array(trace,dtype=np.float32).ravel()
def compare(directory):
 policy=json.loads((ROOT/'psionic/fixtures/deepseek/original/qualification-policy.json').read_text());report={}
 for kind in ['logits','trace']:
  actual=np.fromfile(directory/f'prompt-{kind}.f32',dtype='<f4');expected=np.fromfile(directory/f'reference-{kind}.f32',dtype='<f4');assert actual.shape==expected.shape and np.isfinite(actual).all() and np.isfinite(expected).all();error=np.abs(actual.astype(np.float64)-expected.astype(np.float64));bound=policy[kind+'_absolute']+policy[kind+'_relative']*np.abs(expected.astype(np.float64));bad=np.flatnonzero(error>bound);report[kind]=dict(values=len(actual),max_abs=float(error.max()),violations=len(bad),worst_index=int(np.argmax(error)),actual=float(actual[np.argmax(error)]),expected=float(expected[np.argmax(error)]));assert not len(bad),report
  if kind=='logits':report['greedy']=[int(np.argmax(actual)),int(np.argmax(expected))];assert report['greedy'][0]==report['greedy'][1]
 report['passed']=True;report['policy_sha256']=digest(ROOT/'psionic/fixtures/deepseek/original/qualification-policy.json');(directory/'comparison.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
def slices(original,record):
 path=original/'model.safetensors'
 assert digest(path)=='58858233513d76b8703e72eed6ce16807b523328188e13329257fb9594462945'
 with path.open('rb') as f:n=struct.unpack('<Q',f.read(8))[0];header=json.loads(f.read(n))
 raw=np.memmap(path,mode='r',dtype=np.uint8);begin=8+n
 def tensor(name):
  h=header[name];a,b=h['data_offsets'];return (np.frombuffer(raw[begin+a:begin+b],dtype='<u2').astype(np.uint32)<<16).view(np.float32).reshape(h['shape'])
 h=header['model.embed_tokens.weight'];a=begin+h['data_offsets'][0]+198*1536*2;x=(np.frombuffer(raw[a:a+1536*2],dtype='<u2').astype(np.uint32)<<16).view(np.float32).astype(np.float64)
 norm=(x/np.sqrt(np.sum(x*x)/1536+1e-6)*tensor('model.layers.0.input_layernorm.weight').astype(np.float64)).astype(np.float32)
 q=(tensor('model.layers.0.self_attn.q_proj.weight').astype(np.float64)@norm.astype(np.float64)).astype(np.float32)
 q=(q.astype(np.float64)+tensor('model.layers.0.self_attn.q_proj.bias').astype(np.float64)).astype(np.float32)
 actual=np.fromfile(record/'slice-query.f32',dtype='<f4');assert actual.shape==q.shape and np.isfinite(actual).all()
 error=np.abs(actual.astype(np.float64)-q.astype(np.float64));policy=json.loads((ROOT/'psionic/fixtures/deepseek/original/qualification-policy.json').read_text());assert np.all(error<=policy['logits_absolute']+policy['logits_relative']*np.abs(q.astype(np.float64)))
 q.tofile(record/'slice-reference-query.f32');report=dict(values=len(q),max_abs=float(error.max()),violations=0,passed=True,scope='current Coax layer-0 original biased query versus owned NumPy; native current head compared all retained original logits exactly')
 (record/'slices-comparison.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
def main():
 p=argparse.ArgumentParser();p.add_argument('record',type=Path);p.add_argument('--original',type=Path);p.add_argument('--slices',action='store_true');a=p.parse_args()
 if a.slices:
  assert a.original is not None;slices(a.original,a.record);return
 if a.original:
  assert digest(a.original/'model.safetensors')=='58858233513d76b8703e72eed6ce16807b523328188e13329257fb9594462945'
  tokens=np.fromfile(a.record/'prompt.i64',dtype='<i8').tolist();logits,trace=reference(a.original/'model.safetensors',tokens);logits.tofile(a.record/'reference-logits.f32');trace.tofile(a.record/'reference-trace.f32')
 compare(a.record)
if __name__=='__main__':main()
