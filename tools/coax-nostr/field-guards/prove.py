#!/usr/bin/env python3
"""Source-pinned conditional arithmetic obligations; not target approval."""
import argparse
import ctypes
import hashlib
import json
import platform
from pathlib import Path
import sys
import z3

ROOT = Path(__file__).resolve().parents[3]
PINS = {
    'coax/lib/crypto/secp_field.coax':'8420f6a9ad21054a1f0d37a1721f2bc79c0db8bc489803c9dd985fef9ac38484',
    'coax/lib/crypto/secp_field32.coax':'98702d76f5c7fa69e62d2628df4eadd0c5344b7ca7a85772bff70c493cd7869b',
    'coax/lib/crypto/secp_group.coax':'b80d001e7f03a15c6dd7951c2b2c016305653c7755636b53cbc69b0cc00a1b1c',
    'coax/lib/crypto/secp_group32.coax':'b4e11a4626ba2b7be823078dc8495b800ecd9f579366d706cdd4e421b18f437b',
    'coax/lib/crypto/secp_mult_base.coax':'a82d9737aee8f0c45d6827880042e9b9ad22cd18bedde1ed6dad7b956fc1fe71',
    'coax/lib/crypto/secp_mult_base_web.coax':'3ca9989b48ff368d1511fe9d81e4f91ec9f8f93d4099c6cf4cf0aa31fd391220',
    'coax/lib/crypto/secp_mult_base_native.coax':'5aa9f59a44265e5e812d3081dbd663f1690eaea3482221c7584a90e76adfb8c6',
    'coax/lib/crypto/secp_mult_const.coax':'60957c9aacf9412b82b56083bf3c02e8159a3f18ec62e5f847b305844201d0cf',
    'coax/lib/crypto/secp_scalar.coax':'daede6b4ab97c9565594adac71ca7abd8311ce1c18bcd3091a3bb8af21b0185a',
    'coax/lib/crypto/bytes.coax':'af7b98d78a60dcf3fd13964e20e80d5c8fddd7554571b168a9dba983234ae4bd',
}
B=1<<32; N=1<<256; C=B+977; P=N-C; U=1<<64

def digest(data): return hashlib.sha256(data).hexdigest()
def solver_artifact():
    class DlInfo(ctypes.Structure):
        _fields_=[('name',ctypes.c_char_p),('base',ctypes.c_void_p),('symbol',ctypes.c_char_p),('address',ctypes.c_void_p)]
    try:
        lookup=ctypes.CDLL(None).dladdr;lookup.argtypes=[ctypes.c_void_p,ctypes.POINTER(DlInfo)];lookup.restype=ctypes.c_int
        info=DlInfo();pointer=ctypes.cast(z3.z3core.Z3_get_version.__defaults__[0].f,ctypes.c_void_p)
        if not lookup(pointer,ctypes.byref(info)) or not info.name:return None
        path=Path(info.name.decode()).resolve();return {'path':str(path),'sha256':digest(path.read_bytes())}
    except (AttributeError,OSError,TypeError):return None
def bound(root):
    for name,expected in PINS.items():
        try: actual=digest((root/name).read_bytes())
        except OSError as error: raise ValueError('missing source: '+name) from error
        if actual!=expected: raise ValueError('source pin mismatch: '+name)

def solve(name,conditions,bad,expected='unsat'):
    solver=z3.Solver();solver.set(timeout=60000);solver.add(*conditions);solver.add(bad)
    query=solver.to_smt2();result=solver.check()
    if str(result)!=expected:
        raise RuntimeError(name+': expected '+expected+', got '+str(result)+' '+(solver.reason_unknown() if result==z3.unknown else ''))
    out={'name':name,'result':str(result),'query_sha256':digest(query.encode())}
    if result==z3.sat:out['witness']={str(d):str(solver.model()[d]) for d in solver.model().decls()}
    return out

def domain(xs,limit): return [z3.And(x>=0,x<limit) for x in xs]
def carry26(digits,checked):
    d=digits[:]
    for i in range(9):
        q=d[i]/(1<<26);d[i]=d[i]%(1<<26);d[i+1]=d[i+1]+q;checked.append(d[i+1])
    q=d[9]/(1<<22);d[9]=d[9]%(1<<22)
    return d,q

