#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: raw-block replay supplies exact balances for the real browser and durable branch changes)
"""Desktop/mobile explorer journeys on the API and address-index writer."""
import argparse
import importlib.util
from pathlib import Path
import re
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('address_oracle',Path(__file__).with_name('addrindex.py'))
address=importlib.util.module_from_spec(spec);spec.loader.exec_module(address)

def check(api,index):
    with tempfile.TemporaryDirectory(prefix='bitcoin-explorer-browser-') as temporary:
        directory=Path(temporary);prefix,branches,scripts=address.fixture()
        (directory/'headers').write_bytes(b''.join(b[:80] for b in prefix+branches))
        for i,b in enumerate(prefix+branches):(directory/str(i)).write_bytes(b)
        state=directory/'state';store=directory/'blocks'
        def writer(*args):
            r=subprocess.run([str(index),*map(str,args)],cwd=ROOT,text=True,capture_output=True,timeout=120)
            assert r.returncode==0,(r.stdout,r.stderr)
        writer('init','regtest',state)
        writer('apply','regtest',state,store,directory/'headers',2000000000,*[directory/str(i) for i in range(1,103)])
        proc=subprocess.Popen([str(api),'regtest',str(state),str(store),'0','0',str(ROOT/'bitcoin/explorer')],cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        try:
            line=proc.stdout.readline();match=re.search(r'http://127\.0\.0\.1:(\d+)',line);assert match,(line,proc.poll())
            balances=[sum(r[4] for r in address.expected(active)[0].get(scripts[1],[]) if r[6]) for active in (prefix[1:]+branches[:2],prefix[1:]+branches[2:5],prefix[1:]+branches[:2]+branches[5:])]
            assert balances[0]>0 and balances[1]==0 and balances[2]==balances[0]
            args=['coax/web/tool.sh','bitcoin/explorer/check.coax',match[0]+'/',str(directory),str(index),address.address(scripts[1]),address.tx.transactions(branches[0])[1]['id'][::-1].hex(),address.tx.digest(branches[0][:80])[::-1].hex(),*map(str,balances)]
            r=subprocess.run(args,cwd=ROOT,text=True,capture_output=True,timeout=840)
            assert r.returncode==0,(r.stdout,r.stderr)
            print(r.stdout+r.stderr,end='')
            destination=ROOT/'bitcoin/explorer/build/verification';destination.mkdir(parents=True,exist_ok=True)
            for name in ('desktop.png','mobile.png'):(destination/name).write_bytes((directory/name).read_bytes())
            print('Independent raw-block balances:',balances)
        finally:
            proc.terminate()
            try:_,stderr=proc.communicate(timeout=10)
            except subprocess.TimeoutExpired:proc.kill();_,stderr=proc.communicate(timeout=10)
            assert 'runtime error:' not in stderr and 'ERROR: AddressSanitizer' not in stderr,stderr

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--api-tool',type=Path);p.add_argument('--index-tool',type=Path);p.add_argument('--sanitize-with',type=Path);a=p.parse_args()
    if a.sanitize_with:
        spec=importlib.util.spec_from_file_location('http_oracle',Path(__file__).with_name('explorer.py'));http=importlib.util.module_from_spec(spec);spec.loader.exec_module(http);http.sanitized(a.sanitize_with.resolve(),check)
    elif a.api_tool and a.index_tool:check(a.api_tool.resolve(),a.index_tool.resolve())
    else:p.error('pass --api-tool and --index-tool, or --sanitize-with')
