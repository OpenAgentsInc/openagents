#!/usr/bin/env python3
"""Recorded upstream tokenizers/Jinja DATA, no external runtime dependencies."""
import gzip,hashlib,json,sys
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'fixtures/tokenizer-families'
def c(s):return '"'+''.join('\\%03o'%b for b in s.encode())+'"'
def main():
 j=json.loads((D/'source.json').read_text())
 for f,r in j['artifacts'].items():
  data=(D/f).read_bytes();assert len(data)==r['bytes'] and hashlib.sha256(data).hexdigest()==r['sha256'],f
 if '--header' not in sys.argv:print('Pinned full BERT/T5 tokenizers, upstream ID vectors and Jinja outputs verify');return
 for model in ['bert','t5']:
  j=json.loads(gzip.decompress((D/(model+'-tokenizer.json.gz')).read_bytes()));raw=j['model']['vocab'];pieces=[(t,i,0.) for t,i in raw.items()] if model=='bert' else [(x[0],i,x[1]) for i,x in enumerate(raw)];pieces.sort(key=lambda p:p[0].encode());print('static coax_t_TfPiece '+model+'_vocab[]={')
  for t,i,s in pieces:print('{.text={'+c(t)+','+str(len(t.encode()))+'},.id='+str(i)+',.score='+repr(s)+'},')
  print('};')
  for n,v in enumerate(json.loads((D/(model+'-vectors.json')).read_text())):
   print('static const int64_t '+model+'_ids_'+str(n)+'[]={'+(','.join(map(str,v['ids'])) or '0')+'};')
  print('static const struct vector '+model+'_vectors[]={')
  for n,v in enumerate(json.loads((D/(model+'-vectors.json')).read_text())):print('{'+c(v['text'])+','+str(len(v['text'].encode()))+','+model+'_ids_'+str(n)+','+str(len(v['ids']))+'},')
  print('};')
 print('static const struct chat chats[]={')
 for v in json.loads((D/'chat-vectors.json').read_text()):
  cs=lambda s:'{'+c(s)+','+str(len(s.encode()))+'}'
  messages=','.join('{'+cs(m['role'])+','+cs(m['content'])+'}' for m in v['messages']) or '{{0,0},{0,0}}'
  print('{'+cs(v['template'])+','+cs(v['bos'])+','+cs(v['eos'])+','+cs(v['expected'])+','+str(int(v['generation']))+','+str(len(v['messages']))+',{'+messages+'}},')
 print('};')
if __name__=='__main__':main()
