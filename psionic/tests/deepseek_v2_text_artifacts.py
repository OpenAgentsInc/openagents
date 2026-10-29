#!/usr/bin/env python3
"""Pack original token bytes and a reviewed original-recipe prompt. No remote code."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re
import struct
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent))
from text_fixtures import byte_alphabet
ROOT=Path(__file__).resolve().parents[2]
DEST=ROOT/'psionic/fixtures/deepseek/v2-original'


def pieces(text,pre):
    segments=[text]
    for step in pre['pretokenizers']:
        if step['type']=='ByteLevel':continue
        pattern=step['pattern']['Regex'] if step['type']=='Split' else r'\d'
        result=[]
        for value in segments:
            at=0
            for match in re.finditer(pattern,value):
                if match.start()!=at:result.append(value[at:match.start()])
                result.append(match[0]);at=match.end()
            if at!=len(value):result.append(value[at:])
        segments=result
    return segments


def encode(text,j):
    inv=byte_alphabet();alphabet={v:k for k,v in inv.items()};vocab=j['model']['vocab'];rank={tuple(pair.split(' ') if isinstance(pair,str) else pair):i for i,pair in enumerate(j['model']['merges'])};out=[]
    for piece in pieces(text,j['pre_tokenizer']):
        symbols=[alphabet[x] for x in piece.encode()]
        while len(symbols)>1:
            choices=[(rank[pair],i) for i,pair in enumerate(zip(symbols,symbols[1:])) if pair in rank]
            if not choices:break
            _,i=min(choices);symbols[i:i+2]=[symbols[i]+symbols[i+1]]
        out.extend(vocab[x] for x in symbols)
    return out


def main():
    p=argparse.ArgumentParser();p.add_argument('--original',type=Path);p.add_argument('--write',action='store_true');a=p.parse_args()
    raw=(a.original/'tokenizer.json').read_bytes() if a.original else gzip.decompress((DEST/'tokenizer.json.gz').read_bytes());j=json.loads(raw)
    assert j['model']['type']=='BPE' and j['normalizer']=={'type':'Sequence','normalizers':[]};assert [(t['id'],t['content']) for t in j['added_tokens']]==[(100000,'<｜begin▁of▁sentence｜>'),(100001,'<｜end▁of▁sentence｜>')]
    inv=byte_alphabet();vocab=[None]*100002
    for text,i in j['model']['vocab'].items():vocab[i]=bytes(inv[c] for c in text)
    for t in j['added_tokens']:vocab[t['id']]=t['content'].encode()
    assert all(vocab);pool=b'';index=b''
    for value in vocab:index+=struct.pack('<II',len(pool),len(value));pool+=value
    table=b'DV2T0001'+struct.pack('<II',len(vocab),len(pool))+index+pool
    text='User: Hi\n\nAssistant:';tokens=[100000]+encode(text,j);assert b''.join(vocab[i] for i in tokens[1:])==text.encode()
    prompt=dict(schema=1,revision='85864749cd611b4353ce1decdb286193298f64c7',messages=[dict(role='user',content='Hi')],rendered_after_bos=text,tokens=tokens,
                scope='Pinned original recipe and tokenizer DATA; ASCII fixture uses independent ordered-regex splitting and ranked BPE. General Coax text encoding is not claimed here.')
    target='// Original-derived token-byte DATA; no tokenizer runtime dependency.\nDV_TEXT_BYTES :: '+str(len(table))+';\nDV_TEXT_SHA :: "'+hashlib.sha256(table).hexdigest()+'";\n'
    toy_pieces=[b'a','😀'.encode(),b'</think>'];toy_pool=b''.join(toy_pieces);toy=b'DV2T0001'+struct.pack('<II',3,len(toy_pool));at=0
    for part in toy_pieces:toy+=struct.pack('<II',at,len(part));at+=len(part)
    toy+=toy_pool;target+='DV_TEXT_TOY :: ['+str(len(toy))+']u8.['+','.join(map(str,toy))+'];\n'
    target+='DV_PROMPT_HI :: ['+str(len(tokens))+']int.['+','.join(map(str,tokens))+'];\n'
    manifest=dict(schema=1,revision=prompt['revision'],source_tokenizer_bytes=len(raw),source_tokenizer_sha256=hashlib.sha256(raw).hexdigest(),table_bytes=len(table),table_sha256=hashlib.sha256(table).hexdigest(),entries=len(vocab),bos=100000,eos=100001)
    outputs={'tokenizer.json.gz':gzip.compress(raw,mtime=0),'v2-token-bytes.bin':table,'text-targets.coax':target.encode(),'prompt-hi.json':(json.dumps(prompt,indent=2)+'\n').encode(),'text-manifest.json':(json.dumps(manifest,indent=2)+'\n').encode()}
    for name,data in outputs.items():
        if a.write:(DEST/name).write_bytes(data)
        else:assert (DEST/name).read_bytes()==data,f'stale {name}'
    print('V2 token DATA:',len(vocab),'entries;',len(table),'bytes; Hi prompt',tokens)


if __name__=='__main__':main()
