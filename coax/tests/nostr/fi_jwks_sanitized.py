#!/usr/bin/env python3
"""ASan/UBSan exercise of strict unintegrated JWKS parsing."""
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/nostr/block/fi-jwks-parse.coax'
COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
n=bytes([128])+bytes(254)+b'\x01';key={'kid':'k','kty':'RSA','alg':'RS256','use':'sig','n':base64.urlsafe_b64encode(n).rstrip(b'=').decode(),'e':'AQAB'}
valid=json.dumps({'keys':[key]},separators=(',',':')).encode()
dup=json.dumps({'keys':[key,key]},separators=(',',':')).encode()
nums=lambda b:','.join(map(str,b))
driver='''#include "generated.c"
#include <assert.h>
#include <stdint.h>
int main(void) {
 char valid[]={VALID};char dup[]={DUP};
 coax_string a={valid,sizeof(valid)},b={dup,sizeof(dup)};
 assert(fi_jwks_parse(a).ok);assert(!fi_jwks_parse(b).ok);
 for(int i=0;i<(int)sizeof(valid);i++){a.count=i;assert(!fi_jwks_parse(a).ok);}
 a.count=sizeof(valid);valid[0]='[';assert(!fi_jwks_parse(a).ok);
 return 0;
}
'''.replace('VALID',nums(valid)).replace('DUP',nums(dup))
with tempfile.TemporaryDirectory(prefix='fi-jwks-sanitized-') as d:
 d=Path(d)
 for mode in ('checked','release'):
  source=d/'generated.c';test=d/'driver.c';exe=d/mode
  cmd=[str(COMPILER),'c',str(SOURCE),'--library','-o',str(source)]
  if mode=='release':cmd.append('--release')
  subprocess.run(cmd,check=True);test.write_text(driver)
  subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',str(test),'-lm','-o',str(exe)],check=True)
  subprocess.run([str(exe)],check=True,timeout=60)
  print(f'{mode}: ASan/UBSan passed; executable sha256={hashlib.sha256(exe.read_bytes()).hexdigest()}')
