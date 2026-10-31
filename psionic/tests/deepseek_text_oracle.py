#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent regex + heap BPE and original Jinja DATA for authoring vectors."""
import argparse,gzip,heapq,json,struct,unicodedata,sys
from pathlib import Path
import regex
from jinja2 import Template
ROOT=Path(__file__).resolve().parents[2];DEST=ROOT/'psionic/fixtures/deepseek'
sys.path.insert(0,str(Path(__file__).parent))
from text_fixtures import byte_alphabet
ORIGINAL=DEST/'original'
SAMPLES=["","Hi","ababa hello's","a\nb","a\rb"," \t \nx"," \t x","a b","  hello","!!!\r\n","1234567890","'RE 've 'll 'm 'd 't 's", "e\u0301 a\u0308\u0301","'ſauce","\nhello\rworld","b\u0308y","!? a! 123", "  \t  ","a\n\rb", "\r\na\nb", "\u00a0hello", "中文 日本語 한국어"]
def encode_fn(vocab,merges,pattern,alphabet):
 lookup={v:i for i,v in enumerate(vocab)};ranks={(a,b):(rank,id) for a,b,rank,id in merges}
 def piece(s):
  ids=[alphabet[x] for x in s];heap=[];n=len(ids);prev=list(range(-1,n-1));nxt=list(range(1,n+1));alive=[True]*n;version=[0]*n
  def push(i):
   if i<0 or i>=n or not alive[i] or nxt[i]>=n:return
   pair=ranks.get((ids[i],ids[nxt[i]]))
   if pair is not None:heapq.heappush(heap,(pair[0],i,nxt[i],version[i],version[nxt[i]],pair[1]))
  for i in range(n):push(i)
  while heap:
   rank,a,b,va,vb,id=heapq.heappop(heap)
   if not alive[a] or not alive[b] or nxt[a]!=b or version[a]!=va or version[b]!=vb:continue
   ids[a]=id;version[a]+=1;alive[b]=False;nxt[a]=nxt[b]
   if nxt[b]<n:prev[nxt[b]]=a
   push(prev[a]);push(a)
  return [ids[i] for i in range(n) if alive[i]]
 def encode(s):
  text=unicodedata.normalize('NFC',s);return [id for m in regex.finditer(pattern,text) for id in piece(m.group().encode())]
 return encode
def main():
 p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');a=p.parse_args();j=json.loads(gzip.decompress((ORIGINAL/'tokenizer.json.gz').read_bytes()));pattern=j['pre_tokenizer']['pretokenizers'][0]['pattern']['Regex'];base=j['model']['vocab'];inv=byte_alphabet();vocab=[None]*(max(base.values())+1)
 for text,id in base.items():vocab[id]=bytes(inv[c] for c in text)
 merges=[(base[x],base[y],i,base[x+y]) for i,pair in enumerate(j['model']['merges']) for x,y in [pair.split(' ')]];alphabet=[base[next(c for c,v in inv.items() if v==b)] for b in range(256)];encode=encode_fn(vocab,merges,pattern,alphabet)
 gold=[(s,encode(s)) for s in SAMPLES]
 # Toy shares exactly the byte alphabet and merge semantics; its properties cover <1024.
 toy=(ROOT/'psionic/fixtures/text/ch05/toy.pst').read_bytes();counts=struct.unpack_from('<7I',toy,4);spans=1056;at=spans+8*counts[0];tm=list(struct.iter_unpack('<4I',toy[at:at+16*counts[1]]));toy_alphabet=struct.unpack_from('<256I',toy,32);enc_toy=encode_fn([],tm,pattern,toy_alphabet)
 cases=[]
 for s in SAMPLES[:-1]:
  raw=s.encode();ends=[]
  for m in regex.finditer(pattern,s):ends.append(len(s[:m.end()].encode()))
  if raw:cases.append((s,ends))
 code='// Independently authored original-regex span and heap-merge vectors.\n'
 for c,(s,ends) in enumerate(cases):
  code+=f'#test "DeepSeek independent regex spans {c}" {{table:=PT_TOY;t:=pt_layout(table[..]);text:='+json.dumps(s,ensure_ascii=False)+';at:=0;\n'
  for end in ends:code+=f'#assert ds_piece(table[..],t,text,at)=={end};at={end};\n'
  code+='}\n'
 for c,s in enumerate(SAMPLES[:-1]):
  ids=enc_toy(s);code+=f'#test "DeepSeek independent heap BPE {c}" {{table:=PT_TOY;out:[256]int;stage:[256]int;normal:[1024]u8;scalars:[1024]int;symbols:[1024]int;r:=ds_encode(table[..],'+json.dumps(s,ensure_ascii=False)+',out[..],stage[..],normal[..],scalars[..],symbols[..],false);#assert r.status==PtStatus.ok && r.written=='+str(len(ids))+' && r.work=='+str(sum(len(m.group().encode())**2 for m in regex.finditer(pattern,unicodedata.normalize('NFC',s))))+';'
  for i,id in enumerate(ids):code+=f'#assert out[{i}]=={id};'
  code+='}\n'
 header='static const char *ds_texts[]={'+','.join(json.dumps(s,ensure_ascii=False) for s,_ in gold)+'};\nstatic const int ds_text_counts[]={'+','.join(str(len(ids)) for _,ids in gold)+'};\nstatic const int64_t ds_text_ids[][256]={'+','.join('{'+','.join(map(str,ids))+'}' if ids else '{0}' for _,ids in gold)+'};\n'
 # Interpret only the pinned original template DATA, not upstream model code.
 config=json.loads((ORIGINAL/'tokenizer_config.json').read_bytes());messages=[dict(role='system',content='Be brief.'),dict(role='user',content='Hi'),dict(role='assistant',content='<think>private</think>Hello!'),dict(role='user',content='1+1?')];rendered=Template(config['chat_template']).render(messages=messages,bos_token=config['bos_token']['content'],add_generation_prompt=True)
 added={x['content']:x['id'] for x in j['added_tokens']};special=regex.compile('|'.join(regex.escape(x) for x in sorted(added,key=len,reverse=True)));tokens=[];at=0
 for m in special.finditer(rendered):tokens+=encode(rendered[at:m.start()]);tokens.append(added[m.group()]);at=m.end()
 tokens+=encode(rendered[at:]);header+='static const int64_t ds_history_ids[]={'+','.join(map(str,tokens))+'};\n'
 outputs={'text-goldens.coax':code.encode(),'text-goldens.h':header.encode(),'text-reference.json':(json.dumps(dict(regex=regex.__version__,unicode=unicodedata.unidata_version,samples=gold,rendered=rendered,history_tokens=tokens),ensure_ascii=False,indent=2)+'\n').encode()}
 for n,b in outputs.items():
  if a.write:(DEST/n).write_bytes(b)
  assert (DEST/n).read_bytes()==b,'text oracle drift: '+n
 print('DeepSeek original regex, heap-BPE and original Jinja-template vectors verify')
if __name__=='__main__':main()
