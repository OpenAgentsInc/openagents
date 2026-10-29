#!/usr/bin/env python3
"""Owned tooling: pack pinned tokenizer DATA and Unicode 17 tables, never execute it."""
import argparse, gzip, hashlib, json, struct, unicodedata as ud
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
DEST=ROOT/'psionic/fixtures/text/ch05'
ORIGINAL_SHA='a5cd9732badce41de57e6efce8302930ded1c1188c5f81feb2bd6c24c4a1941f'
REGEX=r"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?[\p{L}\p{M}]+|\p{N}| ?[^\s\p{L}\p{M}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+"
def sha(b):return hashlib.sha256(b).hexdigest()
def words(items):return b''.join(struct.pack('<I',x) for x in items)
def properties():
    assert ud.unidata_version=='17.0.0','fixture authoring requires Unicode 17.0.0'
    result=[];last=None
    for cp in range(0x110000):
        c=chr(cp);cat=ud.category(c)
        flags=(1 if cat.startswith('L') else 2 if cat.startswith('M') else 4 if cat.startswith('N') else 0)
        if c.isspace():flags|=16
        if c.isspace() and cp not in range(0x1c,0x20):flags|=8
        key=(flags,ud.combining(c))
        if key!=last:
            if last is not None:result.append((start,cp-1,*last))
            start=cp;last=key
    result.append((start,0x10ffff,*last));return [r for r in result if r[2] or r[3]]

def normalization():
    nfd=[];comps=[]
    for cp in range(0x110000):
        c=chr(cp);d=ud.normalize('NFD',c)
        if d!=c:
            code=list(map(ord,d));assert len(code)<=4
            nfd.append((cp,len(code),*(code+[0]*(4-len(code)))))
        raw=ud.decomposition(c)
        if raw and not raw.startswith('<'):
            pair=[int(s,16) for s in raw.split()]
            if len(pair)==2 and ud.normalize('NFC',''.join(map(chr,pair)))==c:comps.append((*pair,cp))
    # Unicode 17 exposes Hangul LVT decomposition fully, rather than as LV+T.
    for cp in range(0xac00,0xd7a4):
        tail=(cp-0xac00)%28
        if tail:comps.append((cp-tail,0x11a7+tail,cp))
    return nfd,sorted(set(comps))

def pack(vocab,merges,added,props,nfd,comps,byte_ids):
    pool=bytearray();spans=[]
    for token in vocab:spans.append((len(pool),len(token)));pool.extend(token)
    counts=[len(vocab),len(merges),len(props),len(nfd),len(comps),len(added),len(pool)]
    assert counts[0]<=262144 and counts[1]<=262144 and counts[2]<=8192 and counts[3]<=16384 and counts[4]<=16384 and counts[5]<=64 and counts[6]<=8388608
    return b'PST5'+words(counts)+words(byte_ids)+b''.join(words(row) for section in [spans,sorted(merges),props,nfd,comps] for row in section)+words(sorted(added,key=lambda i:(-len(vocab[i]),i)))+pool

def byte_alphabet():
    raw=list(range(33,127))+list(range(161,173))+list(range(174,256));chars=raw[:];n=0
    for b in range(256):
        if b not in raw:raw.append(b);chars.append(256+n);n+=1
    return {chr(c):b for b,c in zip(raw,chars)}

