#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python models coins and crafts mined blocks, and an isolated Bitcoin Core reference decides their consensus verdicts)
"""Chapter 57 differential block validation; no keys or wallet required.

--core-dir contains the official bitcoind executable, independently hash
verified before this tool runs it. The node has no peers and no wallet.
Each case forks the same 100-block prefix. Invalid cases never advance it.
"""
import argparse
import ctypes as C
import hashlib
import json
import re
from pathlib import Path
import socket
import struct
import subprocess
import sys
import tempfile
import time
import urllib.request

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
import lightchain as L
import tx_reference as T


class View(C.Structure):
    _fields_ = [('data', C.c_void_p), ('count', C.c_int64)]


class Result(C.Structure):
    _fields_ = [(n, C.c_int64) for n in ('code', 'detail', 'tx', 'input', 'fees', 'sigops', 'used')]


def view(data):
    if isinstance(data, bytes):
        data = (C.c_uint8 * len(data)).from_buffer_copy(data)
    return View(C.cast(data, C.c_void_p), len(data))


def free_port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]


class Core:
    def __init__(self, exe, directory, network='regtest'):
        self.directory = directory
        self.network = network
        self.port = free_port()
        self.log = (directory / 'process.log').open('wb')
        self.p = subprocess.Popen([str(exe), f'-datadir={directory}', '-chain=' + network, '-server', '-listen=0',
                                   '-networkactive=0', '-disablewallet', '-printtoconsole=0',
                                   '-checkblocks=0', '-persistmempool=0', f'-rpcport={self.port}'],
                                  stdout=self.log, stderr=self.log)
        try:
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                if self.p.poll() is not None:
                    raise AssertionError((directory / 'process.log').read_text())
                try:
                    self.call('getblockcount')
                    return
                except (OSError, ValueError):
                    time.sleep(.1)
            raise AssertionError('isolated Core startup timed out')
        except BaseException:
            if self.p.poll() is None:
                self.p.terminate()
                self.p.wait(timeout=30)
            self.log.close()
            raise

    def call(self, method, *params):
        import base64
        cookie = (self.directory / self.network / '.cookie').read_bytes().strip()
        body = json.dumps(dict(jsonrpc='2.0', id=1, method=method, params=params)).encode()
        request = urllib.request.Request(f'http://127.0.0.1:{self.port}', body,
                                         {'Content-Type': 'application/json', 'Authorization': 'Basic ' + base64.b64encode(cookie).decode()})
        with urllib.request.urlopen(request, timeout=20) as response:
            answer = json.load(response)
        if answer.get('error', {}).get('code') == -28:
            raise ValueError('Core is warming up')
        assert not answer.get('error'), (method, answer.get('error'))
        return answer['result']

    def close(self):
        if self.p.poll() is None:
            self.call('stop')
            self.p.wait(timeout=30)
        self.log.close()


def coins_after(prefix):
    coins = {}
    for height, raw in enumerate(prefix):
        if not height:
            continue  # Core never inserts the genesis coinbase.
        for ident, inputs, outputs in L.transactions(raw):
            cb = inputs[0] == bytes(32) + b'\xff' * 4
            if not cb:
                for point in inputs:
                    del coins[point]
            for n, (amount, script) in enumerate(outputs):
                if len(script) <= 10000 and script[:1] != b'\x6a':
                    coins[ident + struct.pack('<I', n)] = (amount, height, cb, script)
    return coins


def packed(coins):
    records = []
    for point, (amount, height, cb, script) in sorted(coins.items()):
        key = b'C' + point
        value = struct.pack('<qIB', amount, height, cb) + script
        records.append(struct.pack('<HI', len(key), len(value)) + key + value)
    return b''.join(records)


