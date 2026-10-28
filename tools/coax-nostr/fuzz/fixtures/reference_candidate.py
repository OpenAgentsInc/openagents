#!/usr/bin/env python3
"""Harness fixture ONLY: shares the oracle and is never conformance evidence."""
import argparse
import json
from pathlib import Path
import sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from differential import reference
from oracle import P,NativeOracle,DEFAULT_LIBRARY
p=argparse.ArgumentParser();p.add_argument('--fault',choices=['none','carry','default-hash'],default='none');a=p.parse_args()
r=json.loads(sys.stdin.read(1048576))
native=None
if r['module']=='schnorr' and r['op']=='sign':
    native=NativeOracle(DEFAULT_LIBRARY)
actual=reference(r,native)
if native is not None:native.close()
if a.fault=='carry' and r['module']=='field' and r['op']=='mul':
    actual={'ok':True,'value':format((int(r['a'],16)*int(r['b'],16)&((1<<256)-1))%P,'064x')}
if a.fault=='default-hash' and r['module']=='ecdh':
    native=NativeOracle(DEFAULT_LIBRARY)
    value=native.ecdh(bytes.fromhex(r['secret']),bytes.fromhex(r['peer']),default_hash=True);native.close()
    actual={'ok':False} if value is None else {'ok':True,'value':value.hex()}
print(json.dumps(actual,sort_keys=True))
