#!/usr/bin/env python3
"""Capture only text 029 with the archived Stage 6 Metal test binary."""
import argparse,json,os,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;RUN=HERE.parent;BASE=RUN.parent
sys.path.insert(0,str(RUN));import mps_reference as ref
BINARY=BASE/'stage6/psionic-serve-tests'
BINARY_SHA='746950810194cbb9228ab6c49776beb5bbb45415b831e5c94efb821ad2dde566'

def main():
 p=argparse.ArgumentParser();p.add_argument('--output-dir',type=Path,required=True);a=p.parse_args()
 resources=set(os.environ.get('OPENAGENTS_LEASES','').split(','));ref.require({'gpu','memory','disk'}<=resources and not {'build','quiet'}&resources and os.environ.get('CLEF_TEXT029_NATIVE_GUARDED')=='1','owned native capture guard required')
 ref.require(not a.output_dir.exists(),'preserve previous output');a.output_dir.mkdir(parents=True)
 encoded=json.loads((RUN/'encoder-source-run-1/native-encoded.json').read_text())['cases'][29]
 tokens=[int(x) for x in (HERE/'tokens-029.txt').read_text().split()]
 ref.require(encoded['id']=='text-029' and len(tokens)==359 and tokens==encoded['input_ids'],'exact text029 tokens differ')
 ref.require(ref.sha(BINARY)==BINARY_SHA,'archived Stage 6 binary differs')
 metadata=json.loads((BASE/'stage6/test-binaries.json').read_text());ref.require(any(x['path']==BINARY.name and x['sha256']==BINARY_SHA for x in metadata),'archived binary manifest differs')
 command=[str(BINARY),'clef::tests::dump_layer_rows','--exact','--nocapture','--test-threads=1']
 env=dict(os.environ);env.update(PSIONIC_CLEF_LAYER_DIR=str(a.output_dir/'rows'))
 result={'schema':'clef.text029-native-layer-capture.v1','status':'running','tokens':tokens,'input_tokens':359,'hidden_size':5120,'layers':64,'command':command,'binary_sha256':BINARY_SHA,'stage6_source_manifest_sha256':ref.sha(BASE/'stage6/source-manifest.json'),'request_sha256':encoded['request_sha256'],'mode':'Metal fused-tensorops key32, weight-shapes GEMM, sequential DeltaNet, q4k-on-demand FFN','head_source':'Embedded head is loaded but not executed. Observer outputs precede the head hidden LayerNorm.','observation_limit':'Per-layer observer inserts completion waits and readback. Final rows must be compared with the prior normal-request pooled export before treating this capture as identical to normal dispatch.','files':[]}
 path=a.output_dir/'receipt.json';ref.write(path,result)
 try:
  # The native child stays in this wrapper's owned process group.
  with (a.output_dir/'test.log').open('xb') as log:code=subprocess.call(command,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log)
  result['exit_code']=code;ref.require(code==0,'native layer test failed')
  log=(a.output_dir/'test.log').read_text();ref.require('1 passed; 0 failed;' in log and 'clef::tests::dump_layer_rows ...' in log and 'skipped:' not in log,'exact native test did not complete')
  import numpy as np
  names=[f'l_out-{i}.f32' for i in range(64)]+['result_norm.f32'];rows=a.output_dir/'rows'
  ref.require(set(p.name for p in rows.iterdir())==set(names),'native output files differ')
  for name in names:
   f=rows/name;ref.require(f.stat().st_size==359*5120*4,'native row byte count differs')
   values=np.fromfile(f,dtype='<f4');ref.require(values.size==359*5120 and np.isfinite(values).all(),'native row shape/finiteness differs')
   result['files'].append({'name':name,'bytes':f.stat().st_size,'sha256':ref.sha(f),'finite':True});del values
  result.update(status='complete',completed=True,all_layer_rows_checked=True,model_weights_unchanged=True)
 except BaseException as error:result.update(status='failed',error_type=type(error).__name__,error=str(error));raise
 finally:ref.write(path,result)

if __name__=='__main__':main()
