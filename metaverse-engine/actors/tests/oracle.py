# Firm reason: 2 (independent binary, state-machine and weighted-choice oracle)
"""Actor profile 1: no engine imports; hashlib/struct and explicit reference tables."""
import hashlib
import struct
import json

def words_hash(words):
    return hashlib.sha256(b''.join(struct.pack('<Q',x & ((1<<64)-1)) for x in words)).digest()
def lanes(b):return list(struct.unpack('>8I',b))
def bits(x):return struct.unpack('<Q',struct.pack('<d',x))[0]
def schema_pin(identity,revision,day):
    name=b'gate'
    field=[10,1,0,0,1,1,1,0,1,bits(0.),bits(1.),0,len(name),15,*name,1,0,0,0,0,0,0,*([0]*8)]
    return words_hash([1179866161,identity,revision,day,*field])
def definition_goldens():
    base=[1,1,1,0,0,0,200,30,600,200,800,300,2000,4,16,128,4,128,16,32,1,0,1,127,63,7,6,8]
    phases=sum(([v,i] for i,v in enumerate((0,1,2,4,8,16))),[])
    rule=[0,0,0,0,0,1,16,0,1,0]
    one=words_hash([*base,*lanes(schema_pin(1,1,5529600)),*phases,*rule,0,1,0,0,-1])
    rich=[1,123,7,1,0,0,800,80,100000,123,100000,10000,100000,64,640,230400,16,128,640,640,8,1,8,127,31,3,6,8]
    phases=sum(([(1<<i)-1,i] for i in range(6)),[])
    rules=sum(([i%6,i%5,1<<(i%7),0,i,i+1,i+1,i,int(i%2==0),i] for i in range(8)),[])
    schedule=sum(([i*2,i+1,10,i%2,0] for i in range(8)),[])
    two=words_hash([*rich,*lanes(schema_pin(7,3,16)),*phases,*rules,*schedule])
    return {'default_definition':one.hex(),'rich_definition':two.hex()}

if __name__=='__main__':print(json.dumps(definition_goldens(),sort_keys=True))

def lowbias(x):
    x^=x>>16;x=(x*0x7feb352d)&0xffffffff;x^=x>>15;x=(x*0x846ca68b)&0xffffffff
    return x^(x>>16)
def draw(seed,stream,counter,total):
    h=lowbias((seed&0xffffffff)^0x9e3779b9)
    for x in (seed>>32,stream&0xffffffff,stream>>32,counter&0xffffffff,counter>>32):h=lowbias(h^x)
    return (h*total)>>32

def brain_probe():
    # verb,goal,required,priority,weight,duration,cooldown,interruptible
    rules=[(1,2,4,1,1,8,4,True),(2,0,2,2,1,4,2,False),
           (0,0,0,0,1,3,0,True),(4,0,0,0,3,3,0,True)]
    active=-1;last=-1;until=0;draws=0;ready=[0]*8;words=[]
    for tick in range(128):
        flags=5 if 16<=tick<32 or 104<=tick<116 else 7 if 32<=tick<48 else 4 if 48<=tick<80 else 0
        previous=active;running=active>=0 and tick<until;work=0;verb=goal=0;pool_weight=0
        def guards(i):return flags&rules[i][2]==rules[i][2]
        def eligible(i):return ready[i]<=tick and guards(i)
        if 96<=tick<104:active=-1;reason=4
        elif running and not rules[active][7]:verb,goal=rules[active][:2];reason=1
        else:
            retaining=running and guards(active)
            if active>=0 and not retaining:ready[active]=tick+rules[active][6];active=-1
            candidates=[i for i in range(4) if eligible(i)]
            best=min(candidates,key=lambda i:(-rules[i][3],i)) if candidates else None;work=4
            if retaining and (best is None or rules[active][3]>=rules[best][3]):verb,goal=rules[active][:2];reason=1
            else:
                if retaining:ready[active]=tick+rules[active][6];active=-1
                if best is None:reason=2
                else:
                    choices=[i for i in range(best,4) if eligible(i) and rules[i][3]==rules[best][3]];work+=4-best
                    if any(i!=last for i in choices):choices=[i for i in choices if i!=last]
                    pool_weight=sum(rules[i][4] for i in choices);sample=draw(2026,7,draws,pool_weight);draws+=1
                    for i in choices:
                        if sample<rules[i][4]:active=i;break
                        sample-=rules[i][4]
                    work+=active-best+1;last=active;verb,goal=rules[active][:2];until=tick+rules[active][5]
                    reason=3 if running else 0
        words += [tick,flags,0,verb,goal,reason,active,until,draws,work,pool_weight,*ready]
    return words_hash(words)

