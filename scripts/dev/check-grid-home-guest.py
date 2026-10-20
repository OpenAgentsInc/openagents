#!/usr/bin/env python3
"""Check free /home replies with a fresh local guest and clean up its chats.

Run only through `openagents browser run -- python3 ...` (a fresh profile).
Requires the preview's real worker; no owner cookies or credentials are used.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import time
import urllib.request
from urllib.parse import urlsplit
import uuid

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--url', default='http://localhost:4331')
parser.add_argument('--out', required=True, type=Path)
parser.add_argument('--timeout', type=float, default=90)
args = parser.parse_args()
origin = args.url.rstrip('/')
assert urlsplit(origin).hostname in ('localhost', '127.0.0.1', '::1'), 'Use a local preview.'
args.out.mkdir(parents=True, exist_ok=True)
root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('browser_cdp', root/'bench/verse/2026-10-05/platform-clients/browser-cdp.py')
module = importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
port = os.environ['OPENAGENTS_CHROME_PORT']
target = json.load(urllib.request.urlopen(urllib.request.Request(f'http://127.0.0.1:{port}/json/new?about:blank', method='PUT')))
cdp = module.Cdp(target['webSocketDebuggerUrl'])
cdp.call('Page.enable')
cdp.call('Page.navigate', {'url': origin+'/home'})
created = []
receipt = {'origin': origin, 'answers': [], 'cleanup': []}


def evaluate(expression):
    result = cdp.eval(expression)
    if 'exceptionDetails' in result:raise RuntimeError(result['exceptionDetails'])
    return result['result'].get('value')


def request(method, path, body=None, home=True):
    # The CSRF value stays inside the browser; receipts contain no cookies or tokens.
    script = """(async(method,path,body,home)=>{
        const headers={"content-type":"application/json","x-openagents-csrf":globalThis.__guestCsrf,"x-openagents-test":"1"};
        if(home)headers["x-openagents-chat-surface"]="home";
        const r=await fetch(path,{method,headers,body:body===null?undefined:JSON.stringify(body)});
        const text=await r.text();let data;try{data=JSON.parse(text)}catch{data=text}
        return {status:r.status,data};
    })"""
    return evaluate(script+'('+','.join(json.dumps(x) for x in (method,path,body,home))+')')


def answer(chat, minimum=2, expected=None):
    deadline=time.monotonic()+args.timeout
    while time.monotonic()<deadline:
        result=request('GET', '/v1/threads/'+chat)
        assert result['status']==200, result
        messages=result['data'].get('messages', [])
        outcome=result['data'].get('thread',{}).get('outcome')
        if outcome in ('failed','unknown','stopped'):
            raise AssertionError({'chat':chat,'outcome':outcome})
        if len(messages)>=minimum and messages[-1].get('role')=='assistant' and outcome=='answered':
            text=messages[-1].get('text','')
            assert text.strip(), 'Empty assistant answer'
            if expected:assert expected in text, text
            receipt['answers'].append({'chat':chat,'messages':len(messages),'outcome':outcome,'nonempty':True})
            return
        time.sleep(.2)
    raise TimeoutError('No answer for this test guest')


def start(text):
    chat=str(uuid.uuid4());created.append(chat)
    deadline=time.monotonic()+10
    while True:
        result=request('POST','/v1/threads',{'request_id':chat,'text':text})
        if result['status'] in (200,201):break
        if result['data'].get('error',{}).get('code')!='busy' or time.monotonic()>=deadline:
            raise AssertionError(result)
        time.sleep(.05)
    assert 'left' not in result['data'], result
    return chat


def reply(chat, text):
    deadline=time.monotonic()+10
    request_id=str(uuid.uuid4())
    while True:
        result=request('POST','/v1/threads/'+chat+'/messages',{'request_id':request_id,'text':text})
        if result['status']==202:return
        if result['data'].get('error',{}).get('code')!='busy' or time.monotonic()>=deadline:
            raise AssertionError(result)
        time.sleep(.05)


try:
    for _ in range(40):
        if evaluate('location.pathname==="/home" && document.readyState!=="loading"'):break
        time.sleep(.1)
    state=evaluate('''fetch('/lander8/session').then(r=>r.json()).then(s=>{globalThis.__guestCsrf=s.csrf;return {signed_in:s.signed_in}})''')
    assert state['signed_in'] is False, 'This check requires a fresh signed-out profile.'
    chat=start('How do OpenAgents pylons help with running coding agents? Give one practical example.')
    answer(chat)
    reply(chat,'How would I connect an existing computer for that workflow?')
    answer(chat,4)
    # Exhaust the old five-conversation policy on this test visitor only.
    for _ in range(4):
        other=start('What is OpenAgents?');answer(other)
    denied=request('POST','/v1/threads',{'request_id':str(uuid.uuid4()),'text':'Old app policy'},home=False)
    assert denied['data'].get('error',{}).get('code')=='sign_in_required', denied
    receipt['old_allowance_exhausted']=True
    fresh=start('How can a pylon help with an OpenAgents coding task? Describe two steps a new user can follow.')
    answer(fresh)
    reply(chat,'For that OpenAgents workflow, how would I decide between using my own computer and a pylon?')
    answer(chat,6)
    receipt['free_after_exhaustion']=True
    print(json.dumps({'free_replies':True,'exhausted_visitor':True,'chats':len(created)}))
finally:
    for chat in created:
        try:
            deadline=time.monotonic()+180
            while True:
                result=request('DELETE','/v1/threads/'+chat)
                if result['status']!=409 or time.monotonic()>=deadline:break
                request('POST','/v1/threads/'+chat+'/stop',{})
                time.sleep(.25)
            gone=request('GET','/v1/threads/'+chat)['status']==404
            receipt['cleanup'].append({'chat':chat,'status':result['status'],'gone':gone})
        except Exception as error:
            receipt['cleanup'].append({'chat':chat,'error':type(error).__name__})
    (args.out/'guest-check.json').write_text(json.dumps(receipt,indent=2)+'\n')
    urllib.request.urlopen(f'http://127.0.0.1:{port}/json/close/'+target['id']).close()
    assert all(r.get('gone') and (200<=r.get('status',0)<300 or r.get('status')==404) for r in receipt['cleanup']), receipt['cleanup']
