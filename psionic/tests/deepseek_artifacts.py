#!/usr/bin/env python3
"""Pin original DATA and pack its byte-BPE tables. Never imports model code."""
import argparse,gzip,hashlib,json,struct,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(Path(__file__).parent))
from deepseek_fixture import SLOTS,tensor_shapes
from text_fixtures import byte_alphabet,pack
DEST=ROOT/'psionic/fixtures/deepseek/original'
REV='ad9f0ae0864d7fbcd1cd905e3c6c5b069cc8b562'
SHAPE=dict(layers=28,width=1536,hidden=8960,heads=12,kv_heads=2,head=128,vocab=151936,capacity=128)
WEIGHT_SHA='58858233513d76b8703e72eed6ce16807b523328188e13329257fb9594462945'
def sha(b):return hashlib.sha256(b).hexdigest()
def original_tables(raw):
 j=json.loads(raw);m=j['model'];base=m['vocab'];inv=byte_alphabet();vocab=[None]*(max(x['id'] for x in j['added_tokens'])+1)
 assert j['normalizer']=={'type':'NFC'} and m['type']=='BPE' and m['dropout'] is None and not m['byte_fallback']
 expected=r"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?\p{L}+|\p{N}| ?[^\s\p{L}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+"
 assert j['pre_tokenizer']['pretokenizers'][0]['pattern']['Regex']==expected
 for text,i in base.items():vocab[i]=bytes(inv[c] for c in text)
 for x in j['added_tokens']:
  assert all(not x[k] for k in ['normalized','single_word','lstrip','rstrip'])
  vocab[x['id']]=x['content'].encode()
 assert all(vocab)
 merges=[]
 for rank,pair in enumerate(m['merges']):
  a,b=pair.split(' ') if isinstance(pair,str) else pair;merges.append((base[a],base[b],rank,base[a+b]))
 # Reuse the already published Unicode-17 DATA, not the host Python's database.
 data=(ROOT/'psionic/fixtures/text/ch05/qwen35.pst').read_bytes();counts=struct.unpack_from('<7I',data,4);at=1056;sections=[]
 for n,w in zip(counts,[8,16,16,24,12,4,1]):sections.append(data[at:at+n*w]);at+=n*w
 def rows(k,w):return list(struct.iter_unpack('<'+'I'*w,sections[k]))
 byte_ids=[base[next(c for c,v in inv.items() if v==b)] for b in range(256)]
 return pack(vocab,merges,[x['id'] for x in j['added_tokens']],rows(2,4),rows(3,6),rows(4,3),byte_ids)
def main():
 p=argparse.ArgumentParser();p.add_argument('--original',type=Path);p.add_argument('--write',action='store_true');a=p.parse_args()
 if a.original:
  raw=(a.original/'tokenizer.json').read_bytes();side={n:(a.original/n).read_bytes() for n in ['config.json','tokenizer_config.json','generation_config.json']}
  with (a.original/'model.safetensors').open('rb') as f:
   header_len=struct.unpack('<Q',f.read(8))[0];header=f.read(header_len)
  size=(a.original/'model.safetensors').stat().st_size
 else:
  raw=gzip.decompress((DEST/'tokenizer.json.gz').read_bytes());side={n:(DEST/n).read_bytes() for n in ['config.json','tokenizer_config.json','generation_config.json']};header=(DEST/'header.json').read_bytes();size=3554214621
 j=json.loads(header);expected=tensor_shapes(SHAPE);assert set(j)==set(expected)|{'__metadata__'} or set(j)==set(expected)
 for n,shape in expected.items():assert j[n]['dtype']=='BF16' and j[n]['shape']==shape,(n,j[n])
 encoded=original_tables(raw);files=[('config.json',side['config.json']),('tokenizer_config.json',side['tokenizer_config.json']),('tokenizer.json',raw)]
 pins=[dict(name=n,bytes=len(b),sha=sha(b)) for n,b in files];pins.append(dict(name='model.safetensors',bytes=size,sha=WEIGHT_SHA));pins.append(dict(name='r1.pst',bytes=len(encoded),sha=sha(encoded)))
 code='// Pinned original DATA, revision '+REV+'. Never execute upstream code.\nDS_REVISION :: "'+REV+'";\nDS_ORIGINAL_SHAPE :: DsShape.{'+','.join(f'{k}={v}' for k,v in SHAPE.items())+'};\n'
 code+='DS_PINS :: [5]DsArtifact.['+','.join('DsArtifact.{name='+json.dumps(x['name'])+',bytes='+str(x['bytes'])+',sha='+json.dumps(x['sha'])+'}' for x in pins)+'];\n'
 specs=[];start=8+len(header)
 for n,shape in expected.items():
  b,e=j[n]['data_offsets'];specs.append((n,shape,start+b,start+e))
 code+='DS_ORIGINAL_PAYLOAD :: '+str(start)+';\n'
 code+='DS_SPECS :: ['+str(len(specs))+']DsSpec.['+','.join('DsSpec.{name='+json.dumps(n)+',rank='+str(len(sh))+',rows='+str(sh[0])+',cols='+str(sh[1] if len(sh)==2 else 1)+',begin='+str(b)+',end='+str(e)+'}' for n,sh,b,e in specs)+'];\n'
 manifest=dict(schema=1,profile='deepseek-r1-qwen2-cpu-wide-v1',repository='deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B',revision=REV,shape=SHAPE,artifacts=pins,tokenizer_bos=151646,tokenizer_eos=151643,config_bos=151643,unicode='17.0.0',tensor_count=len(specs))
 outputs={**side,'tokenizer.json.gz':gzip.compress(raw,mtime=0),'r1.pst':encoded,'header.json':header,'targets.coax':code.encode(),'manifest.json':(json.dumps(manifest,indent=2)+'\n').encode()}
 for n,b in outputs.items():
  if a.write:(DEST/n).write_bytes(b)
  assert (DEST/n).read_bytes()==b,'artifact drift: '+n
 print('DeepSeek: pinned original config, tokenizer, packed Unicode-17 BPE and 339 tensor intervals verify')
if __name__=='__main__':main()
