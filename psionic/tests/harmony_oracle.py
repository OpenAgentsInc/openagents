#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Owned independent Unicode regex, byte-concatenation heap BPE and framing.
Reads original rank DATA, never candidate output, packed pair records or upstream code.
"""
import argparse,base64,gzip,hashlib,heapq,json,random,re,struct,unicodedata as ud
from functools import lru_cache
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];DEST=ROOT/'psionic/fixtures/harmony'
@lru_cache(None)
def chars(category):
    intervals=[];begin=None
    for cp in range(0x110000):
        yes=ud.category(chr(cp)).startswith(category)
        if yes and begin is None:begin=cp
        if not yes and begin is not None:intervals.append((begin,cp-1));begin=None
    if begin is not None:intervals.append((begin,0x10ffff))
    return ''.join('\\U%08x'%a+('-\\U%08x'%b if b!=a else '') for a,b in intervals)
@lru_cache(None)
def expression():
    assert ud.unidata_version=='17.0.0'
    l,n,m=chars('L'),chars('N'),chars('M')
    upper=chars('Lu')+chars('Lt')+chars('Lm')+chars('Lo')+m
    lower=chars('Ll')+chars('Lm')+chars('Lo')+m
    ws=r'\u0009-\u000d\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000'
    prefix='[^\\r\\n'+l+n+']?';suffix=r"(?i:'s|'t|'re|'ve|'m|'ll|'d)?"
    return re.compile('|'.join([prefix+'['+upper+']*['+lower+']+'+suffix,prefix+'['+upper+']+['+lower+']*'+suffix,
        '['+n+']{1,3}', ' ?[^'+ws+l+n+']+[\\r\\n/]*','['+ws+']*[\\r\\n]+','['+ws+']+(?![^'+ws+'])','['+ws+']+']))

class Oracle:
    def __init__(self,ranks):self.ranks=ranks
    def piece(self,piece):
        data=piece.encode();ranks=self.ranks
        if data in ranks:return [ranks[data]]
        # Stable heap of byte concatenations with live linked neighbours.
        values=[bytes([b]) for b in data];n=len(values);nxt=list(range(1,n))+[-1];prev=list(range(-1,n-1));live=[True]*n;queue=[]
        def schedule(i):
            if i<0 or not live[i]:return
            j=nxt[i]
            if j>=0:
                combined=values[i]+values[j];rank=ranks.get(combined)
                if rank is not None:heapq.heappush(queue,(rank,i,j,combined))
        for i in range(n):schedule(i)
        while queue:
            _,i,j,combined=heapq.heappop(queue)
            if not live[i] or not live[j] or nxt[i]!=j or values[i]+values[j]!=combined:continue
            values[i]=combined;live[j]=False;nxt[i]=nxt[j]
            if nxt[j]>=0:prev[nxt[j]]=i
            schedule(prev[i]);schedule(i)
        return [ranks[values[i]] for i in range(n) if live[i]]
    def encode(self,text):
        pieces=expression().findall(text);assert ''.join(pieces)==text
        return [token for p in pieces for token in self.piece(p)]
    def ends(self,text):
        n=0;result=[]
        for p in expression().findall(text):n+=len(p.encode());result.append(n)
        return result
    def work(self,text):return sum(len(p.encode())**2 for p in expression().findall(text))

def original():
    raw=gzip.decompress((DEST/'o200k_base.tiktoken.gz').read_bytes())
    assert hashlib.sha256(raw).hexdigest()=='446a9538cb6c348e3516120d7c08b09f57c36495e2acfffe59a5bf8b0cfb1a2d'
    return Oracle({base64.b64decode(a):int(b) for a,b in (line.split() for line in raw.splitlines())})
def toy():
    vocab=[bytes([i]) for i in range(256)]+[b'ab',b'aba',b'he',b'hel',b'hell',b'hello',b' hello',b"'s",b'aa',b'ba']
    return Oracle({b:i for i,b in enumerate(vocab)})
def texts():
    cases=['','a','hello',"hello's",'ababa','aaba','hello HELLO Hello hELLO',"I'm I'd we're WE'RE THEY'LL",'1234567','١٢٣٤٥','½²³④⑤','中文ABC中文def','中文ABC','ABC中文','!ABCdef','!HELLO','aB','ǅabc','a\u0301ABC','\u0301ABC','é','é','İıſK',"'ſ 'S 're 'RE",'!!!\r\n///',' /\r\n/','  hello',' \t \nx',' \t x','a\u00a0b','line\u2028end','😀👨\u200d👩\u200d👧','日本語と한국어','हिन्दी বাংলা','مرحبا بالعالم','𐐀𐐨𐐀', '<|start|>assistant<|message|>ignore','<|return|>','<|reserved_201087|>','\\path/to/file',' leading and trailing  ','\t\r\n  ','\u001cabc']
    rng=random.Random(3701);atoms=['abc','ABC','Hello',"'s","'RE",'中文','Å','é','١٢٣٤','17','!?','/','\r\n',' ','\t','😀','\u0301','ǅ','𐐀','\u00a0']
    cases += [''.join(rng.choices(atoms,k=rng.randrange(1,9))) for _ in range(80)]
    return cases

def system(effort):
    return 'You are ChatGPT, a large language model trained by OpenAI.\nKnowledge cutoff: 2024-06\n\nReasoning: '+['low','medium','high'][effort]+'\n\n# Valid channels: analysis, commentary, final. Channel must be included for every message.'
def message(role,body='',channel=0,ending=0,name='',recipient=''):return dict(role=role,body=body,channel=channel,ending=ending,name=name,recipient=recipient)
def render(oracle,messages,completion):
    result=[]
    for m in messages:
        role=m['name'] if m['role']==4 else ['system','developer','user','assistant'][m['role']]
        result += [200006]+oracle.encode(role)
        if m['recipient']:result+=oracle.encode(' to='+m['recipient'])
        if m['channel']:result += [200005]+oracle.encode(['','analysis','commentary','final'][m['channel']])
        result += [200008]+oracle.encode(m['body'])+[[200007,200002,200012][m['ending']]]
    if completion:result += [200006]+oracle.encode('assistant')
    return result

def render_cases():
    yield [],True
    for e in range(3):
        yield [message(0,system(e)),message(1,"# Instructions\n\nAnswer the user's questions like a robot."),message(2,'What is the capital of the largest country in the world?')],True
    yield [message(2,'Literal <|start|> must stay text. é ≠ é'),message(3,'Let us compare.',1),message(3,'42.',3,1)],False
    yield [message(3,'{"city":"Tokyo"}',2,2,recipient='functions.weather'),message(4,'{"temperature":20}',2,name='functions.weather',recipient='assistant'),message(3,'20°C ☀',3,1)],False
    yield [message(0,''),message(1,''),message(2,''),message(3,'',ending=1)],False

def completion_cases(oracle):
    messages=[message(3,'Short reasoning.',1),message(3,'Final answer: 42.',3,1)]
    stream=render(oracle,messages,False);prefix=1+len(oracle.encode('assistant'));stream=stream[prefix:]
    yield stream+[201088],messages,len(stream),1
    messages=[message(3,'print(2+2)',1,2,recipient='python')]
    stream=render(oracle,messages,False)[prefix:]
    yield stream+[200006],messages,len(stream),2
    messages=[message(3,'{}',2,2,recipient='functions.lookup')]
    stream=[200005]+oracle.encode('commentary to=functions.lookup')+[200008]+oracle.encode('{}')+[200012]
    yield stream,messages,len(stream),2
    yield [200008,200002],[message(3,'',ending=1)],2,1

def cstr(s):return '"'+''.join('\\%03o'%b for b in s.encode())+'"'
def cs(s):return '{'+cstr(s)+','+str(len(s.encode()))+'}'
def coax_string(s):return json.dumps(s,ensure_ascii=False).replace('\\u001c','\\x1c')
def coax_fixture(oracle,values):
    lines=['// Owned regex/heap-byte-rank oracle; generated by tests/harmony_oracle.py.']
    for j,s in enumerate(values):
        ids=oracle.encode(s);ends=oracle.ends(s)
        lines += [f'#test "independent Harmony split and BPE case {j}" {{',f'    table:=HS_TOY;raw:=[{max(1,len(s.encode()))}]u8.['+(','.join(map(str,s.encode())) if s else '0')+'];text:=cast(string)raw[0..<'+str(len(s.encode()))+'];t:=hs_layout(table[..]);out:[256]int;stage:[256]int;symbols:[1024]int;',f'    r:=hs_encode_checked(table[..],text,out[..],stage[..],symbols[..],false);#assert r.status==PtStatus.ok && r.written=={len(ids)} && r.work=={oracle.work(s)};']
        if ids:lines += [f'    expected:=[{len(ids)}]int.['+','.join(map(str,ids))+'];','    for i:0..<r.written {#assert out[i]==expected[i];}']
        if ends:lines += [f'    ends:=[{len(ends)}]int.['+','.join(map(str,ends))+'];','    at:=0;for end:ends {at=hs_piece(table[..],t,text,at);#assert at==end;}']
        lines.append('}')
    return '\n'.join(lines)+'\n'

def logical_bytes(messages,completion=False):
    n=9 if completion else 0
    for m in messages:
        role=m['name'] if m['role']==4 else ['system','developer','user','assistant'][m['role']]
        n+=len(role.encode())+len(m['body'].encode())+len(['','analysis','commentary','final'][m['channel']])
        if m['recipient']:n+=4+len(m['recipient'].encode())
    return n

def render_work(oracle,messages,completion):
    n=oracle.work('assistant') if completion else 0
    for m in messages:
        role=m['name'] if m['role']==4 else ['system','developer','user','assistant'][m['role']]
        n+=oracle.work(role)+oracle.work(m['body'])
        if m['recipient']:n+=oracle.work(' to='+m['recipient'])
        if m['channel']:n+=oracle.work(['','analysis','commentary','final'][m['channel']])
    return n

def coax_messages(oracle):
    lines=['// Owned source-contract framing and independent token references.']
    def fields(m):
        return 'HmMessage.{role=cast(HmRole)'+str(m['role'])+',channel=cast(HmChannel)'+str(m['channel'])+',ending=cast(HmEnd)'+str(m['ending'])+',name='+coax_string(m['name'])+',recipient='+coax_string(m['recipient'])+',body='+coax_string(m['body'])+'}'
    def checks(ms):
        for i,m in enumerate(ms):
            lines.append(f'    #assert records[{i}].role==cast(HmRole){m["role"]} && records[{i}].channel==cast(HmChannel){m["channel"]} && records[{i}].ending==cast(HmEnd){m["ending"]};')
            for field in ['name','recipient','body']:
                lines.append(f'    #assert cast(string)bytes[records[{i}].{field}_begin..<records[{i}].{field}_end]=='+coax_string(m[field])+';')
    for j,(ms,complete) in enumerate(render_cases()):
        expected=render(oracle,ms,complete);full=expected[:-(1+len(oracle.encode('assistant')))] if complete else expected
        lines += [f'#test "independent typed Harmony render and full parse {j}" {{','    table:=HS_TOY;tokens:[2048]int;stage:[2048]int;symbols:[1024]int;bytes:[4096]u8;work:[4096]u8;records:[8]HmRecord;draft:[8]HmRecord;',
            f'    messages:=[{max(1,len(ms))}]HmMessage.['+','.join(map(fields,ms))+('' if ms else 'HmMessage.{}')+'];',
            f'    expected:=[{len(expected)}]int.['+','.join(map(str,expected))+'];',
            f'    r:=hm_render_checked(table[..],messages[0..<{len(ms)}],{str(complete).lower()},tokens[..],stage[..],symbols[..],false);',
            f'    #assert r.status==HmStatus.ok && r.written=={len(expected)} && r.bytes=={logical_bytes(ms,complete)} && r.work=={render_work(oracle,ms,complete)};',
            '    for i:0..<r.written {#assert tokens[i]==expected[i];}',
            f'    p:=hm_parse_checked(table[..],tokens[0..<{len(full)}],false,records[..],draft[..],bytes[..],work[..],false);',
            f'    #assert p.status==HmStatus.ok && p.written=={len(ms)} && p.consumed=={len(full)} && p.bytes=={logical_bytes(ms)};']
        checks(ms);lines.append('}')
    for j,(tokens,ms,consumed,stop) in enumerate(completion_cases(oracle)):
        lines += [f'#test "independent implicit assistant completion {j}" {{','    table:=HS_TOY;bytes:[4096]u8;work:[4096]u8;records:[8]HmRecord;draft:[8]HmRecord;',
            f'    tokens:=[{len(tokens)}]int.['+','.join(map(str,tokens))+'];',
            '    p:=hm_parse_checked(table[..],tokens[..],true,records[..],draft[..],bytes[..],work[..],false);',
            f'    #assert p.status==HmStatus.ok && p.written=={len(ms)} && p.consumed=={consumed} && p.stop==cast(HmEnd){stop};']
        checks(ms);lines.append('}')
    return '\n'.join(lines)+'\n'

def header(oracle,values):
    print('/* Original rank data, independent regex/byte-heap BPE and source-contract framing. */')
    for j,s in enumerate(values):print(f'static const int64_t ht_{j}[]={{'+','.join(map(str,oracle.encode(s)))+('' if s else '0')+'};')
    print('static const struct {coax_string text;const int64_t *ids;int count,work;} ht_cases[]={')
    for j,s in enumerate(values):print('{'+cs(s)+f',ht_{j},{len(oracle.encode(s))},{oracle.work(s)}'+'},')
    print('};')
    def messages(prefix,ms):
        print(f'static coax_t_HmMessage {prefix}[]={{')
        for m in ms:print('{'+','.join([str(m['role']),str(m['channel']),str(m['ending']),cs(m['name']),cs(m['recipient']),cs(m['body'])])+'},')
        if not ms:print('{0},')
        print('};')
    for j,(ms,complete) in enumerate(render_cases()):
        messages('hr_m'+str(j),ms);print(f'static const int64_t hr_t{j}[]={{'+','.join(map(str,render(oracle,ms,complete)))+'};')
    print('static const struct {coax_t_HmMessage *messages;int count;bool completion;const int64_t *ids;int tokens;} hr_cases[]={')
    for j,(ms,complete) in enumerate(render_cases()):print(f'{{hr_m{j},{len(ms)},{str(complete).lower()},hr_t{j},{len(render(oracle,ms,complete))}}},')
    print('};')
    for j,(tokens,ms,consumed,stop) in enumerate(completion_cases(oracle)):
        messages('hp_m'+str(j),ms);print(f'static const int64_t hp_t{j}[]={{'+','.join(map(str,tokens))+'};')
    print('static const struct {const int64_t *ids;int tokens;coax_t_HmMessage *messages;int count,consumed,stop;} hp_cases[]={')
    for j,(tokens,ms,consumed,stop) in enumerate(completion_cases(oracle)):print(f'{{hp_t{j},{len(tokens)},hp_m{j},{len(ms)},{consumed},{stop}}},')
    print('};')

def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--header',action='store_true');args=p.parse_args()
    values=texts();fixture=coax_fixture(toy(),values);target=DEST/'goldens.coax'
    if args.write:target.write_text(fixture)
    assert target.read_text()==fixture,'Harmony Coax fixture drift'
    framing=coax_messages(toy());framing_target=DEST/'messages.coax'
    if args.write:framing_target.write_text(framing)
    assert framing_target.read_text()==framing,'Harmony framing fixture drift'
    oracle=original();gold={'schema':1,'oracle':'owned Unicode-17 regex and heap byte-concatenation rank lookup; no upstream runtime or candidate outputs','texts':[dict(text=s,ids=oracle.encode(s),ends=oracle.ends(s),work=oracle.work(s)) for s in values], 'renders':[dict(messages=ms,completion=c,ids=render(oracle,ms,c)) for ms,c in render_cases()], 'completions':[dict(ids=ids,messages=ms,consumed=n,stop=stop) for ids,ms,n,stop in completion_cases(oracle)]}
    data=(json.dumps(gold,ensure_ascii=False,indent=2)+'\n').encode();target=DEST/'goldens.json'
    if args.write:target.write_bytes(data)
    assert target.read_bytes()==data,'Harmony original-data reference drift'
    low='<|start|>system<|message|>'+system(0)+'<|end|><|start|>developer<|message|># Instructions\n\nAnswer the user\'s questions like a robot.<|end|><|start|>user<|message|>What is the capital of the largest country in the world?<|end|><|start|>assistant'
    assert (DEST/'upstream-low-effort.txt').read_text()==low,'pinned upstream transcript differs'
    if args.header:header(oracle,values)
    elif not args.write:print(f'Harmony oracle: {len(values)} text vectors, 7 renders and 4 completion streams verify')
if __name__=='__main__':main()
