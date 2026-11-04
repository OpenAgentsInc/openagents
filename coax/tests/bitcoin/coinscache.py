#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: compare cached validated coins with Core, exercise durable flushes and replay, and export exhaustive crash-cut cases)
"""Chapter 63's bounded private cache, real block validation and durable flushes."""
import argparse
import ctypes as C
import hashlib
import json
import os
import struct
import subprocess
from pathlib import Path
import tempfile

import chainreorg as R
import chainstate as S
import lightchain as L
import validation as V


class Result(C.Structure):
    _fields_=[('code',C.c_int64),('used',C.c_int64),('n',C.c_int64),('generation',C.c_uint64)]


def configure(lib):
    R.configure(lib)
    for name,args,result in [
        ('cc_native_size',[],C.c_int64),('cc_native_used',[S.View],C.c_int64),
        ('cc_native_height',[S.View],C.c_int64),('cc_native_pending',[S.View],C.c_bool),
        ('cc_init_vartime',[S.View,S.View,C.c_int64,C.c_int64]+[S.View]*3,C.c_int64),
        ('cc_accept_vartime',[S.View,S.View,C.c_int64]+[S.View]*6,C.c_int64),
        ('cc_view_vartime',[S.View,S.View,C.c_int64]+[S.View]*3,Result),
        ('cc_flush_vartime',[S.View,S.View,C.c_int64]+[S.View]*3,Result),
        ('cc_ack_vartime',[S.View,C.c_uint64,S.View,C.c_int64],C.c_bool),
        ('cc_abort_before_append',[S.View,C.c_uint64],C.c_bool),
    ]:
        f=getattr(lib,name);f.argtypes=args;f.restype=result


class Cache:
    def __init__(self,node,capacity=32768):
        self.node=node;self.lib=node.lib
        self.state=(C.c_uint8*self.lib.cc_native_size())();self.c=S.View(C.cast(self.state,C.c_void_p),1)
        self.delta=(C.c_uint8*capacity)();self.scratch=(C.c_uint8*capacity)();self.record=(C.c_uint8*65537)()
        self.stage=(C.c_uint8*64000000)();self.batch=(C.c_uint8*64000000)()
        self.limit=len(self.state)+len(self.delta)+len(self.scratch)+len(self.record)
        assert self.lib.cc_init_vartime(self.c,S.view(node.table),node.used,self.limit-1,
               S.view(self.delta),S.view(self.scratch),S.view(self.record))==2
        assert self.lib.cc_init_vartime(self.c,S.view(node.table),node.used,self.limit,
               S.view(self.delta),S.view(self.scratch),S.view(self.record))==0

    def materialize(self):
        n=self.node
        r=self.lib.cc_view_vartime(self.c,S.view(n.table),n.used,S.view(self.delta),S.view(self.stage),S.view(self.batch))
        assert r.code==0,(r.code,r.used,r.n)
        return r

    def flat(self):
        r=self.materialize()
        used=self.lib.validation_native_canonical(S.view(self.stage),r.used,S.view(self.node.canonical))
        assert used>=0
        return bytes(self.node.canonical[:used])

    def accept(self,body):
        n=self.node
        return self.lib.cc_accept_vartime(self.c,S.view(n.table),n.used,S.view(self.delta),S.view(body),
             S.view(self.scratch),S.view(self.record),S.view(self.stage),S.view(self.batch))

    def flush(self,path,target,cut_dir=None):
        n=self.node;old=n.flat();prefix=n.journal
        r=self.lib.cc_flush_vartime(self.c,S.view(n.table),n.used,S.view(self.delta),S.view(self.stage),S.view(self.batch))
        assert r.code==0
        if not r.n:
            assert not self.lib.cc_native_pending(self.c)
            return None
        assert self.lib.cc_native_pending(self.c)
        assert self.accept(bytes(12))==1,'pending cache accepted another transition'
        frame=(C.c_uint8*(r.n+8))()
        assert self.lib.cs_native_frame(S.View(C.cast(self.batch,C.c_void_p),r.n),n.sequence+1,S.view(frame))==len(frame)
        # The real file is durable before acknowledgement or active publication.
        with path.open('ab') as f:f.write(bytes(frame));f.flush();os.fsync(f.fileno())
        assert path.read_bytes()==prefix+bytes(frame)
        C.memmove(n.table,self.stage,r.used);n.used=r.used;n.sequence+=1;n.journal+=bytes(frame)
        new=n.flat()
        assert self.lib.cc_ack_vartime(self.c,r.generation,S.view(n.table),n.used)
        assert self.lib.cr_native_publish(n.d,S.view(n.table),n.used,target)
        assert self.lib.cc_native_used(self.c)==0 and not self.lib.cc_native_pending(self.c)
        if cut_dir:
            cut_dir.mkdir(parents=True,exist_ok=True)
            p=cut_dir/f'{target}.json'
            p.write_text(json.dumps(dict(prefix=prefix.hex(),frame=bytes(frame).hex(),old=old.hex(),new=new.hex()))+'\n')
            return dict(target=target,cuts=len(frame)+1,case=hashlib.sha256(p.read_bytes()).hexdigest())
        return None


