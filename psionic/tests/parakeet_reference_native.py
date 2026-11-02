#!/usr/bin/env python3
# Firm reason: 2 (independent actual resident C-ABI/model/artifact observation harness).
from pathlib import Path
import argparse,hashlib,importlib.util,json,os,shutil,sys,time
ROOT=Path(__file__).resolve().parents[2]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('model',type=Path);p.add_argument('library',type=Path);p.add_argument('reference',type=Path);p.add_argument('out',type=Path);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=False);a.library=a.library.resolve();a.reference=a.reference.resolve()
 reference=json.loads((a.reference/'record.json').read_text());assert reference['passed'] and len(reference['clips'])==29
 inputs=a.out/'inputs.tsv';inputs.write_text(''.join(x['id']+'\t'+str(a.reference/x['artifacts']['encoded']['file'])+'\t'+str(x['encoded_frames'])+'\n' for x in reference['clips']))
 artifacts=a.out/'product';artifacts.mkdir();binary=a.out/'reference-native';cc=Path(shutil.which('cc')).resolve();source=ROOT/'psionic/tests/parakeet_reference_native.c';header=ROOT/'psionic/platform/psionic-parakeet.h'
 paths=[Path(__file__).resolve(),source,header,a.library,cc,inputs,a.reference/'record.json',*[a.reference/x['artifacts']['encoded']['file'] for x in reference['clips']]];before={str(p):sha(p) for p in paths}
 spec=importlib.util.spec_from_file_location('pkref_bp',ROOT/'coax/tools/bounded_process.py');bp=importlib.util.module_from_spec(spec);spec.loader.exec_module(bp);runs=[];deadline=time.monotonic()+830
 def run(name,argv):
  r=bp.run(list(map(str,argv)),ROOT,os.environ.copy(),max(1,deadline-time.monotonic()),33554432,deadline,quiet_faults=True)
  for k in ['stdout','stderr']:(a.out/(name+'.'+k)).write_bytes(r.pop(k))
  r['argv']=list(map(str,argv));(a.out/(name+'.json')).write_text(json.dumps(r,indent=2)+'\n');runs.append({'name':name,'record':name+'.json','returncode':r['returncode'],'elapsed_s':r['elapsed_s'],'timeout':r['timeout']});return r
 argv=[cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-I',ROOT/'psionic/platform',source,a.library,'-lm','-lpthread','-o',binary]
 if sys.platform=='darwin':argv+=['-framework','Metal','-framework','Foundation']
 compiled=run('compile',argv);assert compiled['returncode']==0,(a.out/'compile.stderr').read_text();native=run('native',[binary,a.model,inputs,artifacts]);after={str(p):sha(p) for p in paths}
 rows=[]
 for expected in reference['clips']:
  path=artifacts/(expected['id']+'.tokens.json')
  if not path.exists():continue
  got=json.loads(path.read_text());rows.append({'id':expected['id'],'language':expected['language'],'tokens_equal':got['tokens']==expected['tokens'],'frames_equal':got['frames']==expected['frames'],'text_equal':got['text']==expected['text'],'actual_backend':got['backend'],'artifact':str(path.relative_to(a.out)),'sha256':sha(path)})
 flags=['timeout','deadline_exhausted','stdout_limit','stderr_limit','output_limit','group_killed','launch_errno','launch_error','kill_errno','read_errno'];clean=native['returncode']==0 and native['started'] and native['stdout_eof'] and native['stderr_eof'] and not any(native[k] for k in flags)
 stdout=(a.out/'native.stdout').read_text();passed=clean and before==after and len(rows)==29 and all(r['tokens_equal'] and r['frames_equal'] and r['text_equal'] and r['actual_backend']=='cpu-exact' for r in rows) and 'SECOND_OWNER_REFUSED' in stdout and 'REAL_MODEL_FAILURE_SNAPSHOTS 4 passed' in stdout
 record={'schema':'psionic.parakeet-resident-decoder-reference.v1','scope':'One real pinned resident model;29CPU-only decoder calls on actual independently recorded NeMo encoded tensors. No CPU encoder execution. Named public snapshot preservation on4refusals; second owner refused.','passed':passed,'inputs_before':before,'inputs_after':after,'inputs_unchanged':before==after,'native_transport_clean':clean,'binary_sha256':sha(binary),'runs':runs,'clips':rows,'full_encoder_qualification':False};(a.out/'record.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'passed':passed,'clips':len(rows),'seconds':native['elapsed_s'],'record':str(a.out/'record.json')}));raise SystemExit(0 if passed else 1)
if __name__=='__main__':main()
