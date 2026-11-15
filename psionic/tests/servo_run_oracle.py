# Firm reason: 2 (independent IEEE/struct/hashlib oracle for the published SLR1 profile).
"""No product code/imports: independent finite scalar equations and wire packing."""
import hashlib, json, struct, sys
from pathlib import Path

def tagged(tag, data):
    t=hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(t+t+data).digest()

def fixture():
    coefficients=[.01,2.,.08,.02,.001,.15,1.,.002,.002]
    initial_numbers=coefficients+[.1,-1.,1.,0.]
    ids=bytes([1])+bytes(31)+bytes([2])+bytes(31)+bytes([3])+bytes(31)
    target=.1;revision=1;queue=[0.]*16;cursor=0;q=rate=output=0.
    def packet(tick,row):
        count=min(256,1+tick//10)
        config=coefficients+[target,-1.,1.,0.]
        return b'SLV1'+struct.pack('<III',312,7,2)+ids+struct.pack('<13d',*config)+struct.pack('<4I',0,revision,tick,count)+struct.pack('<10d',*row)
    def snapshot(tick):
        return struct.pack('<19d2I',q,rate,output,*queue,cursor,tick)
    first=packet(0,[target,0.,q,rate,output,0.,0.,0.,0.,.15]);first_state=snapshot(0)
    trace=tagged('psionic.robotics.servo-trace.v1',first)
    for tick in range(12):
        if tick==4:target=-.1;revision+=1
        applied=queue[cursor];queue[cursor]=target;cursor=(cursor+1)%2
        proportional=2.*(applied-q);derivative=0.-.08*rate;requested=proportional+derivative
        delivered=-.15 if requested<-.15 else .15 if requested>.15 else requested
        direction=1. if rate>0 else -1. if rate<0 else 0.
        passive=(0.-.02*rate)-.001*direction;net=delivered+passive
        acceleration=net/.01;rate=rate+.002*acceleration;q=q+.002*rate
        low=q-.002;high=q+.002
        if output<low:output=low
        elif output>high:output=high
        last=packet(tick+1,[target,applied,q,rate,output,requested,delivered,passive,net,.15])
        trace=hashlib.sha256(trace+last).digest()
    prefix=b'SLR1'+struct.pack('<III',1100,1,0)+hashlib.sha256(b'servo-native-wasm-f64-v1').digest()+bytes(32)+trace+first+first_state+last+snapshot(12)+struct.pack('<Id',4,-.1)
    assert len(prefix)==1068
    identity=tagged('psionic.robotics.servo-run.v1',prefix);data=prefix+identity
    return {'hex':data.hex(),'identity':identity.hex(),'trace':trace.hex(),'initial_config_numbers':initial_numbers,'final_snapshot_words':[struct.unpack_from('<Q',data,896+i*8)[0] for i in range(19)]}

if __name__=='__main__':
    path=Path('proto/psionic/servo-run-v1.json')
    record={'id':'psionic.robotics.servo-run.v1','spec':'docs/protocol/servo-run-v1.md','profile':'servo-native-wasm-f64-v1','limits':{'commands':32,'steps':4096,'bytes':1472},'fixture':fixture()}
    if '--write' in sys.argv:path.write_text(json.dumps(record,indent=2)+'\n')
    else:
        actual=json.loads(path.read_text());assert actual==record
        print('SLR1: 1100 independent bytes, complete queue and trace identity match')