def parsed_block(raw):
    """Independent streaming boundary reader, then chapter 34's full reader."""
    r = T.Reader(raw, (1 << 30,) * 3)
    r.take(80)
    count = r.compact()
    txs = []
    for _ in range(count):
        start = r.at
        r.take(4)
        inputs = T.read_inputs(r, 1 << 30)
        flags = 0
        if not inputs:
            flags = r.take(1)[0]
            if flags:
                inputs = T.read_inputs(r, 1 << 30)
                T.read_outputs(r, 1 << 30)
        else:
            T.read_outputs(r, 1 << 30)
        if flags & 1:
            for _ in inputs:
                for _ in range(r.compact()):
                    r.bytes()
            flags ^= 1
        assert not flags
        r.take(4)
        txs.append(T.unserialize(raw[start:r.at], True))
    assert r.at == len(raw)
    return txs


def after_block(coins, raw, height):
    result = dict(coins)
    if not height:
        return result
    for tx in parsed_block(raw):
        cb = T.is_coinbase(tx)
        if not cb:
            for vin in tx['ins']:
                del result[vin['prevout']]
        for n, out in enumerate(tx['outs']):
            script = out['script']
            if len(script) <= 10000 and script[:1] != b'\x6a':
                result[T.txid(tx) + struct.pack('<I', n)] = (int.from_bytes(out['value'], 'little', signed=True), height, cb, script)
    return result


def mined(parent, height, txs, change=None, high_hash=False):
    header = bytearray(struct.pack('<I', 4) + parent + L.merkle(T.txid(T.unserialize(t, True)) for t in txs)
                       + struct.pack('<III', 1296688602 + height * 600, 0x207fffff, 0))
    if change:
        change(header)
    bits = struct.unpack_from('<I', header, 72)[0]
    target = (bits & 0x7fffff) << (8 * ((bits >> 24) - 3))
    for nonce in range(1000000):
        struct.pack_into('<I', header, 76, nonce)
        under = int.from_bytes(L.sha256d(header), 'little') <= target
        if under != high_hash:
            return bytes(header) + T.compact(len(txs)) + b''.join(txs)
    raise AssertionError('regtest mining bound exceeded')


