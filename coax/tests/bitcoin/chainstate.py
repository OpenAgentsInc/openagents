#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python replays coins and Core undo, compares real Core state and rev files, and cuts atomic storage frames)
"""Chapter 58: validated connects, exact disconnects and durable snapshots.

The historical range supplies an independent archival parent context and the
complete subset of coins the range touches, not a global chainstate import.
The live regtest journey starts at genesis and compares the entire UTXO set.
"""
import argparse
import ctypes as C
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

import lightchain as L
import tx_reference as T
import utxo as U
import validation as V

View = V.View
view = V.view


class Result(C.Structure):
    _fields_ = [(n, C.c_int64) for n in ('code', 'vcode', 'detail', 'tx', 'input', 'used', 'n', 'undo_n')]


class Open(C.Structure):
    _fields_ = [('code', C.c_int64), ('used', C.c_int64), ('seq', C.c_uint64),
                ('frames', C.c_int64), ('keep', C.c_int64), ('cut', C.c_bool), ('mask', C.c_uint64)]


def configure(lib):
    for name, args, result in [
        ('validation_native_size', [], C.c_int64),
        ('validation_native_init', [View, C.c_int64, View], C.c_int64),
        ('validation_native_fixture', [View, C.c_int64, C.c_int64, View], C.c_int64),
        ('validation_native_indexed', [View, View], C.c_int64),
        ('validation_native_canonical', [View, C.c_int64, View], C.c_int64),
        ('cs_init_vartime', [C.c_int64, View, C.c_int64, View, View], Result),
        ('cs_native_fixture_tip', [View, C.c_int64, C.c_int64, C.c_int64, View], C.c_int64),
        ('cs_native_connect', [View, C.c_int64, View, View, C.c_int64, View, View, View], Result),
        ('cs_disconnect_vartime', [View, C.c_int64, View, View, View], Result),
        ('cs_native_undo', [View, C.c_int64, View], C.c_int64),
        ('cs_native_muhash', [View, C.c_int64, View], C.c_int64),
        ('cs_native_frame', [View, C.c_uint64, View], C.c_int64),
        ('cs_native_open', [View, View], Open),
    ]:
        fn = getattr(lib, name); fn.argtypes = args; fn.restype = result


def table_values(flat):
    values = {}; at = 0
    while at < len(flat):
        kn, vn = struct.unpack_from('<HI', flat, at); at += 6
        key = flat[at:at+kn]; at += kn
        assert key not in values
        values[key] = flat[at:at+vn]; at += vn
    assert at == len(flat)
    return values


def coin_values(flat):
    result = {}
    for key, value in table_values(flat).items():
        if key[:1] == b'C':
            assert len(key) == 37 and len(value) >= 13
            amount, height, cb = struct.unpack_from('<qIB', value)
            result[key[1:]] = (amount, height, bool(cb), value[13:])
    return result


def muhash(coins):
    # The existing independent ChaCha20/big-integer model, without executing
    # upstream reference code. The live Core node is the separate full-set oracle.
    packed = {point: struct.pack('<QIB', amount, height, cb)+script
              for point, (amount,height,cb,script) in coins.items()}
    return L.muhash(packed)[::-1].hex()


def undo_and_after(coins, raw, height):
    result = dict(coins); txs = V.parsed_block(raw); undo = T.compact(len(txs)-1)
    for ordinal, tx in enumerate(txs):
        if ordinal:
            undo += T.compact(len(tx['ins']))
            for vin in tx['ins']:
                amount, birth, cb, script = result.pop(vin['prevout'])
                record = U.coin_write(amount, birth, cb, script)
                code = U.varint(birth * 2 + cb)
                assert record.startswith(code)
                undo += code + (b'\0' if birth else b'') + record[len(code):]
        for n, out in enumerate(tx['outs']):
            script = out['script']
            if len(script) <= 10000 and script[:1] != b'\x6a':
                result[T.txid(tx) + struct.pack('<I', n)] = (int.from_bytes(out['value'], 'little', signed=True), height, not ordinal, script)
    return undo, result


