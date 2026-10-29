#!/usr/bin/env python3
"""Replay reviewed historical field queries; source correspondence is a separate premise."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import z3
ROOT=Path(__file__).resolve().parents[3]
BASE=ROOT/'tools/coax-nostr/field-guards/prove.py'
BASE_SHA='15d72eb3b45bfe5cf62a968f70cf8f650f88eff360f44bd77c31d2e5c16d3d58'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 parser=argparse.ArgumentParser();parser.add_argument('--report',type=Path,required=True);args=parser.parse_args()
 if sha(BASE)!=BASE_SHA:raise ValueError('historical model changed')
 spec=importlib.util.spec_from_file_location('historical_field_model',BASE);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
 checks=m.obligations()
 out={'schema':'coax.public-peer-proof-replay.v1','scope':'historical solver queries; manual correspondence to current source is separate','reference_source_sha256':m.PINS,'reference_script_sha256':sha(BASE),'script_sha256':sha(Path(__file__)),'solver':'z3 '+z3.get_version_string(),'solver_library':m.solver_artifact(),'checks':checks}
 args.report.write_text(json.dumps(out,indent=2)+'\n')
if __name__=='__main__':main()
