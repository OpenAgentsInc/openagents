#!/usr/bin/env python3
"""Owned tiny Granite GGUF: two layers, every matrix format, a byte-level
tokenizer. Synthetic, deterministic weights; no model data.
  granite_fixture.py --gguf PATH     write the tiny model file
  granite_fixture.py --coax          print its header and offsets as Coax DATA
"""
import argparse,struct

def string(s):
    b=s.encode() if isinstance(s,str) else s
    return struct.pack('<Q',len(b))+b
FMT={4:'I',5:'i',6:'f',7:'B',10:'Q'}
def value(kind,v):
    if kind==8:return string(v)
    if kind==9:
        item,values=v;return struct.pack('<IQ',item,len(values))+b''.join(value(item,x) for x in values)
    return struct.pack('<'+FMT[kind],v)

def gpt2_bytes():
    keep=list(range(33,127))+list(range(161,173))+list(range(174,256));chars=keep[:];n=0
    for b in range(256):
        if b not in keep:keep.append(b);chars.append(256+n);n+=1
    return {b:chr(c) for b,c in zip(keep,chars)}

class Rng:
    def __init__(s,seed):s.x=seed&0xffffffffffffffff or 1
    def next(s):
        s.x^=(s.x<<13)&0xffffffffffffffff;s.x^=s.x>>7;s.x^=(s.x<<17)&0xffffffffffffffff;return s.x
    def byte(s):return s.next()>>56

def half(x):return struct.unpack('<H',struct.pack('<e',x))[0]
def q4k_block(r):
    b=bytearray(144);struct.pack_into('<HH',b,0,half(0.004+0.002*(r.byte()/255)),half(0.002+0.001*(r.byte()/255)))
    for i in range(4,144):b[i]=r.byte()
    return bytes(b)
def q6k_block(r):
    b=bytearray(210)
    for i in range(192):b[i]=r.byte()
    for i in range(192,208):b[i]=(r.byte()%31)+1
    struct.pack_into('<H',b,208,half(0.0005+0.0005*(r.byte()/255)));return bytes(b)
def q8_block(r):
    b=bytearray(34);struct.pack_into('<H',b,0,half(0.0005+0.0005*(r.byte()/255)))
    for i in range(2,34):b[i]=r.byte()
    return bytes(b)
BLOCK={12:(256,q4k_block),14:(256,q6k_block),8:(32,q8_block)}

LAYERS=2;WIDTH=256;HIDDEN=512;HEADS=2;KV=1;HEAD=128
SPECIAL=['<|im_start|>','<|im_end|>','<think>','</think>','<s>','<unk>','<tool_call>','</tool_call>']
MERGES=[('Ġ','t'),('h','e'),('Ġt','he'),('i','n'),('Ġ','a'),('e','r'),('Ġ','s'),('o','n'),('r','e'),('Ġ','w'),('l','l'),('Ġ','h'),('he','ll'),('hell','o'),('Ġ','Ġ'),('ĠĠ','ĠĠ'),('1','2'),('Ċ','Ċ')]
def tokenizer():
    m=gpt2_bytes();tokens=[m[b] for b in range(256)];types=[1]*256;merges=[]
    for a,b in MERGES:
        merges.append(a+' '+b)
        if a+b not in tokens:tokens.append(a+b);types.append(1)
    while len(tokens)%8!=0 or len(tokens)<300:tokens.append('<|unused_%d|>'%len(tokens));types.append(3)
    for s in SPECIAL:tokens.append(s);types.append(3 if s.startswith('<|') or s in ('<s>','<unk>') else 4)
    while len(tokens)%8:tokens.append('<|pad_%d|>'%len(tokens));types.append(3)
    return tokens,types,merges

