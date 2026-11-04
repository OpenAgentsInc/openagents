#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: compares a native explorer through both real Core-generated forks against recorded Core/Electrs responses while a headless Coax browser drives the live header changes)
"""Own-node HTTP and browser over the Core/Electrs reorg fixtures."""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location("reference",Path(__file__).with_name("explorer-reference.py"))
ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)


def check(node,fixture):
    with tempfile.TemporaryDirectory(prefix="coax-core-browser-") as temporary:
        d=Path(temporary)
        for name in ("blk00000.dat","headers","headers-b","headers-c"):shutil.copyfile(fixture/name,d/name)
        records=[json.loads((fixture/f"reference-{label}.json").read_text()) for label in "abc"]
        journey=json.loads((fixture/"journey.json").read_text())
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1",0));port=reservation.getsockname()[1]
        api=f"http://127.0.0.1:{port}";browser=None
        with (d/"node.log").open("w+") as log:
            proc=subprocess.Popen([str(node),"regtest",str(d/"state"),str(d),str(d/"headers"),"2000000000",str(port),str(ROOT/"bitcoin/explorer"),"268435456"],cwd=ROOT,stdout=log,stderr=log)
            try:
                deadline=time.monotonic()+60
                while "explorer-node ready" not in (d/"node.log").read_text():
                    assert proc.poll() is None,(d/"node.log").read_text()
                    assert time.monotonic()<deadline;time.sleep(.02)
                ref.compare(api,records[0])
                browser=subprocess.Popen([str(ROOT/"coax/web/tool.sh"),"bitcoin/explorer/node-check.coax",api,str(d),journey["address"],journey["txid"],journey["hash"],*map(str,journey["balances"])],cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
                for expected in records[1:]:
                    deadline=time.monotonic()+60
                    while True:
                        assert proc.poll() is None,(d/"node.log").read_text()
                        if browser.poll() is not None:
                            out,err=browser.communicate();raise AssertionError((browser.returncode,out,err,"browser exited before the branch changed"))
                        with urllib.request.urlopen(api+"/api/v1/tip",timeout=5) as r:tip=json.load(r)["snapshot"]
                        if tip==expected["snapshot"]:break
                        assert time.monotonic()<deadline,(tip,expected["snapshot"]);time.sleep(.1)
                    ref.compare(api,expected)
                out,err=browser.communicate(timeout=60);assert browser.returncode==0,(out,err)
                assert proc.poll() is None and "runtime error:" not in (d/"node.log").read_text() and "ERROR: AddressSanitizer" not in (d/"node.log").read_text()
                shutil.copyfile(d/"native-node.png",fixture/"core-browser.png")
                print(err.strip(),flush=True)
                print("Real Core/Electrs fork references and one open native explorer browser agree through 102 → 103 → 104.",flush=True)
            finally:
                if browser and browser.poll() is None:browser.terminate();browser.communicate(timeout=10)
                proc.terminate();proc.wait(timeout=10)


if __name__=="__main__":
    p=argparse.ArgumentParser();p.add_argument("--node",type=Path,required=True);p.add_argument("--fixture",type=Path,required=True);a=p.parse_args();check(a.node.resolve(),a.fixture.resolve())