class Node:
    def __init__(self, lib, indexed, capacity=64000000):
        self.lib = lib; self.indexed = indexed; self.used = 0; self.sequence = 0
        self.state = (C.c_uint8 * lib.validation_native_size())()
        self.d = View(C.cast(self.state, C.c_void_p), 1)
        self.table = (C.c_uint8 * capacity)(); self.stage = (C.c_uint8 * capacity)()
        self.batch = (C.c_uint8 * 64000000)(); self.undo = (C.c_uint8 * 32000000)()
        self.canonical = (C.c_uint8 * capacity)()
        header = b'OAKV\x01\0\0\0' + bytes(8)
        self.journal = header + L.sha256d(header)[:4]

    def flat(self):
        n = self.lib.validation_native_canonical(view(self.table), self.used, view(self.canonical))
        assert n >= 0
        return bytes(self.canonical[:n])

    def accept(self, r):
        assert r.code == 0, (r.code, r.vcode, r.detail, r.tx, r.input)
        C.memmove(self.table, self.stage, r.used); self.used = r.used
        frame = (C.c_uint8 * (r.n+8))()
        assert self.lib.cs_native_frame(View(C.cast(self.batch, C.c_void_p), r.n), self.sequence+1, view(frame)) == len(frame)
        self.sequence += 1; self.journal += bytes(frame)
        return bytes(frame)

    def initialize(self):
        # Init produces a canonical packed table. Reopening the accepted frame
        # below checks the selected representation without migrating any light state.
        r = self.lib.cs_init_vartime(5, view(self.table), 0, view(self.stage), view(self.batch))
        self.accept(r)
        if self.indexed:
            flat = self.flat()
            self.used = self.lib.validation_native_indexed(view(self.table), view(flat))
            assert self.used > 0

    def connect(self, height, raw, expected):
        before = self.flat()
        undo, after = undo_and_after(expected, raw, height)
        r = self.lib.cs_native_connect(self.d, height, view(raw), view(self.table), self.used,
                                        view(self.stage), view(self.batch), view(self.undo))
        assert self.flat() == before, 'preparation changed published state'
        assert r.code == 0, (height, r.code, r.vcode, r.detail, r.tx, r.input)
        assert bytes(self.undo[:r.undo_n]) == undo, (height, 'Core undo serialization')
        frame = self.accept(r)
        assert coin_values(self.flat()) == after, (height, 'post-block coin map')
        assert self.lib.cs_native_undo(view(self.table), self.used, view(self.undo)) == len(undo)
        assert bytes(self.undo[:len(undo)]) == undo
        return after, undo, frame

    def disconnect(self, expected):
        before = self.flat()
        r = self.lib.cs_disconnect_vartime(view(self.table), self.used, view(self.stage), view(self.batch), view(self.undo))
        assert self.flat() == before
        self.accept(r)
        assert coin_values(self.flat()) == expected

    def own_muhash(self):
        out = (C.c_uint8 * 32)()
        assert self.lib.cs_native_muhash(view(self.table), self.used, view(out)) == 0
        return bytes(out)[::-1].hex()

    def crash_cuts(self, prefix, frame, old, new):
        # Every physical cut must retain exactly the old or complete new frame.
        replay = (C.c_uint8 * len(self.table))()
        data = prefix+frame
        for cut in range(len(frame)+1):
            r = self.lib.cs_native_open(view(data[:len(prefix)+cut]), view(replay))
            assert r.code == 0, (cut, r.code)
            flat = bytes(replay[:r.used])
            assert flat == (new if cut == len(frame) else old), cut
            assert r.keep == (len(data) if cut == len(frame) else len(prefix)), cut
        print(f'atomic frame: all {len(frame)+1} physical cuts retain one complete snapshot', flush=True)


def core_rev(path, chain):
    key = (path/'xor.dat').read_bytes()
    stored = (path/'rev00000.dat').read_bytes()
    raw = bytes(b ^ key[i % 8] for i, b in enumerate(stored))
    at = 0; records = []
    for body in chain[1:]:
        assert raw[at:at+4] == bytes.fromhex('fabfb5da')
        n = struct.unpack_from('<I', raw, at+4)[0]
        undo = raw[at+8:at+8+n]
        assert raw[at+8+n:at+40+n] == L.sha256d(body[4:36] + undo)
        records.append(undo); at += 40+n
    assert not any(stored[at:]) or not any(raw[at:]), 'nonzero unparsed undo records'
    return records


