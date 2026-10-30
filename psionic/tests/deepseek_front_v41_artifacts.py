#!/usr/bin/env python3
"""Pack pinned tokenizer DATA and independently author CPU-only text vectors."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import random
import struct
import sys
from functools import lru_cache

ROOT = Path(__file__).resolve().parents[2]
DEST = ROOT / 'psionic/fixtures/deepseek'
sys.path.insert(0, str(Path(__file__).parent))
from text_fixtures import byte_alphabet, pack

TOKEN_SHA = 'c90dfa01249db1be4245780a052ede752e1361c612ac6d08e2bdada7d599476b'
SAMPLES = ['', 'Hi', 'hello world', 'e\u0301 é', '1234567890', '中a文日本語カタカナひらがな',
           '你好，world! 한국어', '!foo?BAR_a', '  hello', ' \t \nx', ' \t x',
           'a\r\nb\nc', '!!!\r\n', '😀+x=12', '\x00\x00a', '\x00\x00',
           '\u0301\u0308a', 'a\u00a0b', '<think>｜DSML｜<｜User｜>',
           '𰀀新字 ١٢٣٤⑤⑥', 'a\u200db', '\x1c\x1dX', '豈が가',
           '\u3040\u3097\u3098\u309f\u30ff', '龥龦𰀀a', '\u0301!a', '\n中文', '!é']
SAMPLES += ['a\u0558\u0558b','a\U00016100\U00016100b','a\u1c89\u1c89b','a\U0001e4d0\U0001e4d0b']
CONTROL_IDS = [0,1,128799,128803,128804,128821,128822,128825]
UNICODE_SOURCES = {
    'front-v41-unicode-data.txt.gz': ('UnicodeData.txt','ff58e5823bd095166564a006e47d111130813dcf8bf234ef79fa51a870edb48f'),
    'front-v41-unicode-props.txt.gz': ('PropList.txt','53d614508e2a0b2305a8aa21cd60d993de9326cdf65993660dfcce4503548583')}


def recipes():
    cases=[]
    histories=[[(1,'<think>｜DSML｜你好')],
               [(0,'rules'),(1,'Hi'),(2,'answer'),(1,'next')],
               [(1,'a'),(3,'b'),(1,'c')],
               [(0,'rules'),(1,'<think>'),(2,'answer'),(3,'｜DSML｜')]]
    for messages in histories:
        for thinking in [False,True]:
            # Independent bounded recipe equations. Input content is always
            # ordinary; only the renderer's explicit boundaries are controls.
            result=[('control',0)]
            if thinking or messages[0][0]==0:
                result.append(('control',2))
            if thinking:
                result.append(('text','Reasoning Effort: 75 (range 1-100, the higher the value, the more thorough the reasoning)\n\n'))
            for i,(role,text) in enumerate(messages):
                if role==0:
                    result.append(('text',text))
                elif role==2:
                    result.extend([('control',4),('control',6),('text',text),('control',1)])
                else:
                    result.append(('text','\n\n') if i and messages[i-1][0] in [1,3] else ('control',3))
                    result.append(('text','<tool_result>'+text+'</tool_result>' if role==3 else text))
            result.extend([('control',4),('control',5 if thinking else 6)])
            # Join textual neighbors before independently encoding each run.
            runs=[]
            for kind,value in result:
                if kind=='text' and runs and runs[-1][0]=='text':
                    runs[-1]=('text',runs[-1][1]+value)
                else:
                    runs.append((kind,value))
            cases.append(dict(messages=messages,thinking=thinking,runs=runs))
    return cases


def sha(data):
    return hashlib.sha256(data).hexdigest()


@lru_cache(maxsize=1)
def unicode_flags():
    sources={}
    for name,(_,expected) in UNICODE_SOURCES.items():
        raw=gzip.decompress((DEST/name).read_bytes());assert sha(raw)==expected;sources[name]=raw.decode()
    flags=bytearray(0x110000);first=None
    for line in sources['front-v41-unicode-data.txt.gz'].splitlines():
        fields=line.split(';');cp=int(fields[0],16);category=fields[2]
        bit={'L':1,'M':2,'N':4,'P':32,'S':32}.get(category[0],0)
        if fields[1].endswith(', First>'):
            first=cp
        elif fields[1].endswith(', Last>'):
            assert first is not None
            flags[first:cp+1]=bytes([bit])*(cp-first+1);first=None
        else:
            flags[cp]=bit
    assert first is None
    for line in sources['front-v41-unicode-props.txt.gz'].splitlines():
        line=line.split('#',1)[0].strip()
        if not line:continue
        span,prop=map(str.strip,line.split(';'))
        if prop!='White_Space':continue
        ends=[int(x,16) for x in span.split('..')];lo=ends[0];hi=ends[-1]
        for cp in range(lo,hi+1):flags[cp]|=8
    return flags


@lru_cache(maxsize=1)
def unicode_sets():
    flags=unicode_flags();result={}
    for bit in [1,2,4,8,32]:
        ranges=[];start=None
        for cp,flag in enumerate(flags):
            if flag&bit:
                if start is None:start=cp
            elif start is not None:
                ranges.append((start,cp-1));start=None
        if start is not None:ranges.append((start,0x10ffff))
        result[bit]=''.join(f'\\U{lo:08x}' if lo==hi else f'\\U{lo:08x}-\\U{hi:08x}' for lo,hi in ranges)
    return result


def oracle_patterns(j):
    import regex
    sets=unicode_sets();result=[]
    for step in j['pre_tokenizer']['pretokenizers']:
        if step['type']!='Split':continue
        pattern=step['pattern']['Regex']
        # V1 nested sets let the independent regex engine use the pinned
        # Unicode 16 classes even when its own Unicode database is newer.
        # P and S occur together in both source alternatives, so share a union.
        for name,bit in [('L',1),('M',2),('N',4),('P',32),('S',32)]:
            pattern=pattern.replace('\\p{'+name+'}','['+sets[bit]+']')
        pattern=pattern.replace(r'\s','['+sets[8]+']').replace(r'\S','[^'+sets[8]+']')
        result.append(regex.compile('(?V1)'+pattern))
    return result


def build(raw):
    j = json.loads(raw)
    assert sha(raw) == TOKEN_SHA
    assert j['normalizer'] == {'type': 'Sequence', 'normalizers': []}
    m = j['model']; base = m['vocab']; inv = byte_alphabet()
    assert m['type'] == 'BPE' and m['dropout'] is None and not m['byte_fallback']
    assert not m.get('ignore_merges', False)
    vocab = [None] * 129280
    for text, i in base.items():
        vocab[i] = text.encode() if i < 3 else bytes(inv[c] for c in text)
    for t in j['added_tokens']:
        assert all(not t[k] for k in ['single_word', 'lstrip', 'rstrip'])
        vocab[t['id']] = t['content'].encode()
    assert all(vocab)
    merges = []
    for rank, pair in enumerate(m['merges']):
        a, b = pair.split(' ') if isinstance(pair, str) else pair
        merges.append((base[a], base[b], rank, base[a+b]))
    # Full Unicode 16 DATA, matching the original tokenizer's regex engine.
    props = []; last = None; start = 0
    for cp,flags in enumerate(unicode_flags()):
        if flags != last:
            if last:
                props.append((start, cp-1, last, 0))
            start = cp; last = flags
    if last:
        props.append((start, 0x10ffff, last, 0))
    alphabet = [base[next(c for c, value in inv.items() if value == b)] for b in range(256)]
    # Added-token spelling matching is intentionally absent. Typed boundaries
    # supply control IDs; ordinary text can never enter that path.
    return pack(vocab, merges, [], props, [], [], alphabet), j


def oracle(j, samples):
    inv = byte_alphabet(); alphabet = {b: c for c, b in inv.items()}
    base = j['model']['vocab']
    ranks = {tuple(pair.split(' ') if isinstance(pair, str) else pair): i
             for i, pair in enumerate(j['model']['merges'])}
    patterns = oracle_patterns(j)
    result = []
    for text in samples:
        pieces = [text] if text else []
        for pattern in patterns:
            next_pieces = []
            for piece in pieces:
                at = 0
                for match in pattern.finditer(piece):
                    if match.start() > at:
                        next_pieces.append(piece[at:match.start()])
                    next_pieces.append(match[0]); at = match.end()
                if at < len(piece):
                    next_pieces.append(piece[at:])
            pieces = next_pieces
        ends = []; n = 0; ids = []
        for piece in pieces:
            n += len(piece.encode()); ends.append(n)
            symbols = [alphabet[b] for b in piece.encode()]
            while len(symbols) > 1:
                choices = [(ranks[pair], i) for i, pair in enumerate(zip(symbols, symbols[1:])) if pair in ranks]
                if not choices:
                    break
                _, at = min(choices); symbols[at:at+2] = [''.join(symbols[at:at+2])]
            ids.extend(base[s] for s in symbols)
        result.append(dict(text=text, ends=ends, ids=ids))
    return result


def main():
    parser = argparse.ArgumentParser()
    action = parser.add_mutually_exclusive_group()
    action.add_argument('--write', action='store_true')
    action.add_argument('--external', action='store_true')
    parser.add_argument('--unicode', action='store_true',help='with --external, compare every Unicode scalar probe')
    action.add_argument('--header', action='store_true')
    args = parser.parse_args()
    if args.unicode and not args.external:
        parser.error('--unicode requires --external')
    if args.header:
        cases = json.loads(gzip.decompress((DEST/'front-v41-goldens.json.gz').read_bytes()))['cases']
        print('/* Independent regex and ranked-BPE vectors. */')
        for i, case in enumerate(cases):
            print(f'static const unsigned char dvf_text_{i}[]={{'+','.join(map(str, case['text'].encode()))+',0};' if case['text'] else f'static const unsigned char dvf_text_{i}[]={{0}};')
        print('static const unsigned char *dvf_texts[]={'+','.join(f'dvf_text_{i}' for i in range(len(cases)))+'};')
        for key in ['ends', 'ids']:
            for i, case in enumerate(cases):
                print(f'static const int64_t dvf_{key}_{i}[]={{'+(','.join(map(str,case[key])) or '0')+'};')
            print(f'static const int64_t *dvf_{key}[]={{'+','.join(f'dvf_{key}_{i}' for i in range(len(cases)))+'};')
            print(f'static const int64_t dvf_{key}_counts[]={{'+','.join(str(len(c[key])) for c in cases)+'};')
        print('static const int64_t dvf_lengths[]={'+','.join(str(len(c['text'].encode())) for c in cases)+'};')
        manifest=json.loads((DEST/'front-v41-source.json').read_text())
        print('#define DVF_SHA "'+manifest['artifacts']['front-v41.pst']['sha256']+'"')
        print('#define DVF_DECODE_SHA "'+manifest['ordinary_decode_sha256']+'"')
        print('#define DVF_DECODE_BYTES '+str(manifest['ordinary_decode_bytes']))
        print('#define DVF_UNICODE_SHA "'+manifest['unicode_flags_sha256']+'"')
        recipe_data=json.loads(gzip.decompress((DEST/'front-v41-goldens.json.gz').read_bytes()))['recipes']
        for i,recipe in enumerate(recipe_data):
            print(f'static const int64_t dvf_recipe_{i}[]={{'+','.join(map(str,recipe['ids']))+'};')
        print('static const int64_t *dvf_recipes[]={'+','.join(f'dvf_recipe_{i}' for i in range(len(recipe_data)))+'};')
        print('static const int64_t dvf_recipe_counts[]={'+','.join(str(len(r['ids'])) for r in recipe_data)+'};')
        return
    if not args.write:
        source = (DEST/'engram-tokenizer.json.gz').read_bytes()
        manifest = json.loads((DEST/'front-v41-source.json').read_text())
        assert sha(gzip.decompress(source)) == TOKEN_SHA
        for name,(_,expected) in UNICODE_SOURCES.items():
            assert sha(gzip.decompress((DEST/name).read_bytes()))==expected
        for name, identity in manifest['artifacts'].items():
            data = (DEST/name).read_bytes()
            assert len(data) == identity['bytes'] and sha(data) == identity['sha256'], name
        if args.external:
            from tokenizers import Tokenizer
            j = json.loads(gzip.decompress(source)); j['added_tokens'] = []
            tokenizer = Tokenizer.from_str(json.dumps(j))
            cases = json.loads(gzip.decompress((DEST/'front-v41-goldens.json.gz').read_bytes()))['cases']
            for case in cases:
                assert tokenizer.encode(case['text'], add_special_tokens=False).ids == case['ids'], case
            recipe_cases=json.loads(gzip.decompress((DEST/'front-v41-goldens.json.gz').read_bytes()))['recipes']
            for case in recipe_cases:
                ids=[]
                for kind,value in case['runs']:
                    ids.extend([CONTROL_IDS[value]] if kind=='control' else tokenizer.encode(value,add_special_tokens=False).ids)
                assert ids==case['ids']
            if args.unicode:
                patterns=oracle_patterns(j);inv=byte_alphabet();count=0
                for cp in range(0x110000):
                    if 0xd800<=cp<=0xdfff:continue
                    text='a'+chr(cp)*2+'b';parts=[text]
                    for pattern in patterns:
                        next_parts=[]
                        for piece in parts:
                            at=0
                            for match in pattern.finditer(piece):
                                if match.start()>at:next_parts.append(piece[at:match.start()])
                                next_parts.append(match[0]);at=match.end()
                            if at<len(piece):next_parts.append(piece[at:])
                        parts=next_parts
                    actual=[bytes(inv[c] for c in s) for s,_ in tokenizer.pre_tokenizer.pre_tokenize_str(text)]
                    assert actual==[s.encode() for s in parts],hex(cp)
                    count+=1
                print('V4.1 external Unicode scalar probes:',count,'original Rust and pinned Unicode 16 regex splits agree')
            print('V4.1 external Rust tokenizer parity:', len(cases), 'ordinary text cases')
        print('V4.1 tokenizer: pinned DATA and retained independent vectors verify')
        return
    raw = gzip.decompress((DEST/'engram-tokenizer.json.gz').read_bytes())
    table, j = build(raw)
    rng = random.Random(5741)
    alphabet = ['a','B','z','!','?',':','_',' ','\t','\n','\r','0','1','2','9','\u0301','é','中','あ','ア','😀','\x00','\u200d','\u00a0','١','⑤']
    samples = SAMPLES + [''.join(rng.choice(alphabet) for _ in range(rng.randrange(1,35))) for _ in range(256)]
    cases = oracle(j, samples)
    recipe_cases=recipes()
    runs=[value for case in recipe_cases for kind,value in case['runs'] if kind=='text']
    encoded=iter(oracle(j,runs))
    for case in recipe_cases:
        ids=[]
        for kind,value in case['runs']:
            ids.extend([CONTROL_IDS[value]] if kind=='control' else next(encoded)['ids'])
        case['ids']=ids
    pattern = oracle_patterns(j)[2]
    probes = ['\x00\x00a','\x00\x00','\r\na',' \t \nx',' \t x','é!','\u0301!','\u0301\u0308!','a b c','\r  a','\n  a','\t\x00']
    probes += [s for c in map(chr,range(128)) for s in [c,c+'a',c+'é',c+'\u0301','a'+c+'b',c+'\x00']]
    ends=[];pieces=[]
    for s in probes:
        match=pattern.match(s);ends.append(len(match[0].encode()) if match else 0)
        found=pattern.search(s)
        pieces.append(len((found[0] if found.start()==0 else s[:found.start()]).encode()) if found else len(s.encode()))
    unit='// Independent original third-regex probes on owned byte-BPE data.\n'
    assert max(len(s.encode()) for s in probes)<=8
    unit+='DVF_PROBES :: ['+str(len(probes))+'][8]u8.['+','.join('[8]u8.['+','.join(map(str,list(s.encode())+[0]*(8-len(s.encode()))))+']' for s in probes)+'];\n'
    unit+='DVF_PROBE_LENGTHS :: ['+str(len(probes))+']int.['+','.join(str(len(s.encode())) for s in probes)+'];\n'
    for name,values in [('MATCH',ends),('PIECE',pieces)]:
        unit+='DVF_'+name+' :: ['+str(len(values))+']int.['+','.join(map(str,values))+'];\n'
    outputs = {'front-v41.pst': table, 'front-v41-goldens.json.gz': gzip.compress((json.dumps(dict(schema=1,cases=cases,recipes=recipe_cases),ensure_ascii=False)+'\n').encode(),mtime=0), 'front-v41-goldens.coax':unit.encode()}
    inv=byte_alphabet();vocab={i:text for text,i in j['model']['vocab'].items()}
    decoded=b''.join(struct.pack('<I',len(data))+data for i in range(3,128000) for data in [bytes(inv[c] for c in vocab[i])])
    manifest = dict(schema=1,revision='1bae0e1088993b9b82de08f59de5be5a34e42170',source_tokenizer_sha256=TOKEN_SHA,
                    profile='deepseek-front-v41-text-v1',entries=129280,merges=len(j['model']['merges']),normalization='none',added_spelling_matching=False,
                    unicode='16.0.0',unicode_sources={name:dict(url='https://www.unicode.org/Public/16.0.0/ucd/'+filename,raw_sha256=digest) for name,(filename,digest) in UNICODE_SOURCES.items()},
                    unicode_flags_sha256=sha(unicode_flags()),
                    ordinary_decode_bytes=len(decoded),ordinary_decode_sha256=sha(decoded),
                    artifacts={name:dict(bytes=len(data),sha256=sha(data)) for name,data in outputs.items()})
    for name, data in outputs.items():
        (DEST/name).write_bytes(data)
    (DEST/'front-v41-source.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print('V4.1 DATA packed:',len(table),'bytes;',len(cases),'independent text cases')


if __name__ == '__main__':
    main()
