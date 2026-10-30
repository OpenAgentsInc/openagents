#!/usr/bin/env python3
"""Owned GGUF struct oracle; downloaded Python is parsed as inert AST DATA only."""
import argparse,ast,gzip,hashlib,json,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];DEST=ROOT/'psionic/fixtures/gguf'
WIDTH=[1,1,2,2,4,4,4,1,0,0,8,8,8]
def sha(b):return hashlib.sha256(b).hexdigest()
def layouts(source):
 tree=ast.parse(source);enum=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='GGMLQuantizationType');ids={n.targets[0].id:ast.literal_eval(n.value) for n in enum.body if isinstance(n,ast.Assign)}
 node=next(n for n in tree.body if isinstance(n,ast.AnnAssign) and getattr(n.target,'id','')=='GGML_QUANT_SIZES')
 def value(n):
  if isinstance(n,ast.Constant) and type(n.value) is int:return n.value
  if isinstance(n,ast.Name) and n.id=='QK_K':return 256
  if isinstance(n,ast.BinOp):
   a,b=value(n.left),value(n.right)
   if isinstance(n.op,ast.Add):return a+b
   if isinstance(n.op,ast.Mult):return a*b
   if isinstance(n.op,ast.FloorDiv):return a//b
  raise ValueError(ast.dump(n))
 return {ids[k.attr]:(value(v.elts[0]),value(v.elts[1])) for k,v in zip(node.value.keys,node.value.values)}
def string(s):
 b=s.encode();return struct.pack('<Q',len(b))+b

def scalar(kind,v):
 if kind==8:return string(v)
 if kind==9:
  item,values=v;return struct.pack('<IQ',item,len(values))+b''.join(scalar(item,x) for x in values)
 return struct.pack('<'+['B','b','H','h','I','i','f','B','','','Q','q','d'][kind],v)
def owned(table,all_types=False):
 metadata=[('general.architecture',8,'fixture'),('general.alignment',4,64)]
 metadata += [(f'test.k{k}',k,v) for k,v in enumerate([255,-128,65535,-32768,4294967295,-2147483648,1.25,1,'é 😀',(9,[(8,['x','']),(0,[0,255])]),18446744073709551615,-9223372036854775808,-2.5])]
 tensors=[('matrix',0,[4,2]),('quants',2,[32,2])] if not all_types else [(f'type.{k}',k,[block,2]) for k,(block,width) in sorted(table.items())]
 b=b'GGUF'+struct.pack('<IQQ',3,len(tensors),len(metadata));marks={}
 for key,kind,v in metadata:
  marks[key]=len(b);b+=string(key)+struct.pack('<I',kind)+scalar(kind,v)
 offset=0;payload=b''
 for name,kind,dims in tensors:
  block,width=table[kind];size=__import__('math').prod(dims)//block*width
  marks[name]=len(b);b+=string(name)+struct.pack('<I',len(dims))+b''.join(struct.pack('<Q',d) for d in dims)+struct.pack('<IQ',kind,offset)
  payload+=bytes(size);payload+=bytes((-len(payload))%64);offset=len(payload)
 b+=bytes((-len(b))%64);return b+payload,marks

