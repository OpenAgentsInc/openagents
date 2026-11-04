#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python mines regtest fork fixtures and compares validated reorganization state with a running Core node)
"""Chapter 60 fixtures and differential validated reorganization checks."""
import argparse
import ctypes as C
import hashlib
import json
import struct
import tempfile
from pathlib import Path

import lightchain as L
import chainstate as S
import validation as V

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT/'bitcoin/fixtures/validated-reorg.json'
VECTORS = ROOT/'coax/tests/bitcoin/chainreorg_vectors.coax'

class Result(C.Structure):
    _fields_=[(n,C.c_int64) for n in ('code','vcode','detail','tx','input','used','n','disconnected','connected','block')]


def configure(lib):
    S.configure(lib)
    for name,args,result in [
        ('cr_native_receive',[S.View,C.c_int64,S.View],C.c_int64),
        ('cr_native_status',[S.View,C.c_int64],C.c_uint32),
        ('cr_native_tip',[S.View],C.c_int64),
        ('cr_native_best',[S.View],C.c_int64),
        ('cr_native_publish',[S.View,S.View,C.c_int64,C.c_int64],C.c_bool),
        ('cr_native_prepare',[S.View,S.View,C.c_int64,C.c_int64]+[S.View]*8,Result),
    ]:
        f=getattr(lib,name);f.argtypes=args;f.restype=result


def journey(lib,indexed,core_dir,record,cut_dir):
    common=L.make_fixture(); prefix=list(map(bytes.fromhex,common['prefix']))
    bodies=prefix+list(map(bytes.fromhex,common['branches']))+list(map(bytes.fromhex,fixture()['extra']))
    node=S.Node(lib,indexed); scratch=(C.c_uint8*64000000)(); temp=(C.c_uint8*64000000)()
    assert lib.validation_native_init(node.d,5,S.view(b''.join(b[:80] for b in bodies)))==110
    node.initialize()
    def receive(i): assert lib.cr_native_receive(node.d,i,S.view(bodies[i]))==0,i
    def publish(i):
        assert lib.cr_native_publish(node.d,S.view(node.table),node.used,i)
        assert lib.cr_native_tip(node.d)==i
    def prepare(target,indices,override=None):
        selected=[bodies[i] for i in indices] if override is None else override
        offsets=[0]
        for b in selected:offsets.append(offsets[-1]+len(b))
        ii=(C.c_int64*len(indices))(*indices); oo=(C.c_int64*len(offsets))(*offsets)
        before=node.flat(); tip=lib.cr_native_tip(node.d)
        r=lib.cr_native_prepare(node.d,S.view(node.table),node.used,target,S.View(C.cast(ii,C.c_void_p),len(ii)),
            S.view(b''.join(selected)),S.View(C.cast(oo,C.c_void_p),len(oo)),S.view(node.stage),S.view(scratch),
            S.view(node.batch),S.view(temp),S.view(node.undo))
        assert before==node.flat() and tip==lib.cr_native_tip(node.d),'prepare published prematurely'
        if r.code: assert r.n==r.used==r.disconnected==r.connected==0
        return r
    states=[]; cuts=0
    with tempfile.TemporaryDirectory(prefix='coax-validated-reorg-') as tmp:
        core=V.Core(core_dir.resolve()/'bitcoind',Path(tmp))
        try:
            for b in bodies[1:]:assert core.call('submitheader',b[:80].hex()) is None
            receive(0);publish(0);coins={}
            for h,b in enumerate(prefix[1:],1):
                receive(h);coins,_,_=node.connect(h,b,coins)
                assert core.call('submitblock',b.hex()) is None
            publish(100)
            def compare(index,expected):
                assert S.coin_values(node.flat())==expected
                actual=core.call('gettxoutsetinfo','muhash')
                digest=S.muhash(expected)
                assert actual['muhash']==digest==node.own_muhash(),(index,actual,digest)
                assert actual['bestblock']==L.sha256d(bodies[index][:80])[::-1].hex()
                states.append(dict(index=index,height=actual['height'],hash=actual['bestblock'],muhash=digest))
            def accept(target,indices,detach,attach,expected):
                nonlocal cuts
                old,journal=node.flat(),node.journal
                r=prepare(target,indices);assert (r.code,r.disconnected,r.connected)==(0,detach,attach),tuple(getattr(r,n) for n,_ in Result._fields_)
                frame=node.accept(r);publish(target)
                case=dict(prefix=journal.hex(),frame=frame.hex(),old=old.hex(),new=node.flat().hex())
                cut_dir.mkdir(parents=True,exist_ok=True)
                (cut_dir/f'{target}.json').write_text(json.dumps(case)+'\n')
                cuts+=len(frame)+1
                assert S.coin_values(node.flat())==expected
            for i in (101,102):receive(i);assert core.call('submitblock',bodies[i].hex()) is None
            acoins=V.coins_after(prefix+bodies[101:103]);accept(102,[101,102],0,2,acoins);compare(102,acoins)
            side=[]
            for i in (103,104):
                receive(i);reply=core.call('submitblock',bodies[i].hex());side.append(reply)
                assert reply in (None,'inconclusive'),(i,reply)
            assert core.call('getbestblockhash')==L.sha256d(bodies[102][:80])[::-1].hex()
            assert prepare(104,[103,104]).code==5
            for i in (105,106,107):receive(i)
            old=node.flat();r=prepare(107,[103,104,105,106,107])
            assert r.code==3 and r.block==106 and node.flat()==old
            assert lib.cr_native_status(node.d,106)&32 and lib.cr_native_status(node.d,107)&32
            assert lib.cr_native_best(node.d)==105
            reasons=[]
            for i in (105,106,107):reasons.append(core.call('submitblock',bodies[i].hex()))
            assert reasons[0] is None and reasons[1]=='bad-txns-inputs-missingorspent' and reasons[2] is not None,reasons
            bcoins=V.coins_after(prefix+bodies[103:106]);accept(105,[103,104,105],2,3,bcoins);compare(105,bcoins)
            for i in (108,109):receive(i)
            changed=bytearray(bodies[108]);changed[-1]^=1
            r=prepare(109,[101,102,108,109],[bodies[101],bodies[102],bytes(changed),bodies[109]])
            assert r.code==3 and r.block==108 and not lib.cr_native_status(node.d,108)&96
            assert core.call('submitblock',bytes(changed).hex())=='bad-txnmrklroot'
            for i in (108,109):
                reply=core.call('submitblock',bodies[i].hex())
                assert reply in (None,'inconclusive'),(i,reply)
            final=V.coins_after(prefix+bodies[101:103]+bodies[108:110]);accept(109,[101,102,108,109],3,4,final);compare(109,final)
            # Restart the durable store into a fresh index. V is authoritative;
            # validation caches may be rebuilt, but never move ahead of V.
            reopened=S.Node(lib,indexed)
            assert lib.validation_native_init(reopened.d,5,S.view(b''.join(b[:80] for b in bodies)))==110
            opened=lib.cs_native_open(S.view(node.journal),S.view(reopened.table));assert opened.code==0 and not opened.cut
            reopened.used=opened.used
            assert S.table_values(reopened.flat())==S.table_values(node.flat())
            assert lib.cr_native_publish(reopened.d,S.view(reopened.table),reopened.used,109)
            assert lib.cr_native_tip(reopened.d)==109 and reopened.own_muhash()==S.muhash(final)
            version=core.call('getnetworkinfo')['subversion']
        finally:core.close()
    evidence=dict(schema=1,version=version,indexed=indexed,states=states,invalid_reasons=reasons,
                  crash_cut_cases=cuts,side_replies=side,restart=True)
    if record:record.write_text(json.dumps(evidence,indent=1)+'\n')
    print(f'validated reorg: Core agrees on three full coin sets/MuHashes, invalid descendant fallback, both fork directions and restart; {cuts} physical cuts exported',flush=True)


