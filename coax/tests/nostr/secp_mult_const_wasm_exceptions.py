#!/usr/bin/env python3
"""Exercise bounded invalid and infinity peers on actual Wasm."""
import os as _os,sys as _sys;_sys.path.insert(0,_os.path.dirname(_os.path.abspath(__file__)));from wasm_toolchain import WASM_CLANG,WASM_SYSROOT,WASM_RESOURCES
import json
from pathlib import Path
import unittest
import secp_field as base
from secp_field_mul import file_sha
from secp_mult_const import VariableBase,ROOT
class ExceptionalWasm(VariableBase):
 def test_exceptional_wasm_peers(self):
  rows=[]
  for name,_ in self.libs:
   wasm=self.dir/(name+'-exceptions.wasm')
   base.run(WASM_CLANG,'--target=wasm32-wasip1','--sysroot='+WASM_SYSROOT,'-resource-dir='+WASM_RESOURCES,'-std=c11','-O2','-I'+str(self.dir),self.dir/(name+'.c'),ROOT/'coax/tests/nostr/secp_mult_const_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base,-z,stack-size=1048576','-lm','-o',wasm)
   rows.append({'mode':name,'artifact_sha256':file_sha(wasm),'stack_bytes':1048576,**json.loads(base.run('node',ROOT/'coax/tests/nostr/secp_mult_const_wasm_exceptions.cjs',wasm,timeout=1800).stdout)})
  self.report['wasm_exceptional']=rows
  self.report['exception_test_sha256']=file_sha(Path(__file__))
  self.report['exception_driver_sha256']=file_sha(ROOT/'coax/tests/nostr/secp_mult_const_wasm_exceptions.cjs')
if __name__=='__main__':unittest.main(defaultTest='ExceptionalWasm.test_exceptional_wasm_peers')