def regtest(lib, indexed, core_dir, record):
    fixture = L.make_fixture(); prefix = [bytes.fromhex(b) for b in fixture['prefix']]
    # Two blocks include a mature spend and a child in the same block.
    branch = [bytes.fromhex(b) for b in fixture['branches'][:2]] if 'branches' in fixture else [bytes.fromhex(b) for b in fixture['branch_a']]
    chain = prefix+branch
    node = Node(lib, indexed)
    assert lib.validation_native_init(node.d, 5, view(b''.join(b[:80] for b in chain))) == len(chain)
    node.initialize(); coins = {}; own_undos = []; snapshots = []; verdicts = []
    with tempfile.TemporaryDirectory(prefix='coax-chainstate-core-') as tmp:
        directory = Path(tmp); core = V.Core(core_dir.resolve()/'bitcoind', directory)
        try:
            for height, body in enumerate(chain[1:], 1):
                if height >= 100: snapshots.append(dict(coins))
                old, journal = node.flat(), node.journal
                coins, undo, frame = node.connect(height, body, coins); own_undos.append(undo)
                assert core.call('submitblock', body.hex()) is None
                if height in (100,101,102):
                    expected = muhash(coins); actual = core.call('gettxoutsetinfo', 'muhash')
                    assert actual['muhash'] == expected == node.own_muhash()
                    verdicts.append(dict(height=height, hash=L.sha256d(body[:80])[::-1].hex(), muhash=expected, undo_sha256=hashlib.sha256(undo).hexdigest()))
                if height == 101: node.crash_cuts(journal, frame, old, node.flat())
            for height in (102,101):
                node.disconnect(snapshots[height-100])
                core.call('invalidateblock', L.sha256d(chain[height][:80])[::-1].hex())
                actual = core.call('gettxoutsetinfo', 'muhash')['muhash']
                assert actual == muhash(snapshots[height-100]) == node.own_muhash()
            version = core.call('getnetworkinfo')['subversion']
        finally:
            core.close()
        records = core_rev(directory/'regtest/blocks', chain)
        assert records == own_undos, 'actual Core rev bodies differ'
    if record:
        record.write_text(json.dumps(dict(schema=1, version=version, states=verdicts,
                                          undo_records=len(records), disconnects=2), indent=1)+'\n')
    print(f'regtest: {len(records)} Core undo bodies match byte for byte; full UTXO/MuHash agrees through both disconnects', flush=True)


def historical(lib, indexed, paths):
    cases = [json.loads(p.read_text()) for p in paths]
    assert all(c['chain'] == cases[0]['chain'] for c in cases)
    assert [c['height'] for c in cases] == list(range(cases[0]['height'], cases[-1]['height']+1))
    coins = {}; created = set(); headers = {}
    for c in cases:
        headers.update(c['headers'])
        for point, (a,h,cb,s) in c['coins'].items():
            key = bytes.fromhex(point); coin = (a,h,bool(cb),bytes.fromhex(s))
            if key not in created:
                assert key not in coins or coins[key] == coin
                coins[key] = coin
        for tx in V.parsed_block(bytes.fromhex(c['raw'])):
            created.update(T.txid(tx)+struct.pack('<I', i) for i in range(len(tx['outs'])))
    node = Node(lib, indexed)
    records = b''.join(struct.pack('<I', int(h))+bytes.fromhex(b) for h,b in sorted(headers.items(),key=lambda x:int(x[0])))
    assert lib.validation_native_fixture(node.d, cases[0]['chain'], cases[-1]['height'], view(records)) == cases[-1]['height']
    flat = V.packed(coins)
    if indexed:
        node.used = lib.validation_native_indexed(view(node.table), view(flat))
    else:
        C.memmove(node.table, flat, len(flat)); node.used = len(flat)
    parent = bytes.fromhex(cases[0]['raw'])[4:36]
    node.used = lib.cs_native_fixture_tip(view(node.table), node.used, cases[0]['chain'], cases[0]['height']-1, view(parent))
    original = node.flat(); before = dict(coins); prior = []
    for c in cases:
        prior.append(dict(coins)); coins, undo, _ = node.connect(c['height'], bytes.fromhex(c['raw']), coins)
        assert node.own_muhash() == muhash(coins)
        print(f'{c["network"]} {c["height"]}: validated, {c["transactions"]} transactions, {len(undo)} Core undo bytes', flush=True)
    for expected in reversed(prior): node.disconnect(expected)
    assert node.flat() == original and coins != before
    assert node.own_muhash() == muhash(before)
    print(f'historical range: exact original state and MuHash restored after {len(cases)} disconnects; independent archival parent boundary', flush=True)