def cut_checks(lib,path,shard):
    case=json.loads(path.read_text());prefix,frame,old,new=(bytes.fromhex(case[k]) for k in ('prefix','frame','old','new'))
    part,total=map(int,shard.split('/'));assert 1<=part<=total
    replay=(C.c_uint8*64000000)();data=prefix+frame;checked=0
    for cut in range(part-1,len(frame)+1,total):
        r=lib.cs_native_open(S.view(data[:len(prefix)+cut]),S.view(replay))
        assert r.code==0 and bytes(replay[:r.used])==(new if cut==len(frame) else old),(cut,r.code)
        assert r.keep==(len(data) if cut==len(frame) else len(prefix)),cut
        checked+=1
    proof=dict(schema=1,case=hashlib.sha256(path.read_bytes()).hexdigest(),frame=hashlib.sha256(frame).hexdigest(),
               shard=shard,checked=checked,total=len(frame)+1)
    print(json.dumps(proof,sort_keys=True),flush=True)


def fixture():
    common = L.make_fixture()
    branches = list(map(bytes.fromhex, common['branches']))
    bad105 = L.block(L.sha256d(branches[5][:80]),105,7)
    a103 = L.block(L.sha256d(branches[1][:80]),103,8)
    a104 = L.block(L.sha256d(a103[:80]),104,8)
    extra = [b.hex() for b in (bad105,a103,a104)]
    return dict(schema=1,network='regtest',source='independent Python writer/miner; common prefix and branches from lightchain.py',
                extra=extra,sha256=[hashlib.sha256(bytes.fromhex(b)).hexdigest() for b in extra])


def vector_source(data):
    return ('// Independently generated by chainreorg.py --write.\n'
            '// B105 descends from invalid B104; A103/A104 extend the other fork.\n'
            'CR_EXTRA :: [3] string.[\n'+''.join('  "'+b+'",\n' for b in data['extra'])+'];\n')


def main():
    p=argparse.ArgumentParser();p.add_argument('--write',action='store_true')
    p.add_argument('--shared',type=Path);p.add_argument('--core-dir',type=Path)
    p.add_argument('--indexed',action='store_true');p.add_argument('--record',type=Path)
    p.add_argument('--cut-dir',type=Path);p.add_argument('--cuts',type=Path);p.add_argument('--shard',default='1/1');a=p.parse_args()
    data=fixture(); encoded=json.dumps(data,indent=1)+'\n'; vectors=vector_source(data)
    if a.write:
        FIXTURE.write_text(encoded);VECTORS.write_text(vectors)
    else:
        assert FIXTURE.read_text()==encoded and VECTORS.read_text()==vectors
    print('validated reorg: three independently mined extra blocks and their digests match')
    if a.shared:
        lib=C.CDLL(str(a.shared.resolve()));configure(lib)
        if a.cuts:cut_checks(lib,a.cuts,a.shard)
        else:
            assert a.core_dir and a.cut_dir
            journey(lib,a.indexed,a.core_dir,a.record,a.cut_dir)


if __name__=='__main__':main()
