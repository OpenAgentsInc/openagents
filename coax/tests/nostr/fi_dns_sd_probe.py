#!/usr/bin/env python3
"""Bounded macOS-only DNS-SD lifecycle experiment, not a provider."""
import os
from pathlib import Path
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[3]
compiler=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
source=ROOT/'coax/lib/nostr/block/fi-dns-observation.coax'
fixture=ROOT/'coax/tests/nostr/fi_dns_sd_probe.c'
with tempfile.TemporaryDirectory(prefix='fi-dnssd-probe-') as td:
 d=Path(td)
 for mode in ('checked','release'):
  cmd=[str(compiler),'c',str(source),'--library','--header',str(d/'generated.h'),'-o',str(d/'generated.c')]
  if mode=='release':cmd.append('--release')
  subprocess.run(cmd,check=True)
  for sanitize in (False,True):
   out=d/('probe-sanitized' if sanitize else 'probe')
   flags=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'] if sanitize else ['-O2']
   subprocess.run(['cc','-std=c11','-D_DEFAULT_SOURCE','-fblocks',*flags,'-I'+str(d),str(fixture),str(d/'generated.c'),'-lm','-o',str(out)],check=True)
   subprocess.run([str(out)],check=True,timeout=15)
  print(mode,'macOS DNS-SD probe passed in native and ASan/UBSan modes',flush=True)
