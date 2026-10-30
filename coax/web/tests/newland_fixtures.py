#!/usr/bin/env python3
"""Public throwaway fixtures, signed by an independent bigint BIP-340 oracle.
Test tooling only. Never reads a key or touches a relay.
"""
import copy
import hashlib
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT/'tools/coax-nostr/fuzz'))
from oracle import N, point_mul, tagged, schnorr_verify


def sign(kind, tags, content, at, secret=3):
    point = point_mul(secret)
    pub = point[0].to_bytes(32, 'big')
    d = N-secret if point[1]&1 else secret
    e = dict(pubkey=pub.hex(), created_at=at, kind=kind, tags=tags, content=content)
    message = json.dumps([0,e['pubkey'],at,kind,tags,content],ensure_ascii=False,separators=(',',':')).encode()
    digest = hashlib.sha256(message).digest()
    t = bytes(a^b for a,b in zip(d.to_bytes(32,'big'),tagged('BIP0340/aux',bytes(32))))
    nonce = int.from_bytes(tagged('BIP0340/nonce',t+pub+digest),'big')%N
    rp = point_mul(nonce); k = N-nonce if rp[1]&1 else nonce
    r = rp[0].to_bytes(32,'big')
    challenge = int.from_bytes(tagged('BIP0340/challenge',r+pub+digest),'big')%N
    signature = r+((k+challenge*d)%N).to_bytes(32,'big')
    assert schnorr_verify(digest,pub,signature)
    return dict(e,id=digest.hex(),sig=signature.hex())


def fixtures():
    text=(ROOT/'psionic/lib/pylon/beacon.coax').read_text()
    body=json.loads(json.loads(re.search(r'^PB_CONTENT :: (".*");$',text,re.M)[1]))
    now=2_000_000_000
    body.update(observed_at=now,since=now-200,valid_until=now+300)
    def beacon(value,override=None):
        content=json.dumps(value,sort_keys=True,separators=(',',':'))
        tags=[['d',value['pylon']],['t','oa:pylon-beacon:v1'],['x',hashlib.sha256(content.encode()).hexdigest()],['expiration',str(value['valid_until'])]]
        if override:override(tags)
        return sign(30200,tags,content,value["observed_at"])
    valid=beacon(body)
    variants={}
    for name,change in {
        'provider':lambda v:v.update(provider='0'*64),
        'future':lambda v:v.update(observed_at=now+31,valid_until=now+331),
        'slots':lambda v:v['slots'].update(free=65),
        'window':lambda v:v.update(valid_until=now+301),
    }.items():
        value=copy.deepcopy(body);change(value);variants[name]=beacon(value)
    variants['digest']=beacon(body,lambda tags:tags[2].__setitem__(1,'0'*64))
    variants['expiration']=beacon(body,lambda tags:tags[3].__setitem__(1,str(now+299)))
    request=sign(25910,[['p',body['provider']]],'public test fixture',now-4,7)
    answer=sign(26910,[['p',request['pubkey']],['e',request['id']]],'public test fixture',now)
    status=sign(27010,[['p',request['pubkey']],['e',request['id']]],'opaque public fixture',now)
    wrong=sign(27010,status['tags'],status['content'],now,7)
    older_answer=sign(26910,answer['tags'],answer['content'],now-1)
    updated=copy.deepcopy(body);updated.update(generation=8,observed_at=now+1,valid_until=now+301);updated['slots']['free']=0
    return dict(now=now,beacon=valid,updated_beacon=beacon(updated),invalid_beacons=variants,request=request,answer=answer,older_answer=older_answer,status=status,wrong_author=wrong)


if __name__=='__main__':
    print(json.dumps(fixtures(),separators=(',',':')))
