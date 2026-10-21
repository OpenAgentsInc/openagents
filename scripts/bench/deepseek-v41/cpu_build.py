#!/usr/bin/env python3
"""Build the checked-in CPU qualification example without unrelated model stacks.

The temporary Cargo wrapper includes the exact library source files by path;
there is no second inference implementation. Dependency versions come from the
Psionic workspace lock. Keep the generated Cargo lock with qualification evidence.
"""
import argparse
import json
import re
from pathlib import Path


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--source',type=Path,required=True)
    parser.add_argument('--build-dir',type=Path,required=True)
    args=parser.parse_args()
    root=args.source.resolve();build=args.build_dir.resolve();build.mkdir(parents=True,exist_ok=True)
    packages=[]
    for part in (root/'crates/psionic/Cargo.lock').read_text().split('[[package]]')[1:]:
        name=re.search(r'^name = "([^"]+)"',part,re.M)
        version=re.search(r'^version = "([^"]+)"',part,re.M)
        if name and version:packages.append((name[1],version[1]))
    families={'serde':'1.','serde_json':'1.','sha2':'0.10.','thiserror':'2.','half':'2.','hex':'0.4.','tokenizers':'0.23.','rayon':'1.'}
    manifest='[package]\nname="v41-cpu-qualification"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[lib]\nname="psionic_models"\npath="lib.rs"\n[[bin]]\nname="deepseek_v41_cpu_qualify"\npath='+json.dumps(str(root/'crates/psionic/crates/psionic-models/examples/deepseek_v41_cpu_qualify.rs'))+'\n[dependencies]\n'
    for name,prefix in families.items():
        version=next(v for n,v in packages if n==name and v.startswith(prefix))
        extras=',features=["derive"]' if name=='serde' else ',default-features=false,features=["onig"]' if name=='tokenizers' else ''
        manifest+=name+'={version="='+version+'"'+extras+'}\n'
    manifest+='[profile.release]\ncodegen-units=16\nlto="thin"\n'
    (build/'Cargo.toml').write_text(manifest)
    modules=''
    for name,path in [('deepseek_v41_weights','deepseek_v41_weights.rs'),('deepseek_v41_cpu','deepseek_v41_cpu/mod.rs')]:
        modules+='#[path='+json.dumps(str(root/'crates/psionic/crates/psionic-models/src'/path))+']\npub mod '+name+';\n'
    (build/'lib.rs').write_text(modules)
    print(build/'Cargo.toml')


if __name__=='__main__':main()