def build_original(original):
    assert sha(original)==ORIGINAL_SHA
    j=json.loads(original);m=j['model']
    assert j['normalizer']=={'type':'NFC'} and j['truncation'] is None and j['padding'] is None
    assert j['pre_tokenizer']['pretokenizers'][0]['pattern']['Regex']==REGEX
    assert all(not t[k] for t in j['added_tokens'] for k in ['normalized','single_word','lstrip','rstrip'])
    assert all(t['content'].startswith('<') for t in j['added_tokens'])
    assert m['type']=='BPE' and m['dropout'] is None and not m['ignore_merges'] and not m['byte_fallback']
    inv=byte_alphabet();base=m['vocab'];vocab=[None]*(len(base)+len(j['added_tokens']))
    for text,idx in base.items():vocab[idx]=bytes(inv[c] for c in text)
    for token in j['added_tokens']:vocab[token['id']]=token['content'].encode()
    assert all(vocab)
    merges=[(base[a],base[b],rank,base[a+b]) for rank,(a,b) in enumerate(m['merges'])]
    assert len({(a,b) for a,b,_,_ in merges})==len(merges)
    byte_ids=[base[next(c for c,v in inv.items() if v==b)] for b in range(256)]
    nfd,comps=normalization();props=properties()
    packed=pack(vocab,merges,[t['id'] for t in j['added_tokens']],props,nfd,comps,byte_ids)
    return packed,dict(vocab=len(vocab),merges=len(merges),properties=len(props),nfd=len(nfd),compositions=len(comps),added=len(j['added_tokens']),unicode=ud.unidata_version)

def toy():
    vocab=[bytes([i]) for i in range(256)]
    merges=[]
    for a,b in [(97,98),(256,97),(104,101),(258,108),(259,108),(260,111),(32,261),(39,115)]:
        merges.append((a,b,len(merges),len(vocab)));vocab.append(vocab[a]+vocab[b])
    added=[]
    for s in ['<|im_start|>','<|im_end|>']:
        added.append(len(vocab));vocab.append(s.encode())
    for a,b in [(97,97),(98,97)]:
        merges.append((a,b,len(merges),len(vocab)));vocab.append(vocab[a]+vocab[b])
    props=[]
    for cp in range(1024):
        c=chr(cp);category=ud.category(c)
        flags=(1 if category.startswith('L') else 2 if category.startswith('M') else 4 if category.startswith('N') else 0)|(16 if c.isspace() else 0)|(8 if c.isspace() and cp not in range(28,32) else 0)
        if flags or ud.combining(c):props.append((cp,cp,flags,ud.combining(c)))
    nfd,comps=normalization();chosen=set(range(1024))|{0x1f82,0xac00,0xac01}
    return pack(vocab,merges,added,props,[row for row in nfd if row[0] in chosen],[row for row in comps if row[2] in chosen],list(range(256)))

def coax_blob(name,data):return name+' :: ['+str(len(data))+']u8.['+','.join(map(str,data))+'];\n'
def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--original');args=p.parse_args()
    original=Path(args.original).read_bytes() if args.original else gzip.decompress((DEST/'tokenizer.json.gz').read_bytes())
    packed,stats=build_original(original);small=toy()
    outputs={'tokenizer.json.gz':gzip.compress(original,mtime=0),'qwen35.pst':packed,'toy.pst':small,
             'toy.coax':coax_blob('PT_TOY',small).encode()}
    pins='// Deterministic table identities; original tokenizer DATA, not executable code.\nPT_QWEN_SHA :: '+json.dumps(sha(packed))+';\nPT_ORIGINAL_SHA :: '+json.dumps(ORIGINAL_SHA)+';\n'
    outputs['pins.coax']=pins.encode()
    manifest={'schema':1,'profile':'qwen35-text-v1','original_sha256':ORIGINAL_SHA,'elephant_revision':'b3f8a93462fb1051a9de031e94a7ff380e254072','mammoth_revision':'61cec44c281c693992e3d6ebeb44dd3402dfa186','runtime_revision':'81ce62f1597c91e46767d4d02d6ac2e18534fe94',**stats,'files':{name:{'bytes':len(data),'sha256':sha(data)} for name,data in outputs.items()}}
    outputs['manifest.json']=(json.dumps(manifest,indent=2)+'\n').encode()
    for name,data in outputs.items():
        if args.write:(DEST/name).write_bytes(data)
        assert (DEST/name).read_bytes()==data,'fixture drift: '+name
    print('pinned tokenizer/Unicode tables and owned toy fixture verify:',stats)
if __name__=='__main__':main()
