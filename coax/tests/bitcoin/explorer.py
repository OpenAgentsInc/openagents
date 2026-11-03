#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: raw-block replay computes every HTTP/RPC block, transaction, balance and spend answer, including both reorg directions and corrupt storage)
"""Explorer HTTP v1 against independently parsed durable chain fixtures."""
import argparse
import importlib.util
import json
from pathlib import Path
import re
import socket
import struct
import subprocess
import tempfile
import urllib.error
import urllib.request

ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location("address_oracle",Path(__file__).with_name("addrindex.py"))
address=importlib.util.module_from_spec(spec);spec.loader.exec_module(address)
tx=address.tx


def check(api,index):
    with tempfile.TemporaryDirectory(prefix="coax-explorer-http-") as temporary:
        directory=Path(temporary);prefix,branches,scripts=address.fixture();blocks=prefix+branches
        headers=directory/"headers";headers.write_bytes(b"".join(b[:80] for b in blocks)); paths=[]
        for i,b in enumerate(blocks):p=directory/str(i);p.write_bytes(b);paths.append(p)
        state=directory/"state";store=directory/"blocks";active=[];archived={};queries=0

        def command(*args):
            return subprocess.run([str(index),*map(str,args)],cwd=ROOT,text=True,capture_output=True,check=True,timeout=120).stdout

        command("init","regtest",state)
        proc=subprocess.Popen([str(api),"regtest",str(state),str(store),"0"],cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        try:
            line=proc.stdout.readline();match=re.search(r"http://127\.0\.0\.1:(\d+)",line);assert match,(line,proc.poll())
            base=f"http://127.0.0.1:{match[1]}/api/v1/"

            def request(path,data=None,status=200,raw=False):
                nonlocal queries
                req=urllib.request.Request(base+path,data=data,headers={"Content-Type":"application/json"} if data else {})
                try:r=urllib.request.urlopen(req,timeout=15)
                except urllib.error.HTTPError as e:r=e
                with r:
                    payload=r.read();assert r.status==status,(path,r.status,payload)
                    assert r.headers["Cache-Control"]=="no-store" and r.headers["Connection"]=="close"
                    if r.status==204:assert "Content-Length" not in r.headers and not payload
                    else:assert int(r.headers["Content-Length"])==len(payload)
                    queries+=1
                    return payload if raw else json.loads(payload)

            def snapshot():
                return dict(chain="regtest",height=len(active),hash=tx.digest((active[-1] if active else prefix[0])[:80])[::-1].hex(),profile="pow-only")

            def block_answer(b):
                h=tx.digest(b[:80]);height=0 if b==prefix[0] else next(i for i,x in enumerate(prefix+branches) if x==b)
                if b in branches:
                    heights={tx.digest(x[:80]):i for i,x in enumerate(prefix)}
                    for x in branches:heights[tx.digest(x[:80])]=heights[x[4:36]]+1
                    height=heights[h]
                is_active=b==prefix[0] or b in active
                return dict(snapshot=snapshot(),hash=h[::-1].hex(),height=height,previous=b[4:36][::-1].hex(),time=struct.unpack_from("<I",b,68)[0],size=len(b),active=is_active,confirmations=len(active)-height+1 if is_active else 0,txids=[t["id"][::-1].hex() for t in tx.transactions(b)])

            def get_block(b,by_height=False):
                want=block_answer(b);key=str(want["height"]) if by_height else want["hash"]
                assert request("block/"+key)==want

            def out_answer(t,vout,block,height):
                value,script=t["outs"][vout];result=dict(vout=vout,value_sat=value,script_hex=script.hex())
                if script in scripts[:3]:result["address"]=address.address(script)
                if block!=prefix[0] and block not in active:result["status"]="stale";return result
                if block==prefix[0] or script[:1]==b"\x6a" or len(script)>10000:result["status"]="unspendable";return result
                model,_=address.expected(active);point=t["id"]+struct.pack("<I",vout);h=tx.digest(block[:80])
                row,=[r for r in model[script] if r[0]==point and r[1]==h]
                spend,unspent=row[5:];result["status"]="unspent" if unspent else ("spent" if spend else "replaced")
                if spend:result["spend"]=dict(txid=spend[0][::-1].hex(),block=spend[1][::-1].hex(),vin=spend[2],height=spend[3])
                return result

            def get_tx(txid):
                candidates=[]
                for sequence,b in enumerate([prefix[0],*archived.values()]):
                    for order,t in enumerate(tx.transactions(b)):
                        if t["id"]==txid:candidates.append((b==prefix[0] or b in active,sequence,b,order,t))
                is_active,_,b,order,t=max(candidates,key=lambda x:x[:2]);info=block_answer(b);height=info["height"]
                want=dict(snapshot=snapshot(),occurrence=dict(txid=txid[::-1].hex(),block=info["hash"],height=height,order=order),wtxid=tx.digest(t["raw"])[::-1].hex(),raw=t["raw"].hex(),active=is_active,confirmations=info["confirmations"],inputs=[dict(txid=p[:32][::-1].hex(),vout=int.from_bytes(p[32:],"little"),coinbase=len(t["ins"])==1 and p==bytes(32)+b"\xff"*4) for p in t["ins"]],outputs=[out_answer(t,i,b,height) for i in range(len(t["outs"]))])
                got=request("transaction/"+txid[::-1].hex());assert got==want,(got,want)
                return got

            def get_addresses():
                model,hist=address.expected(active)
                for script in scripts[:3]:
                    key=address.address(script);got=request("address/"+key)
                    assert got["snapshot"]==snapshot() and got["address"]==key and got["script_hex"]==script.hex()
                    rows=model.get(script,[]);assert got["balance_sat"]==sum(r[4] for r in rows if r[6])
                    assert {(x["txid"],x["block"],x["height"],x["order"]) for x in got["transactions"]} == {(i[::-1].hex(),b[::-1].hex(),h,o) for i,b,h,o in hist.get(script,set())}
                    expected_rows=[]
                    for point,h,height,order,value,spend,unspent in rows:
                        block=next(b for b in active if tx.digest(b[:80])==h);t=next(t for t in tx.transactions(block) if t["id"]==point[:32]);vout=int.from_bytes(point[32:],"little")
                        expected_rows.append(dict(occurrence=dict(txid=point[:32][::-1].hex(),block=h[::-1].hex(),height=height,order=order),output=out_answer(t,vout,block,height)))
                    sort=lambda row:(row["occurrence"]["txid"],row["occurrence"]["block"],row["output"]["vout"])
                    assert sorted(got["outputs"],key=sort)==sorted(expected_rows,key=sort)
                    assert request("search/"+key)==dict(snapshot=snapshot(),kind="address",key=key)

            def apply(indices,new_active):
                nonlocal active
                command("apply","regtest",state,store,headers,2000000000,*[paths[i] for i in indices])
                for i in indices:archived.setdefault(tx.digest(blocks[i][:80]),blocks[i])
                active=list(new_active)
                assert request("tip")==dict(snapshot=snapshot());get_addresses()

            get_block(prefix[0],True);get_tx(tx.transactions(prefix[0])[0]["id"]);get_addresses()
            apply(range(1,101),prefix[1:]);get_block(prefix[50],True)
            apply([101,102],prefix[1:]+branches[:2]);saved=state.read_bytes()
            pay=tx.transactions(branches[0])[1]["id"];onward=tx.transactions(branches[0])[2]["id"]
            get_tx(pay);get_tx(onward);get_block(branches[0],True)
            assert request("search/"+pay[::-1].hex().upper())==dict(snapshot=snapshot(),kind="transaction",key=pay[::-1].hex())
            assert request("search/101")==dict(snapshot=snapshot(),kind="block",key=tx.digest(branches[0][:80])[::-1].hex())
            apply([103,104,105],prefix[1:]+branches[2:5]);get_tx(pay);get_tx(onward);get_block(branches[0])
            complete=state.read_bytes();state.write_bytes(complete[:-1]);active=prefix[1:]+branches[:2]
            assert request("tip")==dict(snapshot=snapshot());get_addresses();state.write_bytes(complete);active=prefix[1:]+branches[2:5]
            apply([101,102,106,107],prefix[1:]+branches[:2]+branches[5:]);get_tx(pay);get_tx(onward)
            for method,key,path in [("tip",None,"tip"),("block","101","block/101"),("transaction",pay[::-1].hex(),"transaction/"+pay[::-1].hex()),("address",address.address(scripts[0]),"address/"+address.address(scripts[0])),("search","101","search/101")]:
                body=json.dumps(dict(jsonrpc="2.0",id='a"b',method="explorer."+method,params={} if key is None else dict(key=key))).encode()
                assert request("rpc",body)==dict(jsonrpc="2.0",id='a"b',result=request(path))
            for body,code in [(b"{",-32700),(b"{}",-32600),(b'{"jsonrpc":"2.0","id":1,"method":"getblock","params":{}}',-32601),(b'{"jsonrpc":"2.0","id":1,"method":"explorer.block","params":{}}',-32602)]:
                assert request("rpc",body)["error"]["code"]==code
            assert request("rpc",b'{"jsonrpc":"2.0","method":"explorer.tip"}',status=204,raw=True)==b""
            request("block/2147483647",status=404);request("transaction/"+"11"*32,status=404);request("address/not-an-address",status=400)
            original=store.read_bytes();damaged=bytearray(original);damaged[8]^=1;store.write_bytes(damaged)
            request("block/1",status=503);store.write_bytes(original)
            original_state=state.read_bytes();state.unlink();request("tip",status=503);state.write_bytes(original_state);assert request("tip")==dict(snapshot=snapshot())
            with socket.create_connection(("127.0.0.1",int(match[1])),timeout=5) as stream:
                stream.sendall(b"GET /api/v1/tip HTTP/1.1\r\nHost:x\r\nHost:y\r\n\r\n");assert stream.recv(4096).startswith(b"HTTP/1.1 400 ")
            print(f"explorer HTTP/RPC: {queries} independent response comparisons; same-block spends, active/stale lookups, both reorgs, torn state and corruption refusals pass",flush=True)
        finally:
            proc.terminate()
            try:_,stderr=proc.communicate(timeout=10)
            except subprocess.TimeoutExpired:proc.kill();_,stderr=proc.communicate(timeout=10)
            assert "runtime error:" not in stderr and "ERROR: AddressSanitizer" not in stderr,stderr


def sanitized(compiler):
    with tempfile.TemporaryDirectory(prefix="coax-explorer-sanitized-") as temporary:
        directory=Path(temporary);platform=ROOT/"coax/platform";policy=directory/"entropy.c";header=directory/"entropy-policy.h"
        policy.write_text(address.oracle_command([compiler,"c","coax/lib/crypto/entropy.coax","--library","--header",header]))
        binaries=[]
        for name in ("explorer-api","address-index"):
            source=directory/(name+".c");binary=directory/name
            source.write_text(address.oracle_command([compiler,"c",f"bitcoin/examples/live/{name}.coax","--library"]))
            address.oracle_command(["cc","-std=c11","-O1","-g","-fsanitize=address,undefined","-fno-omit-frame-pointer","-D_DARWIN_C_SOURCE","-I"+str(directory),"-I"+str(platform),source,policy,*[platform/n for n in ("tool-native.c","process-native.c","store-file-native.c","read-file-native.c","socket-native.c","entropy-native.c","entropy-foreign.c")],"-framework","Security","-lm","-o",binary])
            binaries.append(binary)
        check(*binaries)
        print("explorer HTTP/RPC: API and index writer ASan/UBSan pass",flush=True)


if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("--api-tool");parser.add_argument("--index-tool");parser.add_argument("--sanitize-with");args=parser.parse_args()
    if args.sanitize_with:sanitized(Path(args.sanitize_with).resolve())
    elif args.api_tool and args.index_tool:check(Path(args.api_tool).resolve(),Path(args.index_tool).resolve())
    else:parser.error("pass --api-tool and --index-tool, or --sanitize-with")
