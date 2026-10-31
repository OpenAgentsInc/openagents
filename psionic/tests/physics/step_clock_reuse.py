#!/usr/bin/env python3
"""Bind an existing independent proof to identical new outputs, not rerun it."""
import argparse,hashlib,json,pathlib,tempfile
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def verify(current,qualified):
    r=json.loads((qualified/'reference.json').read_text())
    assert r['passed'] and r['schema']=='psionic-step-clock-reference-v1'
    assert r['event_records']==72000 and r['exact_integer_words']==1368000
    base=pathlib.Path(__file__).resolve().parents[2]
    assert r['oracle_sha256']==sha(pathlib.Path(__file__).with_name('step_clock_reference.py'))
    assert r['frozen_profile_sha256']==sha(base/'fixtures/physics/step-clock-profile.json')
    assert sha(current/'oscillator-clock.h')==sha(qualified/'oscillator-clock.h')
    for suffix,key in [('events.i64','events_sha256'),('summary.json','summary_sha256')]:
        assert sha(qualified/('debug.'+suffix))==r[key]
        for mode in ['debug','release','sanitized']:assert sha(current/(mode+'.'+suffix))==r[key]
    return r
def check_refusals(current,qualified):
    r=json.loads((qualified/'reference.json').read_text())
    bad_fields=[('passed',False),('schema','wrong'),('events_sha256','0'*64),('summary_sha256','0'*64),('oracle_sha256','0'*64),('frozen_profile_sha256','0'*64),('event_records',71999),('exact_integer_words',1367999)]
    for key,bad in bad_fields:
        with tempfile.TemporaryDirectory(prefix='step-clock-proof-negative-') as folder:
            d=pathlib.Path(folder)
            for n in ['oscillator-clock.h','debug.events.i64','debug.summary.json']:(d/n).symlink_to((qualified/n).resolve())
            altered=dict(r);altered[key]=bad;(d/'reference.json').write_text(json.dumps(altered))
            try:verify(current,d)
            except AssertionError:pass
            else:raise AssertionError('accepted changed prior proof: '+key)
    return len(bad_fields)
def main():
    p=argparse.ArgumentParser();p.add_argument('current',type=pathlib.Path);p.add_argument('qualified',type=pathlib.Path);a=p.parse_args()
    r=verify(a.current,a.qualified)
    count=check_refusals(a.current,a.qualified)
    (a.current/'reference.json').write_text(json.dumps(r,indent=2,sort_keys=True)+'\n')
    print(f'Exact prior event proof, oracle/profile identities and unchanged nominal header bound; {count} changed-proof refusals; no numerical oracle rerun.')
if __name__=='__main__':main()
