#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: replays raw Bitcoin transactions into address histories, balances and spend links, and compares the native Coax index and atomic storage frames)
"""Address-index oracle. Reuses only independent fixture/serialization helpers."""
import argparse
import ctypes as C
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("tx_oracle", Path(__file__).with_name("txindex.py"))
tx = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tx)


def oracle_command(args):
    result=subprocess.run(list(map(str,args)),cwd=ROOT,capture_output=True,text=True,timeout=840)
    if result.returncode:
        raise AssertionError((list(map(str,args)),result.returncode,result.stdout,result.stderr))
    return result.stdout


class Spend(C.Structure):
    _fields_ = [(n,C.c_int64) for n in ("code","vin","height","order")]+[("txid",C.c_uint8*32),("block",C.c_uint8*32)]


class Output(C.Structure):
    _fields_ = [(n,C.c_int64) for n in ("height","order","value","vout","status")]+[("txid",C.c_uint8*32),("block",C.c_uint8*32),("spend",Spend)]


class History(C.Structure):
    _fields_ = [("height",C.c_int64),("order",C.c_int64),("txid",C.c_uint8*32),("block",C.c_uint8*32)]


class Result(C.Structure):
    _fields_ = [(n,C.c_int64) for n in ("code","balance","outputs","transactions")]


class Outputs(C.Structure):
    _fields_ = [("data",C.POINTER(Output)),("count",C.c_int64)]


class Histories(C.Structure):
    _fields_ = [("data",C.POINTER(History)),("count",C.c_int64)]


def fixture():
    prefix = list(map(bytes.fromhex,json.loads(tx.BASE_PATH.read_text())["prefix"]))
    a = bytes.fromhex("0014"+"11"*20)
    b = bytes.fromhex("5120"+"22"*32)
    c = bytes.fromhex("76a914"+"33"*20+"88ac")
    coin = tx.transactions(prefix[1])[0]["id"]+bytes(4)
    pay = tx.light.transaction([(coin,b"")],[(2000000000,a),(1000000000,a),(0,b),(1999999000,b"\x51")])
    point = tx.digest(pay)+bytes(4)
    onward = tx.light.transaction([(point,b"")],[(1999998000,b)])
    a101 = tx.mine(tx.digest(prefix[-1][:80]),101,[tx.light.coinbase(101,1),pay,onward])
    a102 = tx.mine(tx.digest(a101[:80]),102,[tx.light.coinbase(102,1)])
    b101 = tx.mine(tx.digest(prefix[-1][:80]),101,[tx.light.coinbase(101,2),pay])
    other = tx.light.transaction([(point,b"")],[(1999997000,c)])
    b102 = tx.mine(tx.digest(b101[:80]),102,[tx.light.coinbase(102,2),other])
    b103 = tx.mine(tx.digest(b102[:80]),103,[tx.light.coinbase(103,2)])
    a103 = tx.mine(tx.digest(a102[:80]),103,[tx.light.coinbase(103,1)])
    a104 = tx.mine(tx.digest(a103[:80]),104,[tx.light.coinbase(104,1)])
    return prefix,[a101,a102,b101,b102,b103,a103,a104],(a,b,c,b"\x51",b"",b"\x6a")


def expected(active):
    outputs,coins,spends,histories = {},{},{},{}
    for height,block in enumerate(active,1):
        h = tx.digest(block[:80])
        for order,t in enumerate(tx.transactions(block)):
            if order:
                for vin,point in enumerate(t["ins"]):
                    old = coins.pop(point)
                    spends[old] = (t["id"],h,vin,height,order)
                    histories.setdefault(outputs[old][0],set()).add((t["id"],h,height,order))
            for vout,(value,script) in enumerate(t["outs"]):
                if len(script)>10000 or script[:1] == b"\x6a":continue
                point = t["id"]+struct.pack("<I",vout)
                key = (point,h,height,order)
                outputs[key] = (script,value)
                coins[point] = key
                histories.setdefault(script,set()).add((t["id"],h,height,order))
    result = {}
    for key,(script,value) in outputs.items():
        point,h,height,order = key
        spend = spends.get(key)
        result.setdefault(script,[]).append((point,h,height,order,value,spend,coins.get(point)==key))
    return result,histories