def cases(prefix):
    parent = L.sha256d(prefix[-1][:80])
    point = L.sha256d(L.coinbase(1, 1)) + bytes(4)
    spend = L.transaction([(point, b'')], [(4999999000, b'\x51')])
    child = L.transaction([(L.sha256d(spend) + bytes(4), b'')], [(4999998000, b'\x51')])
    tag = 1
    def block(txs, cb=None, change=None, high_hash=False):
        nonlocal tag
        tag += 1
        return mined(parent, 101, [L.coinbase(101, tag) if cb is None else cb, *txs], change, high_hash)
    yield 'valid-parent-child', block([spend, child]), 'ok', 2000
    yield 'missing', block([L.transaction([(b'\x09' * 32 + bytes(4), b'')], [(1, b'\x51')])]), 'bad-txns-inputs-missingorspent', None
    immature = L.sha256d(L.coinbase(2, 1)) + bytes(4)
    yield 'immature', block([L.transaction([(immature, b'')], [(4999999000, b'\x51')])]), 'bad-txns-premature-spend-of-coinbase', None
    yield 'overspend', block([L.transaction([(point, b'')], [(5000000001, b'\x51')])]), 'bad-txns-in-belowout', None
    double = L.transaction([(point, b'')], [(4999998000, b'\x51')])
    yield 'double-spend', block([spend, double]), 'bad-txns-inputs-missingorspent', None
    dup = L.transaction([(point, b''), (point, b'')], [(1, b'\x51')])
    yield 'duplicate-input', block([dup]), 'bad-txns-inputs-duplicate', None
    overcb = L.transaction([(bytes(32) + b'\xff' * 4, L.height_script(101) + b'\x02')], [(5000001001, b'\x51')])
    yield 'coinbase-overpay', block([spend], overcb), 'bad-cb-amount', None
    badheight = L.transaction([(bytes(32) + b'\xff' * 4, b'\x01\x64')], [(5000000000, b'\x51')])
    yield 'coinbase-height', block([], badheight), 'bad-cb-height', None
    nonfinal = bytearray(spend)
    nonfinal[-4:] = struct.pack('<I', 101)
    nonfinal[42:46] = bytes(4)
    yield 'absolute-lock', block([bytes(nonfinal)]), 'bad-txns-nonfinal', None
    relative = bytearray(spend)
    relative[:4] = struct.pack('<I', 2)
    relative[42:46] = struct.pack('<I', 101)
    yield 'relative-lock', block([bytes(relative)]), 'bad-txns-nonfinal', None
    # A parent creates OP_FALSE; a child then attempts to spend it.
    false_parent = L.transaction([(point, b'')], [(4999999000, b'\x00')])
    false_child = L.transaction([(L.sha256d(false_parent) + bytes(4), b'')], [(4999998000, b'\x51')])
    yield 'false-script', block([false_parent, false_child]), 'block-script-verify-flag-failed', None
    yield 'output-empty', block([L.transaction([(point, b'')], [])]), 'bad-txns-vout-empty', None
    yield 'output-negative', block([L.transaction([(point, b'')], [(-1, b'\x51')])]), 'bad-txns-vout-negative', None
    yield 'output-too-large', block([L.transaction([(point, b'')], [(2100000000000001, b'\x51')])]), 'bad-txns-vout-toolarge', None
    yield 'output-total', block([L.transaction([(point, b'')], [(2100000000000000, b'\x51')] * 2)]), 'bad-txns-txouttotal-toolarge', None
    null = bytes(32) + b'\xff' * 4
    yield 'null-prevout', block([L.transaction([(point, b''), (null, b'')], [(1, b'\x51')])]), 'bad-txns-prevout-null', None
    for length in (1, 101):
        cb = L.transaction([(null, b'\x51' * length)], [(5000000000, b'\x51')])
        yield f'coinbase-length-{length}', block([], cb), 'bad-cb-length', None
    yield 'multiple-coinbase', block([L.coinbase(101, 90)]), 'bad-cb-multiple', None
    yield 'no-coinbase', mined(parent, 101, [spend]), 'bad-cb-missing', None
    # Different coinbase from the valid case prevents a cached-invalid
    # header from masking the duplicate-leaf check (CVE-2012-2459).
    yield 'merkle-duplicate', block([spend, child, child]), 'bad-txns-duplicate', None
    def wrong_root(h): h[36] ^= 1
    yield 'merkle-root', block([spend], change=wrong_root), 'bad-txnmrklroot', None
    huge_cb = L.transaction([(null, L.height_script(101) + b'\x7f')], [(1, b'\x51' * 1000000)])
    yield 'stripped-size', block([], huge_cb), 'bad-blk-length', None
    for n, reason in ((20000, 'ok'), (20001, 'bad-blk-sigops')):
        cb = L.transaction([(null, L.height_script(101) + b'\x7e')], [(1, b'\xac' * n)])
        yield f'legacy-sigops-{n}', block([], cb), reason, 0 if reason == 'ok' else None

    def witness_block(kind):
        nonlocal tag
        tag += 1
        cb = T.unserialize(L.coinbase(101, tag), True)
        other = [spend]
        if kind == 'weight':
            t = T.unserialize(spend, True)
            t['ins'][0]['witness'] = [b'\x01' * 4000000]
            other = [T.serialize(t, True)]
        root = L.merkle([bytes(32), *[T.wtxid(T.unserialize(t, True)) for t in other]])
        commitment = bytearray(L.sha256d(root + bytes(32)))
        if kind == 'mismatch': commitment[0] ^= 1
        if kind != 'unexpected':
            cb['outs'].append(dict(value=bytes(8), script=bytes.fromhex('6a24aa21a9ed') + commitment))
        cb['ins'][0]['witness'] = [bytes(31 if kind == 'nonce' else 32)]
        return mined(parent, 101, [T.serialize(cb, True), *other])
    for kind, reason in (('valid', 'ok'), ('nonce', 'bad-witness-nonce-size'), ('mismatch', 'bad-witness-merkle-match'),
                         ('unexpected', 'unexpected-witness'), ('weight', 'bad-blk-weight')):
        yield 'witness-' + kind, witness_block(kind), reason, 1000 if reason == 'ok' else None
    yield 'header-high-hash', block([], high_hash=True), 'high-hash', None
    yield 'header-bad-version', block([], change=lambda h: struct.pack_into('<I', h, 0, 3)), 'bad-version', None
    yield 'header-time-old', block([], change=lambda h: struct.pack_into('<I', h, 68, 1296688602 + 95 * 600)), 'time-too-old', None
    yield 'header-time-new', block([], change=lambda h: struct.pack_into('<I', h, 68, 2000008000)), 'time-too-new', None
    yield 'header-difficulty', block([], change=lambda h: struct.pack_into('<I', h, 72, 0x207ffffe)), 'bad-diffbits', None


