#!/usr/bin/env python3
"""Clean preparation entry for the explicit one-shot grid (no plant/SDK).

The historical native_reference --prepare failure is retained verbatim. This
small adapter supplies the private generated-header basename alias before the
first effects compilation. Default checks use panda_gain_search_cached.py.
"""
from pathlib import Path
import argparse,platform,sys
import panda_gain_search_native_reference as core
class AliasRun(core.Run):
 def __init__(self,out,record):
  record['source'][str(Path(__file__).resolve().relative_to(core.ROOT))]=core.identity(__file__)
  super().__init__(out,record)
 def command(self,name,argv,env=None):
  result=super().command(name,argv,env)
  if name in['emit-debug','emit-release']:
   h=Path(argv[argv.index('--header')+1]);alias=h.parent/'panda-joint-pd-tracking.h';alias.write_bytes(h.read_bytes());core.require(core.identity(alias)==core.identity(h),'exactprivateheaderalias');self.record.setdefault('private_header_aliases',{})[name.removeprefix('emit-')]=core.identity(alias);self.save()
  return result

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('--compiler',type=Path,default=core.COMPILER);p.add_argument('--baseline',type=Path,default=core.BASELINE);a=p.parse_args();core.require(platform.system()=='Darwin'and platform.machine()=='arm64'and sys.byteorder=='little','qualifiedDarwinarm64');e,b=core.frozen(a.baseline);core.Run=AliasRun;core.prepare(a,e,b)
if __name__=='__main__':main()
