#!/usr/bin/env python3
"""Owned BF16 Qwen2 fixture and independent cache-free, dense scalar oracle."""
import argparse, hashlib, json, math, struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
DEST=ROOT/'psionic/fixtures/deepseek/tiny'
S=dict(layers=2,width=4,hidden=6,heads=2,kv_heads=1,head=2,vocab=9,capacity=8)
TOKENS=[1,4,2,7]
SLOTS=['input_layernorm.weight','post_attention_layernorm.weight','self_attn.q_proj.weight','self_attn.q_proj.bias','self_attn.k_proj.weight','self_attn.k_proj.bias','self_attn.v_proj.weight','self_attn.v_proj.bias','self_attn.o_proj.weight','mlp.gate_proj.weight','mlp.up_proj.weight','mlp.down_proj.weight']
def f32(x):return struct.unpack('<f',struct.pack('<f',x))[0]
def bf(x):return struct.pack('<H',struct.unpack('<I',struct.pack('<f',x))[0]>>16)
def shapes(s):
 d,h,k=s['width'],s['hidden'],s['kv_heads']*s['head']
 return [[d],[d],[d,d],[d],[k,d],[k],[k,d],[k],[d,d],[h,d],[h,d],[d,h]]
def tensor_shapes(s):
 d,v=s['width'],s['vocab'];r={'model.embed_tokens.weight':[v,d],'model.norm.weight':[d],'lm_head.weight':[v,d]}
 for l in range(s['layers']):
  for n,sh in zip(SLOTS,shapes(s)):r[f'model.layers.{l}.{n}']=sh
 return r
def fixture():
 tensors={};header={};payload=bytearray()
 for idx,(name,shape) in enumerate(tensor_shapes(S).items()):
  n=math.prod(shape);values=[(1+((i+idx)%3)*.125 if 'layernorm' in name or name=='model.norm.weight' else ((i*7+idx*11)%23-11)/32) for i in range(n)]
  raw=b''.join(bf(x) for x in values);tensors[name]=[struct.unpack('<f',b'\0\0'+raw[i:i+2])[0] for i in range(0,len(raw),2)]
  header[name]={'dtype':'BF16','shape':shape,'data_offsets':[len(payload),len(payload)+len(raw)]};payload.extend(raw)
 encoded=json.dumps(header,separators=(',',':')).encode();encoded+=b' '*((-len(encoded))%8);begin=8+len(encoded)
 data=struct.pack('<Q',len(encoded))+encoded+payload
 refs=[reference(tensors,TOKENS[:i+1],S) for i in range(len(TOKENS))]
 return data,header,begin,refs