def own_reason(r):
    if r.code == 0: return 'ok'
    if r.code == 3:
        return ['ok', 'bad block', 'bad-txnmrklroot', 'bad-txns-duplicate', 'bad-blk-length', 'bad-cb-missing', 'bad-cb-multiple', 'bad transaction', 'bad-blk-sigops', 'bad-blk-weight', 'capacity'][r.detail]
    if r.code == 4:
        return ['ok', 'bad-txns-vin-empty', 'bad-txns-vout-empty', 'bad-txns-oversize', 'bad-txns-vout-negative', 'bad-txns-vout-toolarge', 'bad-txns-txouttotal-toolarge', 'bad-txns-inputs-duplicate', 'bad-cb-length', 'bad-txns-prevout-null', 'bad-txns-inputs-missingorspent', 'bad-txns-premature-spend-of-coinbase', 'bad-txns-inputvalues-outofrange', 'bad-txns-in-belowout', 'bad-txns-fee-outofrange', 'bad-txns-nonfinal', 'bad context'][r.detail]
    if r.code == 6: return ['ok', 'bad-cb-height', 'bad-cb-amount', 'bad-txns-accumulated-fee-outofrange', 'bad context'][r.detail]
    if r.code == 5: return {1: 'bad-witness-nonce-size', 2: 'bad-witness-merkle-match', 3: 'unexpected-witness'}[r.detail]
    return {1: 'bad context', 2: 'capacity', 7: 'bad-txns-BIP30', 8: 'block-script-verify-flag-failed', 9: 'bad-signet-blksig'}.get(r.code, 'witness')


def core_script_text(code):
    upstream = HERE.parent / 'vectors/upstream/bitcoin-core'
    pins = {'script_error.h': 'a2ea50346edfa59b979747b5d7573027989fabdad90af103119d23ff9db46bb9',
            'script_error.cpp': '92404c34ad460931639e6fb05c615d06c2a3410d7ac357084226c59ca72382c3'}
    texts = {}
    for name, digest in pins.items():
        data = (upstream / name).read_bytes()
        assert hashlib.sha256(data).hexdigest() == digest
        texts[name] = data.decode()
    names = re.findall(r'^\s*(SCRIPT_ERR_[A-Z0-9_]+)(?:\s*=\s*\d+)?\s*,', texts['script_error.h'], re.M)
    messages = dict(re.findall(r'case (SCRIPT_ERR_[A-Z0-9_]+):\s*return "([^"\n]+)";', texts['script_error.cpp']))
    return messages[names[code]]


