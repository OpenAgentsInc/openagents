#!/usr/bin/env python3
"""Test-only scalar adapter for the existing bounded JSON differential contract."""
import argparse
import ctypes as C
import json
from pathlib import Path
import sys

class Scalar(C.Structure):_fields_=[('limbs',C.c_uint32*8)]
class Inverse(C.Structure):_fields_=[('value',Scalar),('ok',C.c_bool)]
def scalar(value):return Scalar((C.c_uint32*8)(*[(value>>(i*32))&0xffffffff for i in range(8)]))
def integer(value):return sum(int(x)<<(i*32) for i,x in enumerate(value.limbs))
def library(path):
 lib=C.CDLL(str(Path(path).resolve()))
 for name in ['sc_add','sc_sub','sc_mul']:
  fn=getattr(lib,name);fn.argtypes=[Scalar,Scalar];fn.restype=Scalar
 for name in ['sc_normalize','sc_negate']:
  fn=getattr(lib,name);fn.argtypes=[Scalar];fn.restype=Scalar
 lib.sc_inverse.argtypes=[Scalar];lib.sc_inverse.restype=Inverse
 return lib
def evaluate(lib,request):
 if request.get('module')!='scalar':return {'ok':False}
 op=request.get('op');a=int(request['a'],16);b=int(request.get('b','0'),16)
 if not 0<=a<2**256 or not 0<=b<2**256:return {'ok':False}
 x=scalar(a);y=scalar(b)
 if op in ['normalize','negate']:value=getattr(lib,'sc_'+op)(x)
 elif op in ['add','sub','mul']:value=getattr(lib,'sc_'+op)(x,y)
 elif op=='inverse':
  inverse=lib.sc_inverse(x)
  if not inverse.ok:return {'ok':False}
  value=inverse.value
 else:return {'ok':False}
 return {'ok':True,'value':format(integer(value),'064x')}
if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--library',type=Path,required=True);args=parser.parse_args()
 data=sys.stdin.buffer.read(1048577)
 if len(data)>1048576:raise SystemExit('request limit')
 print(json.dumps(evaluate(library(args.library),json.loads(data))))
