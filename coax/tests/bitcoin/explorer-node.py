#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python encodes XOR block files and replays raw transactions to compare the native node's HTTP answers through durable restarts and both reorgs)
"""The full native node on independent fixtures, not a teaching writer."""
import argparse
import importlib.util
import json
from pathlib import Path
import select
import socket
import struct
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("address_oracle", Path(__file__).with_name("addrindex.py"))
address = importlib.util.module_from_spec(spec); spec.loader.exec_module(address)
tx = address.tx


def check(node,browser=False):
    with tempfile.TemporaryDirectory(prefix="coax-native-explorer-") as temporary:
        root = Path(temporary); state = root/"state"; headers = root/"headers"
        prefix, branches, scripts = address.fixture(); all_blocks = prefix+branches
        mask = bytes.fromhex("719d0206f78250a3"); (root/"xor.dat").write_bytes(mask)
        # Multiple files, positional XOR, and physical zero preallocation.
        for file, start in enumerate(range(0, len(all_blocks), 30)):
            plain = b"".join(bytes.fromhex("fabfb5da")+struct.pack("<I",len(b))+b for b in all_blocks[start:start+30])
            encoded = bytes(v ^ mask[i%8] for i,v in enumerate(plain))
            (root/f"blk{file:05}.dat").write_bytes(encoded+bytes(1231))
        total = 0; balances = []; pauses=0; replay_pauses=0
        for known, active in ((all_blocks[:103], prefix[1:]+branches[:2]),
                              (all_blocks[:106], prefix[1:]+branches[2:5]),
                              (all_blocks, prefix[1:]+branches[:2]+branches[5:])):
            headers.write_bytes(b"".join(b[:80] for b in known))
            with socket.socket() as reservation:
                reservation.bind(("127.0.0.1",0)); port = reservation.getsockname()[1]
            log = root/"node.log"
            with log.open("w+") as stream:
                proc = subprocess.Popen([str(node),"regtest",str(state),str(root),str(headers),"2000000000",str(port),str(ROOT/"bitcoin/explorer"),"268435456",str(root/"overflow"),"65536","1"],cwd=ROOT,stdout=stream,stderr=stream)
                try:
                    deadline = time.monotonic()+60; resumed=set()
                    while "explorer-node ready" not in log.read_text():
                        text=log.read_text();assert proc.poll() is None,text
                        assert time.monotonic()<deadline,text
                        for line in text.splitlines():
                            if line.startswith(("import paused","replay paused")) and line not in resumed:
                                try: urllib.request.urlopen(f"http://127.0.0.1:{port}/api/v1/tip",timeout=5)
                                except urllib.error.HTTPError as error: assert error.code==503
                                else: raise AssertionError("partial import was exposed as ready")
                                resumed.add(line);Path(str(state)+".continue").touch();pauses+=1
                                if line.startswith("replay paused"):replay_pauses+=1
                        time.sleep(.02)
                    snap = dict(chain="regtest",height=len(active),hash=tx.digest(active[-1][:80])[::-1].hex(),profile="pow-only")
                    def request(path, status=200):
                        nonlocal total
                        try: result = urllib.request.urlopen(f"http://127.0.0.1:{port}/api/v1/{path}",timeout=10)
                        except urllib.error.HTTPError as error: result=error
                        with result:
                            raw=result.read(); assert result.status==status,(path,result.status,raw)
                            total+=1; return json.loads(raw)
                    assert request("tip")==dict(snapshot=snap)
                    for height in (0,1,50,100,len(active)):
                        b=prefix[0] if height==0 else active[height-1]; h=tx.digest(b[:80])[::-1].hex()
                        by_height=request("block/"+str(height)); assert by_height==request("block/"+h)
                        assert by_height["snapshot"]==snap and by_height["hash"]==h and by_height["height"]==height
                        assert by_height["size"]==len(b) and by_height["txids"]==[t["id"][::-1].hex() for t in tx.transactions(b)]
                        assert request("search/"+str(height))["key"]==h
                    model,histories=address.expected(active)
                    for script in scripts[:3]:
                        key=address.address(script); got=request("address/"+key); rows=model.get(script,[])
                        assert got["snapshot"]==snap and got["script_hex"]==script.hex()
                        assert got["balance_sat"]==sum(r[4] for r in rows if r[6])
                        assert {(x["txid"],x["block"],x["height"],x["order"]) for x in got["transactions"]}=={(i[::-1].hex(),b[::-1].hex(),h,o) for i,b,h,o in histories.get(script,set())}
                        assert len(got["outputs"])==len(rows)
                        for row in got["outputs"]:
                            out=row["output"]; occurrence=row["occurrence"]
                            expected,=[r for r in rows if r[0][:32][::-1].hex()==occurrence["txid"] and int.from_bytes(r[0][32:],"little")==out["vout"] and r[1][::-1].hex()==occurrence["block"]]
                            assert out["value_sat"]==expected[4]
                            assert out["status"]==("unspent" if expected[6] else "spent" if expected[5] else "replaced")
                            if expected[5]:
                                spend=expected[5]; assert out["spend"]==dict(txid=spend[0][::-1].hex(),block=spend[1][::-1].hex(),vin=spend[2],height=spend[3])
                        assert request("search/"+key)["kind"]=="address"
                        if script==scripts[1]: balances.append(got["balance_sat"])
                    for b in active[-4:]:
                        for t in tx.transactions(b):
                            got=request("transaction/"+t["id"][::-1].hex())
                            assert got["snapshot"]==snap and got["raw"]==t["raw"].hex() and got["wtxid"]==tx.digest(t["raw"])[::-1].hex() and got["active"]
                            assert request("search/"+t["id"][::-1].hex())["kind"]=="transaction"
                    request("transaction/"+"ff"*32,404)
                    # The exclusive lock must reject a second writer.
                    second=subprocess.run([str(node),"regtest",str(state),str(root),str(headers),"2000000000","0","","268435456",str(root/"overflow"),"65536","1"],cwd=ROOT,capture_output=True,timeout=15)
                    assert second.returncode==1 and b"exclusive ownership" in second.stderr
                finally:
                    proc.terminate(); proc.wait(timeout=10)
                    assert "runtime error:" not in log.read_text() and "ERROR: AddressSanitizer" not in log.read_text(),log.read_text()
        assert balances==[1999998000,0,1999998000],balances
        assert pauses>0,"bounded import did not retain its arena across a pause"
        assert replay_pauses>0,"bounded restart replay did not retain its arena across a pause"
        if browser:
            # A fresh native process follows both updates on one open page.
            headers.write_bytes(b"".join(b[:80] for b in all_blocks[:103]))
            # The competing feed deliberately omits the old branch: our
            # node must retain its own admitted headers across updates.
            (root/"headers-b").write_bytes(b"".join(b[:80] for b in prefix+branches[2:5]))
            (root/"headers-c").write_bytes(b"".join(b[:80] for b in prefix+branches[:2]+branches[5:]))
            with socket.socket() as reservation:
                reservation.bind(("127.0.0.1",0));port=reservation.getsockname()[1]
            with (root/"watch.log").open("w+") as log:
                proc=subprocess.Popen([str(node),"regtest",str(root/"watch-state"),str(root),str(headers),"2000000000",str(port),str(ROOT/"bitcoin/explorer"),"268435456",str(root/"overflow"),"65536","1"],cwd=ROOT,stdout=log,stderr=log)
                try:
                    deadline=time.monotonic()+60;resumed=set()
                    while "explorer-node ready" not in (root/"watch.log").read_text():
                        text=(root/"watch.log").read_text();assert proc.poll() is None,text
                        assert time.monotonic()<deadline,text
                        for line in text.splitlines():
                            if line.startswith("import paused") and line not in resumed:
                                resumed.add(line);(root/"watch-state.continue").touch();pauses+=1
                        time.sleep(.02)
                    p=subprocess.run([str(ROOT/"coax/web/tool.sh"),"bitcoin/explorer/node-check.coax",f"http://127.0.0.1:{port}",str(root),address.address(scripts[1]),tx.transactions(branches[0])[1]["id"][::-1].hex(),tx.digest(branches[0][:80])[::-1].hex()],cwd=ROOT,capture_output=True,text=True,timeout=180)
                    assert p.returncode==0,(p.stdout,p.stderr,(root/"watch.log").read_text())
                    assert proc.poll() is None;print(p.stderr.strip(),flush=True)
                finally:
                    proc.terminate();proc.wait(timeout=10)
            # A feed replacement during a long initial import must still
            # be noticed after the first ready cursor, without another touch.
            headers.write_bytes(b"".join(b[:80] for b in all_blocks[:103]))
            with socket.socket() as reservation:
                reservation.bind(("127.0.0.1",0));port=reservation.getsockname()[1]
            with (root/"late.log").open("w+") as log:
                proc=subprocess.Popen([str(node),"regtest",str(root/"late-state"),str(root),str(headers),"2000000000",str(port),str(ROOT/"bitcoin/explorer"),"268435456",str(root/"overflow"),"65536","1"],cwd=ROOT,stdout=log,stderr=log)
                try:
                    deadline=time.monotonic()+30
                    while "import paused" not in (root/"late.log").read_text():
                        assert proc.poll() is None,(root/"late.log").read_text()
                        assert time.monotonic()<deadline;time.sleep(.02)
                    replacement=root/"late-headers";replacement.write_bytes((root/"headers-b").read_bytes());replacement.replace(headers)
                    (root/"late-state.continue").touch()
                    while True:
                        assert proc.poll() is None,(root/"late.log").read_text()
                        try:
                            with urllib.request.urlopen(f"http://127.0.0.1:{port}/api/v1/tip",timeout=5) as r:tip=json.load(r)["snapshot"]
                            if tip["height"]==103:
                                assert tip["hash"]==tx.digest(branches[4][:80])[::-1].hex();break
                        except urllib.error.HTTPError as error: assert error.code==503
                        assert time.monotonic()<deadline,"header update during import was lost";time.sleep(.1)
                finally:
                    proc.terminate();proc.wait(timeout=10)
            print("Native node notices a header-feed replacement made while its import is paused.",flush=True)
            # Restore the completed restart journey's header feed.
            headers.write_bytes(b"".join(b[:80] for b in all_blocks))

        # A complete durable cursor needs no raw-file catalog scan to restart.
        # Missing immutable bodies refuse only their data route and recover
        # when the same file returns; no stale or synthetic body is served.
        rawfile=root/"blk00000.dat"; held=root/"held-blockfile";rawfile.rename(held)
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1",0));port=reservation.getsockname()[1]
        log=root/"on-demand.log"
        with log.open("w+") as stream:
            proc=subprocess.Popen([str(node),"regtest",str(state),str(root),str(headers),"2000000000",str(port),"","268435456",str(root/"overflow"),"65536","5000"],cwd=ROOT,stdout=stream,stderr=stream)
            try:
                deadline=time.monotonic()+15
                while "explorer-node ready" not in log.read_text():
                    assert proc.poll() is None,log.read_text()
                    assert time.monotonic()<deadline,log.read_text();time.sleep(.02)
                assert request("tip")==dict(snapshot=snap)
                request("block/1",503)
                held.rename(rawfile)
                assert request("block/1")["hash"]==tx.digest(prefix[1][:80])[::-1].hex()
                print("Complete restart reads raw bodies on demand; unavailable body refuses and recovers without a state change.",flush=True)
            finally:
                proc.terminate();proc.wait(timeout=10)
                if held.exists():held.rename(rawfile)
                assert "runtime error:" not in log.read_text() and "ERROR: AddressSanitizer" not in log.read_text(),log.read_text()

        # The last append can tear: reopening discards exactly that frame
        # and reconstructs the final branch from immutable imported blocks.
        segments=[state,*root.glob("state.segment-*"),*(root/"overflow").glob("state.segment-*")]
        assert len(segments)>1,"small segment limit did not exercise rollover"
        last=max(segments,key=lambda p:0 if p==state else int(p.name.rsplit("-",1)[1]))
        # A valid checksum cannot hide a discontinuity between segments.
        first=min((p for p in segments if p!=state),key=lambda p:int(p.name.rsplit("-",1)[1]))
        relocated=(root/"overflow" if first.parent==root else root)/first.name
        if last==first:last=relocated
        first.rename(relocated);first=relocated
        # A missing segment (including an unavailable archive symlink) is
        # never treated as the final durable prefix when later names remain.
        probe=root/f'state.segment-{int(last.name.rsplit("-",1)[1])+1}'
        probe.write_bytes(first.read_bytes()[:20])
        hidden=first.with_name('hidden-journal')
        first.rename(hidden)
        refused=subprocess.run([str(node),"regtest",str(state),str(root),str(headers),"2000000000","0","","268435456",str(root/"overflow"),"65536","5000"],cwd=ROOT,capture_output=True,timeout=60)
        assert refused.returncode==1,(refused.returncode,refused.stdout,refused.stderr)
        first.symlink_to(root/'unavailable-archive')
        refused=subprocess.run([str(node),"regtest",str(state),str(root),str(headers),"2000000000","0","","268435456",str(root/"overflow"),"65536","5000"],cwd=ROOT,capture_output=True,timeout=60)
        assert refused.returncode==1
        first.unlink();hidden.rename(first);probe.unlink()
        saved=first.read_bytes(); damaged=bytearray(saved); n=struct.unpack_from("<I",damaged,20)[0]
        struct.pack_into("<Q",damaged,28,struct.unpack_from("<Q",damaged,28)[0]+7)
        damaged[24:28]=tx.digest(damaged[28:28+n])[:4];first.write_bytes(damaged)
        refused=subprocess.run([str(node),"regtest",str(state),str(root),str(headers),"2000000000","0","","268435456",str(root/"overflow"),"65536","5000"],cwd=ROOT,capture_output=True,timeout=60)
        assert refused.returncode==1 and first.read_bytes()==damaged,(refused.returncode,refused.stdout,refused.stderr)
        first.write_bytes(saved)
        # A torn interior segment must never be trimmed past a later one.
        base_saved=state.read_bytes();state.write_bytes(base_saved[:-1])
        refused=subprocess.run([str(node),"regtest",str(state),str(root),str(headers),"2000000000","0","","268435456",str(root/"overflow"),"65536","5000"],cwd=ROOT,capture_output=True,timeout=60)
        assert refused.returncode==1 and state.read_bytes()==base_saved[:-1],(refused.returncode,refused.stdout,refused.stderr)
        state.write_bytes(base_saved)
        # Creation of the next segment can tear only its known v1 header.
        next_index=int(last.name.rsplit("-",1)[1])+1
        extra=(root/"overflow" if next_index%2 else root)/f"state.segment-{next_index}"
        plain_header=b"OAKV"+bytes([1,0,0,0])+bytes(8);plain_header+=tx.digest(plain_header)[:4]
        for cut in (0,1,4,19):
            extra.write_bytes(plain_header[:cut])
            repaired=subprocess.run([str(node),"regtest",str(state),str(root),str(headers),"2000000000","0","","268435456",str(root/"overflow"),"65536","5000"],cwd=ROOT,capture_output=True,timeout=60)
            assert repaired.returncode==0,(cut,repaired.stdout,repaired.stderr)
            assert extra.read_bytes()==plain_header;extra.unlink()
        extra.write_bytes(b"BAD")
        refused=subprocess.run([str(node),"regtest",str(state),str(root),str(headers),"2000000000","0","","268435456",str(root/"overflow"),"65536","5000"],cwd=ROOT,capture_output=True,timeout=60)
        assert refused.returncode==1 and extra.read_bytes()==b"BAD";extra.unlink()
        original=last.read_bytes(); last.write_bytes(original[:-1])
        recovered=subprocess.run([str(node),"regtest",str(state),str(root),str(headers),"2000000000","0","","268435456",str(root/"overflow"),"65536","5000"],cwd=ROOT,capture_output=True,text=True,timeout=60)
        assert recovered.returncode==0,(recovered.stdout,recovered.stderr)
        print(f"native explorer: {total} independent HTTP comparisons; XOR blocks, journal rollover, physical padding, both reorgs, exclusive writer, budget pause/503/continuation and torn-frame restart pass",flush=True)


if __name__=="__main__":
    parser=argparse.ArgumentParser(); parser.add_argument("--node",type=Path,required=True); parser.add_argument("--browser",action="store_true")
    args=parser.parse_args(); check(args.node.resolve(),args.browser)
