#!/usr/bin/env python3
"""Focused frame-parser native modes; Python is tooling, never a bootstrap step."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess
ROOT=Path(__file__).resolve().parents[3]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def run(args,**kwargs):
    result=subprocess.run([str(a) for a in args],capture_output=True,text=True,timeout=180,**kwargs)
    if result.returncode:raise RuntimeError(result.stderr or result.stdout)
    return result.stdout

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--compiler',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
    output=args.output.resolve();compiler=args.compiler.resolve()
    if output.exists():parser.error('output exists; preserve prior evidence and choose a fresh directory')
    output.mkdir(parents=True);source=output/'parser.c';header=output/'parser.h'
    source.write_text(run([compiler,'c','lib/compiler/parser.coax','--library','--header',header],cwd=ROOT/'coax'))
    fixture=ROOT/'coax/tests/spatial/frame_parser_native.c';cc=os.environ.get('CC','cc');modes={}
    for name,flags in [('debug',['-O0','-g']),('release',['-O2','-DNDEBUG']),('asan-ubsan',['-O1','-g','-fno-omit-frame-pointer','-fsanitize=address,undefined'])]:
        binary=output/('frame-parser-'+name)
        run([cc,'-std=c11',*flags,'-I',output,fixture,source,'-lm','-o',binary])
        result=json.loads(run([binary],env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1:print_stacktrace=1'}))
        if not result.get('passed'):raise RuntimeError('native mode did not pass: '+name)
        modes[name]={**result,'binary_sha256':sha(binary)}
    record={'schema':'coax-frame-parser-native-qualification-v1','passed':True,'compiler_sha256':sha(compiler),'source_sha256':sha(ROOT/'coax/lib/compiler/parser.coax'),'generated_c_sha256':sha(source),'generated_h_sha256':sha(header),'fixture_sha256':sha(fixture),'runner_sha256':sha(Path(__file__)),'modes':modes,'scope':'Focused direct-token frame metadata parsing and raw ABI transport. Full lexer/source cases are qualified separately. No checker semantics or full bootstrap claim.'}
    (output/'record.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record,indent=2))
if __name__=='__main__':main()