def build(variant=False):
    tokens,types,merges=tokenizer();vocab=len(tokens)
    if variant:merges=merges[:1]
    meta=[('general.architecture',8,'granite'),('general.name',8,'psionic tiny granite'),
      ('granite.block_count',4,LAYERS),('granite.context_length',4,4096),('granite.embedding_length',4,WIDTH),
      ('granite.feed_forward_length',4,HIDDEN),('granite.attention.head_count',4,HEADS),('granite.attention.head_count_kv',4,KV),
      ('granite.rope.freq_base',6,10000000.0),('granite.attention.layer_norm_rms_epsilon',6,1e-5),('granite.vocab_size',4,vocab),
      ('granite.rope.dimension_count',4,HEAD),('granite.attention.scale',6,0.0078125),('granite.embedding_scale',6,1.0),
      ('granite.residual_scale',6,1.0),('granite.logit_scale',6,1.0),('tokenizer.ggml.model',8,'gpt2'),('tokenizer.ggml.pre',8,'granite-docling'),
      ('tokenizer.ggml.tokens',9,(8,tokens)),('tokenizer.ggml.token_type',9,(5,types)),('tokenizer.ggml.merges',9,(8,merges)),
      ('tokenizer.ggml.bos_token_id',4,tokens.index('<s>')),('tokenizer.ggml.eos_token_id',4,tokens.index('<|im_end|>')),
      ('tokenizer.ggml.unknown_token_id',4,tokens.index('<unk>')),('tokenizer.chat_template',8,'{% for message in messages %}{{ message[\'content\'] }}{% endfor %}')]
    # The variant keeps one merge and puts the chat template first, for the binding's edge checks.
    if variant:meta=[meta[-1]]+meta[:-1]
    kv=KV*HEAD;r=Rng(0x6772616e697465)
    def layer_kinds(l):
        return [12,12,14,12,12,12,14] if l==0 else [8]*7
    tensors=[('token_embd.weight',12,[WIDTH,vocab]),('output_norm.weight',0,[WIDTH]),('output.weight',14,[WIDTH,vocab])]
    for l in range(LAYERS):
        q,k,v,o,g,u,d=layer_kinds(l)
        tensors+=[(f'blk.{l}.attn_norm.weight',0,[WIDTH]),(f'blk.{l}.attn_q.weight',q,[WIDTH,WIDTH]),(f'blk.{l}.attn_k.weight',k,[WIDTH,kv]),
          (f'blk.{l}.attn_v.weight',v,[WIDTH,kv]),(f'blk.{l}.attn_output.weight',o,[WIDTH,WIDTH]),(f'blk.{l}.ffn_norm.weight',0,[WIDTH]),
          (f'blk.{l}.ffn_gate.weight',g,[WIDTH,HIDDEN]),(f'blk.{l}.ffn_up.weight',u,[WIDTH,HIDDEN]),(f'blk.{l}.ffn_down.weight',d,[HIDDEN,WIDTH])]
    head=b'GGUF'+struct.pack('<IQQ',3,len(tensors),len(meta));marks={}
    for key,kind,v in meta:
        head+=string(key)+struct.pack('<I',kind);marks[key]=len(head);head+=value(kind,v)
    payload=b'';offsets=[]
    for name,kind,dims in tensors:
        if kind==0:
            data=b''.join(struct.pack('<f',0.75+0.5*(r.byte()/255)) for _ in range(dims[0]))
        else:
            block,f=BLOCK[kind];data=b''.join(f(r) for _ in range(dims[0]*dims[1]//block))
        offsets.append(len(payload));payload+=data+bytes((-len(data))%32)
        head+=string(name)+struct.pack('<I',len(dims))+b''.join(struct.pack('<Q',x) for x in dims)+struct.pack('<IQ',kind,offsets[-1])
    head+=bytes((-len(head))%32)
    return head,head+payload,marks,vocab

def main():
    p=argparse.ArgumentParser();p.add_argument('--gguf');p.add_argument('--coax',action='store_true');a=p.parse_args()
    head,file,marks,vocab=build()
    if a.gguf:open(a.gguf,'wb').write(file)
    if a.coax:
        print('// Generated by psionic/tests/granite_fixture.py: the owned tiny Granite GGUF header.')
        print(f'GR_TINY_BYTES :: {len(file)};')
        print(f'GR_TINY_VOCAB :: {vocab};')
        for key,name in [('general.architecture','ARCH'),('granite.embedding_scale','EMBED_SCALE'),('granite.residual_scale','RESIDUAL_SCALE'),('granite.logit_scale','LOGIT_SCALE'),('granite.attention.scale','ATTENTION_SCALE'),('granite.block_count','LAYERS'),('granite.attention.head_count','HEADS'),('granite.rope.dimension_count','ROPE_DIMS')]:
            print(f'GR_TINY_AT_{name} :: {marks[key]};')
        print(f'GR_TINY_HEADER :: [{len(head)}]u8.['+','.join(str(x) for x in head)+'];')
        head,file,marks,vocab=build(True)
        print('// The same model with one merge and the chat template as the first key.')
        print(f'GR_TINY_ONE_BYTES :: {len(file)};')
        print(f'GR_TINY_ONE_HEADER :: [{len(head)}]u8.['+','.join(str(x) for x in head)+'];')
if __name__=='__main__':main()
