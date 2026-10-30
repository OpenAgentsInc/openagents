#!/usr/bin/env python3
"""Owned normalization/hash/row oracle. Never executes upstream model code."""
import argparse,gzip,hashlib,json,math,re,struct,unicodedata
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];DEST=ROOT/'psionic/fixtures/deepseek'
TOKEN_SHA='c90dfa01249db1be4245780a052ede752e1361c612ac6d08e2bdada7d599476b'
MAP_SHA='c60a86322ec17b4142bfef3c57a8d81fb428550cdf487f88a4320cb59fe46481'
MULT=[[76632096046245,4839876093313,35959672319349,73987337458391],[67716810739261,51510806800915,30921347202721,82619226485591]]
def compressed_map(raw):
    j=json.loads(raw);bs=list(range(33,127))+list(range(161,173))+list(range(174,256));chars=bs[:];n=0
    for b in range(256):
        if b not in bs:bs.append(b);chars.append(256+n);n+=1
    reverse={chr(c):b for b,c in zip(bs,chars)};vocab={v:k for k,v in j['model']['vocab'].items()}
    for f in j['added_tokens']:vocab[f['id']]=f['content']
    lookup=[];seen={};keys=[]
    for i in range(len(vocab)):
        text=b''.join(bytes([reverse[c]]) if c in reverse else c.encode() for c in vocab[i]).decode('utf8','replace')
        if '\ufffd' in text:key=vocab[i]
        else:
            norm=unicodedata.normalize('NFD',unicodedata.normalize('NFKC',text));norm=''.join(c for c in norm if not unicodedata.category(c).startswith('M'));norm=norm.lower();norm=re.sub(r'[ \t\r\n]+',' ',norm);norm='\ue000' if norm==' ' else norm;norm=norm.strip().replace('\ue000',' ');key=norm or text
        if key not in seen:seen[key]=len(seen)
        lookup.append(seen[key]);keys.append(key)
    assert len(seen)==99092 and len(lookup)==129280
    return lookup,keys
def prime(n):return n>=2 and all(n%d for d in range(2,math.isqrt(n)+1))
def layout():
    primes=[];at=15999999
    for _ in range(48):
        at+=1
        while not prime(at):at+=1
        primes.append(at)
    offsets=[];rows=[]
    for layer in range(2):
        size=0
        for p in primes[layer*24:(layer+1)*24]:offsets.append(size);size+=p
        rows.append(size)
    assert rows==[384006168,384016682]
    return dict(primes=primes,offsets=offsets,rows=rows,multipliers=MULT)
def hashes(tokens,mask,mapping):
    cfg=layout();history=[];out=[]
    for token,live in zip(tokens,mask):
        history.append(mapping[token] if live else -1);look=[];blocked=False
        for back in range(4):
            value=history[-1-back] if back<len(history) else -1;blocked|=value==-1;look.append(2 if blocked else value)
        for layer in range(2):
            rolling=look[0]*MULT[layer][0]
            for back in range(1,4):
                rolling^=look[back]*MULT[layer][back]
                out +=[rolling%cfg['primes'][layer*24+(back-1)*8+h]+cfg['offsets'][layer*24+(back-1)*8+h] for h in range(8)]
    return out
def vectors(mapping):
    toy=[0,5,2,5,8,0,6,5];tokens=[0,1,3,4,5,6,7,2,0,4,3,7,1,6,0,5,2,7,4];mask=[int(i not in [3,7,8,15]) for i in range(len(tokens))]
    original=[0,2,129279,4384,5387,5713,7057,128000,1,100,129264,23,129265,777,50000,17,99999,128799,42]
    return dict(toy=toy,tokens=tokens,mask=mask,hashes=hashes(tokens,mask,toy),original=original,original_hashes=hashes(original,mask,mapping),layout=layout())
def constants(j):
    s='// Pinned V4.1 Engram constants. Derived by the owned oracle from the original source/config.\n'
    for name,typ,values in [('PRIMES','int',j['layout']['primes']),('OFFSETS','int',j['layout']['offsets']),('ROWS','int',j['layout']['rows']),('MULT','u64',sum(MULT,[]))]:s+=f'EG_{name} :: [{len(values)}]{typ}.['+','.join(map(str,values))+'];\n'
    return s+f'EG_MAP_SHA :: "{MAP_SHA}";\n'
def golden(j):
    s='// Independent full-prefix XOR oracle, including dead spans.\n'
    for key in ['toy','tokens','mask','hashes']:
        values=j[key];typ='u8' if key=='mask' else 'int';s+=f'EGG_{key.upper()} :: [{len(values)}]{typ}.['+','.join(map(str,values))+'];\n'
    raw=struct.pack('<8I',*j['toy']);s+=f'EGG_MAP :: [32]u8.['+','.join(map(str,raw))+'];\n'+f'EGG_MAP_SHA :: "{hashlib.sha256(raw).hexdigest()}";\n';return s
