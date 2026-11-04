#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Bitcoin Core and Electrs supply a fixed-chain reference; Python parses raw transactions and derives exact histories, balances and spend links before comparing the own-node HTTP API)
"""Record a fixed Core/Electrs snapshot, then compare without either backend."""
import argparse
import base64
from decimal import Decimal
import hashlib
import importlib.util
import json
import re
from pathlib import Path
import socket
import struct
import urllib.request

ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location("tx_oracle",Path(__file__).with_name("txindex.py"))
tx=importlib.util.module_from_spec(spec);spec.loader.exec_module(tx)


class Core:
    def __init__(self,url,cookie):
        self.url=url;self.auth="Basic "+base64.b64encode(cookie.read_bytes().strip()).decode();self.calls=0
    def call(self,method,*params):
        self.calls+=1
        body=json.dumps(dict(jsonrpc="2.0",id=self.calls,method=method,params=params)).encode()
        request=urllib.request.Request(self.url,data=body,headers={"Authorization":self.auth,"Content-Type":"application/json"})
        with urllib.request.urlopen(request,timeout=60) as response: result=json.loads(response.read(),parse_float=Decimal)
        assert not result.get("error"),(method,result.get("error"))
        return result["result"]


class ReferenceRpcError(RuntimeError):
    def __init__(self,method,error):self.method,self.error=method,error;super().__init__(method,error)


class Electrs:
    def __init__(self,address):
        host,port=address.rsplit(":",1);self.socket=socket.create_connection((host,int(port)),timeout=30);self.file=self.socket.makefile("rb");self.calls=0
    def call(self,method,*params):
        self.calls+=1;ident=self.calls
        self.socket.sendall(json.dumps(dict(jsonrpc="2.0",id=ident,method=method,params=params)).encode()+b"\n")
        while True:
            raw=self.file.readline(16*1024*1024);assert raw and raw.endswith(b"\n"),"bounded Electrs response missing"
            response=json.loads(raw)
            if response.get("id")==ident:
                if response.get("error"):raise ReferenceRpcError(method,response["error"])
                return response["result"]


