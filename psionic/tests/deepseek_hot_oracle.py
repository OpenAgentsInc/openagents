#!/usr/bin/env python3
"""Owned exact-residency trace oracle; no model download or upstream runtime."""
import argparse,gzip,json,hashlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];D=ROOT/'psionic/fixtures/deepseek'
def plan(heat,slots,experts,frozen=False):
    result=slots[:];moves=[]
    # Independent ordered pairing: original outsiders descend, old slots ascend.
    for layer in range(len(heat)//experts):
        base=layer*3;old=slots[base:base+3]
        candidates=sorted((i for i in range(experts) if i not in old),key=lambda i:(-heat[layer*experts+i],i))
        vacancies=sorted(range(3),key=lambda j:(-1 if old[j]<0 else heat[layer*experts+old[j]],j))
        for slot,incoming in zip(vacancies,candidates):
            victim=old[slot]
            if victim>=0 and (frozen or 4*heat[layer*experts+incoming]<=5*heat[layer*experts+victim]+4096):break
            result[base+slot]=incoming;moves.append([base+slot,victim,layer*experts+incoming])
    return result,moves
def policy():
    j=json.loads((D/"hot-policy.json").read_text());p=j["placement"];assert [p[x] for x in ["heat_units_per_use","decay_interval","decay_numerator","decay_denominator"]]==[1024,256,9,10]
    assert p["swap"]=="4 * incoming > 5 * cold + 4096"
    r=j["reference"];assert hashlib.sha256(gzip.decompress((D/r["excerpt_archive"]).read_bytes())).hexdigest()==r["excerpt_sha256"]

def trace():
    observed=[20,10,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,8,16,3,0,0,0,0];calibration=[key for key,uses in enumerate(observed) for _ in range(uses)];profile=[calibration.count(key) for key in range(24)];heat=[x*1024 for x in profile];slots,moves=plan(heat,[-1]*6,12)
    states=[slots[:]];all_heat=[heat[:]];routes=[];hits=[];plans=[moves]
    for step in range(300):
        phase=0 if step<80 else 4 if step<190 else 8
        ids=[]
        for layer in range(2):
            start=(phase+layer*3+(step%7==0)*2)%12;ids.extend(layer*12+(start+j)%12 for j in range(3))
        routes.append(ids);hits.append([int(k%12 in slots[(k//12)*3:(k//12+1)*3]) for k in ids])
        for key in ids:heat[key]+=1024
        if (step+1)%256==0:heat=[h*9//10 for h in heat]
        slots,moves=plan(heat,slots,12);states.append(slots[:]);all_heat.append(heat[:]);plans.append(moves)
    return dict(calibration_routes=calibration,profile=profile,routes=routes,hits=hits,slots=states,heat=all_heat,moves=plans)
def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');p.add_argument('--header',action='store_true');a=p.parse_args();policy();j=trace()
    assert len(j['routes'])==300 and any(sum(h)<6 for h in j['hits']) and any(sum(h)>0 for h in j['hits'])
    if a.header:
        print('#include <stdint.h>')
        for key in ['profile','routes','hits','slots','heat']:
            values=j[key];flat=values if key=='profile' else sum(values,[])
            print('static const int64_t dch_'+key+'[]={'+','.join(map(str,flat))+'};')
        counts=[len(m) for m in j['moves']];print('static const int64_t dch_counts[]={'+','.join(map(str,counts))+'};')
        flat=[]
        for moves in j['moves']:
            flat.extend(sum(moves,[])+[-999]*(18-3*len(moves)))
        print('static const int64_t dch_moves[]={'+','.join(map(str,flat))+'};');return
    data=(json.dumps(j,separators=(',',':'))+'\n').encode();path=D/'hot-trace.json.gz'
    if a.write:path.write_bytes(gzip.compress(data,mtime=0))
    else:assert gzip.decompress(path.read_bytes())==data
    print('Hot experts: independent 300-step profile/phase-shift trace, all heat, slot maps, hit lanes and swap plans agree')
if __name__=='__main__':main()