def rows_fixture():
    packet=bytearray();bits=[]
    for row in range(48):
        codes=[((row*17+col*13)%127)+(128 if (col+row)%3==0 else 0) for col in range(256)]
        scales=[118+(row*3+g*5)%16 for g in range(8)]
        packet.extend(codes+scales)
        for col,code in enumerate(codes):
            exponent=(code&127)>>3;mantissa=code&7
            number=math.ldexp(mantissa,-9) if exponent==0 else math.ldexp(8+mantissa,exponent-10)
            number=math.ldexp(number,scales[col//32]-127)*(-1 if code&128 else 1)
            word=struct.unpack('<I',struct.pack('<f',number))[0]
            # FP8 times a binary scale is exact in BF16 at these moderate exponents.
            assert word&65535==0;bits.append(word>>16)
    return bytes(packet),struct.pack('<12288H',*bits)
def source_tables():
    source=json.loads((DEST/'engram-source.json').read_text())
    for item in source['sources'].values():
        raw=gzip.decompress((DEST/item['archive']).read_bytes());assert len(raw)==item['bytes'] and hashlib.sha256(raw).hexdigest()==item['sha256']
    config=json.loads(gzip.decompress((DEST/'cache-reference-config.txt.gz').read_bytes()))
    for key,value in dict(vocab_size=129280,engram_layer_ids=[1,14],engram_vocab_size=16000000,engram_num_embeddings=[384006168,384016682],engram_max_ngram_size=4,engram_pad_id=2,engram_compressed_vocab_size=99092,engram_n_heads=8,engram_head_dim=256).items():assert config[key]==value
    tables=[]
    for item,rows in zip(source['headers'],layout()['rows']):
        raw=gzip.decompress((DEST/item['archive']).read_bytes());assert len(raw)==item['prefix_bytes'] and hashlib.sha256(raw).hexdigest()==item['prefix_sha256']
        start=8+struct.unpack('<Q',raw[:8])[0];assert start==len(raw);header=json.loads(raw[8:]);prefix=f"layers.{item['layer']}.engram.embed."
        weight=header[prefix+'weight'];scale=header[prefix+'scale']
        assert weight['dtype']=='F8_E4M3' and weight['shape']==[rows,256] and weight['data_offsets']==[0,rows*256]
        assert scale['dtype']=='F8_E8M0' and scale['shape']==[rows,8] and scale['data_offsets']==[rows*256,rows*264]
        assert start+rows*264<=item['file_bytes'];tables.append([rows,start,start+rows*256,item['file_bytes']])
    return tables
def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--header',action='store_true');p.add_argument('--external',action='store_true');a=p.parse_args()
    raw=gzip.decompress((DEST/'engram-tokenizer.json.gz').read_bytes());assert hashlib.sha256(raw).hexdigest()==TOKEN_SHA
    mapping,keys=compressed_map(raw);packed=struct.pack('<129280I',*mapping);assert hashlib.sha256(packed).hexdigest()==MAP_SHA
    j=vectors(mapping);tables=source_tables()
    if a.external:
        import numpy as np,tokenizers
        from tokenizers import Tokenizer,normalizers,Regex
        t=Tokenizer.from_str(raw.decode());norm=normalizers.Sequence([normalizers.NFKC(),normalizers.NFD(),normalizers.StripAccents(),normalizers.Lowercase(),normalizers.Replace(Regex(r'[ \t\r\n]+'),' '),normalizers.Replace(Regex(r'^ $'),'\ue000'),normalizers.Strip(),normalizers.Replace('\ue000',' ')])
        for i,key in enumerate(keys):
            text=t.decode([i],skip_special_tokens=False);actual=t.id_to_token(i) if '\ufffd' in text else norm.normalize_str(text) or text
            assert actual==key,(i,actual,key)
        bound=((2**63-1)//99092)//2
        for l,expected in zip([1,14],MULT):assert list(map(int,np.random.default_rng(10007*l).integers(0,bound,size=4,dtype=np.int64)*2+1))==expected
        print('External parity: every decoded/normalized token key and all NumPy multipliers agree',tokenizers.__version__,np.__version__);return
    if a.header:
        print('#include <stdint.h>')
        print('static const int64_t egg_tables[2][4]={'+','.join('{'+','.join(map(str,t))+'}' for t in tables)+'};')
        for key in ['tokens','mask','hashes','original','original_hashes']:
            print(f'static const '+('uint8_t' if key=='mask' else 'int64_t')+f' egg_{key}[]={{'+','.join(map(str,j[key]))+'};')
        return
    packet,words=rows_fixture()
    files={'engram-rows.bin':packet,'engram-words.bin':words,'engram-constants.coax':constants(j).encode(),'engram-goldens.coax':golden(j).encode(),'engram-goldens.json.gz':(json.dumps(j,separators=(',',':'))+'\n').encode(),'engram-map.u32.gz':packed}
    for name,data in files.items():
        path=DEST/name
        if a.write:path.write_bytes(gzip.compress(data,mtime=0) if name.endswith('.gz') else data)
        else:assert (gzip.decompress(path.read_bytes()) if name.endswith('.gz') else path.read_bytes())==data,name
    print('Engram: all 129280 original IDs normalize to 99092 exact classes; primes and prefix/dead-span hashes agree')
if __name__=='__main__':main()