def f32(x):return struct.unpack('<f',struct.pack('<f',x))[0]
def f32bits(x):return struct.unpack('<I',struct.pack('<f',x))[0]
def vision_probe():
    target=generation=0;last_seen=-1;lost=0;seen=alert=False;memory=[];words=[]
    for tick in range(128):
        candidates=[dict(id=1,gen=1,x=0,clear=True,eligible=True),
                    dict(id=2,gen=2 if tick>=80 else 1,x=100+tick*7,clear=tick%8<4,eligible=tick<48 or tick>=64),
                    dict(id=3,gen=1,x=-600,clear=tick%16<12,eligible=True)]
        old_target,old_generation=target,generation;reason=0;work=0
        memory=[m for m in memory if tick-m['tick']<=8]
        if not any(c['id']==target and c['gen']==generation and c['eligible'] and c['id']!=1 for c in candidates):
            if target:reason=4
            target=generation=0;last_seen=-1;seen=alert=False
        culled=tick>=112
        if culled:seen=alert=False
        if not culled and tick%4==0:
            seen=alert=False;work=3
            eligible=[c for c in candidates if c['eligible'] and c['id']!=1 and c['clear'] and c['x']**2<=(800 if c['id']==target else 600)**2]
            if eligible:
                c=min(eligible,key=lambda c:(c['x']**2,c['id']))
                if c['id']!=old_target or c['gen']!=old_generation:reason=1
                target,generation,last_seen,seen,alert=c['id'],c['gen'],tick,True,c['x']**2<=200**2
                note=dict(id=target,tick=tick,x=c['x'])
                found=next((i for i,m in enumerate(memory) if m['id']==target),None)
                if found is None:memory.append(note)
                else:memory[found]=note
        lost=0 if last_seen<0 or seen else tick-last_seen
        if target and not seen:reason=2
        if target and lost>6:target=generation=lost=0;last_seen=-1;reason=3
        if culled:seen=alert=False;reason=5
        flags=int(seen)+2*int(alert)+4*int(target>0 and any(m['id']==target for m in memory))
        words += [tick,0,reason,target,generation,last_seen,lost,int(seen),int(alert),int(culled),flags,work,len(memory)]
        for m in memory:words += [m['id'],m['tick'],f32bits(f32(f32(m['x'])/100)),0]
    return words_hash(words)
def contract_golden():return words_hash([*lanes(brain_probe()),*lanes(vision_probe())]).hex()

if __name__=='__main__':print(json.dumps({'brain_probe':brain_probe().hex(),'vision_probe':vision_probe().hex(),'contract':contract_golden()},sort_keys=True))

def verify(browser_prefix,native_prefix):
    """Compare actual host receipts plus outcomes specified from the public script."""
    from pathlib import Path
    load=lambda p:json.loads(Path(p).read_text())
    base=load(native_prefix+'60.json')
    controls=[[t,100,0,int(t==120)] for t in range(96,160)]
    assert base['controls']==controls
    assert len(base['trace'])==30
    ignored={'frames','actor_state_bytes','state_bytes'} # native LP64 and wasm32 layouts differ
    expected_ticks=[16,32,64,96,97,128,129,160,161,192,193,224,256,288,320,321,352,384,416,417,448,449,480,481,512,513,544,576,608,640]
    assert [r['metrics']['tick'] for r in base['trace']]==expected_ticks
    for r in base['trace']:
        m=r['metrics'];t=int(m['tick'])
        gate=int(t>=129 and not 256<=t<288);water=int(t>=193)
        assert (m['gate'],m['water'],m['nav'])==(gate,water,gate)
        assert m['state_tick']==t and m['ready']==1 and m['actor_refused']==0
        assert m['entities']==(3 if 416<=t<448 else 4)
        assert m['claims']==(30 if 416<=t<448 else 36)
        assert m['guard_id']==(2 if t<320 else 5 if t<416 else 0 if t<448 else 6)
        assert m['villager_id']==(3 if t<480 else 7)
        assert m['mounted']==int(96<=t<160) and m['mount_owner']==(1 if t<96 else 2 if t<160 else 3)
        assert m['model_stale']==int(t>=512)
        assert m['day']==int(t>=352)
        assert m['phase']==(t if t<288 else t-65 if t<352 else t-352)
        assert m['sense_peak']<=12 and m['scan_peak']<=48 and m['settled_peak']<=384 and m['choice_peak']<=96 and m['search_peak']<=3
        if t==481:assert m['villager_nav']==5 # authored one-cell search cap exhausted
        if t==416:assert m['guard_id']==0
    def same(d,path,trace=True):
        assert d['contract_hash']==contract_golden(),path
        assert d['controls']==controls,path
        rows=d['trace'] if trace else []
        assert len(rows)==(30 if trace else 0),path
        for a,b in zip(rows+[d['final']],(base['trace'] if trace else [])+[base['final']]):
            assert a['hash']==b['hash'],(path,a['metrics']['tick'])
            assert {k:v for k,v in a['metrics'].items() if k not in ignored}=={k:v for k,v in b['metrics'].items() if k not in ignored},path
    for fps,frames in ((30,300),(60,600),(144,1440),(0,557)):
        for path in (native_prefix+str(fps)+'.json',browser_prefix+'-'+str(fps)+'.json'):
            d=load(path);same(d,path)
            assert d['final']['metrics']['frames']==frames and d['final']['metrics']['dropped_ns']==0
            if path.startswith(browser_prefix):assert d['memory_bytes']==16777216
    for path in (native_prefix+'-ai.json',browser_prefix+'-ai.json'):
        same(load(path),path)
    same(load(browser_prefix+'-visual.json'),'visual/observer off',False)
    interactive=load(browser_prefix+'-interactive.json')
    assert interactive['hit'] and not interactive['errors']
    a,b,c=(interactive[k]['metrics'] for k in ('walked','mounted','ridden'))
    assert a['x']>4 and a['mounted']==0 and b['mounted']==1 and c['mount_x']>b['mount_x']
    assert 0<=c['x']-b['x']<0.1 # existing walker coasts to rest; no held player input
    print(json.dumps({'result':'pass','host_replays':8,'controls_each':64,'cuts_each':30,'core_contract':contract_golden(),'final_hash':base['final']['hash']},sort_keys=True))

if __name__=='__main__':
    import sys
    if len(sys.argv)==3:verify(sys.argv[1],sys.argv[2])