def sanitized_case(binary, network, height, headers, block, before, sparse=False, indexed=False):
    request = struct.pack('<6Q', int(sparse) | (2 if indexed else 0), network, height, len(headers), len(block), len(before)) + headers + block + before
    run = subprocess.run([str(binary.resolve())], input=request, capture_output=True, timeout=720)
    assert run.returncode == 0, run.stderr.decode(errors='replace')
    assert not run.stderr, run.stderr.decode(errors='replace')
    count = struct.unpack_from('<q', run.stdout)[0]
    result = Result.from_buffer_copy(run.stdout[8:64])
    length = struct.unpack_from('<q', run.stdout, 64)[0]
    after = run.stdout[72:]
    assert len(after) == length and (result.code == 0 or length == 0)
    return count, result, after


def native_table(lib, before, indexed):
    table = (C.c_uint8 * (len(before) * 2 + 16000000))()
    if indexed:
        lib.validation_native_indexed.argtypes = [View, View]
        lib.validation_native_indexed.restype = C.c_int64
        used = lib.validation_native_indexed(view(table), view(before))
        assert used >= 256
    else:
        used = len(before)
        C.memmove(table, before, used)
    return table, used


def native_after(lib, table, used, indexed):
    if not indexed:
        return bytes(table[:used])
    lib.validation_native_canonical.argtypes = [View, C.c_int64, View]
    lib.validation_native_canonical.restype = C.c_int64
    out = (C.c_uint8 * len(table))()
    n = lib.validation_native_canonical(view(table), used, view(out))
    assert n >= 0
    return bytes(out[:n])


def exercise(lib, core, prefix, reference=None, indexed=False):
    native = not isinstance(lib, Path)
    if native:
        lib.validation_native_size.restype = C.c_int64
        lib.validation_native_init.argtypes = [View, C.c_int64, View]
        lib.validation_native_init.restype = C.c_int64
        lib.validation_native_block.argtypes = [View, C.c_int64, View, View, C.c_int64]
        lib.validation_native_block.restype = Result
        state = (C.c_uint8 * lib.validation_native_size())()
    headers = b''.join(raw[:80] for raw in prefix)
    coins = coins_after(prefix)
    before = packed(coins)
    records = []
    for label, block, expected, fee in cases(prefix):
        # The header is valid even when its body fails consensus.
        if native:
            count = lib.validation_native_init(View(C.cast(state, C.c_void_p), 1), 5, view(headers + block[:80]))
        else:
            count, result, after = sanitized_case(lib, 5, len(prefix), headers + block[:80], block, before, indexed=indexed)
        if count < 0:
            code = -100-count
            got = {2: 'high-hash', 11: 'bad-diffbits', 12: 'time-too-old', 14: 'time-too-new', 15: 'bad-version'}[code]
            result = None
        else:
            assert count == len(prefix) + 1, (label, 'header', count)
            if native:
                table, used = native_table(lib, before, indexed)
                result = lib.validation_native_block(View(C.cast(state, C.c_void_p), 1), len(prefix), view(block), view(table), used)
            got = own_reason(result)
        answer = core.call('submitblock', block.hex())
        if result is not None and result.code == 8:
            assert answer == 'block-script-verify-flag-failed (' + core_script_text(result.detail) + ')'
        if answer is not None and answer.startswith('bad-version('):
            assert answer == f'bad-version(0x{int.from_bytes(block[:4], "little"):08x})'
        actual = 'ok' if answer is None else answer.split(' (')[0]
        if actual.startswith('bad-version('): actual = 'bad-version'
        assert actual == expected, (label, 'Core', actual, expected)
        assert got == actual, (label, 'own', got, actual)
        if fee is not None:
            assert result.fees == fee, (label, result.fees, fee)
            if native:
                after = native_after(lib, table, result.used, indexed)
            assert after == packed(after_block(coins, block, len(prefix))), (label, 'post-block coins differ')
            core.call('invalidateblock', L.sha256d(block[:80])[::-1].hex())
        print(f'{label}: {got}', flush=True)
        records.append(dict(name=label, block=hashlib.sha256(block).hexdigest(), hash=L.sha256d(block[:80])[::-1].hex(),
                            bytes=len(block), verdict=answer, reason=actual, fees=fee))
    if reference:
        reference.write_text(json.dumps(dict(schema=1, source='isolated Bitcoin Core, peers and wallet disabled',
                                             version=core.call('getnetworkinfo')['subversion'], cases=records), indent=1) + '\n')
    return len(records)


