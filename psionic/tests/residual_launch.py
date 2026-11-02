#!/usr/bin/env python3
# Tooling only: authenticate a completed prospective freeze before Coax training.
"""Launch one explicitly requested residual experiment from an immutable freeze.

No build, acquisition, model, gradient or optimizer arithmetic runs in Python.
All required inputs are rehashed before execution and again after it returns.
The caller keeps those files immutable/exclusive during the native child run.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

DATA_FILES=('train-images-idx3-ubyte.gz','train-images-idx3-ubyte',
            'train-labels-idx1-ubyte.gz','train-labels-idx1-ubyte',
            't10k-images-idx3-ubyte.gz','t10k-images-idx3-ubyte',
            't10k-labels-idx1-ubyte.gz','t10k-labels-idx1-ubyte')

def fact(path):
    path=Path(path).resolve(strict=True)
    return {'path':str(path),'bytes':path.stat().st_size,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}

def checked_path(row,root):
    path=Path(row['path']);path=path if path.is_absolute() else root/path
    observed=fact(path)
    if observed['sha256']!=row['sha256'] or observed['bytes']!=row['bytes']:
        raise ValueError('frozen input drift: '+str(path))
    return Path(observed['path'])

def verify(freeze,expected,root,cc):
    if fact(freeze)['sha256']!=expected:raise ValueError('freeze identity mismatch')
    data=json.loads(Path(freeze).read_text())
    if data['preflight'].get('verified')is not True:raise ValueError('first-image preflight not verified')
    compiler=checked_path(data['compiler'],root)
    provenance=checked_path(data['compiler_provenance'],root)
    words=provenance.read_text().splitlines()
    if len(words)!=4 or words[0]!='coax-provenance-v1' or words[2]!=data['compiler']['sha256']:
        raise ValueError('provenance does not name frozen compiler bytes')
    experiment=checked_path(data['experiment'],root)
    inputs=data['inputs']
    if not isinstance(inputs,list) or not inputs:raise ValueError('missing source/tool inventory')
    admitted={checked_path(row,root) for row in inputs}
    if compiler not in admitted or Path(cc).resolve(strict=True) not in admitted:
        raise ValueError('compiler and host C compiler must be frozen inputs')
    native=data['native_release']
    if len(native)!=3:raise ValueError('incomplete generated/native inventory')
    artifacts={p.name:p for p in (checked_path(row,root) for row in native)}
    if set(artifacts)!={'residual.c','residual.h','residual'}:raise ValueError('incomplete generated/native inventory')
    dataset=data['dataset']
    if len(dataset)!=8:raise ValueError('incomplete dataset inventory')
    files=[checked_path(row,root) for row in dataset]
    if {p.name for p in files}!=set(DATA_FILES) or len({p.parent for p in files})!=1:
        raise ValueError('dataset paths must name one complete pinned cache')
    return artifacts['residual'],files[0].parent,experiment

def run_frozen(freeze,expected,root,output,cc,execute=subprocess.run):
    root=Path(root).resolve(strict=True);output=Path(output).absolute()
    if output.exists() or output.is_symlink():raise ValueError('output directory must be new')
    binary,data,experiment=verify(freeze,expected,root,cc)
    argv=[str(binary),'--train',str(data),str(experiment),str(output)]
    result=execute(argv,cwd=root,timeout=600,check=False)
    verify(freeze,expected,root,cc)
    return result.returncode

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--freeze',type=Path,required=True);p.add_argument('--freeze-sha256',required=True)
    p.add_argument('--output',type=Path,required=True);p.add_argument('--cc',default=shutil.which('cc'))
    a=p.parse_args()
    if not a.cc:p.error('host C compiler is required')
    raise SystemExit(run_frozen(a.freeze,a.freeze_sha256,Path(__file__).resolve().parents[2],a.output,Path(a.cc)))

if __name__=='__main__':main()
