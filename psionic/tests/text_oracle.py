#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent standard-library reference: Unicode regex + priority-queue BPE.
Never imports/runs downloaded code or reads candidate outputs. Fixtures are owned.
"""
import argparse,gzip,heapq,json,re,struct,unicodedata as ud,random,hashlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];DEST=ROOT/'psionic/fixtures/text/ch05'
SYSTEM='Apply the question to the state. Choose exactly one of the listed options. Respond with only its uppercase letter, with no explanation or reasoning.'
EXCLUDED={'BQ','BZ','CJ','CQ','CZ','DQ','DZ','EJ','EY','FJ','FQ','FV','FZ','GJ','GK','GQ','GZ','HJ','IY','JF','JG','JH','JL','JN','JQ'}
SLOTS=list('ABCDEFGHIJKLMNOPQRSTUVWXYZ')+[a+b for a in 'ABCDEFGHIJKLMNOPQRSTUVWXYZ' for b in 'ABCDEFGHIJKLMNOPQRSTUVWXYZ' if a+b not in EXCLUDED][:230]
END='<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n'
def classes(prefix):
    ranges=[];start=None;last=None
    for cp in range(0x110000):
        yes=ud.category(chr(cp)).startswith(prefix)
        if yes and start is None:start=cp
        if not yes and start is not None:ranges.append((start,cp-1));start=None
    if start is not None:ranges.append((start,0x10ffff))
    return ''.join(('\\U%08x'%a)+('-\\U%08x'%b if b!=a else '') for a,b in ranges)
def expression():
    pattern=r"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?[\p{L}\p{M}]+|\p{N}| ?[^\s\p{L}\p{M}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+"
    pattern=pattern.replace(r'|\p{N}|','|['+classes('N')+']|')
    for category in ['L','M','N']:pattern=pattern.replace('\\p{'+category+'}',classes(category))
    ws=r'\u0009-\u000d\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000'
    # Inside a negated character class, replace only the class contents.
    pattern=pattern.replace(r'[^\s', '[^'+ws).replace(r'\S','[^'+ws+']').replace(r'\s','['+ws+']')
    return re.compile(pattern)
def merge_bpe(ids, merges):
    """Maintain linked symbols and a heap ordered by merge rank, then position."""
    if not ids:
        return []
    values=list(ids);prev=[i-1 for i in range(len(ids))]
    next_=[i+1 if i+1<len(ids) else -1 for i in range(len(ids))]
    alive=[True]*len(ids);queue=[]
    def schedule(left):
        if left<0 or not alive[left]:return
        right=next_[left]
        if right<0:return
        pair=(values[left],values[right]);rule=merges.get(pair)
        if rule is not None:heapq.heappush(queue,(rule[0],left,right,pair,rule[1]))
    for i in range(len(ids)-1):schedule(i)
    while queue:
        _,left,right,pair,result=heapq.heappop(queue)
        if not alive[left] or not alive[right] or next_[left]!=right or (values[left],values[right])!=pair:
            continue
        values[left]=result;alive[right]=False;next_[left]=next_[right]
        if next_[right]>=0:prev[next_[right]]=left
        schedule(prev[left]);schedule(left)
    out=[];node=0
    while node>=0:out.append(values[node]);node=next_[node]
    return out

class Oracle:
    def __init__(self,vocab,merges,added,byte_ids,regex):
        self.vocab=vocab;self.merges=merges;self.added=added;self.byte_ids=byte_ids;self.regex=regex
        self.special=re.compile('('+'|'.join(re.escape(x) for x in sorted(added,key=len,reverse=True))+')')
    def work(self,text):
        return sum(len(piece.encode())**2 for segment in self.special.split(text) if segment not in self.added for piece in self.regex.findall(ud.normalize('NFC',segment)))
    def encode(self,text):
        result=[]
        for segment in self.special.split(text):
            if segment in self.added:result.append(self.added[segment]);continue
            normalized=ud.normalize('NFC',segment)
            pieces=self.regex.findall(normalized);assert ''.join(pieces)==normalized
            for part in pieces:
                result.extend(merge_bpe([self.byte_ids[b] for b in part.encode()],self.merges))
        return result

def full_oracle(regex):
    j=json.loads(gzip.decompress((DEST/'tokenizer.json.gz').read_bytes()));base=j['model']['vocab']
    raw=list(range(33,127))+list(range(161,173))+list(range(174,256));chars=raw[:];n=0
    for b in range(256):
        if b not in raw:raw.append(b);chars.append(256+n);n+=1
    byte_ids={b:base[chr(c)] for b,c in zip(raw,chars)}
    return Oracle(base,{(base[a],base[b]):(rank,base[a+b]) for rank,(a,b) in enumerate(j['model']['merges'])},{x['content']:x['id'] for x in j['added_tokens']},byte_ids,regex)
def toy_oracle(regex):
    b=(DEST/'toy.pst').read_bytes();counts=struct.unpack_from('<7I',b,4);at=1056
    starts=[]
    for n,w in zip(counts,[8,16,16,24,12,4,1]):starts.append(at);at+=n*w
    spans=[struct.unpack_from('<II',b,starts[0]+i*8) for i in range(counts[0])]
    vocab=[b[starts[6]+s:starts[6]+s+n] for s,n in spans]
    merges={}
    for i in range(counts[1]):a,c,rank,new=struct.unpack_from('<4I',b,starts[1]+16*i);merges[a,c]=(rank,new)
    added={vocab[struct.unpack_from('<I',b,starts[5]+i*4)[0]].decode():struct.unpack_from('<I',b,starts[5]+i*4)[0] for i in range(counts[5])}
    return Oracle(vocab,merges,added,dict(enumerate(struct.unpack_from('<256I',b,32))),regex)
def entry(e):
    if e is None:return ''
    if isinstance(e,str):return e.strip()
    return json.dumps(e,ensure_ascii=False,indent=2)
def prompt(state,kind,instructions,options):
    rendered_state='' if state is None else state if isinstance(state,str) else json.dumps(state,ensure_ascii=False,indent=2)
    base='<|im_start|>system\n'+SYSTEM+'<|im_end|>\n<|im_start|>user\n<state>\n'+rendered_state+'\n</state>\n\n'
    cut=(base+'QEV_PREFIX_MARK').index('QEV_PREFIX_MARK')
    if kind==0:names=[name for name,_ in options];descs=[entry(d) for _,d in options];default='Which option applies to the state?'
    elif kind==1:
        names=['yes','no'];descs=[entry(options[i][1]) if options and options[i][1] is not None else ['the statement is true / the answer is yes','the statement is false / the answer is no'][i] for i in range(2)];default='Is the statement true, or is the answer to the question yes?'
    else:names=['level '+str(i) for i in range(len(options))];descs=[entry(d) for _,d in options];default='Which level describes the state?'
    lines=[label+'. '+name+(': '+desc if desc else '') for label,name,desc in zip(SLOTS,names,descs)]
    suffix=('Rate the state: ' if kind==2 else 'Question: ')+(entry(instructions) or default)+'\n\nOptions:\n'+'\n'.join(lines)+'\n\nAnswer with one '+('letter' if len(names)<=26 else 'label')+': '+', '.join(SLOTS[:len(names)])+'.'
    full=base+suffix+END;return full,len(full[:cut].encode())
def cstr(text):
    b=text.encode() if isinstance(text,str) else text
    return '"'+''.join('\\%03o'%c for c in b)+'"'
def ce(e):
    if e is None:return '{0,{"",0}}'
    if isinstance(e,str):return '{1,{'+cstr(e)+','+str(len(e.encode()))+'}}'
    raw=json.dumps(e,ensure_ascii=False,separators=(',',':'));return '{2,{'+cstr(raw)+','+str(len(raw.encode()))+'}}'
def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--header',action='store_true');args=p.parse_args()
    assert ud.unidata_version=='17.0.0','reference authoring requires Unicode 17.0.0'
    # A newly formed rank-zero pair must preempt a second rank-one occurrence.
    assert merge_bpe([1,2,1,2],{(1,2):(1,3),(3,1):(0,4)})==[4,2]
    regex=expression();full=full_oracle(regex);toy=toy_oracle(regex)
    texts=['','ababa hello\'s','  hello','1234567890',"'S 'T 'RE 'VE 'M 'LL 'D 'ſ",'é é è è','è́ é̕','가 각','中文 日本語 한국어','العربية','🙂 👩🏽\u200d💻','\r\n\t  hello\n','a\0b','x\u00a0e\u2028a','<|im_start|>user\nhello<|im_end|>','<tool_call>x</tool_call>','<|im_start|><|im_end|>']
    rng=random.Random(5029);alphabet="abhelos' \t\r\n12.,_!éè́̀̕ſ"
    texts += [''.join(rng.choice(alphabet) for _ in range(rng.randrange(1,48))) for _ in range(160)]
    vectors=[dict(text=t,ids=full.encode(t),work=full.work(t)) for t in texts]
    toy_vectors=[dict(text=t,ids=toy.encode(t),work=toy.work(t)) for t in texts]
    # The table's unknown Unicode properties do not change byte-fallback-only
    # spans; restrict toy cases to its known classes where ASCII merges matter.
    toy_vectors=[v for v in toy_vectors if all(ord(c)<128 or c in 'éè́̀̕ſ' or c in '<|>' for c in v['text'])]
    requests=[]
    for state in ['ready',{'ticket':'charged twice','count':2,'nested':[True,None]},'QEV_PREFIX_MARK inside state','  ready \n',None]:
        for kind,opts in [(0,[('go',' continue '),('stop',None)]),(1,[]),(2,[('',None),('',{'condition':'done'})])]:
            for instructions in [None,'  Choose café? \n']:
                text,cut=prompt(state,kind,instructions,opts)
                requests.append(dict(state=state,kind=kind,instructions=instructions,options=opts,prompt=text,prefix_bytes=cut,ids=full.encode(text),candidate=full.encode(text.encode()[:cut].decode())))
    for count in [1,26,27,255]:
        opts=[('name'+str(i),'description '+str(i)) for i in range(count)];text,cut=prompt('ready',0,'Pick.',opts)
        requests.append(dict(state='ready',kind=0,instructions='Pick.',options=opts,prompt=text,prefix_bytes=cut,ids=full.encode(text),candidate=full.encode(text.encode()[:cut].decode())))
    document={'schema':1,'oracle':'owned Python standard-library Unicode 17/re with priority-queue BPE; no upstream runtime executed','texts':vectors,'requests':requests,'slots':[full.encode(s) for s in SLOTS]}
    assert all(len(ids)==1 for ids in document['slots'])
    assert all(ids==[full.vocab[label]] for label,ids in zip(SLOTS,document['slots']))
    normalized=bytearray()
    for cp in range(0x110000):
        ch=chr(cp);nfd=ud.normalize('NFD',ch)
        if nfd==ch:continue
        for value in [ch,nfd]:
            a=value.encode();b=ud.normalize('NFC',value).encode();normalized.extend(struct.pack('<II',len(a),len(b))+a+b)
    outputs={'goldens.json.gz':gzip.compress((json.dumps(document,ensure_ascii=False,indent=2)+'\n').encode(),mtime=0),'normalization.bin':bytes(normalized)}
    coax='// Independently specified toy-tokenizer vectors. No candidate output used.\n'
    # Fixed maximum-width rows simplify evaluator checks without dynamic allocation.
    rows=toy_vectors[:60];maxids=max(len(v['ids']) for v in rows)
    coax+='PT_GOLD_TEXT :: ['+str(len(rows))+']string.['+','.join(json.dumps(v['text'],ensure_ascii=False).replace('\\u0000','\\0').replace('\\b','\\u0008') for v in rows)+'];\n'
    coax+='PT_GOLD_WORK :: ['+str(len(rows))+']int.['+','.join(str(v['work']) for v in rows)+'];\n'
    # Coax only accepts its simple escapes; controls other than NUL/tab/LF/CR
    # are absent in this generated toy corpus.
    coax+='PT_GOLD_COUNT :: ['+str(len(rows))+']int.['+','.join(str(len(v['ids'])) for v in rows)+'];\n'
    coax+='PT_GOLD_IDS :: ['+str(len(rows))+']['+str(maxids)+']int.['+','.join('['+str(maxids)+']int.['+','.join(map(str,v['ids']+[0]*(maxids-len(v['ids']))))+']' for v in rows)+'];\n'
    pieces=[(ud.normalize('NFC',v['text']),list(regex.finditer(ud.normalize('NFC',v['text'])))) for v in toy_vectors]
    width=max(len(matches) for _,matches in pieces)
    coax+='PT_PIECE_TEXT :: ['+str(len(pieces))+']string.['+','.join(json.dumps(t,ensure_ascii=False).replace('\\u0000','\\0') for t,_ in pieces)+'];\n'
    coax+='PT_PIECE_COUNT :: ['+str(len(pieces))+']int.['+','.join(str(len(m)) for _,m in pieces)+'];\n'
    piece_rows=[]
    for text,matches in pieces:
        ends=[len(text[:m.end()].encode()) for m in matches]
        piece_rows.append('['+str(width)+']int.['+','.join(map(str,ends+[0]*(width-len(ends))))+']')
    coax+='PT_PIECE_ENDS :: ['+str(len(pieces))+']['+str(width)+']int.['+','.join(piece_rows)+'];\n'
    outputs['goldens.coax']=coax.encode()
    def coe(e):
        if e is None:return 'PtEntry.{}'
        kind='text' if isinstance(e,str) else 'json'
        value=e if isinstance(e,str) else json.dumps(e,ensure_ascii=False,separators=(',',':'))
        return 'PtEntry.{kind=PtEntryKind.'+kind+',value='+json.dumps(value,ensure_ascii=False)+'}'
    rendered=[]
    for i,r in enumerate(requests):
        lines=['#test "independent rendered request '+str(i)+'" {','    options:[255]PtOption;output:[65536]u8;scratch:[65536]u8;']
        for n,(name,desc) in enumerate(r['options']):lines.append('    options['+str(n)+']=PtOption.{name='+json.dumps(name,ensure_ascii=False)+',description='+coe(desc)+'};')
        lines.append('    r:=pt_render_checked('+coe(r['state'])+',PtQuestion.{kind=cast(PtQuestionKind) '+str(r['kind'])+',instructions='+coe(r['instructions'])+'},options[0..<'+str(len(r['options']))+'],output[..],scratch[..],false);')
        # Enum constructors use the named type, rather than numeric casts.
        lines[-1]=lines[-1].replace('cast(PtQuestionKind) '+str(r['kind']),['PtQuestionKind.choice','PtQuestionKind.noul','PtQuestionKind.score'][r['kind']])
        lines += ['    #assert r.status==PtStatus.ok && r.written=='+str(len(r['prompt'].encode()))+' && r.prefix_bytes=='+str(r['prefix_bytes'])+';', '    #assert cast(string)output[0..<r.written]=='+json.dumps(r['prompt'],ensure_ascii=False)+';', '}']
        rendered.append('\n'.join(lines))
    outputs['render-goldens.coax']=('\n'.join(rendered)+'\n').encode()
    outputs['slots.coax']=('PT_SLOT_LABELS :: [256]string.['+','.join(json.dumps(s) for s in SLOTS)+'];\n').encode()
    for name,data in outputs.items():
        if args.write:(DEST/name).write_bytes(data)
        assert (DEST/name).read_bytes()==data,'reference drift: '+name
    if args.header:
        print('/* Owned independent Unicode/regex/BPE reference vectors. */')
        print('typedef struct {coax_string text;const int64_t *ids;int64_t count;int64_t work;} TextGolden;')
        for i,v in enumerate(vectors):print('static const int64_t golden_ids_'+str(i)+'[]={'+(','.join(map(str,v['ids'])) or '0')+'};')
        print('static const TextGolden text_goldens[]={'+','.join('{{'+cstr(v['text'])+','+str(len(v['text'].encode()))+'},golden_ids_'+str(i)+','+str(len(v['ids']))+','+str(v['work'])+'}' for i,v in enumerate(vectors))+'};')
        print('typedef struct {coax_t_PtEntry state;coax_t_PtQuestion question;const coax_t_PtOption *options;int64_t n;coax_string prompt;int64_t prefix;const int64_t *ids;int64_t count;const int64_t *candidate;int64_t candidate_count;} RequestGolden;')
        for i,r in enumerate(requests):
            print('static const int64_t request_ids_'+str(i)+'[]={'+','.join(map(str,r['ids']))+'};')
            print('static const int64_t request_candidate_'+str(i)+'[]={'+','.join(map(str,r['candidate']))+'};')
            print('static const coax_t_PtOption request_options_'+str(i)+'[]={'+(','.join('{{'+cstr(name)+','+str(len(name.encode()))+'},'+ce(d)+'}' for name,d in r['options']) or '{{"",0},{0,{"",0}}}')+'};')
        print('static const RequestGolden request_goldens[]={'+','.join('{'+ce(r['state'])+',{'+str(r['kind'])+','+ce(r['instructions'])+'},request_options_'+str(i)+','+str(len(r['options']))+',{'+cstr(r['prompt'])+','+str(len(r['prompt'].encode()))+'},'+str(r['prefix_bytes'])+',request_ids_'+str(i)+','+str(len(r['ids']))+',request_candidate_'+str(i)+','+str(len(r['candidate']))+'}' for i,r in enumerate(requests))+'};')
        print('static const int64_t golden_slot_ids[]={'+','.join(str(ids[0]) for ids in document['slots'])+'};')
        print('typedef struct {coax_string text;int64_t id;} AddedGolden;')
        print('static const AddedGolden added_goldens[]={'+','.join('{{'+cstr(s)+','+str(len(s.encode()))+'},'+str(i)+'}' for s,i in full.added.items())+'};')
        print('static const int64_t golden_x_id='+str(full.encode('x')[0])+', golden_y_id='+str(full.encode('y')[0])+';')
    else:print('owned reference vectors:',len(vectors),'texts,',len(rows),'toy evaluator vectors,',len(requests),'requests, 256 slots and',len(normalized),'normalization bytes')
if __name__=='__main__':main()