def journey(lib,core_dir,record,cut_dir,indexed):
    fixture=L.make_fixture();prefix=list(map(bytes.fromhex,fixture['prefix']))
    blocks=prefix+list(map(bytes.fromhex,fixture['branches']))+list(map(bytes.fromhex,R.fixture()['extra']))
    n=S.Node(lib,indexed);headers=b''.join(b[:80] for b in blocks)
    assert lib.validation_native_init(n.d,5,S.view(headers))==110
    n.initialize();cache=Cache(n);receipts=[];coins={};flushes=0;cut_cases=[];states=[]
    for i in (0,):assert lib.cr_native_receive(n.d,i,S.view(blocks[i]))==0
    with tempfile.TemporaryDirectory(prefix='coax-coinscache-core-') as tmp:
        directory=Path(tmp);path=directory/'state.kv'
        with path.open('wb') as f:f.write(n.journal);f.flush();os.fsync(f.fileno())
        assert lib.cr_native_publish(n.d,S.view(n.table),n.used,0)
        core_dir_run=directory/'core';core_dir_run.mkdir();core=V.Core(core_dir.resolve()/'bitcoind',core_dir_run)
        try:
            for b in blocks[1:]:assert core.call('submitheader',b[:80].hex()) is None
            def receive(i):
                assert lib.cr_native_receive(n.d,i,S.view(blocks[i]))==0;receipts.append(i)
            capacity_refused=False
            def connect(i,height):
                nonlocal coins,capacity_refused
                before=n.flat();cursor=lib.cr_native_tip(n.d);r=cache.materialize()
                undo,after=S.undo_and_after(coins,blocks[i],height)
                result=lib.cs_native_connect(n.d,i,S.view(blocks[i]),S.view(cache.stage),r.used,
                      S.view(n.stage),S.view(n.batch),S.view(n.undo))
                assert result.code==0,(i,result.code,result.vcode,result.detail)
                assert bytes(n.undo[:result.undo_n])==undo
                body=bytes(n.batch[:result.n])
                if i==101 and not capacity_refused:
                    small=Cache(n,capacity=256)
                    assert small.accept(body)==2 and lib.cc_native_used(small.c)==0
                    assert lib.cc_native_height(small.c)==100 and not any(small.delta)
                    assert before==n.flat();capacity_refused=True
                assert cache.accept(body)==0
                assert before==n.flat() and lib.cr_native_tip(n.d)==cursor
                assert S.coin_values(cache.flat())==after
                assert lib.cc_native_used(cache.c)<=len(cache.delta)
                assert lib.cc_native_height(cache.c)==height
                coins=after
            def compare(i):
                actual=core.call('gettxoutsetinfo','muhash');expected=S.muhash(coins)
                private=cache.materialize();out=(C.c_uint8*32)()
                assert lib.cs_native_muhash(S.view(cache.stage),private.used,S.view(out))==0
                assert actual['muhash']==expected==bytes(out)[::-1].hex()
                assert actual['bestblock']==L.sha256d(blocks[i][:80])[::-1].hex()
                states.append(dict(index=i,height=actual['height'],hash=actual['bestblock'],muhash=expected))
            for h in range(1,101):
                receive(h);connect(h,h);assert core.call('submitblock',blocks[h].hex()) is None
                if h%25==0:cache.flush(path,h);flushes+=1
            base100=n.flat();journal100=n.journal
            for i in (101,102):receive(i);connect(i,i);assert core.call('submitblock',blocks[i].hex()) is None
            compare(102);private102=cache.flat();assert n.flat()==base100 and lib.cr_native_tip(n.d)==100
            # Lose all unflushed RAM, reopen the actual durable file, and replay
            # the immutable archived blocks through the real validator again.
            lost=cache;restarted=S.Node(lib,indexed)
            assert lib.validation_native_init(restarted.d,5,S.view(headers))==110
            disk=path.read_bytes();opened=lib.cs_native_open(S.view(disk),S.view(restarted.table))
            assert opened.code==0 and not opened.cut
            restarted.used=opened.used;restarted.sequence=opened.seq;restarted.journal=disk
            if indexed:
                flat=restarted.flat();restarted.used=lib.validation_native_indexed(S.view(restarted.table),S.view(flat))
                assert restarted.used>0
            for i in [0]+receipts:assert lib.cr_native_receive(restarted.d,i,S.view(blocks[i]))==0
            assert lib.cr_native_publish(restarted.d,S.view(restarted.table),restarted.used,100)
            n=restarted;cache=Cache(n)
            assert S.table_values(cache.flat())==S.table_values(base100)
            coins=V.coins_after(prefix)
            for i in (101,102):connect(i,i)
            assert cache.flat()==private102 and n.journal==journal100
            del lost
            cut_cases.append(cache.flush(path,102,cut_dir));flushes+=1
            assert n.own_muhash()==states[-1]['muhash']
            for i in (103,104,105):receive(i);assert core.call('submitblock',blocks[i].hex()) in (None,'inconclusive')
            # Prepare the complete reorganization against the private cache view.
            def reorganize(target,indices,expected):
                nonlocal coins
                source=cache.materialize();cursor=lib.cr_native_tip(n.d);before=n.flat()
                ii=(C.c_int64*len(indices))(*indices);offsets=[0]
                for i in indices:offsets.append(offsets[-1]+len(blocks[i]))
                oo=(C.c_int64*len(offsets))(*offsets);spare=(C.c_uint8*64000000)();aggregate=(C.c_uint8*64000000)()
                r=lib.cr_native_prepare(n.d,S.view(cache.stage),source.used,target,S.View(C.cast(ii,C.c_void_p),len(ii)),
                   S.view(b''.join(blocks[i] for i in indices)),S.View(C.cast(oo,C.c_void_p),len(oo)),
                   S.view(n.stage),S.view(spare),S.view(aggregate),S.view(n.batch),S.view(n.undo))
                assert r.code==0,(r.code,r.vcode,r.detail)
                assert cache.accept(bytes(aggregate[:r.n]))==0
                coins=expected;assert S.coin_values(cache.flat())==coins and n.flat()==before and lib.cr_native_tip(n.d)==cursor
            reorganize(105,[103,104,105],V.coins_after(prefix+blocks[103:106]));compare(105)
            cut_cases.append(cache.flush(path,105,cut_dir));flushes+=1
            for i in (108,109):receive(i);assert core.call('submitblock',blocks[i].hex()) in (None,'inconclusive')
            reorganize(109,[101,102,108,109],V.coins_after(prefix+blocks[101:103]+blocks[108:110]));compare(109)
            cut_cases.append(cache.flush(path,109,cut_dir));flushes+=1
            opened=lib.cs_native_open(S.view(path.read_bytes()),S.view(n.stage))
            assert opened.code==0 and not opened.cut
            flat_n=lib.validation_native_canonical(S.view(n.stage),opened.used,S.view(n.canonical))
            assert S.table_values(bytes(n.canonical[:flat_n]))==S.table_values(n.flat())
            version=core.call('getnetworkinfo')['subversion']
        finally:core.close()
    evidence=dict(schema=1,version=version,indexed=indexed,states=states,flushes=flushes,
                  cache_bytes=cache.limit,delta_capacity=len(cache.delta),replayed_unflushed_blocks=2,
                  capacity_refused=capacity_refused,cut_cases=cut_cases)
    if record:record.write_text(json.dumps(evidence,indent=1)+'\n')
    print(f'coins cache: {flushes} durable flushes, full Core coins/MuHash across both reorgs, replayed two lost private blocks; {cache.limit} resident cache bytes',flush=True)


