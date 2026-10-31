#!/usr/bin/env python3
"""Metadata-only aliases for the frozen paired-controller input contract.
No compiler, sample generator, numerical reference or native harness executes.
"""
from pathlib import Path
import argparse,hashlib,json
ROOT=Path(__file__).resolve().parents[3]
PROFILE_SHA='1e1b6c6b9a872d3bc096ae3260634305d1056c9318939b27b7258741f28594ea'
STREAMS=('debug/panda-owned-sample.c','debug/panda-owned-sample.h','release/panda-owned-sample.c','release/panda-owned-sample.h','debug/samples.f64')
def sha(data):return hashlib.sha256(data).hexdigest()
def require(ok,message):
    if not ok:raise ValueError(message)
def validate(profile_bytes,historical_bytes,owned_bytes,generated_record_bytes,streams):
    require(sha(profile_bytes)==PROFILE_SHA,'frozen paired profile identity');profile=json.loads(profile_bytes)
    require(profile['frozen_before_execution'] is True and profile['case_samples']==15,'paired profile scope')
    require(sha(historical_bytes)==profile['observed_inputs']['owned_previous_record_sha256'],'historical execution receipt identity');historical=json.loads(historical_bytes)
    require(historical['passed'] is True and len(historical['modes'])==3,'historical execution status')
    require(len(owned_bytes)==15*144*8 and sha(owned_bytes)==profile['observed_inputs']['owned_sha256'],'historical owned output identity')
    require(set(streams)==set(STREAMS),'exact alias input inventory');require(streams['debug/samples.f64']==owned_bytes,'newly generated owned output differs from historical paired bytes')
    generated=json.loads(generated_record_bytes);require(generated['passed'] is True and generated['schema']=='psionic.panda-owned-sample.qualification.v1' and generated['native_modes_byte_identical'] is True,'newly generated execution status')
    require(generated['reference_cases']==15 and generated['words_per_case']==144,'newly generated execution extent')
    modes=generated['modes'];require(len(modes)==3 and {row['mode'] for row in modes}=={'debug','release','sanitized'},'newly generated native mode inventory')
    for row in modes:
        mode='release' if row['mode']=='release' else 'debug';expected=profile['cached_C_header'][mode+'_C_sha256'];header=profile['cached_C_header']['header_sha256']
        require(sha(streams[mode+'/panda-owned-sample.c'])==expected and sha(streams[mode+'/panda-owned-sample.h'])==header,'frozen generated C/header identity; qualify a new profile if changed')
        require(row['C_sha256']==expected and row['header_sha256']==header and row['output_sha256']==sha(owned_bytes),'newly generated receipt artifact binding')
        require(row['native']['passed'] is True and row['native']['cases']==15 and row['native']['words_per_case']==144,'newly generated native coverage')
    aliases={name:streams[name] for name in STREAMS};aliases['record.json']=historical_bytes
    return aliases,{'schema':'psionic.panda-owned-sample.paired-cache-alias.v1','passed':True,'historical_execution_record_sha256':sha(historical_bytes),'newly_generated_execution_record_sha256':sha(generated_record_bytes),'paired_profile_sha256':sha(profile_bytes),'owned_output_sha256':sha(owned_bytes),'aliases':{name:{'sha256':sha(data),'bytes':len(data)} for name,data in aliases.items()},'scope':'Byte aliases only. record.json remains the exact historical execution receipt required by the paired profile, never relabelled as fresh execution. New generated C/header/output must match its frozen bytes. No compiler/sample/native/SDK/NN execution.'}
def materialize(generated,out):
    require(not out.exists(),'choose a new alias directory');fixture=ROOT/'psionic/fixtures/robotics';profile=(fixture/'panda-owned-controller/experiment.json').read_bytes();historical=(fixture/'panda-owned-sample/native-qualification.json').read_bytes();owned=(fixture/'panda-owned-controller/owned-samples.f64').read_bytes()
    aliases,receipt=validate(profile,historical,owned,(generated/'record.json').read_bytes(),{name:(generated/name).read_bytes() for name in STREAMS})
    out.mkdir(parents=True,exist_ok=False)
    for name,data in aliases.items():p=out/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
    (out/'alias-binding.json').write_text(json.dumps(receipt,indent=2)+'\n');return receipt
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--generated',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();materialize(args.generated.resolve(),args.output.resolve());print(args.output.resolve())
if __name__=='__main__':main()
