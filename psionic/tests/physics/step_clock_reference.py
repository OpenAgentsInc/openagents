#!/usr/bin/env python3
"""Independent closed-form integer/Fraction schedule oracle; no product imports."""
import argparse,hashlib,json,pathlib,struct
from fractions import Fraction

def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def expected(mode,step):
    row,sub=divmod(step,25)
    hold=int(mode!=0 and row==0)
    if hold:
        slot=-1;serial=0 if mode==3 else 1;origin=0;remaining=0;pending=int(mode==1)
        last=qpos=-1 if mode==3 else 0;force=0.0
    else:
        action_row=row-int(mode!=0);chunk,slot=divmod(action_row,8)
        serial=chunk+(1 if mode in (0,3) else 2)
        origin=200*chunk+(25 if mode else 0);remaining=8-slot;pending=0
        last=max(0,origin-1);qpos=origin;force=float(Fraction(slot%3-1,4))
    after_remaining=remaining-int(not hold and sub==24)
    phase=2 if sub<24 else (1 if pending else 0)
    if step==17999:phase=6
    after_hold=hold if sub<24 else 0
    force_bits=struct.unpack('<q',struct.pack('<d',force))[0]
    return (mode,step,step+1,1,serial,origin,remaining,slot,hold,pending,int(sub==0),last,qpos,step,after_remaining,phase,after_hold,pending,force_bits)

def main():
    ap=argparse.ArgumentParser();ap.add_argument('prefix');ap.add_argument('--record',required=True);args=ap.parse_args()
    prefix=pathlib.Path(args.prefix);events=prefix.with_suffix('.events.i64');summary=prefix.with_suffix('.summary.json')
    profile=pathlib.Path(__file__).resolve().parents[2]/'fixtures/physics/step-clock-profile.json'
    frozen=json.loads(profile.read_text());assert frozen['period_seconds']=={'numerator':1,'denominator':500}
    assert frozen['physics_steps']==18000 and frozen['physics_steps_per_action_row']==25 and frozen['consumed_action_rows_per_request']==8
    data=events.read_bytes();assert len(data)==4*18000*19*8
    count=0
    for actual in struct.iter_unpack('<19q',data):
        mode,step=divmod(count,18000);want=expected(mode,step)
        if actual!=want:raise AssertionError({'mode':mode,'step':step,'actual':actual,'expected':want})
        assert Fraction(actual[2],500)-Fraction(actual[1],500)==Fraction(1,500)
        if step%25==24:assert Fraction(actual[2],500)-Fraction(step-24,500)==Fraction(1,20)
        count+=1
    decoded=json.loads(summary.read_text());assert decoded['schema']=='psionic-step-clock-native-v1'
    assert len(decoded['scenarios'])==4
    for mode,item in enumerate(decoded['scenarios']):
        regular=list(range(0 if mode==0 else 25,18000,200))
        origins=([0] if mode in (1,2) else [])+regular
        assert item=={'mode':mode,'steps':18000,'rows':720,'requests':len(origins),'holds':int(mode!=0),'refusals':int(mode in (1,2)),'origins':origins,'steady_ns':len(origins)*1000000000000,'remaining':int(mode!=0),'plant_finite':True}
        assert origins[:3]==frozen['request_origins_prefix'][frozen['scenarios'][mode]]
    result={'schema':'psionic-step-clock-reference-v1','passed':True,'event_records':count,'exact_integer_words':count*19,'fraction_period_checks':count,'fraction_row_checks':4*720,'native_summary':decoded['scenarios'],'events_sha256':sha(events),'summary_sha256':sha(summary),'oracle_sha256':sha(__file__),'frozen_profile_sha256':sha(profile),'scope':'Independent closed-form event fields and exact Fraction periods. New oscillator consumption/finite-publication checks reuse unchanged oscillator numerical proof; no new physics accuracy, SDK cache age or real-time claim.'}
    pathlib.Path(args.record).write_text(json.dumps(result,indent=2,sort_keys=True)+'\n')
    print(f'Independent clock oracle: {count} exact events/{count*19} words; Fraction 500Hz/20Hz; all 4 scenarios')
if __name__=='__main__':main()
