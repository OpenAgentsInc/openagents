"""Frozen test-only native build/admission support for public verification."""
import ctypes as C
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import time
import secp_field as base
ROOT=base.ROOT
SOURCE=ROOT/'coax/lib/crypto/schnorr.coax'
VECTORS=ROOT/'coax/tests/vectors/upstream/bip340/test-vectors.csv'
VECTOR_SHA='34c9d1d9c3a88d524bc80778540dc43f8306ec249a7485293063c376db851c2d'
SOURCES=[SOURCE,ROOT/'coax/lib/crypto/secp_mult.coax',ROOT/'coax/lib/crypto/secp_group.coax',ROOT/'coax/lib/crypto/secp_field.coax',ROOT/'coax/lib/crypto/secp_scalar.coax',ROOT/'coax/lib/crypto/sha256.coax',ROOT/'coax/lib/proto/fingerprint.coax',ROOT/'coax/lib/crypto/bytes.coax',ROOT/'coax/tests/nostr/secp_field.py',Path(__file__),ROOT/'tools/coax-nostr/fuzz/oracle.py',ROOT/'tools/coax-nostr/fuzz/oracle.c',ROOT/'tools/coax-nostr/fuzz/UPSTREAM.json']
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def source_pins():return {str(p.relative_to(ROOT)):sha(p) for p in SOURCES}
def check_inputs():return {str(p.relative_to(ROOT)):sha(p) for p in [ROOT/'coax/tests/nostr/schnorr_checks.coax',ROOT/'coax/tests/nostr/schnorr_fixtures.coax',ROOT/'coax/tests/nostr/make_schnorr_fixtures.py']}
def check_pins(pins):
 for name,want in pins.items():
  if sha(ROOT/name)!=want:raise ValueError('source changed during verification:'+name)
def verify_timed(lib,message,public,signature,nostr=False,counts=None):
 arrays=[base.array(value) for value in [message,public,signature]]
 views=[base.view(value) for value in arrays]
 if counts is not None:
  for v,count in zip(views,counts):v.count=count
 start=time.perf_counter();result=bool(getattr(lib,'nostr_verify_vartime' if nostr else 'schnorr_verify_vartime')(*views));elapsed=time.perf_counter()-start
 if any(bytes(array)!=expected for array,expected in zip(arrays,[message,public,signature])):raise AssertionError('verifier modified borrowed input')
 return result,elapsed
def verify(lib,message,public,signature,nostr=False,counts=None):return verify_timed(lib,message,public,signature,nostr,counts)[0]
def build(directory):
 directory=Path(directory);directory.mkdir(parents=True,exist_ok=True);libs=[];artifacts=[]
 for label,mode in [('checked',[]),('release',['--release'])]:
  source=directory/(label+'.c');source.write_text(base.run(base.COMPILER,'c',SOURCE,'--library','--header',directory/'schnorr.h',*mode).stdout)
  artifact=directory/(label+('.dylib' if platform.system()=='Darwin' else '.so'))
  base.run('cc','-std=c11','-O2','-shared','-fPIC',source,'-lm','-o',artifact);lib=C.CDLL(str(artifact))
  for name in ['schnorr_verify_vartime','nostr_verify_vartime']:
   getattr(lib,name).argtypes=[base.View,base.View,base.View];getattr(lib,name).restype=C.c_bool
  libs.append((label,lib));artifacts.append({'mode':label,'generated_c_sha256':sha(source),'artifact_sha256':sha(artifact),'flags':['-std=c11','-O2','-shared','-fPIC']})
 return libs,artifacts
