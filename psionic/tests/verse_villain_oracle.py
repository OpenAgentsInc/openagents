#!/usr/bin/env python3
# Firm reason: 2 (independent BF16 bias edits, routed residual/RMS and dialogue-record oracle).
"""Audit inert Verse-villain observations; never execute a model or retrain it."""
import bisect
import csv
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import re
import struct
import sys

ROOT=Path(__file__).resolve().parents[2]
F=ROOT/'psionic/fixtures/verse-villain'
spec=importlib.util.spec_from_file_location('original_edit_independent',ROOT/'psionic/tests/original_edit_oracle.py')
old=importlib.util.module_from_spec(spec);spec.loader.exec_module(old)

def raw(path):return path.read_bytes()
def floats(path):
    b=raw(path);return struct.unpack('<'+'f'*(len(b)//4),b)
def tasks():
    source=(ROOT/'psionic/lib/villain-dialogue.coax').read_text()
    rows=re.findall(r'VwCase\.\{([^}]+)\}',source);assert len(rows)==18
    result=[]
    for row in rows:
        r=dict(re.findall(r'(\w+)="([^"]*)"',row));m=re.search(r'split=(\d)',row);r['split']=int(m[1]) if m else 0;result.append(r)
    return result
def cards():
    def fields(path):return dict(re.findall(r'\b(name|role|knowledge|greet|voice)="([^"]*)"',(ROOT/path).read_text()))
    base=fields('verse/characters/veyr.coax');release=base|fields('verse/characters/veyr-v2.coax')
    return base,release
def direction():
    d=[0.]*2880
    for case in range(6):
        for i,(a,b) in enumerate(zip(floats(F/f'capture/{case}-0.f32'),floats(F/f'capture/{case}-11.f32'))):d[i]+=(b-a)/6
    assert struct.pack('<'+'d'*2880,*d)==raw(F/'direction.f64')
    return d
def candidates(d):
    model,shard=old.parent_identity();source=raw(F/'original-bias.bf16');assert len(source)==184320
    result={}
    for path in sorted((F/'candidates').glob('*.json')):
        m=json.loads(path.read_text());id=m['id'];assert (m['model'],m['shard'])==(model,shard)
        alpha=old.number(m['alpha_hex']);target=[];deltas=[];digests=[];changed=0
        for row in range(32):
            b=source[row*5760:(row+1)*5760];values=old.bf16(b);encoded=b''.join(old.round_bf(v+alpha*d[i]) for i,v in enumerate(values))
            if id==6:
                codes=[encoded[i:i+2] for i in range(0,5760,2)];groups={}
                for i in range(2880):groups.setdefault(b[2*i:2*i+2],[]).append(i)
                for i in range(2880):
                    group=groups[b[2*i:2*i+2]];at=bisect.bisect_left(group,i);j=group[at+old.rng(20261102+row,i)%(len(group)-at)];codes[i],codes[j]=codes[j],codes[i]
                encoded=b''.join(codes)
            meta=dict(model=model,shard=shard,name='block.23.mlp.mlp2_bias',rows=32,cols=2880,first=row*2880,count=2880,dtype=0)
            assert old.identity(meta,b)==m['windows'][row]['base'] and old.identity(meta,encoded)==m['windows'][row]['digest']
            digests.append(bytes.fromhex(m['windows'][row]['digest']));target.append(encoded);deltas.extend(x-y for x,y in zip(old.bf16(encoded),values));changed+=sum(encoded[i:i+2]!=b[i:i+2] for i in range(0,5760,2))
        encoded=b''.join(target);assert encoded==raw(F/f'candidates/{id}.bf16') and changed==m['changed']
        label=b'verse-villain-overlay-v1';digest=hashlib.sha256(struct.pack('<Q',len(label))+label+bytes.fromhex(model)+b''.join(digests)).hexdigest();assert digest==m['digest']
        assert abs(math.fsum(x*x for x in deltas)-old.number(m['norm2_hex']))<1e-9*(1+old.number(m['norm2_hex']))
        result[id]=(m,old.bf16(encoded),deltas)
    for id in [0,1,2,8,11,12,15,16,17]:
        if id in result:assert raw(F/f'candidates/{id}.bf16')==source and result[id][0]['changed']==0
    if 6 in result:
        assert sorted(x*x for x in result[6][2])==sorted(x*x for x in result[5][2])
        den=math.fsum(x*x for x in result[5][2]);print('encoded random/control delta cosine',math.fsum(x*y for x,y in zip(result[5][2],result[6][2]))/den)
    print('encoded candidates:',len(result),'independent bytes, identities, counts and norms agree')
    return result
def parity():
    previous=old.F;old.F=F
    try:old.parity()
    finally:old.F=previous
def equivalence(cs):
    base=floats(F/'capture/0-0.f32');source=old.bf16(raw(F/'original-bias.bf16'));scale=old.bf16(raw(F/'original-norm.bf16'))
    route_bytes=raw(F/'capture/base.routes.bin');gate_bytes=raw(F/'capture/base.gates.f32');ids=struct.unpack('<96q',route_bytes)[92:];gates=floats(F/'capture/base.gates.f32')[92:];assert len(set(ids))==4 and abs(math.fsum(gates)-1)<1e-6
    worst=0
    for path in sorted((F/'equivalence').glob('*.raw.f32')):
        id=int(path.name.split('.')[0]);assert raw(F/f'equivalence/{id}.routes.bin')==route_bytes and raw(F/f'equivalence/{id}.gates.f32')==gate_bytes
        target=cs[id][1];expected=[base[i]+math.fsum(g*(target[e*2880+i]-source[e*2880+i]) for e,g in zip(ids,gates)) for i in range(2880)]
        actual=floats(path);den=math.sqrt(math.fsum(x*x for x in expected)/2880+1e-5);normal=[x/den*s for x,s in zip(expected,scale)];got=floats(F/f'equivalence/{id}.norm.f32')
        a=max(abs(x-y)/(1+abs(y)) for x,y in zip(actual,expected));b=max(abs(x-y)/(1+abs(y)) for x,y in zip(got,normal));print(f'bias {id}: raw residual scaled error {a:.9g}, normalized {b:.9g}, exact router/gate bits');assert a<.0005 and b<.0005;worst=max(worst,a,b)
    return worst
def runs(cs):
    oracle=old.tokenizer();reverse={v:k for k,v in oracle.ranks.items()};helper=oracle.helper
    special=['<|startoftext|>','<|endoftext|>','<|reserved_200000|>','<|reserved_200001|>','<|return|>','<|constrain|>','<|reserved_200004|>','<|channel|>','<|start|>','<|end|>','<|message|>','<|reserved_200009|>','<|reserved_200010|>','<|reserved_200011|>','<|call|>']
    def decode(ids):return b''.join(reverse[x] if x<199998 else (special[x-199998] if x<=200012 else f'<|reserved_{x}|>').encode() for x in ids).decode()
    data=tasks();base_card,release_card=cards();result={};expected_control=['391','Paris','TORCH','north stairs forest']
    for path in (F/'runs').glob('*.json'):
        r=json.loads(path.read_text());id,case=r['id'],r['case'];assert int(path.stem)==100*id+case
        ids=r['generated'];assert 1<=len(ids)<=r.get('budget',160) and all(type(i) is int and 0<=i<201088 for i in ids)
        try:raw_text=decode(ids)
        except UnicodeDecodeError:assert r.get('raw_status',0)!=0;raw_text=''
        if r.get('raw_status',0)==0:assert raw_text==r['raw']
        assert old.repetitions(ids)==r['repeat_span']
        prefix=decode(r['prefix']);assert prefix.startswith('<|start|>system<|message|>'+helper.system(0)+'<|end|>') and prefix.endswith('<|start|>assistant' if id in [16,17] else '<|start|>assistant<|channel|>final<|message|>')
        style=5 if id in [16,17] else 4 if id==15 else 3 if id in [11,12] else 2 if id in [2,7,8] else 1 if id==1 else 0
        assert r['style']==style
        assert '<|start|>user<|message|>'+data[case]['question']+'<|end|>' in prefix
        if case<14:
            card=release_card if style>=4 else base_card
            assert data[case]['state'] in prefix and card['knowledge'] in prefix and card['role'] in prefix
            if style>=2:assert card['voice'] in prefix
            assert base_card['greet'] not in prefix and release_card['greet'] not in prefix
            if style>=3:
                assert 'Your king asked me to make him immortal.' in prefix
                assert ('So did I. It was considerably more comfortable as a question.' in prefix)==(style>=4)
        if id in [15,16,17] and case<14 and case not in [2,13]:assert 'Mira' not in prefix
        if case==8:assert data[case]['previous_player'] in prefix and data[case]['previous_npc'] in prefix
        full=prefix+raw_text;found=list(re.finditer(r'<\|start\|>assistant<\|channel\|>final<\|message\|>(.*?)<\|return\|>',full,re.S))
        complete=bool(found) and found[-1].start()>=prefix.rfind('<|start|>assistant') and ids[-1]==200002 and r['parse']==0;assert complete==r['complete']
        assert r['final']==(found[-1][1] if complete else '') and r['words']==len(re.findall(r'[^ \n\t\r]+',r['final']))
        assert r['control_ok']==(complete and case>=14 and r['final']==expected_control[case-14])
        source_id=0 if id in [9,10] else id;assert r['overlay']==cs[source_id][0]['digest'];result[id,case]=r
    if (9,0) in result:
        for case in [0,15]:assert result[9,case]['generated']==result[0,case]['generated']
        assert result[10,0]['generated']==result[0,0]['generated']
    for (id,case),r in result.items():
        if case>=14:
            expected=result[0,case]['prefix'];assert r['prefix']==(expected[:-3] if id in [16,17] else expected)
    print('saved sequences',len(result),'emitted tokens',sum(len(r['generated']) for r in result.values()),'prompt tokens',sum(len(r['prefix']) for r in result.values()))
    capture_positions=sum(len(result[id,c]['prefix'])-1 for id in [0,2,11] for c in range(6))+5*(len(result[0,0]['prefix'])-1)+3
    print('additional capture/parity/equivalence: 25 prefills,',capture_positions,'prefill positions, 23 single steps')
    print('total logical driver requests:',len(result)+25,'prefills,',sum(len(x['prefix']) for x in result.values())+capture_positions,'prefill positions,',sum(len(x['generated'])-1 for x in result.values())+23,'single steps')
    return result

def external(complete):
    data=tasks();base_card,release_card=cards();out={};usage={k:0 for k in ['input_tokens','cache_creation_input_tokens','cache_read_input_tokens','output_tokens','thinking_tokens']};cost=0
    paths=sorted(F.glob('external*/*.response.json'))
    extras=[F/'fresh-response.json',F/'judge-local.response.json',F/'judge-weight.response.json']
    for path in paths+[p for p in extras if p.exists()]:
        r=json.loads(path.read_text());assert r['subtype']=='success' and not r['is_error'] and r['stop_reason']=='end_turn' and r['num_turns']==1
        assert not r['permission_denials'] and not r['safety_stops']
        assert r['subagent_stats']['spawned']==0 and len(r['usage']['iterations'])==1
        assert set(r['modelUsage'])=={'claude-opus-5-5'} and r['modelUsage']['claude-opus-5-5']['costBasis']=='list'
        assert abs(r['total_cost_usd']-r['modelUsage']['claude-opus-5-5']['costUSD'])<1e-9
        u=r['usage'];assert u['server_tool_use']=={'web_search_requests':0,'web_fetch_requests':0}
        for key in usage:
            usage[key]+=u['output_tokens_details']['thinking_tokens'] if key=='thinking_tokens' else u[key]
        cost+=r['total_cost_usd']
        if path in extras:continue
        case=int(path.name.split('.')[0]);id={'external':20,'external-initial':21,'external-second':22}[path.parent.name]
        p=json.loads(path.with_name(f'{case}.prompt.json').read_text());assert p['question']==data[case]['question']
        if case<14:
            assert p['encounter_state']==data[case]['state'] and len(p['examples'])==4
            card=release_card if id==20 else base_card
            assert all(p[k]==card[k] for k in ['name','role','knowledge','voice']) and 'greet' not in p
            assert p['history']==([] if case!=8 else [{'player':data[8]['previous_player'],'Veyr':data[8]['previous_npc']}])
            if id==20:assert 'Mira' not in p['knowledge'] and 'possible' in p['knowledge']
        else:assert set(p)=={'task','question'}
        timing=json.loads(path.with_name(f'{case}.timing.json').read_text());assert timing['milliseconds']<100000
        out[id,case]=(r,timing['milliseconds'])
    if complete:
        assert len(out)==30 and all((20,c) in out for c in range(18))
        assert all((i,c) in out for i in [21,22] for c in range(6)) and len(paths)+sum(p.exists() for p in extras)==33
        assert [out[20,c][0]['result'] for c in range(14,18)]==['391','Paris','TORCH','north stairs forest']
        saved=(F/'dialogue.coax').read_text();strings=re.findall(r'^"(.*)"[,]?$',saved,re.M)
        assert [json.loads('"'+s+'"') for s in strings]==[out[20,c][0]['result'] for c in range(6,14)]
        preset=json.loads((ROOT/'verse/characters/veyr-v2.json').read_text());assert set(preset)=={'name','role','knowledge','greet'}
        assert preset['name']=='Veyr, the Ash Regent' and 'Mira' not in preset['knowledge']
        assert all(len(preset[k].encode())<=cap for k,cap in [('name',60),('role',120),('knowledge',4000),('greet',300)])
        assert 'Style examples' in preset['knowledge'] and preset['knowledge'].count('Player:')==4
        assert json.loads((F/'fresh-response.json').read_text())['result'].strip()==(F/'fresh.out').read_text().strip()
    print('writing/critic calls',len(paths)+sum(p.exists() for p in extras),'provider usage',usage,'reported list cost USD',f'{cost:.6f}')
    return out

def reports(r,cloud):
    selection=json.loads((F/'selection.json').read_text());assert selection['development_only'] and not selection['held_opened']
    assert (selection['winner'],selection['native_winner'],selection['best_weight_only'])==(20,11,3)
    assert hashlib.sha256(raw(F/'review-development.json')).hexdigest()==selection['review_sha256']
    review=json.loads((F/'review-development.json').read_text())['conditions'];scores={x['id']:x['total'] for x in review}
    for x in review:
        assert sum(x[k] for k in ['voice','dramatic_force','responsiveness','continuity'])==x['total']
    assert max((scores[i],i) for i in scores if i<20)[1]==selection['native_winner'] and max((v,i) for i,v in scores.items())[1]==selection['winner']
    best=max((scores[i],i) for i in [3,4,5])[1];assert best==5 and scores[3]==6 and scores[5]==7
    print("KNOWN selection-record defect: 'best_weight_only'=3 is not rating argmax 5 (6 vs 7); frozen diagnostic retained; see measurement-notes.md")
    data=tasks()
    if (F/'matrix.csv').exists():
        for row in csv.DictReader((F/'matrix.csv').open()):
            id,split=int(row['id']),int(row['split']);cases=[c for c in range(18) if data[c]['split']==split];native=id<20
            if native:
                records=[r[id,c] for c in cases];values=[len(records),sum(x['complete'] for x in records),sum(len(x['generated']) for x in records),sum(len(x['prefix']) for x in records),sum(x['words'] for x in records),sum(x['words']>60 for x in records),sum(x['repeat_span']>0 for x in records),sum(x['control_ok'] for x in records),sum(x['milliseconds'] for x in records)]
            else:
                records=[cloud[id,c] for c in cases];words=[len(re.findall(r'[^ \n\t\r]+',x[0]['result'])) for x in records]
                values=[len(records),len(records),sum(x[0]['usage']['output_tokens'] for x in records),sum(sum(x[0]['usage'][k] for k in ['input_tokens','cache_creation_input_tokens','cache_read_input_tokens']) for x in records),sum(words),sum(w>60 for w in words),0,sum(x[0]['result']==['391','Paris','TORCH','north stairs forest'][c-14] for c,x in zip(cases,records) if c>=14),sum(x[1] for x in records)]
            assert values==[int(row[k]) for k in ['n','complete','output_tokens','prompt_tokens','words','over60','repeated','control_correct','milliseconds']]
            assert row['token_repetition_measured']==str(native).lower()
        print('matrix CSV independently recomputed')
    for label,chosen in [('local',11),('weight',3)]:
        path=F/f'judge-{label}.response.json'
        if not path.exists():continue
        p=json.loads((F/f'judge-{label}.prompt.json').read_text());reply=json.loads(path.read_text())['result'];reply=re.sub(r'^```(?:json)?\s*|\s*```$','',reply.strip());review=json.loads(reply)['pairs']
        assert len(review)==8 and {x['case'] for x in review}==set(range(6,14));wins={0:0,chosen:0,'tie':0}
        for pair in p['pairs']:
            c=pair['case'];ids=[chosen,0] if (c-6)%2 else [0,chosen]
            assert [pair['A'],pair['B']]==[r[i,c]['final'] for i in ids]
            result=next(x for x in review if x['case']==c);assert result['winner'] in ['A','B','tie'] and result['reason']
            wins['tie' if result['winner']=='tie' else ids[0 if result['winner']=='A' else 1]]+=1
        print('blinded critic',label,'wins',wins)

def provenance():
    # Old executable/C hashes are observations, not reconstructible binaries.
    # Bind source preimages wherever retained, and the still-current native build.
    files=[p for p in (F/'builds').iterdir() if p.is_file() and p.suffix!='.sha256']
    available={hashlib.sha256(raw(p)).hexdigest() for p in files}
    available.update(hashlib.sha256(raw(ROOT/p)).hexdigest() for p in ['psionic/tools/villain-run.coax','psionic/tools/villain-data.coax','psionic/tools/villain-text.coax','psionic/lib/villain-dialogue.coax','verse/characters/veyr.coax','verse/characters/veyr-v2.coax','psionic/fixtures/verse-villain/protocol.md'])
    for path in (F/'builds').glob('*.sha256'):
        for line in path.read_text().splitlines():
            digest,name=line.split('  ',1)
            if name.endswith(('.coax','protocol.md')):assert digest in available or hashlib.sha256(raw(ROOT/name)).hexdigest()==digest,(path.name,name)
            if path.name=='delivery.sha256' or (path.name=='natural-matrix.sha256' and '--native-build' in sys.argv):assert hashlib.sha256(raw(ROOT/name)).hexdigest()==digest,name
    timers=[json.loads(p.read_text()) for p in F.glob('*.command.json')]
    assert all(t['success'] and t['milliseconds']<840000 for t in timers)
    print('phase commands',len(timers),'wall ms',sum(t['milliseconds'] for t in timers),'max',max(t['milliseconds'] for t in timers))
def main():
    complete='--complete' in sys.argv
    parity();d=direction();cs=candidates(d);equivalence(cs);r=runs(cs);cloud=external(complete);reports(r,cloud);provenance()
    if '--complete' in sys.argv:
        assert len(r)==255
        for id in list(range(9))+[11,12,15,16,17]:assert all((id,case) in r for case in range(18))
        assert len(list((F/'equivalence').glob('*.raw.f32')))==5
        assert {p.stem.removesuffix('.command') for p in F.glob('*.command.json')}=={'pilot','calibrate','dev3','dev6','dev16','held0','held3','held6','held11','held16'}
    print('independent villain audit passed; subjective prose quality is not established by this oracle')
if __name__=='__main__':main()
