#!/usr/bin/env python3
# Firm reason: 2 (an independent oracle: BIP-340 verification written apart from coax/lib/crypto, for the seat ticket v5 fixtures)
# Checks that every ticket in proto/verse/seat-ticket-v5.json is signed by its key under BIP-340, and that the
# fixture public keys are the throwaway secrets' (#13260). Tooling only; never shipped.
#   python3 -I coax/web/room/seat-ticket-oracle.py proto/verse/seat-ticket-v5.json
import hashlib, json, sys
p=0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F
n=0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G=(0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)
def add(P,Q):
    if P is None: return Q
    if Q is None: return P
    if P[0]==Q[0] and P[1]!=Q[1]: return None
    if P==Q: l=3*P[0]*P[0]*pow(2*P[1],p-2,p)%p
    else: l=(Q[1]-P[1])*pow(Q[0]-P[0],p-2,p)%p
    x=(l*l-P[0]-Q[0])%p; return (x,(l*(P[0]-x)-P[1])%p)
def mul(P,k):
    R=None
    for i in range(256):
        if (k>>i)&1: R=add(R,P)
        P=add(P,P)
    return R
def th(tag,m): t=hashlib.sha256(tag.encode()).digest(); return hashlib.sha256(t+t+m).digest()
def lift(x):
    c=(pow(x,3,p)+7)%p; y=pow(c,(p+1)//4,p)
    if pow(y,2,p)!=c: return None
    return (x, y if y%2==0 else p-y)
def verify(pk,m,sig):
    P=lift(int.from_bytes(pk,'big')); r=int.from_bytes(sig[:32],'big'); s=int.from_bytes(sig[32:],'big')
    if P is None or r>=p or s>=n: return False
    e=int.from_bytes(th("BIP0340/challenge",sig[:32]+pk+m),'big')%n
    R=add(mul(G,s),mul(P,n-e))
    return R is not None and R[1]%2==0 and R[0]==r
d=json.load(open(sys.argv[1])); keys=[bytes.fromhex(k) for k in d['public_keys'].split(',')]
for k,sec in zip(keys,[d['secret'],d['second_secret']]):
    P=mul(G,int(sec,16)); assert P[0].to_bytes(32,'big')==k
bad = 0
for t in d['tickets']:
    body,sig=t['text'].rsplit('.',1); k=keys[1 if t['signer']=='second' else 0]
    assert t['text'].split('.')[1]==k.hex()[:8]
    ok = verify(k, body.encode(), bytes.fromhex(sig)); print("ok  " if ok else "BAD ", t["why"]); bad += 0 if ok else 1
sys.exit(1 if bad else 0)