def main():
    p=argparse.ArgumentParser();p.add_argument('--shared',type=Path)
    p.add_argument('--core-dir',type=Path);p.add_argument('--record',type=Path)
    p.add_argument('--cut-dir',type=Path);p.add_argument('--indexed',action='store_true')
    p.add_argument('--tool',type=Path);p.add_argument('--base-tool',type=Path);a=p.parse_args()
    if a.tool:
        assert a.base_tool
        file_checks(a.tool,a.base_tool);return
    assert a.shared and a.core_dir and a.cut_dir
    lib=C.CDLL(str(a.shared.resolve()));configure(lib);journey(lib,a.core_dir,a.record,a.cut_dir,a.indexed)


def file_checks(tool,base_tool):
    fixture=L.make_fixture();prefix=list(map(bytes.fromhex,fixture['prefix']))
    blocks=prefix+list(map(bytes.fromhex,fixture['branches'][:2]))
    missing=L.transaction([(bytes([9])*32+bytes(4),b'')],[(1,b'\x51')])
    bad=L.block(L.sha256d(blocks[101][:80]),102,9,[missing])
    with tempfile.TemporaryDirectory(prefix='coax-cached-file-') as tmp:
        directory=Path(tmp);state=directory/'state.kv';headers=directory/'headers.bin'
        def write(path,data):
            with path.open('wb') as f:f.write(data);f.flush();os.fsync(f.fileno())
        write(headers,b''.join(b[:80] for b in blocks+[bad]))
        paths=[]
        for i,b in enumerate(blocks+[bad]):
            p=directory/f'{i}.blk';write(p,b);paths.append(p)
        def call(exe,*args,status=0):
            run=subprocess.run([str(exe.resolve()),*map(str,args)],capture_output=True,text=True,timeout=120)
            assert run.returncode==status,(run.returncode,run.stdout,run.stderr)
            return run.stdout
        def apply(every,selected,status=0,limit=1000000):
            return call(tool,'regtest',state,headers,2000000000,every,limit,*selected,status=status)
        def check(height):
            values,frames=L.replay(state.read_bytes());expected=V.coins_after(blocks[:height+1])
            flat=b''.join(struct.pack('<HI',len(k),len(v))+k+v for k,v in sorted(values.items()))
            assert S.coin_values(flat)==expected
            assert values[b'V']==struct.pack('<III',1,5,height)+L.sha256d(blocks[height][:80]) and b'L' not in values
            return len(frames),S.muhash(expected)
        call(base_tool,'init','regtest',state);initial=state.read_bytes()
        assert 'cache refused 2' in apply(25,paths[1:2],status=1,limit=589968)
        assert state.read_bytes()==initial
        apply(25,paths[1:101]);frames,_=check(100);assert frames==5
        prefix_file=state.read_bytes()
        assert 'refused 3' in apply(10,[paths[101],paths[-1]],status=1)
        assert state.read_bytes()==prefix_file,'failed private group appended unflushed A101'
        apply(10,paths[101:103]);frames,digest=check(102);assert frames==6
        assert digest=='750ba8d8990d5e5b14951087241be1ca2c0b6830ddd1225d70e9a67b4fdc4912'
        complete=state.read_bytes();assert 'cached blocks: 0' in apply(25,paths[1:103])
        assert state.read_bytes()==complete
        # A failure after an earlier successful group retains that durable group.
        write(state,prefix_file)
        assert 'refused 3' in apply(1,[paths[101],paths[-1]],status=1)
        check(101);assert state.read_bytes()!=prefix_file
        durable=state.read_bytes();write(state,durable+b'\x01\0')
        apply(25,paths[102:103],status=1);assert state.read_bytes()==durable+b'\x01\0'
        call(base_tool,'recover','regtest',state);assert state.read_bytes()==durable
        apply(25,paths[101:103]);check(102)
        # An independently written valid light-profile journal is still refused.
        light=directory/'light.kv';value=struct.pack('<III',1,5,0)+L.sha256d(blocks[0][:80])
        body=struct.pack('<QI',1,1)+b'\x01'+struct.pack('<HI',1,len(value))+b'L'+value
        write(light,initial[:20]+struct.pack('<I',len(body))+L.sha256d(body)[:4]+body)
        call(tool,'regtest',light,headers,2000000000,25,1000000,paths[1],status=1)
    print('cached file tool: budget refusal, four grouped prefix flushes, private failure rollback, earlier durable retention, idempotent resume, tail recovery and full coins/MuHash agree',flush=True)


if __name__=='__main__':main()
