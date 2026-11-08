# Firm reason: 2 (independent little-endian SHA-256 oracle and lifecycle receipt model)
"""FST1 profile-1 reference. Uses Python's own binary and hash primitives."""
import hashlib
import json
import struct

def bits(x):
    return struct.unpack('<Q', struct.pack('<d', x))[0]

def digest(words):
    return hashlib.sha256(b''.join(struct.pack('<Q', w & ((1 << 64)-1)) for w in words)).digest()

def lanes(raw):
    # AssetId holds SHA state words (big-endian digest chunks); FST1 then
    # serializes each numeric word as LE64, as ACK1 does for its state words.
    return list(struct.unpack('>8I', raw))

def value(kind=1, integer=0, xyz=(0., 0., 0.), entity=0, generation=0, content=bytes(32)):
    return [kind, integer, *map(bits, xyz), entity, generation, *lanes(content)]

def field(identity, name, kind=1, scope=0, owner=0, saved=True, retry=True,
          resets=1, low=0, high=1, minimum=0., maximum=1., optional=False, initial=None):
    return ([identity, kind, scope, owner, int(saved), int(retry), resets, low,
             high, bits(minimum), bits(maximum), int(optional), len(name), 15]
            + list(name.encode('ascii')) + (value() if initial is None else initial))

def goldens():
    asset = hashlib.sha256(b'gate').digest()
    vals = [value(0, 1), value(integer=-8), value(2, xyz=(.25, 0., 0.)),
            value(3, xyz=(-1., .5, 2.)), value(4, entity=1, generation=1), value(5, content=asset)]
    fields = [field(11, 'world.flag', 0, retry=False, resets=127, low=-9, high=9, minimum=-3., maximum=5., initial=vals[0]),
              field(12, 'area.counter', scope=1, owner=1, saved=False, resets=0, low=-17, high=42, minimum=-2., maximum=3., optional=True, initial=vals[1]),
              field(13, 'actor.height', 2, 2, 9, resets=4, minimum=-1., maximum=2., initial=vals[2]),
              field(14, 'player.pos', 3, 3, 2, resets=64, minimum=-3., maximum=3., initial=vals[3]),
              field(15, 'options.follow', 4, 4, 0, retry=False, resets=2, optional=True, initial=vals[4]),
              field(16, 'world.asset', 5, resets=8, initial=vals[5])]
    pin = digest([1179866161, 7, 3, 64] + sum(fields, []))
    words = [1179866161, 1, 7, 3, *lanes(pin), 3, 17, 19, 23, 29, 5]
    for i in (0, 2, 3, 4, 5):
        words += [11+i, *vals[i]]
    words += lanes(digest(words))
    return {'schema': pin.hex(), 'snapshot': digest(words).hex(), 'words': len(words),
            'zero_4096': digest([0]*4096).hex()}

if __name__ == '__main__':
    print(json.dumps(goldens(), sort_keys=True))

def contract_golden():
    names=('new','enter','leave','day','load','retry','editor','survivor')
    fields=[field(i+1,name,high=2,resets=1<<i if i<7 else 0,initial=value(integer=1)) for i,name in enumerate(names)]
    pin=digest([1179866161,1299102,1,16]+sum(fields,[]))
    vals=[2]*8;day=phase=0;trail=bytes(32)
    for tick in range(1,641):
        event=tick-1 if tick<=7 else -1
        old=day
        if event!=4:
            phase+=1
            if phase==16:day+=1;phase=0
        if tick==33:day=5;phase=0
        if day!=old:vals[3]=1
        if event>=0:vals[event]=1
        words=[1179866161,1,1299102,1,*lanes(pin),1,tick,day,phase,tick+1,8]
        for i in range(8):words += [i+1,*value(integer=vals[i])]
        words += lanes(digest(words))
        trail=digest([*lanes(trail),*lanes(digest(words))])
    assert (day,phase,vals)==(42,15,[1]*7+[2])
    return trail.hex()