def address(script):
    if script[:2] == b"\x76\xa9":
        payload=b"\x6f"+script[3:23]; data=payload+tx.digest(payload)[:4]
        alphabet="123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"; value=int.from_bytes(data,"big"); result=""
        while value:value,remainder=divmod(value,58);result=alphabet[remainder]+result
        return result
    version=0 if script[0]==0 else script[0]-0x50
    groups=[version]; acc=bits=0
    for byte in script[2:]:
        acc=(acc<<8)|byte;bits+=8
        while bits>=5:bits-=5;groups.append((acc>>bits)&31)
    if bits:groups.append((acc<<(5-bits))&31)
    hrp="bcrt"; values=[ord(c)>>5 for c in hrp]+[0]+[ord(c)&31 for c in hrp]+groups+[0]*6
    check=1
    for value in values:
        top=check>>25;check=((check&0x1ffffff)<<5)^value
        for i,g in enumerate((0x3b6a57b2,0x26508e6d,0x1ea119fa,0x3d4233dd,0x2a1462b3)):
            if top>>i&1:check^=g
    check^=1 if version==0 else 0x2bc830a3
    groups += [(check>>5*(5-i))&31 for i in range(6)]
    return hrp+"1"+"".join("qpzry9x8gf2tvdw0s3jn54khce6mua7l"[g] for g in groups)