def norm_model(magnitude):
    digits=[z3.Int('norm_%s_%d'%(magnitude,i)) for i in range(10)];checked=[]
    constraints=[z3.And(x>=0,x<=magnitude*((1<<(22 if i==9 else 26))-1)) for i,x in enumerate(digits)]
    d,c1=carry26(digits,checked);d[0]=d[0]+977*c1;d[1]=d[1]+64*c1;checked.extend(d[:2])
    d,c2=carry26(d,checked);d[0]=d[0]+977*c2;d[1]=d[1]+64*c2;checked.extend(d[:2])
    d,c3=carry26(d,checked)
    return constraints,c1,c2,c3,checked

def field32_fold(words,top):
    carry=top*977;out=[];wide=[]
    for i in range(8):
        w=words[i]+carry+(top if i==1 else 0);wide.append(w);out.append(w%B);carry=w/B
    return out,carry,wide

def as_int(words): return sum(words[i]*(B**i) for i in range(8))

def obligations():
    out=[];q,c1,c2,c3,wide=norm_model(8)
    out.append(solve('normalize_guard_and_word_bounds',q,z3.Or(c1>7,c2>1,c3!=0,*[z3.Or(x<0,x>=U) for x in wide])))
    q,c1,_,_,_=norm_model(9);out.append(solve('widened_magnitude_breaks_carry_bound',q,c1>7,'sat'))
    a=[z3.Int('add_a%d'%i) for i in range(8)];b=[z3.Int('add_b%d'%i) for i in range(8)]
    A=as_int(a);D=as_int(b);carry=0;limbs=[];wide=[]
    for i in range(8):
        w=a[i]+b[i]+carry;wide.append(w);limbs.append(w%B);carry=w/B
    R=as_int(limbs);constraints=domain(a+b,B)+[A<P,D<P]
    out.append(solve('add_guard_and_limb_identity',constraints,z3.Or(R+carry*N!=A+D,(R+carry*C)/N!=0,*[w>=U for w in wide])))
    x,y=z3.Ints('add_invalid_a add_invalid_b');t=x+y
    out.append(solve('noncanonical_add_guard_counterexample',[x==N-1,y==N-1],((t%N)+(t/N)*C)/N!=0,'sat'))
    words=[z3.Int('fold_word%d'%i) for i in range(8)];top=z3.Int('fold_top');folded,last,wide=field32_fold(words,top)
    out.append(solve('fold_top_identity_and_word_bounds',domain(words,B)+[top>=0,top<=C],z3.Or(as_int(folded)+last*N!=as_int(words)+top*C,*[w>=U for w in wide])))
    aa,bb,old,carry=z3.Ints('mul_a mul_b mul_old mul_carry');w=aa*bb+old+carry
    out.append(solve('schoolbook_word_bound',domain([aa,bb,old,carry],B),z3.Or(w>=U,w/B>=B)))
    low=[z3.Int('low%d'%i) for i in range(8)];high=[z3.Int('high%d'%i) for i in range(8)];carry=0;previous=0;fold=[];wide=[]
    for i in range(8):
        w=low[i]+977*high[i]+previous+carry;wide.append(w);fold.append(w%B);carry=w/B;previous=high[i]
    top=previous+carry;L=as_int(low);H=as_int(high);R=as_int(fold)
    out.append(solve('first_product_fold_identity',domain(low+high,B),z3.Or(R+top*N!=L+H*C,top>C,*[w>=U for w in wide])))
    l,h=z3.Ints('product_low product_high');v=l+h*C;q=v/N;r=v%N;w=r+q*C;q2=w/N;r2=w%N
    out.append(solve('multiply_second_fold_guard',[l>=0,l<N,h>=0,h<N],(r2+q2*C)/N!=0))
    out.append(solve('one_fold_is_not_sufficient',[l>=0,l<N,h>=0,h<N],(r+q*C)/N!=0,'sat'))
    r=z3.Int('canonical_input');folded=z3.If((r+C)/N==1,(r+C)%N,r)
    out.append(solve('canonicalize_256_bit_input',[r>=0,r<N],z3.Or(folded<0,folded>=P,folded%P!=r%P)))
    x,y=z3.Ints('sub_a sub_b');sub=z3.If(x>=y,x-y,x-y+P)
    out.append(solve('subtraction_canonical_closure',[x>=0,x<P,y>=0,y<P],z3.Or(sub<0,sub>=P,sub%P!=(x-y)%P)))
    a=[z3.Int('sub_limb_a%d'%i) for i in range(8)];b=[z3.Int('sub_limb_b%d'%i) for i in range(8)];A=as_int(a);D=as_int(b);borrow=0;raw=[];wide=[]
    for i in range(8):
        w=B+a[i]-b[i]-borrow;wide.append(w);raw.append(w%B);borrow=1-w/B
    carry=0;result=[]
    for i in range(8):
        digit=(P//(B**i))%B;w=raw[i]+digit*borrow+carry;wide.append(w);result.append(w%B);carry=w/B
    expected=z3.If(A>=D,A-D,A-D+P)
    out.append(solve('subtraction_limb_identity',domain(a+b,B)+[A<P,D<P],z3.Or(as_int(result)!=expected,*[z3.Or(w<0,w>=U) for w in wide])))
    limbs=[z3.BitVec('packed_limb%d'%i,64) for i in range(10)];terms=[z3.ULE(x,z3.BitVecVal((1<<(22 if i==9 else 26))-1,64)) for i,x in enumerate(limbs)]
    words=[z3.BitVecVal(0,32) for _ in range(8)];expected=z3.BitVecVal(0,256)
    for i,x in enumerate(limbs):
        word=i*26//32;shift=i*26%32;words[word]=words[word]|z3.Extract(31,0,x<<shift);words[(word+1)%8]=words[(word+1)%8]|z3.Extract(31,0,z3.LShR(x,32-shift));expected=expected|(z3.ZeroExt(192,x)<<(26*i))
    out.append(solve('canonical_26_to_32_bit_packing',terms,z3.Concat(*reversed(words))!=expected))
    out.append(solve('canonical_input_packing_stays_canonical',terms+[z3.ULT(expected,z3.BitVecVal(P,256))],z3.UGE(z3.Concat(*reversed(words)),z3.BitVecVal(P,256))))
    nibble,prior=z3.Ints('nibble prior');value=nibble+prior;next_carry=(value+8)/16
    out.append(solve('signed_digit_carry_is_bit',[nibble>=0,nibble<=15,prior>=0,prior<=1],z3.Or(next_carry<0,next_carry>1)))
    left,right=z3.BitVecs('equal_left equal_right',64);difference=left^right
    for shift in [32,16,8,4,2,1]:difference=difference|z3.LShR(difference,shift)
    bit=(difference&1)^1;mask=(bit&1)*z3.BitVecVal((1<<64)-1,64)
    out.append(solve('word_equality_is_complete_mask',[],mask!=z3.If(left==right,z3.BitVecVal((1<<64)-1,64),z3.BitVecVal(0,64))))
    a,b,m=z3.BitVecs('select_a select_b select_mask',32);choice=b^((a^b)&m)
    out.append(solve('complete_mask_selects_one_input',[z3.Or(m==0,m==z3.BitVecVal((1<<32)-1,32))],choice!=z3.If(m==0,b,a)))
    return out

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--root',type=Path,default=ROOT);ap.add_argument('--report',type=Path);args=ap.parse_args()
    try:bound(args.root);checks=obligations()
    except (ValueError,RuntimeError) as error:ap.exit(1,str(error)+'\n')
    result={'schema':'coax.field-guard-math.v1','scope':'conditional arithmetic models; source-to-model, callers, target and production gates remain separate','source_sha256':PINS,'script_sha256':digest(Path(__file__).read_bytes()),'solver':'z3 '+z3.get_version_string(),'solver_library':solver_artifact(),'python':sys.version,'host':platform.platform(),'checks':checks}
    text=json.dumps(result,indent=2)+'\n'
    if args.report:args.report.write_text(text)
    else:print(text,end='')
if __name__=='__main__':main()
