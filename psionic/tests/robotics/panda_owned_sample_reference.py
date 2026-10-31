#!/usr/bin/env python3
"""Fresh owned PcSample proof against authenticated, already captured SDK fields.

No SDK, simulator, render, whole-policy inference or old numerical gate runs here.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import struct
import subprocess
import sys
import time

ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'psionic/fixtures/robotics/panda-owned-sample'
REFERENCE=ROOT/'psionic/fixtures/robotics/panda-inverse-dynamics'
SOURCE=ROOT/'psionic/lib/robotics/panda-owned-sample.coax'
NATIVE=ROOT/'psionic/tests/robotics/panda_owned_sample_native.c'
EXPERIMENT_SHA='90cbf6ea412a656498a4a70826c1241892b2f932e0f67ec468269a5f471374ea'
CASE_EXPERIMENT_SHA='6b255a86a733e7a1d2f797691cec0b3fa3954b3b0d61e5a70de51a4691a81019'
CASE_SHA='684c892c72d4e6341e695c1b7bae22a5d5e7d01e3310f1349b98c5c0c8271b49'
FIELDS=[('position',0,3),('rotation',3,9),('q',12,7),('qd',19,7),('mass',26,49),('jp',75,21),('jr',96,21),('velocity',117,6),('bias',123,7),('lower',130,7),('upper',137,7)]
BUDGETS=dict(position=2e-12,rotation=2e-12,q=0,qd=0,mass=2e-11,jp=2e-12,jr=2e-12,velocity=2e-11,bias=5e-10,lower=0,upper=0)

def require(ok,message):
    if not ok: raise AssertionError(message)
def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def finite_words(blob,count,label):
    require(len(blob)==8*count,label+' exact byte extent')
    words=list(struct.unpack('<'+str(count)+'d',blob))
    require(all(math.isfinite(x) for x in words),label+' every F64 word finite before comparison')
    return words

def source_inputs(path,seen=None):
    """Hash the actual recursively loaded Coax source graph, not unrelated work."""
    if seen is None: seen={}
    path=path.resolve()
    if path in seen: return seen
    seen[path]=sha(path)
    for name in re.findall(r'^\s*#load\s+"([^"]+)"',path.read_text(),re.M):
        dependency=(path.parent/name).resolve()
        require(dependency.is_file(),'literal load resolves: '+name)
        source_inputs(dependency,seen)
    return seen

def finite_guard_checks():
    good=struct.pack('<2160d',*([0.0]*2160))
    finite_words(good,2160,'guard control')
    bad=[good[:-8],good+b'\0'*8]
    for value in [float('nan'),float('inf'),-float('inf')]: bad.append(good[:-8]+struct.pack('<d',value))
    for index,blob in enumerate(bad):
        try: finite_words(blob,2160,'guard poison '+str(index))
        except AssertionError: pass
        else: raise AssertionError('finite/extent guard admitted poison '+str(index))
    return {'passed':True,'refused_cases':len(bad),'poison_word':2159,'expected_and_actual_share_same_guard':True}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--compiler',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args(); out=a.output.resolve(); compiler=a.compiler.resolve()
    require(not out.exists(),'preserve earlier evidence; choose a new output directory')
    out.mkdir(parents=True)
    record={'schema':'psionic.panda-owned-sample.qualification.v1','passed':False,'commands':[],'modes':[],
      'SDK_calls':0,'simulation_steps':0,'reference_cases':15,'words_per_case':144,
      'independence_scope':'Fifteen paired contexts; some q/v inputs repeat across acceleration cases. Not fifteen independent states.',
      'scope':'Fresh fixed-source sample fields, storage, ABI and admission only; no controller-output or rollout proof.',
      'numeric_profile':'Darwin arm64 F64 scalar libm, -fno-builtin-sin -fno-builtin-cos -fno-fast-math -ffp-contract=off'}
    def save(): (out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,argv):
        began=time.monotonic(); result=subprocess.run([str(x) for x in argv],cwd=ROOT,capture_output=True,timeout=180)
        (out/(name+'.stdout')).write_bytes(result.stdout);(out/(name+'.stderr')).write_bytes(result.stderr)
        record['commands'].append({'name':name,'argv':[str(x) for x in argv],'seconds':time.monotonic()-began,
          'exit':result.returncode,'stderr_bytes':len(result.stderr),'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()})
        save(); require(result.returncode==0 and not result.stderr,(name,result.stdout.decode(errors='replace'),result.stderr.decode(errors='replace')))
        return result.stdout
    began=time.monotonic(); save()
    try:
        require(sys.platform=='darwin' and platform.machine()=='arm64' and sys.byteorder=='little','qualified Darwin-arm64 target')
        require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'exact pre-observation field budgets')
        experiment=json.loads((FIXTURE/'experiment.json').read_text())
        require(experiment['frozen_before_owned_comparison'] and experiment['field_atol']==BUDGETS and experiment['rtol']==0 and experiment['reference_shape']==[15,144],'frozen comparison contract')
        manifest_path=REFERENCE/'sdk-pc-sample-reference.json'; expected_path=REFERENCE/'sdk-pc-samples.f64'
        require(sha(manifest_path)==experiment['reference_manifest_sha256'],'exact new SDK reference manifest')
        manifest=json.loads(manifest_path.read_text())
        require(manifest['raw_SDK_observations_only'] and manifest['owned_adapter_qualified'] is False and manifest['case_indices']==list(range(15)) and manifest['shape']==[15,144] and manifest['bytes']==17280,'raw reference scope and identity')
        require(sha(expected_path)==manifest['sha256']==experiment['reference_sha256'],'exact raw SDK field words')
        require(sha(REFERENCE/'sdk-reference.json')==manifest['capture_reference_sha256'],'SDK capture receipt provenance')
        require(sha(REFERENCE/'experiment.json')==CASE_EXPERIMENT_SHA and sha(REFERENCE/'cases.f64')==CASE_SHA,'original frozen case scope')
        cases=finite_words((REFERENCE/'cases.f64').read_bytes(),24*27,'original q/v/a cases')
        expected=finite_words(expected_path.read_bytes(),15*144,'every original SDK sample word')
        record['finite_guard_self_check']=finite_guard_checks()
        payload=b''.join(struct.pack('<18d',*cases[i*27:i*27+18]) for i in range(15))
        (out/'input.bin').write_bytes(struct.pack('<II',0x31534350,15)+payload)
        deps=source_inputs(SOURCE)
        deps.update({NATIVE:sha(NATIVE),Path(__file__).resolve():sha(Path(__file__)),FIXTURE/'experiment.json':EXPERIMENT_SHA,
          manifest_path:sha(manifest_path),expected_path:sha(expected_path),REFERENCE/'sdk-reference.json':sha(REFERENCE/'sdk-reference.json'),REFERENCE/'cases.f64':CASE_SHA,REFERENCE/'experiment.json':CASE_EXPERIMENT_SHA})
        record['input_sha256']=sha(out/'input.bin');record['source_sha256']={str(k.relative_to(ROOT)):v for k,v in sorted(deps.items())}
        record['compiler_sha256']=sha(compiler)
        (out/'executed-driver.py').write_bytes(Path(__file__).read_bytes());(out/'executed-native.c').write_bytes(NATIVE.read_bytes())
        emitted={}
        for profile,extra in [('debug',[]),('release',['--release'])]:
            d=out/profile;d.mkdir(); source=d/'panda-owned-sample.c'; header=d/'panda-owned-sample.h'
            source.write_bytes(command('emit-'+profile,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',header]))
            emitted[profile]=(source,header)
        blobs=[];cc=os.environ.get('CC','cc')
        for mode,profile,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir(exist_ok=True);source,header=emitted[profile];binary=d/'native'
            command('compile-'+mode,[cc,'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-I'+str(header.parent),source,NATIVE,'-lm','-o',binary])
            metadata=json.loads(command('run-'+mode,[binary,out/'input.bin',d/'samples.f64']))
            require(metadata['passed'] and metadata['cases']==15 and metadata['words_per_case']==144 and metadata['inclusive_boundaries']==36 and metadata['alias_cases']==4,'complete native case inventory')
            blob=(d/'samples.f64').read_bytes();actual=finite_words(blob,15*144,'all decoded '+mode+' output words');blobs.append(blob)
            fields={}
            for name,offset,width in FIELDS:
                errors=[abs(actual[row*144+offset+j]-expected[row*144+offset+j]) for row in range(15) for j in range(width)]
                error=max(errors); budget=BUDGETS[name]
                require(error<=budget,f'{mode} field {name}: {error:.17g} exceeds frozen {budget:g}')
                fields[name]={'values':15*width,'atol':budget,'rtol':0,'max_abs_error':error,'max_budget_fraction':error/budget if budget else 0,'passed':True}
            record['modes'].append({'mode':mode,'native':metadata,'C_sha256':sha(source),'header_sha256':sha(header),'binary_sha256':sha(binary),'output_sha256':sha(d/'samples.f64'),'finite_actual_words':len(actual),'finite_expected_words':len(expected),'fields':fields});save()
        require(blobs[0]==blobs[1]==blobs[2],'all three sample streams byte identical under the declared numerical profile')
        require(all(sha(k)==v for k,v in deps.items()) and sha(compiler)==record['compiler_sha256'],'compiler or source/artifact changed during qualification')
        record['native_modes_byte_identical']=True;record['passed']=True
    except BaseException as error: record['failure']=repr(error);raise
    finally: record['seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'record':str(out/'record.json'),'seconds':record['seconds']}))

if __name__=='__main__':main()