# Independently specified script outcomes; no engine transition code is imported.
def verify(prefix, native):
    from pathlib import Path
    base = json.loads(Path(native + '60-current.json').read_text())
    expected = {
        120:(0,0,0,0,120,1,0),128:(0,0,0,0,128,1,7),
        129:(1,0,0,0,129,2,0),160:(1,0,0,0,160,2,7),
        161:(1,0,1,0,161,3,0),192:(1,0,1,0,192,3,7),
        193:(1,1,1,0,193,4,0),224:(1,1,1,0,224,4,0),
        256:(0,0,1,0,256,5,0),288:(1,1,1,0,223,6,0),
        320:(1,1,1,0,255,6,2),352:(1,0,1,1,0,7,0),
        384:(1,0,1,1,32,7,8),416:(1,0,1,1,64,7,3),
        448:(1,0,1,1,96,7,7),449:(1,0,0,1,97,8,0),
        480:(0,0,0,1,128,9,0),512:(1,0,0,1,160,10,0),
        640:(1,0,0,1,288,10,0)}
    keys=('gate','npc_mode','water','day','phase','sequence','status')
    for row in base['trace']:
        m=row['metrics'];assert tuple(m[k] for k in keys)==expected[m['tick']],m
        assert m['nav']==m['gate'] and m['state_tick']==m['tick']
        assert m['credits']==int(m['tick']>=129)
        assert m['variant']==m['gate']*6+m['npc_mode']*2+m['water']
        assert m['claims']==18 and m['loading']<=4 and m['peak_bytes']<=3045
        if m['tick']<=128:assert m['x']<=7.2
        if m['tick']>=161:assert m['x']>8.5
    ignored={'frames'}
    for fps,frames in ((30,300),(60,600),(144,1440),(0,557)):
        for path in (Path(prefix+f'-{fps}.json'),Path(native+str(fps)+('-current' if fps==60 else '')+'.json')):
            d=json.loads(path.read_text());assert len(d['trace'])==19
            if path.name.startswith(Path(prefix).name):assert d['memory_bytes']==16777216
            assert d['contract_hash']==contract_golden(),(path,d['contract_hash'],contract_golden())
            for a,b in zip(d['trace']+[d['final']],base['trace']+[base['final']]):
                assert a['hash']==b['hash'],(path,a)
                assert {k:v for k,v in a['metrics'].items() if k not in ignored}=={k:v for k,v in b['metrics'].items() if k not in ignored},path
            m=d['final']['metrics'];assert m['frames']==frames and m['tick']==640 and m['dropped_ns']==0
            assert (m['waiting'],m['refused'],m['applied'],m['saved_bytes'])==(4,3,9,748)
    cold=json.loads(Path(prefix+'-cold.json').read_text());assert all(cold[k] for k in ('imported','refused','preserved'))
    m=cold['cold']['metrics'];assert (m['gate'],m['npc_mode'],m['water'],m['nav'],m['credits'],m['variant'],m['x'])==(1,1,1,1,1,9,4)
    cross=json.loads(Path(prefix+'-cross-save.json').read_text());assert cross['imported'] and cross['sameSave'] and cross['final']['hash']==cold['cold']['hash']
    visual=json.loads(Path(prefix+'-visual.json').read_text());assert visual['hash']==base['final']['hash']
    for role in range(6):
        d=json.loads(Path(prefix+f'-fault{role}.json').read_text());a=d['before']['metrics'];b=d['after']['metrics']
        assert (a['gate'],a['nav'],a['water'],a['credits'],a['variant'],a['status'],a['failed_roles'])==(0,0,0,0,0,7,1<<role)
        assert (b['gate'],b['nav'],b['water'],b['credits'],b['variant'],b['status'],b['failed_roles'])==(1,1,0,1,6,0,1<<role)
    saves=[Path(native+str(f)+'.aesv').read_bytes() for f in (30,60,144,0)]
    import zlib
    raw=saves[0];assert raw[:8]==b'AESV\x02\x02\0\0' and struct.unpack_from('<I',raw,8)[0]==1
    assert zlib.crc32(raw[:-4])==struct.unpack_from('<I',raw,len(raw)-4)[0]
    at=12
    def varint():
        nonlocal at
        n=0;shift=0
        while True:
            b=raw[at];at+=1;n|=(b&127)<<shift
            if b<128:return n
            shift+=7
    wn=varint();assert raw[at:at+wn]==b'state-lab';at+=wn
    n=varint();body=raw[at:at+n];assert at+n==len(raw)-4
    words=list(struct.unpack('<'+'Q'*(len(body)//8),body))
    assert words[:4]==[1179866161,1,12991,1] and words[12:18]==[1,223,0,223,4,4]
    assert words[-8:]==lanes(digest(words[:-8]))
    assert [words[18+i*16] for i in range(4)]==[10,20,30,40]
    assert [words[20+i*16] for i in range(3)]==[1,1,1]
    content=''.join(f'{w:08x}' for w in words[18+3*16+8:18+4*16])
    schema=''.join(f'{w:08x}' for w in words[4:12])
    assert all(b==saves[0] for b in saves)
    print(json.dumps({'result':'pass','replays':8,'reset_contract':contract_golden(),'content':content,'schema':schema,'cuts_each':19,'fault_roles':6,'final_hash':base['final']['hash'],'save_sha256':hashlib.sha256(saves[0]).hexdigest()},sort_keys=True))

if __name__ == '__main__':
    import sys
    if len(sys.argv)==3:verify(sys.argv[1],sys.argv[2])
