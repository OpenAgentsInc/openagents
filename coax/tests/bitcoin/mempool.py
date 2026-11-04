#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: records Bitcoin Core mempool fees for real validated regtest spends, supplies fragmented P2P announcements, and compares the own-node observation API across both reorgs)
"""Independent Core reference, wire collector and native mempool journey."""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import tempfile
import threading
import time
import traceback
import urllib.error
import urllib.request

ROOT=Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('reference',Path(__file__).with_name('explorer-reference.py'))
ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)


def record(a):
    core=ref.Core(a.core_url,a.cookie)
    journey=json.loads((a.fixture/'journey.json').read_text())
    assert core.call('getblockchaininfo')['chain']=='regtest' and core.call('getblockcount')==104
    raw=bytes.fromhex(core.call('getblock',journey['hash'],0));txs=ref.tx.transactions(raw)
    pay,onward=txs[1:];assert pay['id'][::-1].hex()==journey['txid']
    child=ref.tx.light.transaction([(onward['id']+bytes(4),bytes.fromhex('03525193'))],[(1999997000,pay['outs'][2][1])])
    child_id=core.call('sendrawtransaction',child.hex());assert child_id==ref.tx.digest(child)[::-1].hex()
    core.call('invalidateblock',journey['hash']);assert core.call('getblockcount')==100
    wanted=[pay['raw'],onward['raw'],child]
    ids=[ref.tx.digest(b)[::-1].hex() for b in wanted]
    assert set(core.call('getrawmempool'))==set(ids)
    rows=[]
    for raw,ident in zip(wanted,ids):
        parsed=ref.tx.Reader(raw).tx();entry=core.call('getmempoolentry',ident)
        assert not parsed['witness'] and entry['weight']==4*len(raw) and entry['vsize']==len(raw)
        fee=int(entry['fees']['base']*100000000)
        rows.append(dict(txid=ident,wtxid=entry['wtxid'],fee_sat=fee,weight=entry['weight'],vsize=entry['vsize'],rate_sat_kvb=fee*1000//entry['vsize'],raw=raw.hex()))
    assert [r['fee_sat'] for r in rows]==[1000,2000,1000]
    result=dict(core=core.call('getnetworkinfo')['subversion'],height=100,hash=core.call('getbestblockhash'),transactions=rows)
    a.record.write_text(json.dumps(result,indent=1)+'\n');print('Core mempool reference: three valid dependent transactions, fees 1000/2000/1000 sat',flush=True)


def packet(command,payload):
    return bytes.fromhex('fabfb5da')+command.encode().ljust(12,b'\0')+struct.pack('<I',len(payload))+ref.tx.digest(payload)[:4]+payload


def exact(conn,n):
    result=b''
    while len(result)<n:
        part=conn.recv(n-len(result));assert part;result+=part
    return result


def receive(conn):
    header=exact(conn,24);assert header[:4]==bytes.fromhex('fabfb5da')
    payload=exact(conn,int.from_bytes(header[16:20],'little'));assert ref.tx.digest(payload)[:4]==header[20:24]
    return header[4:16].rstrip(b'\0').decode(),payload


def collect(a,d,rows,corrupt=False):
    errors=[]
    with socket.socket() as server:
        server.bind(('127.0.0.1',0));server.listen();port=server.getsockname()[1]
        def peer():
            try:
                conn,_=server.accept()
                with conn:
                    conn.settimeout(10);command,version=receive(conn);assert command=='version' and version[-1]==1
                    assert int.from_bytes(version[:4],'little')==70016,(len(version),version.hex())
                    conn.sendall(packet('version',version)+packet('verack',b''))
                    assert receive(conn)[0]=='verack'
                    if corrupt:
                        bad=bytearray(packet('ping',b'12345678'));bad[-1]^=1;conn.sendall(bad);time.sleep(.2);return
                    nonce=b'coaxpool';conn.sendall(packet('ping',nonce))
                    # Children arrive before parents, both on the wire and in the feed.
                    raws=[bytes.fromhex(r['raw']) for r in reversed(rows)];ids=[ref.tx.digest(b) for b in raws]
                    inv=bytes([len(ids)+1])+b''.join(struct.pack('<I',1)+ident for ident in ids)+struct.pack('<I',2)+bytes(32)
                    conn.sendall(packet('inv',inv));requested=False;pong=False
                    while not (requested and pong):
                        command,payload=receive(conn)
                        if command=='pong':assert payload==nonce;pong=True
                        elif command=='getdata':
                            assert payload==bytes([len(ids)])+b''.join(struct.pack('<I',0x40000001)+ident for ident in ids);requested=True
                        else:raise AssertionError(command)
                    unsolicited=bytearray(raws[0]);unsolicited[-1]^=1
                    conn.sendall(packet('tx',unsolicited))
                    for raw in raws:
                        message=packet('tx',raw);conn.sendall(message[:17]);conn.sendall(message[17:])
                    conn.sendall(packet('inv',inv));time.sleep(2.5)
            except BaseException:errors.append(traceback.format_exc())
        thread=threading.Thread(target=peer);thread.start()
        out=d/('corrupt.feed' if corrupt else 'state.mempool')
        result=subprocess.run([str(a.peer),'regtest','127.0.0.1',str(port),'1793756487',str(out),'2'],capture_output=True,text=True,timeout=20)
        thread.join(timeout=12);assert not thread.is_alive() and not errors,(errors,result.returncode,result.stdout,result.stderr)
        assert result.returncode==(1 if corrupt else 0),(result.returncode,result.stderr)
        if not corrupt:
            data=out.read_bytes();assert data[:8]==b'OMP1'+struct.pack('<I',5)
            at=16;actual=[]
            while at<len(data):
                n=int.from_bytes(data[at:at+4],'little');at+=4;actual.append(data[at:at+n]);at+=n
            assert at==len(data) and actual==[bytes.fromhex(r['raw']) for r in reversed(rows)]
    print('Peer observation: exact witness requests, fragmented frames, requested IDs only, deduplication, ping and corrupt-checksum refusal pass',flush=True) if corrupt else None


def request(api,path='/api/mempool/v1',method='GET',data=None):
    try:
        with urllib.request.urlopen(urllib.request.Request(api+path,data=data,method=method),timeout=10) as r:return r.status,json.load(r)
    except urllib.error.HTTPError as error:return error.code,json.load(error)


def check(a):
    fixture=json.loads((a.fixture/'mempool.json').read_text());rows=fixture['transactions']
    with tempfile.TemporaryDirectory(prefix='coax-mempool-') as temporary:
        d=Path(temporary)
        if a.skip_peer:
            (d/'state.mempool').write_bytes(b'OMP1'+struct.pack('<IQ',5,1793756487)+b''.join(struct.pack('<I',len(bytes.fromhex(r['raw'])))+bytes.fromhex(r['raw']) for r in reversed(rows)))
        else:collect(a,d,rows);collect(a,d,rows,True)
        shutil.copyfile(a.fixture/'blk00000.dat',d/'blk00000.dat')
        (d/'headers').write_bytes((a.fixture/'headers-a').read_bytes()[:101*80])
        for label in 'abc':shutil.copyfile(a.fixture/f'headers-{label}',d/f'headers-{label}')
        with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
        api=f'http://127.0.0.1:{port}';browser=None
        with (d/'node.log').open('w+') as log:
            node=subprocess.Popen([str(a.node),'regtest',str(d/'state'),str(d),str(d/'headers'),'2000000000',str(port),str(ROOT/'bitcoin/explorer'),'268435456'],cwd=ROOT,stdout=log,stderr=log)
            try:
                deadline=time.monotonic()+60
                while 'explorer-node ready' not in (d/'node.log').read_text():
                    assert node.poll() is None,(d/'node.log').read_text();assert time.monotonic()<deadline;time.sleep(.02)
                status,answer=request(api);assert status==200 and answer['snapshot']['hash']==fixture['hash']
                assert answer['validation']=='unvalidated' and answer['complete'] is False and answer['received']==3 and answer['omitted']==0
                def compare(expected):
                    status,answer=request(api);assert status==200,(status,answer)
                    assert {r['txid']:r for r in answer['transactions']}=={r['txid']:{k:v for k,v in r.items() if k!='raw'} for r in expected}
                    assert answer['received']==3 and answer['omitted']==3-len(expected);return answer
                compare(rows);assert request(api,method='POST',data=b'')[0]==405
                good=(d/'state.mempool').read_bytes()
                for bad in (good[:-1],b'BAD!'+good[4:],good[:4]+struct.pack('<I',4)+good[8:],b'x'*(8388608+1)):
                    (d/'state.mempool.next').write_bytes(bad);(d/'state.mempool.next').replace(d/'state.mempool');assert request(api)[0]==503
                (d/'state.mempool.next').write_bytes(good);(d/'state.mempool.next').replace(d/'state.mempool');compare(rows)
                browser=subprocess.Popen([str(ROOT/'coax/web/tool.sh'),'bitcoin/explorer/mempool-check.coax',api,str(d)],cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
                for phase,expected in enumerate((rows,[rows[2]],[],[rows[2]])):
                    deadline=time.monotonic()+60
                    while not (d/f'mempool-phase-{phase}').exists():
                        if browser.poll() is not None:raise AssertionError(browser.communicate())
                        assert time.monotonic()<deadline;time.sleep(.05)
                    compare(expected);(d/f'mempool-go-{phase}').touch()
                out,err=browser.communicate(timeout=30);assert browser.returncode==0,(out,err)
                assert node.poll() is None and 'runtime error:' not in (d/'node.log').read_text() and 'ERROR: AddressSanitizer' not in (d/'node.log').read_text()
                a.output.mkdir(parents=True,exist_ok=True)
                for name in ('mempool-desktop.png','mempool-mobile.png'):shutil.copyfile(d/name,a.output/name)
                print(err.strip(),flush=True);print('Own-node mempool matches recorded Core fees; 3 → 1 → 0 → 1 observations, both reorgs and malformed-feed recovery pass',flush=True)
            finally:
                if (d/'mempool-failed.png').exists():
                    a.output.mkdir(parents=True,exist_ok=True);shutil.copyfile(d/'mempool-failed.png',a.output/'mempool-failed.png')
                if browser and browser.poll() is None:browser.terminate();browser.communicate(timeout=10)
                node.terminate();node.wait(timeout=10)


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--record',type=Path);p.add_argument('--cookie',type=Path);p.add_argument('--core-url',default='http://127.0.0.1:38438');p.add_argument('--fixture',type=Path,required=True);p.add_argument('--node',type=Path);p.add_argument('--peer',type=Path);p.add_argument('--skip-peer',action='store_true');p.add_argument('--output',type=Path,default=ROOT/'bitcoin/explorer/build/verification')
    a=p.parse_args()
    if a.record:record(a)
    else:
        assert a.node and a.peer;a.node=a.node.resolve();a.peer=a.peer.resolve();check(a)
