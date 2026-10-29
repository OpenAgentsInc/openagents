#!/usr/bin/env python3
"""Retain exact numerical build inputs/outputs for opt-in qualification."""
import argparse,gzip,hashlib,json,platform,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    ap=argparse.ArgumentParser();ap.add_argument('out',type=Path);ap.add_argument('compiler',type=Path);a=ap.parse_args()
    deps=json.loads(subprocess.check_output([str(a.compiler),'deps','psionic/lib/gptoss.coax','--run-steps','2000000'],cwd=ROOT));inputs=[Path(p) for p in deps['sources']]
    inputs += [ROOT/p for p in ['psionic/platform/gptoss-memory.h','psionic/platform/gptoss-memory.c','psionic/platform/spans.h','psionic/tests/gptoss_native.c','psionic/tests/gptoss_lab.c','psionic/fixtures/harmony/gptoss.hst']]
    record=dict(profile='gpt-oss-cpu-wide-v1',compiler_sha256=digest(a.compiler),compiler_version=subprocess.check_output([str(a.compiler),'version'],text=True).strip(),base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),host=platform.platform(),flags=['-std=c11','-O3','-ffp-contract=off','-fno-fast-math'],inputs={str(p.relative_to(ROOT)):digest(p) for p in inputs},outputs={})
    for name in ['gptoss.c','gptoss.h','gptoss-tiny.h','native','lab']:
        p=a.out/name
        if p.exists():record['outputs'][name]=dict(sha256=digest(p),bytes=p.stat().st_size)
    generated=a.out/'gptoss.c'
    if generated.exists():(a.out/'generated.c.gz').write_bytes(gzip.compress(generated.read_bytes(),mtime=0))
    (a.out/'build.json').write_text(json.dumps(record,indent=2)+'\n')
if __name__=='__main__':main()
