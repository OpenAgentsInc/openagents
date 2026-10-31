#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent recount-and-merge BPE. Downloaded tokenizer Python is inert AST DATA."""
import argparse,ast,collections,gzip,hashlib,json,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];D=ROOT/'psionic/fixtures/bpe-train'
def sha(b):return hashlib.sha256(b).hexdigest()
def c(s):return '"'+''.join('\\%03o'%b for b in s.encode())+'"'
def model(documents,pattern,requested=64):
 import regex
 pieces=[list(p.encode()) for text in documents for p in regex.findall(pattern,text)];groups=len(pieces);total=sum(map(len,pieces));rules=[];vocab=[bytes([i]) for i in range(256)]
 for rank in range(requested):
  counts=collections.Counter(pair for p in pieces for pair in zip(p,p[1:]))
  if not counts:break
  pair,freq=min(counts.items(),key=lambda kv:(-kv[1],kv[0]))
  if freq<2:break
  token=vocab[pair[0]]+vocab[pair[1]];assert len(token)<=64;new=256+rank;replaced=0;updated=[]
  for p in pieces:
   q=[];i=0
   while i<len(p):
    if i+1<len(p) and (p[i],p[i+1])==pair:q.append(new);i+=2;replaced+=1
    else:q.append(p[i]);i+=1
   updated.append(q)
  pieces=updated;vocab.append(token);rules.append(dict(left=pair[0],right=pair[1],frequency=freq,replaced=replaced))
 return dict(merges=len(rules),source_bytes=total,pieces=groups,remaining=sum(map(len,pieces)),rules=rules),vocab

def pack(m,vocab,classes):
 counts=list(struct.unpack_from('<7I',classes,4));at=1056+counts[0]*8+counts[1]*16;props=classes[at:at+counts[2]*16];pool=b''.join(vocab);offset=0;spans=[]
 for token in vocab:spans.append((offset,len(token)));offset+=len(token)
 rules=sorted((r['left'],r['right'],i,256+i) for i,r in enumerate(m['rules']));words=lambda xs:struct.pack('<'+'I'*len(xs),*xs)
 return b'PST5'+words([len(vocab),len(rules),counts[2],0,0,0,len(pool)])+words(list(range(256)))+b''.join(words(x) for x in spans)+b''.join(words(x) for x in rules)+props+pool

def training_goldens(rows):
 n=len(rows);out='// Independently recounted small-corpus training traces.\n'
 fields=[('BT_REQUEST',[r['requested'] for r in rows]),('BT_DOC_COUNT',[len(r['documents']) for r in rows])]+[('BT_'+k.upper(),[r['model'][k] for r in rows]) for k in ['merges','source_bytes','pieces','remaining']]
 for name,values in fields:out+=name+' :: ['+str(n)+']int.['+','.join(map(str,values))+'];\n'
 out+='BT_DOCS :: ['+str(n)+'][4]string.['+','.join('[4]string.['+','.join(json.dumps(t) for t in r['documents']+['']*(4-len(r['documents'])))+']' for r in rows)+'];\n'
 for field in ['left','right','frequency','replaced']:
  out+='BT_'+field.upper()+' :: ['+str(n)+'][64]int.['+','.join('[64]int.['+','.join(str(r['model']['rules'][i][field]) if i<len(r['model']['rules']) else '0' for i in range(64))+']' for r in rows)+'];\n'
 return out.replace('\\u0000','\\0')