def file_checks(tool):
    fixture = L.make_fixture(); blocks = list(map(bytes.fromhex, fixture['prefix']+fixture['branches'][:2]))
    bad = next(b for name,b,_,_ in V.cases(blocks[:101]) if name == 'missing')
    with tempfile.TemporaryDirectory(prefix='coax-validated-file-') as tmp:
        directory = Path(tmp); state = directory/'state.kv'; headers = directory/'headers.bin'
        headers.write_bytes(b''.join(b[:80] for b in blocks+[bad]))
        paths = []
        for i,b in enumerate(blocks+[bad]):
            path = directory/f'{i}.blk'; path.write_bytes(b); paths.append(path)
        def call(*args, status=0):
            run = subprocess.run([str(tool.resolve()), *map(str,args)], capture_output=True, text=True, timeout=120)
            assert run.returncode == status, (run.returncode,run.stdout,run.stderr)
            return run.stdout
        def check(coins, height):
            values,_ = L.replay(state.read_bytes())
            own = {k[1:]: (struct.unpack_from('<q',v)[0], struct.unpack_from('<I',v,8)[0],bool(v[12]),v[13:]) for k,v in values.items() if k[:1] == b'C'}
            assert own == coins and b'L' not in values
            assert values[b'V'] == struct.pack('<III',1,5,height)+L.sha256d(blocks[height][:80])
        call('init','regtest',state); call('init','regtest',state,status=2)
        call('apply','regtest',state,headers,2000000000,*paths[1:101])
        coins = V.coins_after(blocks[:101]); check(coins,100); original = state.read_bytes()
        assert 'refused 3' in call('apply','regtest',state,headers,2000000000,paths[-1],status=1)
        assert state.read_bytes() == original
        call('apply','regtest',state,headers,2000000000,*paths[101:103]); check(V.coins_after(blocks),102)
        call('disconnect','regtest',state); check(V.coins_after(blocks[:102]),101)
        call('disconnect','regtest',state); check(coins,100)
        complete = state.read_bytes(); state.write_bytes(complete+b'\x01\x00')
        call('tip','regtest',state,status=1); assert state.read_bytes() == complete+b'\x01\x00'
        call('recover','regtest',state); assert state.read_bytes() == complete
        call('tip','regtest',state)
        # A well-formed durable LIGHT profile is refused, even at genesis.
        header = b'OAKV\x01\0\0\0'+bytes(8); meta = struct.pack('<III',1,5,0)+L.sha256d(blocks[0][:80])
        body = struct.pack('<QI',1,1)+b'\x01\x01\x00'+struct.pack('<I',len(meta))+b'L'+meta
        light = directory/'light.kv'; light.write_bytes(header+L.sha256d(header)[:4]+struct.pack('<I',len(body))+L.sha256d(body)[:4]+body)
        call('tip','regtest',light,status=1)
    print('real file tool: full coins agree through validated connects, refusal, both disconnects and torn-tail recovery; light profile refused',flush=True)