def record(core,electrs,extra_heights=()):
    version=electrs.call("server.version","coax-explorer-reference","1.4")
    chain=core.call("getblockchaininfo");height=chain["blocks"];tip=chain["bestblockhash"]
    remote=electrs.call("blockchain.headers.subscribe")
    assert remote["height"]==height and tx.digest(bytes.fromhex(remote["hex"]))[::-1].hex()==tip,"references must share one frozen chain"
    snapshot=dict(chain=chain["chain"],height=height,hash=tip,profile="pow-only")
    hashes={};block_ids={};raw_cache={};candidates={};transactions={};blocks=[]
    def block_hash(h):
        if h not in hashes:hashes[h]=core.call("getblockhash",h)
        return hashes[h]
    def order(h,ident):
        if h not in block_ids:block_ids[h]=core.call("getblock",block_hash(h),1)["tx"]
        return block_ids[h].index(ident)
    def raw_transaction(ident):
        if ident not in raw_cache:
            raw=bytes.fromhex(electrs.call("blockchain.transaction.get",ident,False));parsed=tx.Reader(raw).tx()
            assert parsed["n"]==len(raw) and parsed["id"][::-1].hex()==ident
            raw_cache[ident]=(raw,parsed)
        return raw_cache[ident]
    heights=sorted({0,1,min(100000,height),max(1,height-100),max(1,height-2),height,*extra_heights})
    for h in heights:
        ident=block_hash(h);raw=bytes.fromhex(core.call("getblock",ident,0));parsed=tx.transactions(raw)
        assert tx.digest(raw[:80])[::-1].hex()==ident and tx.merkle(t["id"] for t in parsed)==raw[36:68]
        ids=[t["id"][::-1].hex() for t in parsed];block_ids[h]=ids
        blocks.append(dict(snapshot=snapshot,hash=ident,height=h,previous=raw[4:36][::-1].hex(),time=struct.unpack_from("<I",raw,68)[0],size=len(raw),active=True,confirmations=height-h+1,txids=ids))
        for ordinal in sorted({0,min(1,len(parsed)-1),len(parsed)//2,len(parsed)-1}):
            t=parsed[ordinal]
            if len(t["outs"])>64 or len(t["ins"])>64:continue
            key=t["id"][::-1].hex();decoded=core.call("decoderawtransaction",t["raw"].hex())
            assert decoded["txid"]==key and decoded["hash"]==tx.digest(t["raw"])[::-1].hex()
            raw_cache[key]=(t["raw"],t);outs=[]
            for vout,(value,script) in enumerate(t["outs"]):
                meta=decoded["vout"][vout]["scriptPubKey"];assert meta["hex"]==script.hex()
                unspendable=h==0 or len(script)>10000 or script[:1]==b"\x6a"
                coin=None if unspendable else core.call("gettxout",key,vout,False)
                status="unspendable" if unspendable else ("unspent" if coin else "spent")
                if coin:assert int(coin["value"]*100000000)==value and coin["scriptPubKey"]["hex"]==script.hex()
                row=dict(vout=vout,value_sat=value,script_hex=script.hex(),status=status)
                if "address" in meta:
                    row["address"]=meta["address"]
                    if ordinal:candidates.setdefault(meta["address"],script)
                outs.append(row)
            transactions[key]=dict(snapshot=snapshot,occurrence=dict(txid=key,block=ident,height=h,order=ordinal),wtxid=decoded["hash"],raw=t["raw"].hex(),active=True,confirmations=height-h+1,inputs=[dict(txid=p[:32][::-1].hex(),vout=int.from_bytes(p[32:],"little"),coinbase=len(t["ins"])==1 and p==bytes(32)+b"\xff"*4) for p in t["ins"]],outputs=outs)
    addresses=[];spent_count=0;skipped_busy=[]
    # Mix old and recent outputs, including an empty balance and live UTXOs.
    selected=list(candidates.items());selected=selected[-12:]+selected[:12]
    for address,script in selected:
        if any(row["address"]==address for row in addresses):continue
        sh=hashlib.sha256(script).digest()[::-1].hex()
        try:history=[row for row in electrs.call("blockchain.scripthash.get_history",sh) if row["height"]>0]
        except ReferenceRpcError as error:
            if error.error.get("code")!=10001 and not re.fullmatch(r">[0-9]+ index entries, query may take too long",error.error.get("message","")):raise
            skipped_busy.append(address);continue
        if not 1<=len(history)<=80:continue
        created={};parsed_history={};occurrences=[]
        for entry in history:
            key=entry["tx_hash"];h=entry["height"];raw,t=raw_transaction(key);ordinal=order(h,key)
            occurrence=dict(txid=key,block=block_hash(h),height=h,order=ordinal);occurrences.append(occurrence);parsed_history[key]=(h,t)
            for vout,(value,actual) in enumerate(t["outs"]):
                if actual==script:
                    point=t["id"]+struct.pack("<I",vout);assert point not in created
                    created[point]=dict(occurrence=occurrence,output=dict(vout=vout,value_sat=value,script_hex=script.hex(),address=address,status="unspent"))
        for key,(h,t) in parsed_history.items():
            for vin,point in enumerate(t["ins"]):
                if point in created:
                    out=created[point]["output"];assert out["status"]=="unspent"
                    out["status"]="spent";out["spend"]=dict(txid=key,block=block_hash(h),vin=vin,height=h);spent_count+=1
        if len(created)>200:continue
        balance=sum(r["output"]["value_sat"] for r in created.values() if r["output"]["status"]=="unspent")
        assert electrs.call("blockchain.scripthash.get_balance",sh)["confirmed"]==balance
        unspent=electrs.call("blockchain.scripthash.listunspent",sh)
        want={(r["occurrence"]["txid"],r["output"]["vout"],r["output"]["value_sat"]) for r in created.values() if r["output"]["status"]=="unspent"}
        assert {(r["tx_hash"],r["tx_pos"],r["value"]) for r in unspent if r["height"]>0}==want
        for row in created.values():
            coin=core.call("gettxout",row["occurrence"]["txid"],row["output"]["vout"],False)
            assert bool(coin)==(row["output"]["status"]=="unspent")
            if coin:assert int(coin["value"]*100000000)==row["output"]["value_sat"]
        addresses.append(dict(snapshot=snapshot,address=address,script_hex=script.hex(),balance_sat=balance,transactions=occurrences,outputs=list(created.values())))
        if len(addresses)>=8 and any(r["balance_sat"] for r in addresses) and any(not r["balance_sat"] for r in addresses):break
    spent_count=sum(r["output"]["status"]=="spent" for a in addresses for r in a["outputs"])
    assert len(addresses)>=3 and spent_count>0 and any(r["balance_sat"] for r in addresses),"reference samples need spends and live outputs"
    assert core.call("getblockchaininfo")["bestblockhash"]==tip and electrs.call("blockchain.headers.subscribe")["height"]==height,"reference moved during recording"
    return dict(schema=1,snapshot=snapshot,core=core.call("getnetworkinfo")["subversion"],electrs=version,blocks=blocks,transactions=list(transactions.values()),addresses=addresses,reference_calls=dict(core=core.calls,electrs=electrs.calls),skipped_busy=skipped_busy)


def compare(api,fixture):
    count=0
    def request(path):
        nonlocal count
        with urllib.request.urlopen(api.rstrip("/")+"/api/v1/"+path,timeout=60) as r: result=json.load(r)
        count+=1;return result
    snap=fixture["snapshot"];assert request("tip")==dict(snapshot=snap)
    for expected in fixture["blocks"]:
        for key in (str(expected["height"]),expected["hash"]):
            assert request("block/"+key)==expected,("block",key)
            assert request("search/"+key)==dict(snapshot=snap,kind="block",key=expected["hash"])
    for expected in fixture["transactions"]:
        key=expected["occurrence"]["txid"];got=request("transaction/"+key)
        for field,value in expected.items():
            if field=="outputs":
                assert len(got[field])==len(value)
                for actual,want in zip(got[field],value):
                    for name,v in want.items():assert actual[name]==v,("transaction output",key,name)
            else:assert got[field]==value,("transaction",key,field)
        assert request("search/"+key)==dict(snapshot=snap,kind="transaction",key=key)
    def normalized(row):
        row=dict(row);row["transactions"]=sorted(row["transactions"],key=lambda r:(r["height"],r["order"],r["txid"]))
        row["outputs"]=sorted(row["outputs"],key=lambda r:(r["occurrence"]["txid"],r["occurrence"]["block"],r["output"]["vout"]))
        return row
    for expected in fixture["addresses"]:
        key=expected["address"];got=request("address/"+key)
        assert normalized(got)==normalized(expected),("address",key)
        assert request("search/"+key)==dict(snapshot=snap,kind="address",key=key)
    print(f"Core/Electrs comparison: {count} HTTP answers match at {snap['chain']} height {snap['height']}; {len(fixture['addresses'])} full address histories/balances/spend sets",flush=True)


if __name__=="__main__":
    p=argparse.ArgumentParser();p.add_argument("--api");p.add_argument("--fixture",type=Path);p.add_argument("--record",type=Path);p.add_argument("--cookie",type=Path);p.add_argument("--core-url",default="http://127.0.0.1:38418");p.add_argument("--electrs",default="127.0.0.1:38420");a=p.parse_args()
    if a.record:
        assert a.cookie,"recording requires the reference cookie path"
        f=record(Core(a.core_url,a.cookie),Electrs(a.electrs));a.record.write_text(json.dumps(f,indent=1)+"\n")
        print(f"Recorded independent {f['snapshot']['chain']} references at height {f['snapshot']['height']}; {f['reference_calls']}",flush=True)
    else:
        assert a.fixture and a.api,"comparison requires a fixture and own-node API"
        f=json.loads(a.fixture.read_text())
    if a.api:compare(a.api,f)