def historical(lib, paths, indexed=False):
    native = not isinstance(lib, Path)
    if native:
        lib.validation_native_size.restype = C.c_int64
        lib.validation_native_fixture.argtypes = [View, C.c_int64, C.c_int64, View]
        lib.validation_native_fixture.restype = C.c_int64
        lib.validation_native_init.argtypes = [View, C.c_int64, View]
        lib.validation_native_init.restype = C.c_int64
        lib.validation_native_block.argtypes = [View, C.c_int64, View, View, C.c_int64]
        lib.validation_native_block.restype = Result
    for path in paths:
        case = json.loads(path.read_text())
        records = b''.join(struct.pack('<I', int(h)) + bytes.fromhex(raw) for h, raw in case['headers'].items())
        coins = {bytes.fromhex(point): (a, h, cb, bytes.fromhex(s)) for point, (a, h, cb, s) in case['coins'].items()}
        before = packed(coins)
        started = time.monotonic()
        if native:
            state = (C.c_uint8 * lib.validation_native_size())()
            d = View(C.cast(state, C.c_void_p), 1)
            if case['height'] == 1:
                headers = b''.join(bytes.fromhex(case['headers'][str(h)]) for h in range(2))
                assert lib.validation_native_init(d, case['chain'], view(headers)) == 2
            else:
                assert lib.validation_native_fixture(d, case['chain'], case['height'], view(records)) == case['height']
            table, used = native_table(lib, before, indexed)
            r = lib.validation_native_block(d, case['height'], view(bytes.fromhex(case['raw'])), view(table), used)
            after = native_after(lib, table, r.used, indexed) if r.code == 0 else b''
        else:
            sparse = case['height'] != 1
            headers = records if sparse else b''.join(bytes.fromhex(case['headers'][str(h)]) for h in range(2))
            count, r, after = sanitized_case(lib, case['chain'], case['height'], headers, bytes.fromhex(case['raw']), before, sparse, indexed)
            assert count == (case['height'] if sparse else 2)
        assert r.code == 0, (path.name, own_reason(r), r.tx, r.input, r.detail)
        assert r.fees == case['fees'], (path.name, r.fees, case['fees'])
        assert after == packed(after_block(coins, bytes.fromhex(case['raw']), case['height'])), (path.name, 'post-block coins differ')
        print(f'{path.name}: {case["transactions"]} transactions, {r.fees} sat fees, {r.sigops} sigops, {time.monotonic()-started:.3f}s; valid', flush=True)


