#!/usr/bin/env python3
"""Explicit test-only derivation. Never part of the compiler/app build.

Keeps upstream CSV bytes untouched and includes no test secret keys.
"""
import argparse
import csv
import hashlib
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'tools/coax-nostr/fuzz'))
from oracle import P,N,lift_x,tagged
SOURCE=ROOT/'coax/tests/vectors/upstream/bip340/test-vectors.csv'
DEST=ROOT/'coax/tests/nostr/schnorr_fixtures.coax'
PIN='34c9d1d9c3a88d524bc80778540dc43f8306ec249a7485293063c376db851c2d'
def array(data):return '['+str(max(1,len(data)))+'] u8.['+','.join(str(n) for n in (data or b'\0'))+']'
def scalar(n):return 'ScScalar.{limbs=[8] u32.['+','.join(str((n>>(32*i))&0xffffffff) for i in range(8))+']}'
def field(n):return 'SecpField.{limb=[10] u64.['+','.join(str((n>>(26*i))&((1<<(22 if i==9 else 26))-1)) for i in range(10))+']}'
def generate():
 raw=SOURCE.read_bytes()
 if hashlib.sha256(raw).hexdigest()!=PIN:raise ValueError('pinned upstream BIP340 bytes changed')
 out=['// Derived component checks, NOT full interpreter signature verification.',f'// Upstream SHA-256:{PIN}','// Generator:coax/tests/nostr/make_schnorr_fixtures.py; explicit Python test tooling.','// Only public key, message and signature are copied; no private-key fields.']
 with SOURCE.open(newline='') as stream:rows=list(csv.DictReader(stream))
 for row in rows:
  msg=bytes.fromhex(row['message']);pub=bytes.fromhex(row['public key']);sig=bytes.fromhex(row['signature']);r=int.from_bytes(sig[:32],'big');s=int.from_bytes(sig[32:],'big');point=lift_x(int.from_bytes(pub,'big'));ready=point is not None and r<P and s<N
  out += [f'#test "BIP340 component preparation row{row["index"]}, message{len(msg)} bytes, no full equation" {{',f'  message := {array(msg)};',f'  public_key := {array(pub)};',f'  signature := {array(sig)};',f'  prepared := sv_prepare_vartime(message[0..<{len(msg)}],public_key[..],signature[..]);',f'  #assert prepared.ready == {str(ready).lower()};']
  if ready:
   e=int.from_bytes(tagged('BIP0340/challenge',sig[:32]+pub+msg),'big')%N
   out += [f'  #assert sc_equal(prepared.s,{scalar(s)}) && sc_equal(prepared.negative_e,{scalar((-e)%N)});',f'  #assert sf_equal_mask(prepared.r,{field(r)}) == CB_MAX64;',f'  expected := SgPoint.{{x={field(point[0])},y={field(point[1])},z=SecpField.{{limb=[10]u64.[1,0,0,0,0,0,0,0,0,0]}}}};', '  #assert sg_equal_mask(prepared.point,expected) == CB_MAX64;']
  if len(msg)!=32:out += [f'  #assert !sv_prepare_nostr_vartime(message[0..<{len(msg)}],public_key[..],signature[..]).ready;']
  elif row['index']=='0':out += ['  #assert sv_prepare_nostr_vartime(message[..],public_key[..],signature[..]).ready;']
  out += ['}']
 return '\n'.join(out)+'\n'
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');args=p.parse_args();text=generate()
 if args.write:DEST.write_text(text)
 elif not DEST.exists() or DEST.read_text()!=text:raise SystemExit('derived Coax preparation fixtures drift; run explicit --write')
 print('19 source-preserving preparation rows checked; full verification is tested in native/Wasm')
