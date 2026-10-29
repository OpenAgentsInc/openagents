#!/usr/bin/env python3
"""Source-pinned integer lemmas; source translation remains a manual review."""
import argparse,hashlib,importlib.util,json,re
from pathlib import Path
import z3
ROOT=Path(__file__).resolve().parents[3]
BASE=ROOT/'tools/coax-nostr/field-guards/prove.py';BASE_SHA='15d72eb3b45bfe5cf62a968f70cf8f650f88eff360f44bd77c31d2e5c16d3d58'
PINS={'coax/lib/crypto/ecdh.coax': '2f6509d0bb427fa811c274838ad19aa6d1a22550663b62720d2cd595e4d35c39', 'coax/lib/crypto/ecdh_fast.coax': '3c7d7949f2b69677c199faa3ee85b253c9281f9e24f4a1e762728417acc88915', 'coax/lib/crypto/secp_field.coax': '8420f6a9ad21054a1f0d37a1721f2bc79c0db8bc489803c9dd985fef9ac38484', 'coax/lib/crypto/secp_field32_total.coax': '961e5da4378a23e1227fbc6e13308914aa310f2660107c05ac1775e8221ffccf', 'coax/lib/crypto/secp_field32.coax': '98702d76f5c7fa69e62d2628df4eadd0c5344b7ca7a85772bff70c493cd7869b', 'coax/lib/crypto/secp_group32.coax': '699f3ddb109dd26acfb7f0162edd9a5f1d7dfc44e353c0b2964b56fadc1f403a', 'coax/lib/crypto/secp_group.coax': 'b80d001e7f03a15c6dd7951c2b2c016305653c7755636b53cbc69b0cc00a1b1c', 'coax/lib/crypto/secp_mult_const.coax': '948b31ef93663792305347ca5f65e22259c5777e783edf7b9502b09033de622b', 'coax/lib/crypto/secp_scalar.coax': 'daede6b4ab97c9565594adac71ca7abd8311ce1c18bcd3091a3bb8af21b0185a', 'coax/lib/crypto/bytes.coax': 'af7b98d78a60dcf3fd13964e20e80d5c8fddd7554571b168a9dba983234ae4bd'}
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--report',type=Path,required=True);args=ap.parse_args()
 if sha(BASE)!=BASE_SHA:raise ValueError('base model changed')
 for name,h in PINS.items():
  if sha(ROOT/name)!=h:raise ValueError('source changed: '+name)
 spec=importlib.util.spec_from_file_location('field_model',BASE);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
 M=m.N;B=m.B;C=m.C;P=m.P;v,b=z3.Ints('fused_low fused_carry');candidate=v+C;q=candidate/M;r=candidate%M
 terms=[v>=0,v<M,b>=0,b<=1,z3.Implies(b==1,v<C*C)];result=z3.If(z3.Or(b==1,q==1),r,v)
 checks=[m.solve('fused_canonical_range_and_congruence',terms,z3.Or(result<0,result>=P,(result-(v+b*C))%P!=0)),m.solve('incoming_carry_is_required',terms+[b==1,v==0],z3.If(q==1,r,v)!=(v+b*C)%P,'sat')]
 u,top=z3.Ints('fold_low fold_top');fold=u+top*C;carry=fold/M;residual=fold%M
 checks.append(m.solve('product_first_fold_carry_and_residual',[u>=0,u<M,top>=0,top<=C],z3.Or(carry>1,z3.And(carry==1,residual>=C*C))))
 checks.append(m.solve('add_first_fold_carry_and_residual',[u>=0,u<M,top>=0,top<=1],z3.Or(carry>1,z3.And(carry==1,residual>=C))))
 aa,bb,old,carry=z3.Ints('school_a school_b school_old school_carry');wide=aa*bb+old+carry
 checks.append(m.solve('schoolbook_wide_and_carry',m.domain([aa,bb,old,carry],B),z3.Or(wide>=m.U,wide/B>=B)))
 low=[z3.Int('lo%d'%i) for i in range(8)];high=[z3.Int('hi%d'%i) for i in range(8)];carry=0;previous=0;digits=[];wides=[]
 for i in range(8):
  w=low[i]+977*high[i]+previous+carry;wides.append(w);digits.append(w%B);carry=w/B;previous=high[i]
 top=previous+carry;checks.append(m.solve('product_high_fold_identity',m.domain(low+high,B),z3.Or(m.as_int(digits)+top*M!=m.as_int(low)+m.as_int(high)*C,top>C,*[w>=m.U for w in wides])))
 text=(ROOT/'coax/lib/crypto/secp_field.coax').read_text();part=text.split('SF_FIELD_CHAIN ::',1)[1].split('];',1)[0]
 steps=[tuple(map(int,x)) for x in re.findall(r'left\s*=\s*(\d+),\s*squares\s*=\s*(\d+),\s*right\s*=\s*(\d+)',part)]
 if len(steps)!=12:raise ValueError('chain shape changed')
 powers=[1]
 for index,(left,squares,right) in enumerate(steps):
  if not (0<=left<=index and 0<=right<=index and 0<=squares<=256):raise ValueError('invalid chain step')
  powers.append((powers[left]<<squares)+powers[right])
 power9=3*powers[1];power45=(power9<<2)+power9;exponent=(powers[12]<<10)+power45
 if exponent!=P-2:raise ValueError('inverse exponent mismatch')
 result={'schema':'coax.ecdh-fused-proof.v1','scope':'integer lemmas and public chain exponent; manual source correspondence and caller closure remain separate','source_sha256':PINS,'script_sha256':sha(Path(__file__)),'base_model_sha256':BASE_SHA,'solver':'z3 '+z3.get_version_string(),'solver_library':m.solver_artifact(),'chain':steps,'inverse_exponent_hex':hex(exponent),'checks':checks}
 args.report.write_text(json.dumps(result,indent=2)+'\n')
if __name__=='__main__':main()
