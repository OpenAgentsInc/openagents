#!/usr/bin/env python3
"""Explicit test-only fixture generation; never part of an app/compiler build."""
import copy
import hashlib
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
PUBLIC='79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798'
def canonical(event):
    return json.dumps([0,event['pubkey'],event['created_at'],event['kind'],event['tags'],event['content']],ensure_ascii=False,separators=(',',':')).encode()
def fixture():
    e={'pubkey':PUBLIC,'created_at':0,'kind':1,'tags':[[],['x','é'],['x','/\n']],'content':'a"\\\n\r\t\b\f\x00é😀/','sig':'ff'*64}
    e['id']=hashlib.sha256(canonical(e)).hexdigest()
    return e
def encoded(event):return json.dumps(event,ensure_ascii=True,separators=(',',':'))
def coax_literal(value):return json.dumps(value,ensure_ascii=False)
def generate():
    e=fixture();text=encoded(e);preimage=canonical(e).decode()
    lines=['// Derived public fixtures: Python standard JSON + SHA-256. Explicit test tooling.','// event.py independently compares ECMAScript JSON.stringify preimages and hashes.']
    for key,value in [('NE_FIXTURE',text),('NE_PREIMAGE',preimage),('NE_FIXTURE_ID',e['id'])]:lines.append(key+' :: '+coax_literal(value)+';')
    empty=copy.deepcopy(e);empty.update(kind=0,tags=[],content='');empty['id']=hashlib.sha256(canonical(empty)).hexdigest()
    lines.append('NE_EMPTY :: '+coax_literal(encoded(empty))+';')
    lines.append('NE_EMPTY_PREIMAGE :: '+coax_literal(canonical(empty).decode())+';')
    maximum=copy.deepcopy(empty);maximum.update(created_at=9007199254740991,kind=65535);maximum['id']=hashlib.sha256(canonical(maximum)).hexdigest()
    lines.append('NE_MAXIMUM :: '+coax_literal(encoded(maximum))+';')
    lines.append('NE_MAXIMUM_PREIMAGE :: '+coax_literal(canonical(maximum).decode())+';')
    wrong=copy.deepcopy(e);wrong['id']='0'*64;lines.append('NE_WRONG_ID :: '+coax_literal(encoded(wrong))+';')
    unknown=copy.deepcopy(e);unknown['extra']={'z':[False,None,{'x':'retained'}]}
    lines.append('NE_UNKNOWN :: '+coax_literal(json.dumps(unknown,ensure_ascii=False,indent=1))+';')
    bad=[]
    for name in ['id','pubkey','created_at','kind','tags','content','sig']:
        v=copy.deepcopy(e);del v[name];bad.append((encoded(v),'NE_SHAPE'))
    for name in ['id','pubkey','sig']:
        for value in ['',e[name][:-1],e[name]+'0','g'+e[name][1:]]:
            v=copy.deepcopy(e);v[name]=value;bad.append((encoded(v),'NE_HEX'))
        v=copy.deepcopy(e);v[name]=e[name].upper();bad.append((encoded(v),'NE_HEX'))
    for name,value,error in [('created_at',-1,'NE_SHAPE'),('created_at',9007199254740992,'NE_JSON'),('created_at',1.0,'NE_JSON'),('kind',65536,'NE_SHAPE'),('kind',-1,'NE_SHAPE'),('kind','1','NE_JSON'),('content',None,'NE_SHAPE'),('tags',{},'NE_SHAPE'),('tags',[1],'NE_SHAPE'),('tags',[[None]],'NE_SHAPE'),('tags',[[[]]],'NE_SHAPE'),('tags',[['x']*17],'NE_SHAPE'),('id',None,'NE_SHAPE')]:
        v=copy.deepcopy(e);v[name]=value;bad.append((encoded(v),error))
    for suffix in [',"x":1,"\\u0078":2}',',"unknown":"\\ud800"}',',"unknown":NaN}']:
        bad.append((text[:-1]+suffix,'NE_JSON'))
    bad.extend([('[]','NE_SHAPE'),('{','NE_JSON'),(text+' false','NE_JSON')])
    lines.append('NE_BAD_EVENTS :: ['+str(len(bad))+']string.['+','.join(coax_literal(x) for x,_ in bad)+'];')
    lines.append('NE_BAD_ERRORS :: ['+str(len(bad))+']int.['+','.join(x for _,x in bad)+'];')
    return '\n'.join(lines)+'\n'
if __name__=='__main__':
    (ROOT/'coax/tests/nostr/event_fixtures.coax').write_text(generate())
