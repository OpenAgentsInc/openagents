#!/usr/bin/env python3
"""Owned packing of original DATA. No upstream code is imported or executed."""
import argparse
import base64
import gzip
import hashlib
import json
import struct
import unicodedata as ud
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
DEST=ROOT/'psionic/fixtures/harmony'
ORIGINAL_SHA='446a9538cb6c348e3516120d7c08b09f57c36495e2acfffe59a5bf8b0cfb1a2d'
PATTERN=r"[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]*[\p{Ll}\p{Lm}\p{Lo}\p{M}]+(?i:'s|'t|'re|'ve|'m|'ll|'d)?|[^\r\n\p{L}\p{N}]?[\p{Lu}\p{Lt}\p{Lm}\p{Lo}\p{M}]+[\p{Ll}\p{Lm}\p{Lo}\p{M}]*(?i:'s|'t|'re|'ve|'m|'ll|'d)?|\p{N}{1,3}| ?[^\s\p{L}\p{N}]+[\r\n/]*|\s*[\r\n]+|\s+(?!\S)|\s+"
GOSS='5681a6b6552635e7b29cde94a61ca6a7aee90ebd'
HARMONY='abd677f7ac962629c808197caa1feb9e3e95d2b0'
def sha(b):return hashlib.sha256(b).hexdigest()
def words(xs):return b''.join(struct.pack('<I',x) for x in xs)
def properties():
    assert ud.unidata_version=='17.0.0'
    rows=[];last=None
    for cp in range(0x110000):
        cat=ud.category(chr(cp));ws=chr(cp).isspace() and cp not in range(28,32)
        flags=(1 if cat.startswith('L') else 2 if cat.startswith('M') else 4 if cat.startswith('N') else 0)
        flags|=(8 if ws else 0)|(16 if cat in ('Lu','Lt') else 32 if cat=='Ll' else 64 if cat in ('Lm','Lo') else 0)
        if flags!=last:
            if last:rows.append((begin,cp-1,last,0))
            begin=cp;last=flags
    if last:rows.append((begin,0x10ffff,last,0))
    return rows

def pack(vocab,pairs,props):
    pool=bytearray();spans=[]
    for b in vocab:spans.append((len(pool),len(b)));pool.extend(b)
    order=sorted(range(len(vocab)),key=lambda i:vocab[i])
    counts=[len(vocab),len(pairs),len(props),0,0,len(order),len(pool)]
    byte_ids={b[0]:i for i,b in enumerate(vocab) if len(b)==1}
    assert len(byte_ids)==256 and counts[1]<=524288
    data=b'HST7'+words(counts)+words(byte_ids[i] for i in range(256))
    data+=b''.join(words(row) for rows in (spans,sorted(pairs),props) for row in rows)+words(order)+pool
    return data,counts

def build(original):
    assert sha(original)==ORIGINAL_SHA
    ranks={base64.b64decode(a):int(b) for a,b in (line.split() for line in original.splitlines())}
    assert len(ranks)==199998 and set(ranks.values())==set(range(199998))
    vocab=[None]*len(ranks)
    for b,i in ranks.items():vocab[i]=b
    # Every possible concatenation must be available, including components newer
    # than the resulting rank. A single historical training pair is insufficient.
    pairs=[(ranks[b[:i]],ranks[b[i:]],rank,rank) for b,rank in ranks.items() for i in range(1,len(b)) if b[:i] in ranks and b[i:] in ranks]
    return pack(vocab,pairs,properties())

def toy():
    vocab=[bytes([i]) for i in range(256)];pairs=[]
    for token in [b'ab',b'aba',b'he',b'hel',b'hell',b'hello',b' hello',b"'s",b'aa',b'ba']:
        idx=len(vocab)
        lookup={b:i for i,b in enumerate(vocab)}
        for split in range(1,len(token)):
            if token[:split] in lookup and token[split:] in lookup:pairs.append((lookup[token[:split]],lookup[token[split:]],idx-256,idx))
        vocab.append(token)
    return pack(vocab,pairs,properties())

def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--original');args=p.parse_args()
    original=Path(args.original).read_bytes() if args.original else gzip.decompress((DEST/'o200k_base.tiktoken.gz').read_bytes())
    table,counts=build(original);small,small_counts=toy()
    outputs={'o200k_base.tiktoken.gz':gzip.compress(original,mtime=0),'gptoss.hst':table,'toy.hst':small,
        'toy.coax':('HS_TOY :: ['+str(len(small))+']u8.['+','.join(map(str,small))+'];\n').encode(),
        'pins.coax':('// Original ranks and deterministic derived table; no executable upstream code.\nHS_ORIGINAL_SHA :: '+json.dumps(ORIGINAL_SHA)+';\nHS_TABLE_SHA :: '+json.dumps(sha(table))+';\n').encode()}
    manifest={'schema':1,'profile':'gpt-oss-harmony-text-v1','format':'HST7','gpt_oss_revision':GOSS,'harmony_revision':HARMONY,'original_url':'https://openaipublic.blob.core.windows.net/encodings/o200k_base.tiktoken','original_sha256':ORIGINAL_SHA,'unicode':ud.unidata_version,'pattern':PATTERN,'ordinary_ranks':counts[0],'pair_records':counts[1],'property_ranges':counts[2],'vocabulary_entries':201088,'highest_id':201087,'sections':counts,'files':{k:{'bytes':len(v),'sha256':sha(v)} for k,v in outputs.items()}}
    outputs['manifest.json']=(json.dumps(manifest,indent=2)+'\n').encode()
    for name,data in outputs.items():
        if args.write:(DEST/name).write_bytes(data)
        assert (DEST/name).read_bytes()==data,'fixture drift: '+name
    print(f'Harmony data verified: {counts[0]} original ranks, {counts[1]} concatenation pairs, Unicode {ud.unidata_version}')
if __name__=='__main__':main()
