#!/usr/bin/env python3
"""C and C++ consumers load the actual published dylib after relocation."""
import hashlib,json,pathlib,shutil,subprocess,sys,tempfile
if len(sys.argv)!=3:raise SystemExit('usage: ecdh_owned_public_link.py CHECKED_DIR RELEASE_DIR')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
with tempfile.TemporaryDirectory(prefix='coax-owned-public-link-') as td:
 root=pathlib.Path(td)
 for mode,path in zip(('checked','release'),sys.argv[1:]):
  src=pathlib.Path(path);manifest=json.loads((src/'manifest.json').read_text());artifact=src/'libcoax_ecdh_owned.dylib';header=src/'ecdh_owned.h'
  assert manifest['target']=='native' and manifest['profile']==mode and sha(artifact)==manifest['artifact_sha256'] and sha(header)==manifest['public_header_sha256']
  relocated=root/mode;relocated.mkdir();shutil.copyfile(artifact,relocated/artifact.name);shutil.copyfile(header,relocated/header.name)
  assert sha(relocated/artifact.name)==sha(artifact)
  for lang,compiler,ext,std in [('C','cc','c','c11'),('C++','c++','cpp','c++17')]:
   source=relocated/('consumer.'+ext);source.write_text('#include "ecdh_owned.h"\nint main(void){uint64_t h=coax_ecdh_owned_acquire();return h && !coax_ecdh_owned_close(h) ? 0 : 1;}\n');exe=relocated/('consumer-'+lang.replace('+','p'))
   subprocess.run([compiler,'-std='+std,'-I'+str(relocated),str(source),str(relocated/artifact.name),'-Wl,-rpath,@loader_path','-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
   print(mode,lang,'relocated public artifact passed',flush=True)
