#!/usr/bin/env python3
"""Source-bound integer lemmas for total field reductions; manual review remains required."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import z3
ROOT=Path(__file__).resolve().parents[3]
BASE=ROOT/'tools/coax-nostr/field-guards/prove.py'
BASE_SHA='15d72eb3b45bfe5cf62a968f70cf8f650f88eff360f44bd77c31d2e5c16d3d58'
# Populated from the frozen candidate; changed inputs refuse.
PINS={'coax/lib/crypto/secp_field32.coax': '98702d76f5c7fa69e62d2628df4eadd0c5344b7ca7a85772bff70c493cd7869b', 'coax/lib/crypto/secp_field32_total.coax': '961e5da4378a23e1227fbc6e13308914aa310f2660107c05ac1775e8221ffccf', 'coax/lib/crypto/secp_group32.coax': '699f3ddb109dd26acfb7f0162edd9a5f1d7dfc44e353c0b2964b56fadc1f403a', 'coax/lib/crypto/secp_mult_const.coax': '948b31ef93663792305347ca5f65e22259c5777e783edf7b9502b09033de622b', 'coax/lib/crypto/secp_group.coax': 'b80d001e7f03a15c6dd7951c2b2c016305653c7755636b53cbc69b0cc00a1b1c', 'coax/lib/crypto/secp_field.coax': '8420f6a9ad21054a1f0d37a1721f2bc79c0db8bc489803c9dd985fef9ac38484'}
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def run():
 if sha(BASE)!=BASE_SHA:raise ValueError('base arithmetic model changed')
 for name,h in PINS.items():
  if sha(ROOT/name)!=h:raise ValueError('source pin mismatch: '+name)
 spec=importlib.util.spec_from_file_location('reviewed_field_lemmas',BASE);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
 N=m.N;B=m.B;C=m.C;P=m.P
 a,b=z3.Ints('total_add_a total_add_b');x=a+b
 def fold(v):return v%N+(v/N)*C
 y=fold(x);z=fold(y);q=[]
 q.append(m.solve('total_add_two_folds_never_carry',[a>=0,a<N,b>=0,b<N],z>=N))
 q.append(m.solve('total_add_one_fold_is_insufficient',[a==N-1,b==N-1],y>=N,'sat'))
 q.append(m.solve('total_add_value_mod_p',[a>=0,a<N,b>=0,b<N],(z-x)%P!=0))
 # One exact fold identity composes without asking the solver to expand a
 # nonlinear multiplication through multiple 256-bit quotient expressions.
 value=z3.Int('fold_value');q.append(m.solve('fold_preserves_residue',[value>=0],(fold(value)-value)%P!=0))
 limbs=[z3.Int('total_fold_word%d'%i) for i in range(8)];top=z3.Int('total_fold_top')
 digits,last,wide=m.field32_fold(limbs,top)
 q.append(m.solve('unrolled_fold_identity_and_u64_bounds',m.domain(limbs,B)+[top>=0,top<=C],z3.Or(m.as_int(digits)+last*N!=m.as_int(limbs)+top*C,*[z3.Or(w<0,w>=m.U) for w in wide])))
 q0,c1,c2,c3,wide=m.norm_model(8)
 q.append(m.solve('public_peer_normalize_guard_under_admitted_bound',q0,z3.Or(c1>7,c2>1,c3!=0,*[z3.Or(v<0,v>=m.U) for v in wide])))
 aa,bb,old,carry=z3.Ints('school_a school_b school_old school_carry');wide=aa*bb+old+carry
 q.append(m.solve('schoolbook_wide_and_carry',[*m.domain([aa,bb,old,carry],B)],z3.Or(wide>=m.U,wide/B>=B)))
 low=[z3.Int('lo%d'%i) for i in range(8)];high=[z3.Int('hi%d'%i) for i in range(8)];carry=0;previous=0;digits=[];wide=[]
 for i in range(8):
  w=low[i]+977*high[i]+previous+carry;wide.append(w);digits.append(w%B);carry=w/B;previous=high[i]
 top=previous+carry
 q.append(m.solve('product_high_fold_identity',m.domain(low+high,B),z3.Or(m.as_int(digits)+top*N!=m.as_int(low)+m.as_int(high)*C,top>C,*[w>=m.U for w in wide])))
 lo,hi=z3.Ints('product_low product_high');first=lo+hi*C;second=fold(first);third=fold(second)
 terms=[lo>=0,lo<N,hi>=0,hi<N]
 q.append(m.solve('product_final_fold_never_carries',terms,third>=N))
 q.append(m.solve('product_first_fold_can_carry',terms,second>=N,'sat'))
 value=z3.Int('canonical_word');canonical=z3.If((value+C)/N==1,(value+C)%N,value)
 q.append(m.solve('canonical_word_range_and_residue',[value>=0,value<N],z3.Or(canonical<0,canonical>=P,(canonical-value)%P!=0)))
 a,b=z3.Ints('canonical_sub_a canonical_sub_b');sub=z3.If(a>=b,a-b,a-b+P)
 q.append(m.solve('canonical_sub_closure',[a>=0,a<P,b>=0,b<P],z3.Or(sub<0,sub>=P,(sub-(a-b))%P!=0)))
 return {'schema':'coax.total-field-proof.v1','scope':'integer model only; manual source correspondence, caller closure and target review remain separate','source_sha256':PINS,'base_model_sha256':BASE_SHA,'script_sha256':sha(Path(__file__)),'solver':'z3 '+z3.get_version_string(),'solver_library':m.solver_artifact(),'python':sys.version,'checks':q}
def main():
 p=argparse.ArgumentParser();p.add_argument('--report',type=Path);a=p.parse_args()
 try:result=run()
 except (ValueError,RuntimeError) as e:p.exit(1,str(e)+'\n')
 text=json.dumps(result,indent=2)+'\n'
 if a.report:a.report.write_text(text)
 else:print(text,end='')
if __name__=='__main__':main()
