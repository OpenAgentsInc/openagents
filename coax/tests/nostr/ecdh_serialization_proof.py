#!/usr/bin/env python3
"""Source-pinned bit and index lemmas; source correspondence is a review premise."""
import argparse,hashlib,importlib.util,json
from pathlib import Path
import z3
ROOT=Path(__file__).resolve().parents[3]
BASE=ROOT/'tools/coax-nostr/field-guards/prove.py';BASE_SHA='15d72eb3b45bfe5cf62a968f70cf8f650f88eff360f44bd77c31d2e5c16d3d58'
PINS={'coax/lib/crypto/ecdh.coax':'2f6509d0bb427fa811c274838ad19aa6d1a22550663b62720d2cd595e4d35c39','coax/lib/crypto/bytes.coax':'af7b98d78a60dcf3fd13964e20e80d5c8fddd7554571b168a9dba983234ae4bd'}
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def solve(name,formula,expected):
 s=z3.Solver();s.add(formula);result=str(s.check())
 if result!=expected:raise AssertionError((name,result,expected))
 return {'name':name,'result':result,'smt_sha256':hashlib.sha256(s.sexpr().encode()).hexdigest()}
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--report',type=Path,required=True);args=ap.parse_args()
 if sha(BASE)!=BASE_SHA:raise ValueError('changed solver identity helper')
 for f,h in PINS.items():
  if sha(ROOT/f)!=h:raise ValueError('changed source '+f)
 source=(ROOT/'coax/lib/crypto/ecdh.coax').read_text();needle='for i:0..<32 {work[0].staged[31-i]=cast(u8)((product.affine_x.limb[i/4]>>((i%4)*8))&255);}'
 if source.count(needle)!=1:raise ValueError('serialization source shape changed')
 old=(ROOT/'coax/lib/crypto/bytes.coax').read_text()
 for needle in ['if width == 4 && value > 4294967295 { return false; }','index := ifx big then at + width - 1 - i else at + i;','out[index] = cast(u8) ((value >> (i * 8)) & 255);','return cb_store_word(out, at, cast(u64) value, 4, true);']:
  if old.count(needle)!=1:raise ValueError('checked helper source changed')
 word=z3.BitVec('word',32);checks=[]
 for j in range(4):
  old_byte=z3.Extract(7,0,z3.LShR(z3.ZeroExt(32,word),j*8));new_byte=z3.Extract(7,0,z3.LShR(word,j*8));checks.append(solve('word_byte_'+str(j),old_byte!=new_byte,'unsat'))
 i,j,k,l=z3.Ints('i j k l');index=(7-i)*4+3-j;other=(7-k)*4+3-l
 domain=z3.And(i>=0,i<8,j>=0,j<4,k>=0,k<8,l>=0,l<4)
 checks.append(solve('bounds_and_unique_positions',z3.And(domain,z3.Or(index<0,index>=32,z3.And(index==other,z3.Or(i!=k,j!=l)))),'unsat'))
 n=4*i+j;checks.append(solve('single_loop_matches_word_and_byte_indices',z3.And(domain,z3.Or(n<0,n>=32,n/4!=i,n%4!=j,31-n!=index)),'unsat'))
 checks.append(solve('reversed_byte_index_control',z3.And(domain,j==0,index!=(7-i)*4+j,z3.Extract(7,0,word)!=z3.Extract(31,24,word)),'sat'))
 spec=importlib.util.spec_from_file_location('field_model',BASE);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
 result={'schema':'coax.ecdh-fixed-serialization-proof.v1','scope':'word-byte and public index lemmas; actual source and call correspondence require independent review','source_sha256':PINS,'script_sha256':sha(Path(__file__)),'base_model_sha256':BASE_SHA,'solver':'z3 '+z3.get_version_string(),'solver_library':m.solver_artifact(),'checks':checks}
 args.report.write_text(json.dumps(result,indent=2)+'\n')
if __name__=='__main__':main()