def reference(t,tokens,s=S):
 d,h,hd=s['width'],s['hidden'],s['head'];heads,kv=s['heads'],s['kv_heads'];xs=[t['model.embed_tokens.weight'][i*d:(i+1)*d] for i in tokens];trace=[]
 def project(w,x,b=None):return [f32(math.fsum(a*c for a,c in zip(w[i*d:(i+1)*d],x))) for i in range(len(w)//d)] if b is None else [f32(v+z) for v,z in zip(project(w,x),b)]
 def norm(x,w):
  r=math.sqrt(math.fsum(a*a for a in x)/d+1e-6);return [f32(a/r*b) for a,b in zip(x,w)]
 def rotate(x,pos):
  out=x[:]
  for head in range(len(x)//hd):
   for j in range(hd//2):
    angle=pos*10000**(-2*j/hd);c,si=f32(math.cos(angle)),f32(math.sin(angle));i=head*hd+j;k=i+hd//2
    out[i]=f32(x[i]*c-x[k]*si);out[k]=f32(x[k]*c+x[i]*si)
  return out
 for l in range(s['layers']):
  p=f'model.layers.{l}.';ns=[norm(x,t[p+SLOTS[0]]) for x in xs];q=[rotate(project(t[p+SLOTS[2]],x,t[p+SLOTS[3]]),i) for i,x in enumerate(ns)];k=[rotate(project(t[p+SLOTS[4]],x,t[p+SLOTS[5]]),i) for i,x in enumerate(ns)];v=[project(t[p+SLOTS[6]],x,t[p+SLOTS[7]]) for x in ns];ys=[]
  for pos,x in enumerate(xs):
   a=[]
   for head in range(heads):
    kh=head//(heads//kv);scores=[math.fsum(q[pos][head*hd+j]*k[z][kh*hd+j] for j in range(hd))/math.sqrt(hd) for z in range(pos+1)];mx=max(scores);e=[math.exp(z-mx) for z in scores];den=math.fsum(e)
    a.extend(f32(math.fsum(e[z]*v[z][kh*hd+j] for z in range(pos+1))/den) for j in range(hd))
   att=project(t[p+SLOTS[8]],a);y=[f32(a+b) for a,b in zip(x,att)];n=norm(y,t[p+SLOTS[1]]);gate=project(t[p+SLOTS[9]],n);up=project(t[p+SLOTS[10]],n);act=[f32(g/(1+math.exp(-g))*u) for g,u in zip(gate,up)];down=t[p+SLOTS[11]];z=[f32(math.fsum(down[i*h+j]*act[j] for j in range(h))) for i in range(d)];ys.append([f32(a+b) for a,b in zip(y,z)])
   if pos==len(xs)-1:trace.extend(y);trace.extend(ys[-1])
  xs=ys
 return dict(logits=project(t['lm_head.weight'],norm(xs[-1],t['model.norm.weight'])),trace=trace)
def arr(name,values,typ):return f'{name} :: [{len(values)}]{typ}.['+','.join(str(v).lower() for v in values)+'];\n'
def main():
 p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--header',action='store_true');p.add_argument('--wide',action='store_true');a=p.parse_args();
 global S,DEST
 if a.wide:S=dict(layers=3,width=16,hidden=7,heads=4,kv_heads=2,head=4,vocab=9,capacity=5);DEST=ROOT/'psionic/fixtures/deepseek/wide'
 DEST.mkdir(exist_ok=True)
 data,header,begin,refs=fixture()
 code='// Owned independently generated original-format bytes and dense oracle.\n'+arr('DS_TINY',data,'u8')+'DS_TINY_SHAPE :: DsShape.{'+','.join(f'{k}={v}' for k,v in S.items())+'};\n'
 code+='ds_tiny_model :: (data:[]u8,blocks:[]DsBlock)->DsModel {m:=DsModel.{shape=DS_TINY_SHAPE};\n'
 names=['model.embed_tokens.weight','model.norm.weight','lm_head.weight']
 for i,n in enumerate(names):
  b,e=header[n]['data_offsets'];code+=f'm.p[{i}]=data[{begin+b}..<{begin+e}];\n'
 for l in range(S['layers']):
  for i,n in enumerate(SLOTS):
   b,e=header[f'model.layers.{l}.{n}']['data_offsets'];code+=f'blocks[{l}].p[{i}]=data[{begin+b}..<{begin+e}];\n'
 code+='m.blocks=blocks[0..<'+str(S['layers'])+'];return m;}\n'+arr('DS_TOKENS',TOKENS,'int')
 code+='DS_TINY_PINS :: [1]DsArtifact.[DsArtifact.{name="tiny.safetensors",bytes='+str(len(data))+',sha="'+hashlib.sha256(data).hexdigest()+'"}];\n'
 code+='DS_TINY_PAYLOAD :: '+str(begin)+';\n'
 compact=data[begin-8:];code+=arr('DS_COMPACT',compact,'u8')+'DS_COMPACT_SHA :: '+json.dumps(hashlib.sha256(compact).hexdigest())+';\n'
 code+='DS_TINY_SPECS :: ['+str(len(header))+']DsSpec.['+','.join('DsSpec.{name='+json.dumps(n)+',rank='+str(len(x['shape']))+',rows='+str(x['shape'][0])+',cols='+str(x['shape'][1] if len(x['shape'])==2 else 1)+',begin='+str(begin+x['data_offsets'][0])+',end='+str(begin+x['data_offsets'][1])+'}' for n,x in header.items())+'];\n'
 for i,r in enumerate(refs):code+=arr('DS_LOGITS'+str(i),r['logits'],'f32')+arr('DS_TRACE'+str(i),r['trace'],'f32')
 if a.wide:
  for old,new in [('ds_tiny_model','ds_wide_model'),('DS_TINY','DS_WIDE'),('DS_COMPACT','DS_WIDE_COMPACT'),('DS_TOKENS','DS_WIDE_TOKENS'),('DS_LOGITS','DS_WIDE_LOGITS'),('DS_TRACE','DS_WIDE_TRACE')]:code=code.replace(old,new)
 outputs={'model.safetensors':data,'model.coax':code.encode(),'reference.json':(json.dumps(refs,indent=2)+'\n').encode()}
 for n,b in outputs.items():
  if a.write:(DEST/n).write_bytes(b)
  assert (DEST/n).read_bytes()==b,n
 if a.header:
  print('static const int64_t ds_tokens[]={'+','.join(map(str,TOKENS))+'};')
  for key in ['logits','trace']:
   print('static const float ds_'+key+'[4]['+str(len(refs[0][key]))+']={'+','.join('{'+','.join(str(x)+'f' for x in r[key])+'}' for r in refs)+'};')
 else:print('DeepSeek: owned BF16 Qwen2 tensors and four dense-prefix scalar references verify')
if __name__=='__main__':main()
