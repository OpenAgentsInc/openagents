#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Core mines and validates secretless P2SH spends on isolated regtest branches; Electrs independently indexes each branch, and their public reference fixtures qualify the own-node explorer)
"""Generate real Core/Electrs references for both balance-changing reorgs."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess
import time

spec=importlib.util.spec_from_file_location("reference",Path(__file__).with_name("explorer-reference.py"))
ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)


def generate(a):
    a.out.mkdir(parents=True,exist_ok=True);core=ref.Core(a.core_url,a.cookie);electrs=ref.Electrs(a.electrs)
    assert core.call("getblockchaininfo")["chain"]=="regtest"
    initial=core.call("getblockcount");assert initial==0 or (a.resume_a and initial==102),"requires fresh regtest or its recorded first branch"
    redeems=[bytes.fromhex(x) for x in ("51","517551","525193","535193")]
    addresses=[core.call("decodescript",r.hex())["p2sh"] for r in redeems]
    scripts=[bytes.fromhex(core.call("validateaddress",address)["scriptPubKey"]) for address in addresses]
    prefix=core.call("generatetoaddress",100,addresses[0]) if initial==0 else [core.call("getblockhash",h) for h in range(1,101)];coin=core.call("getblock",prefix[0],1)["tx"][0]
    point=bytes.fromhex(coin)[::-1]+bytes(4)
    def sig(r):return bytes([len(r)])+r
    pay=ref.tx.light.transaction([(point,sig(redeems[0]))],[(2000000000,scripts[1]),(1000000000,scripts[2]),(1999999000,scripts[3])])
    next_point=ref.tx.digest(pay)+bytes(4)
    onward=ref.tx.light.transaction([(next_point,sig(redeems[1]))],[(1999998000,scripts[2])])
    other=ref.tx.light.transaction([(next_point,sig(redeems[1]))],[(1999997000,scripts[3])])
    raw_blocks={};references=[]
    def capture(label,height):
        deadline=time.monotonic()+60
        while True:
            tip=electrs.call("blockchain.headers.subscribe")
            if tip["height"]==height and ref.tx.digest(bytes.fromhex(tip["hex"]))[::-1].hex()==core.call("getbestblockhash"):break
            assert time.monotonic()<deadline,"Electrs did not follow the controlled reorg";time.sleep(.1)
        expected=json.loads((a.out/"reference-a.json").read_text()) if label=="a" and initial==102 else ref.record(core,electrs,(101,))
        assert expected["snapshot"]["height"]==height and expected["snapshot"]["hash"]==core.call("getbestblockhash")
        references.append(expected)
        (a.out/f"reference-{label}.json").write_text(json.dumps(expected,indent=1)+"\n")
        for h in range(height+1):
            ident=core.call("getblockhash",h)
            if ident not in raw_blocks:raw_blocks[ident]=bytes.fromhex(core.call("getblock",ident,0))
        headers=a.out/f"headers-{label}"
        p=subprocess.run([str(a.sync),"--chain","regtest","--now",str(int(time.time())),"--peer",a.peer,"--blocks","1","--headers-out",str(headers),"--seconds","60"],capture_output=True,text=True,timeout=120)
        assert p.returncode==0,(p.returncode,p.stdout,p.stderr)
        assert len(headers.read_bytes())==(height+1)*80
    if initial==0:
        a101=core.call("generateblock",addresses[0],[pay.hex(),onward.hex()])["hash"]
        assert len(core.call("getblock",a101,1)["tx"])==3
        core.call("generatetoaddress",1,addresses[0])
    else:a101=core.call("getblockhash",101)
    capture("a",102)
    core.call("invalidateblock",a101)
    b101=core.call("generateblock",addresses[0],[pay.hex()])["hash"]
    assert len(core.call("getblock",b101,1)["tx"])==2
    core.call("generateblock",addresses[0],[other.hex()]);core.call("generatetoaddress",1,addresses[0]);capture("b",103)
    core.call("invalidateblock",b101);core.call("reconsiderblock",a101)
    assert core.call("getblockcount")==102
    core.call("generatetoaddress",2,addresses[0]);capture("c",104)
    balances=[next(x["balance_sat"] for x in r["addresses"] if x["address"]==addresses[2]) for r in references]
    assert balances==[2999998000,1000000000,2999998000],balances
    # The own node reads this durable, independent import file. Neither
    # Core nor Electrs participates in its subsequent query path.
    with (a.out/"blk00000.dat").open("wb") as f:
        for block in raw_blocks.values():f.write(bytes.fromhex("fabfb5da")+struct.pack("<I",len(block))+block)
        f.flush();os.fsync(f.fileno())
    (a.out/"headers").write_bytes((a.out/"headers-a").read_bytes())
    summary=dict(chain="regtest",heights=[102,103,104],balances=balances,address=addresses[2],txid=ref.tx.digest(pay)[::-1].hex(),hash=a101,blocks=len(raw_blocks),core=references[0]["core"],electrs=references[0]["electrs"])
    (a.out/"journey.json").write_text(json.dumps(summary,indent=1)+"\n")
    print(f"Core/Electrs validated regtest references: {len(raw_blocks)} blocks; {balances} through both reorgs",flush=True)


if __name__=="__main__":
    p=argparse.ArgumentParser();p.add_argument("--resume-a",action="store_true");p.add_argument("--core-url",default="http://127.0.0.1:38438");p.add_argument("--cookie",type=Path,required=True);p.add_argument("--electrs",default="127.0.0.1:38440");p.add_argument("--peer",default="127.0.0.1:38439");p.add_argument("--sync",type=Path,required=True);p.add_argument("--out",type=Path,required=True);generate(p.parse_args())