def capture(source):
 import regex
 raw=(source/'tokenizer.py').read_bytes();tree=ast.parse(raw);pattern=next(ast.literal_eval(n.value) for n in tree.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='SPLIT_PATTERN' for t in n.targets));assert r'\p{N}{1,2}' in pattern
 data=json.loads((ROOT/'psionic/fixtures/byte-lm/dataset.json').read_text());documents=[x['text'] for x in data['documents'] if x['split']=='train'];documents+=['12 12 1212 34 34 3434 56 56 5656 78 78 7878 90 90 9090'];m,vocab=model(documents,pattern);classes=(ROOT/'psionic/fixtures/text/ch05/qwen35.pst').read_bytes();table=pack(m,vocab,classes)
 texts=['','123456789','12 12','Hello, world!',"We're I'LL I'ſ hello's",'é é ́a','😀😺 hi','क ख १२३४५','١٢٣٤٥ ⅣⅤⅥ','  x','   ','\r\n \t\n x','!!!\r\n','\t x','[MASK] <|bos|>',*['a'+chr(i)+'b' for i in range(128)],*[x['text'] for x in data['documents'] if x['split']!='train']]
 vectors=[]
 for text in texts:
  pieces=regex.findall(pattern,text);assert ''.join(pieces)==text;ids=[];ends=[];end=0
  for piece in pieces:
   p=list(piece.encode());end+=len(piece.encode());ends.append(end)
   for rank,r in enumerate(m['rules']):
    q=[];i=0
    while i<len(p):
     if i+1<len(p) and p[i]==r['left'] and p[i+1]==r['right']:q.append(256+rank);i+=2
     else:q.append(p[i]);i+=1
    p=q
   ids+=p
  assert b''.join(vocab[i] for i in ids)==text.encode();vectors.append(dict(text=text,ids=ids,ends=ends))
 (D/'tokenizer.py.gz').write_bytes(gzip.compress(raw,mtime=0));(D/'LICENSE').write_bytes((source/'LICENSE').read_bytes());(D/'learned.pst').write_bytes(table)
 r=dict(schema=1,profile='byte-bpe-nanochat2-u17-v1',tie_rule='maximum overlapping adjacent-pair count; lowest (left ID,right ID); disjoint left-to-right replacements',normalization='none',source=json.loads((source/'source.json').read_text()),pattern=pattern,regex_version=regex.__version__,classes_sha256=sha(classes),dataset_sha256=sha((ROOT/'psionic/fixtures/byte-lm/dataset.json').read_bytes()),license='CC0-1.0 for chapter 112 synthetic training documents and numeric fixture; pinned nanochat source retains its LICENSE',documents=documents,model=m,vectors=vectors,packed_sha256=sha(table))
 rows=json.loads((D/'training-vectors.json').read_text())
 for case in rows:case['model']=model(case['documents'],pattern,case['requested'])[0]
 (D/'training-vectors.json').write_text(json.dumps(rows,indent=2)+'\n');(D/'training-goldens.coax').write_text(training_goldens(rows))
 r['training_vectors_sha256']=sha((D/'training-vectors.json').read_bytes());r['training_goldens_sha256']=sha((D/'training-goldens.coax').read_bytes())
 (D/'reference.json').write_text(json.dumps(r,ensure_ascii=False,indent=2)+'\n');print('Captured',m['merges'],'merges and',len(vectors),'independent regex/encoding/round-trip vectors')
def main():
 p=argparse.ArgumentParser();p.add_argument('--write',type=Path);p.add_argument('--header',action='store_true');a=p.parse_args()
 if a.write:capture(a.write);return
 r=json.loads((D/'reference.json').read_text());assert sha((D/'training-vectors.json').read_bytes())==r['training_vectors_sha256'];assert sha((D/'training-goldens.coax').read_bytes())==r['training_goldens_sha256'];assert sha((D/'learned.pst').read_bytes())==r['packed_sha256'];assert sha((ROOT/'psionic/fixtures/text/ch05/qwen35.pst').read_bytes())==r['classes_sha256'];assert sha((ROOT/'psionic/fixtures/byte-lm/dataset.json').read_bytes())==r['dataset_sha256'];assert sha(gzip.decompress((D/'tokenizer.py.gz').read_bytes()))==r['source']['files']['nanochat/tokenizer.py']['sha256'];assert sha((D/'LICENSE').read_bytes())==r['source']['files']['LICENSE']['sha256']
 if not a.header:print('Pinned nanochat pattern, owned training corpus, independent merges/table/vectors verify');return
 print('static coax_string bt_documents[]={'+','.join('{'+c(s)+','+str(len(s.encode()))+'}' for s in r['documents'])+'};')
 m=r['model'];print('static const int64_t bt_summary[]={'+','.join(str(m[k]) for k in ['merges','source_bytes','pieces','remaining'])+'};');print('static const int64_t bt_rules[][4]={'+','.join('{'+','.join(str(x[k]) for k in ['left','right','frequency','replaced'])+'}' for x in m['rules'])+'};')
 for n,v in enumerate(r['vectors']):
  for k in ['ids','ends']:print('static const int64_t bt_'+k+'_'+str(n)+'[]={'+(','.join(map(str,v[k])) or '0')+'};')
 print('static const struct bt_vector bt_vectors[]={')
 for n,v in enumerate(r['vectors']):print('{{'+c(v['text'])+','+str(len(v['text'].encode()))+'},bt_ids_'+str(n)+','+str(len(v['ids']))+',bt_ends_'+str(n)+','+str(len(v['ends']))+'},')
 print('};')
if __name__=='__main__':main()