def inspect(b,table):
 at=0
 def number(fmt):
  nonlocal at
  n=struct.calcsize('<'+fmt);v=struct.unpack_from('<'+fmt,b,at)[0];at+=n;return v
 def text():
  nonlocal at
  n=number('Q');s=b[at:at+n].decode();assert len(b[at:at+n])==n;at+=n;return s
 def item(kind):
  if kind==8:return text()
  if kind==9:
   t,n=number('I'),number('Q');return [t,[item(t) for _ in range(n)]]
  return number(['B','b','H','h','I','i','f','B','','','Q','q','d'][kind])
 assert b[:4]==b'GGUF';at=4;version=number('I');assert version==3
 nt,nm=number('Q'),number('Q');metas=[];alignment=32
 for _ in range(nm):
  key=text();kind=number('I');begin=at;v=item(kind);metas.append(dict(key=key,kind=kind,begin=begin,end=at,value=({'element_type':v[0],'count':len(v[1]),'wire_sha256':sha(b[begin:at])} if kind==9 else v)))
  if key=='general.alignment':alignment=v
 tensors=[]
 for _ in range(nt):
  name=text();rank=number('I');dims=[number('Q') for _ in range(rank)];kind=number('I');offset=number('Q');block,width=table[kind];elements=__import__('math').prod(dims);assert dims[0]%block==0
  tensors.append(dict(name=name,rank=rank,dims=dims,kind=kind,offset=offset,elements=elements,bytes=elements//block*width))
 data=(at+alignment-1)//alignment*alignment if nt else 0
 return dict(metadata=metas,tensors=tensors,header_end=at,data=data,alignment=alignment,file_bytes=len(b))
def main():
 p=argparse.ArgumentParser();p.add_argument('--write',type=Path);p.add_argument('--extract',type=Path);p.add_argument('--header',action='store_true');a=p.parse_args()
 if a.write:
  src=a.write;raw=(src/'constants.py').read_bytes();table=layouts(raw.decode());blocks=[table.get(k,(0,0))[0] for k in range(44)];sizes=[table.get(k,(0,0))[1] for k in range(44)]
  out='// Pinned GGML layout DATA and independently authored GGUF fixtures.\n'
  for name,values in [('PG_BLOCKS',blocks),('PG_WIDTHS',sizes)]:out+=name+' :: [44]int.['+','.join(map(str,values))+'];\n'
  outputs={};record={'schema':1,'llama_cpp_revision':(src/'revision').read_text().strip(),'ggml_spec_revision':(src/'ggml-revision').read_text().strip(),'layouts':{str(k):list(v) for k,v in table.items()},'files':{}}
  for f in ['constants.py','gguf.cpp','gguf.md']:
   outputs['source-'+f.replace('.','-')+'.gz']=gzip.compress((src/f).read_bytes(),mtime=0)
  for label,all_types in [('owned',False),('layouts',True)]:
   data,marks=owned(table,all_types);outputs[label+'.gguf.gz']=gzip.compress(data,mtime=0);outputs[label+'.json']=(json.dumps(dict(inspect(data,table),marks=marks),ensure_ascii=False,indent=2)+'\n').encode();out+='PG_'+label.upper()+' :: ['+str(len(data))+']u8.['+','.join(map(str,data))+'];\n'
  for label,filename in [('tiny','upstream-le.gguf'),('vocab','upstream-vocab.gguf'),('big-endian','upstream-tiny.gguf')]:
   data=(src/filename).read_bytes();outputs[label+'.gguf.gz']=gzip.compress(data,mtime=0)
   info=json.loads((src/('upstream-le-source.json' if label=='tiny' else 'upstream-vocab-source.json' if label=='vocab' else 'upstream-tiny-source.json')).read_text());record['files'][label]=info
   if label!='big-endian':outputs[label+'.json']=(json.dumps(inspect(data,table),ensure_ascii=False,indent=2)+'\n').encode()
  base,marks=owned(table);parsed=inspect(base,table);faults=[]
  def fault(at,width,value,status):faults.append((at,width,value,status))
  for value in [0,1,2,0x03000000]:fault(4,4,value,6)
  fault(0,4,0,5)
  for at in [8,16]:
   for value in [4097,18446744073709551615]:fault(at,8,value,7)
  for value,status in [(0,5),(257,7),(18446744073709551615,7)]:fault(24,8,value,status)
  fault(32,1,0,5);fault(32,1,255,8)
  for m in parsed['metadata']:
   fault(m['begin']-4,4,13,6)
   if m['key']=='general.alignment':
    for value in [0,4,24,1048577]:fault(m['begin'],4,value,11)
   if m['kind']==7:fault(m['begin'],1,2,5)
   if m['kind']==8:fault(m['begin'],8,1048577,7)
   if m['kind']==9:
    fault(m['begin'],4,13,6);fault(m['begin']+4,8,18446744073709551615,7)
  at=marks['test.k1']+8+len('test.k1')-1;fault(at,1,ord('0'),9)
  for tensor in parsed['tensors']:
   start=marks[tensor['name']];rank_at=start+8+len(tensor['name']);dims_at=rank_at+4;type_at=dims_at+8*tensor['rank'];off_at=type_at+4
   for value in [0,5]:fault(rank_at,4,value,10)
   fault(dims_at,8,0,10);fault(dims_at,8,17592186044417,7)
   for value in [4,44]:fault(type_at,4,value,6)
   for value,status in [(1,11),(2048,11),(1099511627777,7)]:fault(off_at,8,value,status)
   if tensor['kind']==2:fault(dims_at,8,31,10);fault(off_at,8,0,11);fault(start+8,6,int.from_bytes(b'matrix','little'),9)
  fault(575,1,1,12)
  for name,col,typ in [('PG_BAD_AT',0,'int'),('PG_BAD_WIDTH',1,'int'),('PG_BAD_WORD',2,'u64'),('PG_BAD_STATUS',3,'int')]:out+=name+' :: ['+str(len(faults))+']'+typ+'.['+','.join(str(f[col]) for f in faults)+'];\n'
  for depth in [8,9]:
   node=(0,[7])
   for _ in range(depth-1):node=(9,[node])
   wire=scalar(9,node);out+='PG_DEPTH'+str(depth)+' :: ['+str(len(wire))+']u8.['+','.join(map(str,wire))+'];\n'
  ranks=b'GGUF'+struct.pack('<IQQ',3,4,0);payload=b''
  for rank in range(1,5):
   name='x'*64 if rank==1 else chr(96+rank);dims=[4]*rank
   ranks+=string(name)+struct.pack('<I',rank)+struct.pack('<'+'Q'*rank,*dims)+struct.pack('<IQ',0,len(payload));payload+=bytes(4*4**rank);payload+=bytes((-len(payload))%32)
  ranks+=bytes((-len(ranks))%32);ranks+=payload;out+='PG_RANKS :: ['+str(len(ranks))+']u8.['+','.join(map(str,ranks))+'];\n'
  outputs['goldens.coax']=out.encode();record['artifacts']={n:dict(bytes=len(b),sha256=sha(b)) for n,b in outputs.items()}
  for n,b in outputs.items():(DEST/n).write_bytes(b)
  (DEST/'source.json').write_text(json.dumps(record,indent=2)+'\n');print('GGUF: inert-source layouts and owned/upstream DATA authored');return
 record=json.loads((DEST/'source.json').read_text())
 for n,pin in record['artifacts'].items():
  b=(DEST/n).read_bytes();assert sha(b)==pin['sha256'] and len(b)==pin['bytes'],n
 if a.extract:
  a.extract.mkdir(exist_ok=True,parents=True)
  for label in ['owned','layouts','tiny','vocab','big-endian']:(a.extract/(label+'.gguf')).write_bytes(gzip.decompress((DEST/(label+'.gguf.gz')).read_bytes()))
 if a.header:
  for label in ['owned','layouts','tiny','vocab']:
   j=json.loads((DEST/(label+'.json')).read_text());prefix='pg_'+label
   print('static const int64_t '+prefix+'_summary[]={'+','.join(map(str,[len(j['metadata']),len(j['tensors']),j['data'],j['alignment']]))+'};')
   print('static const char *'+prefix+'_names[]={'+(','.join(json.dumps(t['name']) for t in j['tensors']) or '0')+'};')
   print('static const int64_t '+prefix+'_tensors[][9]={'+(','.join('{'+','.join(map(str,[t['rank'],t['kind'],t['elements'],t['offset'],t['bytes']]+t['dims']+[1]*(4-len(t['dims']))))+'}' for t in j['tensors']) or '{0}')+'};')
   print('static const char *'+prefix+'_keys[]={'+','.join(json.dumps(t['key']) for t in j['metadata'])+'};')
   print('static const int64_t '+prefix+'_meta[][3]={'+','.join('{'+','.join(map(str,[t['kind'],t['begin'],t['end']]))+'}' for t in j['metadata'])+'};')
 else:print('GGUF pinned sources, independent layouts and fixture identities verify')
if __name__=='__main__':main()
