#!/usr/bin/env python3
"""ASan/UBSan exercise of original-byte public JWS signature check."""
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/nostr/block/fi-jws-signature.coax'
DATA=ROOT/'coax/tests/nostr/data/fi-jws'
COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
token=(DATA/'token.txt').read_bytes().strip();modulus=bytes.fromhex((DATA/'modulus.hex').read_text())
nums=lambda b:','.join(map(str,b))
driver='''#include "generated.c"
#include <assert.h>
#include <stdint.h>
int main(void) {
 uint8_t token[]={TOKEN};uint8_t modulus[]={MODULUS};
 coax_v_u8 t={token,sizeof(token)},n={modulus,sizeof(modulus)};
 assert(fi_jws_signature(t,n).verified);
 token[sizeof(token)-5]^=1;assert(!fi_jws_signature(t,n).verified);token[sizeof(token)-5]^=1;
 token[0]='A';assert(!fi_jws_signature(t,n).verified);token[0]='e';
 for(int i=0;i<8;i++){t.count=i;assert(!fi_jws_signature(t,n).verified);}
 t.count=sizeof(token);n.count=255;assert(!fi_jws_signature(t,n).verified);
 n.count=sizeof(modulus);modulus[0]=0;assert(!fi_jws_signature(t,n).verified);
 return 0;
}
'''.replace('TOKEN',nums(token)).replace('MODULUS',nums(modulus))
with tempfile.TemporaryDirectory(prefix='fi-jws-sanitized-') as d:
 d=Path(d)
 for mode in ('checked','release'):
  source=d/'generated.c';test=d/'driver.c';exe=d/mode
  cmd=[str(COMPILER),'c',str(SOURCE),'--library','-o',str(source)]
  if mode=='release':cmd.append('--release')
  subprocess.run(cmd,check=True);test.write_text(driver)
  subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',str(test),'-lm','-o',str(exe)],check=True)
  subprocess.run([str(exe)],check=True,timeout=60)
  print(f'{mode}: ASan/UBSan passed; executable sha256={hashlib.sha256(exe.read_bytes()).hexdigest()}')