def sanitized_case(binary, network, height, headers, body, coins, sparse, indexed):
    flat = V.packed(coins)
    q = struct.pack('<6Q',int(sparse)+2*indexed,network,height,len(headers),len(body),len(flat))
    run = subprocess.run([str(binary.resolve())],input=q+headers+body+flat,capture_output=True,timeout=720)
    assert run.returncode == 0, (run.returncode,run.stderr.decode(errors='replace')[-4000:])
    assert not run.stderr,run.stderr.decode(errors='replace')
    assert len(run.stdout) >= 96
    header, = struct.unpack_from('<q',run.stdout)
    r = Result.from_buffer_copy(run.stdout[8:72]); an,un,rn = struct.unpack_from('<3q',run.stdout,72)
    after = run.stdout[96:96+an]; undo = run.stdout[96+an:96+an+un]; restored = run.stdout[96+an+un:]
    assert len(restored) == rn
    if header >= 0:
        assert coin_values(restored) == coins
        assert table_values(restored)[b'V'] == struct.pack('<III',1,network,height-1)+body[4:36]
    if r.code == 0:
        expected_undo, expected = undo_and_after(coins,body,height)
        assert coin_values(after) == expected and undo == expected_undo
    else:
        assert an == un == 0 and r.n == 0
    return header,r


def sanitized(binary,indexed,paths,only):
    if paths:
        for path in paths:
            c = json.loads(path.read_text()); sparse = c['height'] != 1
            headers = (b''.join(struct.pack('<I',int(h))+bytes.fromhex(b) for h,b in c['headers'].items()) if sparse
                       else b''.join(bytes.fromhex(c['headers'][str(h)]) for h in range(2)))
            coins = {bytes.fromhex(p):(a,h,bool(cb),bytes.fromhex(s)) for p,(a,h,cb,s) in c['coins'].items()}
            _,r = sanitized_case(binary,c['chain'],c['height'],headers,bytes.fromhex(c['raw']),coins,sparse,indexed)
            assert r.code == 0,(path.name,r.code,r.vcode,r.detail)
            print(f'ASan/UBSan {path.name}: validated, Core undo and exact disconnect agree',flush=True)
    else:
        prefix = list(map(bytes.fromhex,L.make_fixture()['prefix'])); coins = V.coins_after(prefix)
        pinned = json.loads((V.ROOT/'bitcoin/fixtures/validation/core-regtest.json').read_text())['cases']
        for reference,(name,b,expected,_) in zip(pinned,V.cases(prefix),strict=True):
            assert reference['name'] == name and reference['block'] == hashlib.sha256(b).hexdigest() and reference['reason'] == expected
            if only and name not in only: continue
            h,r = sanitized_case(binary,5,101,b''.join(x[:80] for x in prefix)+b[:80],b,coins,False,indexed)
            if h < 0:
                reason = {2:'high-hash',11:'bad-diffbits',12:'time-too-old',14:'time-too-new',15:'bad-version'}[-100-h]
            else:
                assert r.code in (0,3)
                reason = V.own_reason(V.Result(r.vcode,r.detail,r.tx,r.input,0,0,0)) if r.code else 'ok'
            assert reason == expected,(name,reason,expected)
            print(f'ASan/UBSan {name}: {reason}; published state preserved',flush=True)


def main():
    p = argparse.ArgumentParser(); p.add_argument('--shared', type=Path)
    p.add_argument('--core-dir', type=Path); p.add_argument('--historical', nargs='+', type=Path)
    p.add_argument('--indexed', action='store_true'); p.add_argument('--record', type=Path)
    p.add_argument('--tool',type=Path)
    p.add_argument('--sanitized-binary',type=Path)
    p.add_argument('--case',action='append',help='slice the recorded sanitizer corpus by exact case name')
    a = p.parse_args()
    if a.tool:
        file_checks(a.tool); return
    if a.sanitized_binary:
        sanitized(a.sanitized_binary,a.indexed,a.historical,a.case); return
    assert a.shared
    lib = C.CDLL(str(a.shared.resolve())); configure(lib)
    if a.historical: historical(lib, a.indexed, a.historical)
    else:
        assert a.core_dir
        regtest(lib, a.indexed, a.core_dir, a.record)


if __name__ == '__main__': main()
