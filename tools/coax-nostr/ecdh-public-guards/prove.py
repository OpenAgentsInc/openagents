#!/usr/bin/env python3
"""Pinned public-peer guard lemmas; caller/source correspondence is manual."""
import argparse,hashlib,importlib.util,json,re
from pathlib import Path
import z3
ROOT=Path(__file__).resolve().parents[3]
VIEW=ROOT/'docs/verification/2026-10-29-public-view-audit.json'
VIEW_SHA='b98824f262a5b11d11bb13852b75e3de6c2bc56fee6b1db100a5edfa64b06e5e'
REFERENCE=ROOT/'docs/verification/2026-10-29-secp-field-guard-reproof.json'
REFERENCE_SHA='90ce9200475505374f53c9e5315f1125e6193ac7f4514a67418ac222081a0410'
MODEL=ROOT/'tools/coax-nostr/field-guards/prove.py'
MODEL_SHA='15d72eb3b45bfe5cf62a968f70cf8f650f88eff360f44bd77c31d2e5c16d3d58'
SOLVER_SHA='bf9c9e020f19228523ee7bbf8595181328905158fe37e4ecead5f6cec9651be6'
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def solve(name,conditions,bad,expected='unsat'):
 s=z3.Solver();s.set(timeout=60000);s.add(*conditions);s.add(bad);query=s.to_smt2();result=s.check()
 if str(result)!=expected:raise ValueError(name+': '+str(result))
 out={'name':name,'result':str(result),'query_sha256':hashlib.sha256(query.encode()).hexdigest()}
 if result==z3.sat:out['witness']={str(d):str(s.model()[d]) for d in s.model().decls()}
 return out
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--report',type=Path,required=True);args=ap.parse_args()
 for p,h in [(VIEW,VIEW_SHA),(REFERENCE,REFERENCE_SHA),(MODEL,MODEL_SHA)]:
  if sha(p)!=h:raise ValueError('changed reviewed input '+str(p))
 view=json.loads(VIEW.read_text())['real_ecdh_audit'];ref=json.loads(REFERENCE.read_text())['proof'];graph=view['source_sha256']
 for name,h in graph.items():
  if not name.startswith('$REPO/') or sha(ROOT/name[6:])!=h:raise ValueError('changed source graph '+name)
 if view['accepted'] or view['error']!=14 or [(g['line'],g['origin_mask']) for g in view['authored_guard_obligations']]!=[(239,0),(492,0)]:raise ValueError('unexpected guard records')
 spec=importlib.util.spec_from_file_location('field_guard_model',MODEL);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
 solver=m.solver_artifact()
 if z3.get_version_string()!='5.3.2' or not solver or solver['sha256']!=SOLVER_SHA:raise ValueError('solver identity changed')
 checks=[]
 q,c1,c2,c3,wide=m.norm_model(8);normal=m.solve('normalize_guard_and_word_bounds',q,z3.Or(c1>7,c2>1,c3!=0,*[z3.Or(x<0,x>=m.U) for x in wide]))
 old=next(x for x in ref['checks'] if x['name']=='normalize_guard_and_word_bounds')
 if normal['query_sha256']!=old['query_sha256'] or normal['result']!=old['result']:raise ValueError('normalization query differs from reviewed proof')
 checks.append(normal)
 b=[z3.BitVec('byte_%d'%i,8) for i in range(32)];limbs=[z3.BitVecVal(0,64) for _ in range(10)];mask=z3.BitVecVal((1<<26)-1,64)
 for i in range(32):
  digit=(i*8)//26;shift=(i*8)%26;value=z3.ZeroExt(56,b[31-i]);limbs[digit]=limbs[digit]|((value<<shift)&mask);limbs[(digit+1)%10]=limbs[(digit+1)%10]|z3.LShR(value,26-shift)
 packed=z3.BitVecVal(0,256)
 for i,limb in enumerate(limbs):packed=packed|(z3.ZeroExt(192,limb)<<(26*i))
 bad=[z3.UGT(limb,z3.BitVecVal((1<<(22 if i==9 else 26))-1,64)) for i,limb in enumerate(limbs)]
 checks.append(solve('byte_load_is_bounded_and_exact',[],z3.Or(packed!=z3.Concat(*b),*bad)))
 witness=z3.BitVec('unmasked_byte',8);checks.append(solve('omitted_load_mask_breaks_bound',[],z3.UGT(z3.ZeroExt(56,witness)<<24,z3.BitVecVal((1<<26)-1,64)),'sat'))
 source=(ROOT/'coax/lib/crypto/secp_field.coax').read_text();part=source.split('SF_FIELD_CHAIN ::',1)[1].split('];',1)[0];steps=[tuple(map(int,x)) for x in re.findall(r'left\s*=\s*(\d+),\s*squares\s*=\s*(\d+),\s*right\s*=\s*(\d+)',part)]
 if len(steps)!=12 or any(not (0<=left<=i and 0<=right<=i and 0<=squares<=256) for i,(left,squares,right) in enumerate(steps)):raise ValueError('invalid field chain')
 i,left,right,squares=z3.Ints('chain_index chain_left chain_right chain_squares');constraints=[i>=0,i<12,z3.Or(*[z3.And(i==k,left==a,squares==b,right==c) for k,(a,b,c) in enumerate(steps)])]
 checks.append(solve('all_chain_steps_admitted',constraints,z3.Or(left<0,left>i,right<0,right>i,squares<0,squares>256)))
 checks.append(solve('invalid_first_chain_index_control',[i==0,left==1],left>i,'sat'))
 result={'schema':'coax.ecdh-public-guards-proof.v1','scope':'pinned byte-load and chain models plus replayed normalization query; source and caller correspondence require independent manual review','source_sha256':graph,'audit_report_sha256':VIEW_SHA,'reference_report_sha256':REFERENCE_SHA,'reference_model_sha256':MODEL_SHA,'script_sha256':sha(Path(__file__)),'solver':'z3 '+z3.get_version_string(),'solver_library':solver,'chain':steps,'checks':checks}
 args.report.write_text(json.dumps(result,indent=2)+'\n')
if __name__=='__main__':main()