def first_blocks(lib, core_dir, paths, directory, reference=None):
    native = not isinstance(lib, Path)
    if native:
        lib.validation_native_size.restype = C.c_int64
        lib.validation_native_init.argtypes = [View, C.c_int64, View]
        lib.validation_native_init.restype = C.c_int64
        lib.validation_native_block.argtypes = [View, C.c_int64, View, View, C.c_int64]
        lib.validation_native_block.restype = Result
    records = []
    versions = set()
    for path in paths:
        case = json.loads(path.read_text())
        assert case['height'] == 1
        data = directory / case['network']; data.mkdir()
        core = Core(core_dir.resolve() / 'bitcoind', data, case['network'])
        try:
            versions.add(core.call('getnetworkinfo')['subversion'])
            headers = b''.join(bytes.fromhex(case['headers'][str(h)]) for h in range(2))
            if native:
                state = (C.c_uint8 * lib.validation_native_size())()
                d = View(C.cast(state, C.c_void_p), 1)
                assert lib.validation_native_init(d, case['chain'], view(headers)) == 2
            raw = bytes.fromhex(case['raw'])
            changed = bytearray(raw); changed[-1] ^= 1
            # Test the malformed body before Core has the valid body for its
            # identical header, so its duplicate fast path cannot mask a rule.
            for label, body in (('changed-body', bytes(changed)), ('valid-after-refusal', raw)):
                if native:
                    table = (C.c_uint8 * 100000)()
                    own = lib.validation_native_block(d, 1, view(body), view(table), 0)
                else:
                    count, own, _ = sanitized_case(lib, case['chain'], 1, headers, body, b'')
                    assert count == 2
                answer = core.call('submitblock', body.hex())
                actual = 'ok' if answer is None else answer
                print(f'{case["network"]} {label}: own={own_reason(own)}, Core={actual}', flush=True)
                records.append(dict(network=case['network'], name=label, body_sha256=hashlib.sha256(body).hexdigest(), verdict=answer))
                assert own_reason(own) == actual, (case['network'], label, own_reason(own), actual)
            assert core.call('getblockcount') == 1
        finally:
            core.close()
    if reference:
        reference.write_text(json.dumps(dict(schema=1, versions=sorted(versions), cases=records), indent=1) + '\n')


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--coax', type=Path, default=Path('coax/bin/coax'))
    p.add_argument('--core-dir', type=Path)
    p.add_argument('--historical', type=Path, nargs='*')
    p.add_argument('--shared', type=Path, help='reuse a compiled native oracle')
    p.add_argument('--write-reference', type=Path)
    p.add_argument('--sanitized-binary', type=Path)
    p.add_argument('--first-blocks', type=Path, nargs='*')
    p.add_argument('--indexed', action='store_true', help='use the native ordered arena, exporting exact canonical coins for comparison')
    p.add_argument('--mode', choices=('debug', 'release'), default='release')
    a = p.parse_args()
    if a.first_blocks:
        assert (a.shared or a.sanitized_binary) and a.core_dir
        with tempfile.TemporaryDirectory(prefix='coax-validation-first-') as tmp:
            first_blocks(a.sanitized_binary or C.CDLL(str(a.shared.resolve())), a.core_dir, a.first_blocks, Path(tmp), a.write_reference)
        return
    if a.historical:
        assert a.shared or a.sanitized_binary
        historical(a.sanitized_binary or C.CDLL(str(a.shared.resolve())), a.historical, a.indexed)
        return
    prefix = [bytes.fromhex(s) for s in L.make_fixture()['prefix']]
    with tempfile.TemporaryDirectory(prefix='coax-validation-') as tmp:
        directory = Path(tmp)
        data = directory / 'core'; data.mkdir()
        assert a.core_dir
        core = Core(a.core_dir.resolve() / 'bitcoind', data)
        try:
            for block in prefix[1:]:
                assert core.call('submitblock', block.hex()) is None
            shared = a.shared
            if shared is None and not a.sanitized_binary:
                cfile = directory / 'validation.c'
                command = [str(a.coax.resolve()), 'c', str(HERE / 'validation-native.coax'), '--library']
                if a.mode == 'release': command.append('--release')
                with cfile.open('w') as out:
                    subprocess.run(command, cwd=ROOT, stdout=out, check=True, timeout=600)
                shared = directory / 'validation.dylib'
                subprocess.run(['cc', '-std=c11', '-O2', '-shared', '-fPIC', str(cfile), '-o', str(shared), '-lm'], check=True, timeout=120)
            n = exercise(a.sanitized_binary or C.CDLL(str(shared.resolve())), core, prefix, a.write_reference, a.indexed)
            print(f'{a.mode}: {n} whole-block verdicts match Core {core.call("getnetworkinfo")["subversion"]}', flush=True)
        finally:
            core.close()


if __name__ == '__main__':
    main()
