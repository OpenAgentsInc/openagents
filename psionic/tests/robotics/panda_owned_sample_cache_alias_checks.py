#!/usr/bin/env python3
"""Only NEW metadata alias guards, using already qualified artifacts."""
from pathlib import Path
import argparse,copy,json,tempfile,hashlib
import panda_owned_sample_cache_alias as alias
def main():
    p=argparse.ArgumentParser();p.add_argument('--qualified',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
    f=alias.ROOT/'psionic/fixtures/robotics';profile=(f/'panda-owned-controller/experiment.json').read_bytes();history=(f/'panda-owned-sample/native-qualification.json').read_bytes();owned=(f/'panda-owned-controller/owned-samples.f64').read_bytes();record=(a.qualified/'record.json').read_bytes();streams={name:(a.qualified/name).read_bytes() for name in alias.STREAMS};cases=[]
    def check(label,args,passed):
        try:alias.validate(*args)
        except (ValueError,KeyError):assert not passed,label;cases.append({'name':label,'refused':True})
        else:assert passed,label;cases.append({'name':label,'passed':True})
    baseline=(profile,history,owned,record,streams);check('unchanged qualified artifacts',baseline,True)
    for name in alias.STREAMS:
        damaged=dict(streams);damaged[name]=damaged[name]+b'X';check('poison '+name,(profile,history,owned,record,damaged),False)
    check('poison historical record',(profile,history+b' ',owned,record,streams),False);check('poison owned bundle',(profile,history,owned+b'X',record,streams),False)
    check('poison frozen paired profile',(profile+b' ',history,owned,record,streams),False)
    for kind in ('failed','missingmode','duplicatemode','wrongCbinding','wrongextent'):
        wrong=json.loads(record)
        if kind=='failed':wrong['passed']=False
        if kind=='missingmode':wrong['modes'].pop()
        if kind=='duplicatemode':wrong['modes'][2]['mode']='debug'
        if kind=='wrongCbinding':wrong['modes'][0]['C_sha256']='0'*64
        if kind=='wrongextent':wrong['reference_cases']=14
        check('poison new record '+kind,(profile,history,owned,json.dumps(wrong).encode(),streams),False)
    copied=alias.materialize(a.qualified.resolve(),out/'alias');assert (out/'alias/record.json').read_bytes()==history and (out/'alias/debug/samples.f64').read_bytes()==owned
    try:alias.materialize(a.qualified.resolve(),out/'alias')
    except ValueError:cases.append({'name':'existing output directory','refused':True})
    else:raise AssertionError('existing output accepted')
    receipt={'schema':'psionic.panda-owned-sample.cache-alias-qualification.v1','passed':True,'checks':cases,'alias_record_sha256':hashlib.sha256((out/'alias/alias-binding.json').read_bytes()).hexdigest(),'source_sha256':{str(Path(alias.__file__).resolve().relative_to(alias.ROOT)):hashlib.sha256(Path(alias.__file__).read_bytes()).hexdigest(),str(Path(__file__).resolve().relative_to(alias.ROOT)):hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},'scope':'Only new metadata/copy guards and exact historical byte aliases; no sample generation, compiler, controller, native, SDK or numerical gate rerun.'}
    (out/'qualification.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({'passed':True,'checks':len(cases),'record':str(out/'qualification.json')}))
if __name__=='__main__':main()