def cli_checks(tool):
    with tempfile.TemporaryDirectory(prefix="coax-address-file-") as temporary:
        directory=Path(temporary); prefix,branches,scripts=fixture(); allblocks=prefix+branches
        headers=directory/"headers";headers.write_bytes(b"".join(b[:80] for b in allblocks))
        paths=[]
        for i,b in enumerate(allblocks):p=directory/str(i);p.write_bytes(b);paths.append(p)
        state=directory/"state";store=directory/"blocks"

        def call(*args,status=0):
            result=subprocess.run([str(tool),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=120)
            assert result.returncode==status,(args,result.stdout,result.stderr,result.returncode)
            assert "runtime error:" not in result.stderr and "ERROR: AddressSanitizer" not in result.stderr,result.stderr
            return result.stdout

        def query(active):
            model,hist=expected(active)
            for script in scripts[:3]:
                result=call("address","regtest",state,address(script)); rows=model.get(script,[])
                assert f"balance {sum(x[4] for x in rows if x[6])} sat, {len(hist.get(script,set()))} transactions, {len(rows)} outputs\n" in result,result
                for point,h,height,order,value,spend,unspent in rows:
                    line=f"{point[:32][::-1].hex()}:{int.from_bytes(point[32:],'little')} {value} sat status {0 if unspent else (1 if spend else 2)}"
                    if spend:line+=f" spent by {spend[0][::-1].hex()}:{spend[2]}"
                    assert line+"\n" in result,result

        call("init","regtest",state)
        call("apply","regtest",state,store,headers,2000000000,*paths[1:101]);query(prefix[1:])
        call("apply","regtest",state,store,headers,2000000000,*paths[101:103]);query(prefix[1:]+branches[:2])
        before=state.read_bytes()
        call("apply","regtest",state,store,headers,2000000000,*paths[103:106]);query(prefix[1:]+branches[2:5])
        after=state.read_bytes();state.write_bytes(after[:-1]);call("tip","regtest",state,status=1)
        call("recover","regtest",state,store);assert state.read_bytes()==before;query(prefix[1:]+branches[:2])
        state.write_bytes(after)
        call("apply","regtest",state,store,headers,2000000000,*[paths[i] for i in (101,102,106,107)]);query(prefix[1:]+branches[:2]+branches[5:])
        call("disconnect","regtest",state);query(prefix[1:]+branches[:2]+branches[5:6])
        call("address","regtest",state,"not-an-address",status=2)
        print("address file tool: independent P2WPKH/P2TR/P2PKH answers, same-block spends, reorgs, replay and torn-frame recovery pass",flush=True)


def run(compiler,mode):
    with tempfile.TemporaryDirectory(prefix="coax-address-") as temporary:
        directory = Path(temporary)
        source = directory/"index.c"; library = directory/"index.dylib"
        source.write_text(oracle_command([compiler,"c","coax/tests/bitcoin/addrindex-native.coax","--library",*(["--release"] if mode=="release" else [])]))
        oracle_command(["cc","-std=c11","-O2","-shared","-fPIC",source,"-o",library,"-lm"])
        lib = C.CDLL(str(library)); prefix,branches,scripts = fixture()
        lib.txi_native_size.restype = C.c_int64
        state_buffer = (C.c_uint8*lib.txi_native_size())(); state = tx.View(state_buffer,1)
        lib.txi_native_headers.argtypes = [tx.View,tx.View]; lib.txi_native_headers.restype = C.c_int64
        lib.axi_init_vartime.argtypes = [C.c_int64,tx.View,tx.View,C.c_int64,tx.View,tx.View]; lib.axi_init_vartime.restype = tx.Result
        lib.axi_native_reorg.argtypes = [tx.View,tx.View,C.c_int64,tx.IntView,tx.View,tx.IntView,tx.IntView,tx.View,tx.View]; lib.axi_native_reorg.restype = tx.Result
        lib.axi_disconnect_vartime.argtypes = [tx.View,C.c_int64,tx.View,tx.View]; lib.axi_disconnect_vartime.restype = tx.Result
        lib.axi_query_vartime.argtypes = [tx.View,C.c_int64,tx.View,Outputs,Histories]; lib.axi_query_vartime.restype = Result
        lib.lc_native_frame.argtypes = [tx.View,C.c_uint64,tx.View]; lib.lc_native_frame.restype = C.c_int64
        lib.lc_native_open.argtypes = [tx.View,tx.View]; lib.lc_native_open.restype = tx.Open
        assert lib.txi_native_headers(state,tx.view(b"".join(b[:80] for b in prefix+branches))) == len(prefix+branches)
        live,stage,batch,frame,reopened = [(C.c_uint8*(4<<20))() for _ in range(5)]
        outs = (Output*512)(); history = (History*512)(); used=sequence=queries=cuts=0
        header = b"OAKV\x01\x00\x00\x00"+bytes(8); log = header+tx.digest(header)[:4]
        locations = {}; nextpos = 8

        def commit(r,active,crash=False):
            nonlocal used,sequence,log,queries,cuts
            assert r.code == 0,r.code
            before = bytes(live[:used]); after = bytes(stage[:r.used]); sequence+=1
            n = lib.lc_native_frame(tx.view(bytes(batch[:r.n])),sequence,tx.view(frame)); rawframe=bytes(frame[:n])
            if crash:
                # The complete chronological log is checked once below. For
                # every cut, replay the identical pre-reorg table from one
                # sorted snapshot frame; rebuilding 100 old frames per byte
                # only repeats already checked prefix work quadratically.
                records=tx.table_map(before)
                body=struct.pack("<QI",1,len(records))+b"".join(b"\x01"+struct.pack("<HI",len(k),len(v))+k+v for k,v in records.items())
                compact_log=header+tx.digest(header)[:4]+struct.pack("<I",len(body))+tx.digest(body)[:4]+body
                opened=lib.lc_native_open(tx.view(compact_log),tx.view(reopened))
                assert opened.code==0 and bytes(reopened[:opened.used])==before
                lib.lc_native_frame(tx.view(bytes(batch[:r.n])),2,tx.view(frame)); cutframe=bytes(frame[:n])
                for cut in range(n+1):
                    opened = lib.lc_native_open(tx.view(compact_log+cutframe[:cut]),tx.view(reopened))
                    assert opened.code==0 and bytes(reopened[:opened.used]) == (after if cut==n else before)
                    cuts+=1
            log += rawframe
            opened = lib.lc_native_open(tx.view(log),tx.view(reopened))
            assert opened.code == 0 and not opened.cut and bytes(reopened[:opened.used])==after
            C.memmove(live,stage,r.used); used=r.used
            model,hist = expected(active)
            for script in scripts:
                q = lib.axi_query_vartime(tx.view(live),used,tx.view(script),Outputs(outs,512),Histories(history,512))
                expected_outputs = model.get(script,[])
                assert (q.code,q.outputs,q.balance,q.transactions)==(0,len(expected_outputs),sum(x[4] for x in expected_outputs if x[6]),len(hist.get(script,set()))),(mode,script.hex(),q.code,q.outputs,q.balance,q.transactions)
                got_history = {(bytes(x.txid),bytes(x.block),x.height,x.order) for x in history[:q.transactions]}
                assert got_history == hist.get(script,set())
                got = {(bytes(o.txid)+struct.pack("<I",o.vout),bytes(o.block)) : o for o in outs[:q.outputs]}
                for point,h,height,order,value,spend,unspent in expected_outputs:
                    o=got[(point,h)]
                    assert (o.height,o.order,o.value,o.status)==(height,order,value,0 if unspent else (1 if spend else 2))
                    if spend:
                        s=o.spend
                        assert (s.code,bytes(s.txid),bytes(s.block),s.vin,s.height,s.order)==(0,*spend)
                queries+=1

        def apply(indices,bodies,active,crash=False):
            nonlocal nextpos
            offsets=[0];positions=[]
            for b in bodies:
                h=tx.digest(b[:80]); offsets.append(offsets[-1]+len(b))
                if h not in locations:locations[h]=nextpos;nextpos+=len(b)+8
                positions.append(locations[h])
            saved=bytes(live[:used])
            r=lib.axi_native_reorg(state,tx.view(live),used,tx.ints(indices),tx.view(b"".join(bodies)),tx.ints(offsets),tx.ints(positions),tx.view(stage),tx.view(batch))
            assert bytes(live[:used])==saved
            commit(r,active,crash)

        commit(lib.axi_init_vartime(5,tx.view(tx.digest(prefix[0][:80])),tx.view(live),0,tx.view(stage),tx.view(batch)),[])
        for i,b in enumerate(prefix[1:],1):apply([i],[b],prefix[1:i+1])
        apply([101,102],branches[:2],prefix[1:]+branches[:2])
        apply([103,104,105],branches[2:5],prefix[1:]+branches[2:5],True)
        apply([101,102,106,107],[*branches[:2],*branches[5:]],prefix[1:]+branches[:2]+branches[5:])
        commit(lib.axi_disconnect_vartime(tx.view(live),used,tx.view(stage),tx.view(batch)),prefix[1:]+branches[:2]+branches[5:6])
        print(f"address index {mode}: {queries} complete address comparisons, {cuts} atomic reorg crash cuts",flush=True)


def sanitized(compiler):
    with tempfile.TemporaryDirectory(prefix="coax-address-sanitized-") as temporary:
        directory=Path(temporary); source=directory/"address.c";binary=directory/"address"
        source.write_text(oracle_command([compiler,"c","bitcoin/examples/live/address-index.coax","--library"]))
        policy=directory/"entropy.c";header=directory/"entropy-policy.h"
        policy.write_text(oracle_command([compiler,"c","coax/lib/crypto/entropy.coax","--library","--header",header]))
        platform=ROOT/"coax/platform"
        oracle_command(["cc","-std=c11","-O1","-g","-fsanitize=address,undefined","-fno-omit-frame-pointer","-D_DARWIN_C_SOURCE","-I"+str(directory),"-I"+str(platform),source,policy,*[platform/name for name in ("tool-native.c","process-native.c","store-file-native.c","read-file-native.c","socket-native.c","entropy-native.c","entropy-foreign.c")],"-framework","Security","-lm","-o",binary])
        cli_checks(binary)
        print("address file tool: ASan/UBSan pass",flush=True)


if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("--coax");parser.add_argument("--cli-tool");parser.add_argument("--sanitize-with");parser.add_argument("--mode",choices=("debug","release","all"),default="all");a=parser.parse_args()
    if a.coax:
        for mode in (("debug","release") if a.mode=="all" else (a.mode,)):run(Path(a.coax).resolve(),mode)
    if a.cli_tool:cli_checks(Path(a.cli_tool).resolve())
    if a.sanitize_with:sanitized(Path(a.sanitize_with).resolve())
