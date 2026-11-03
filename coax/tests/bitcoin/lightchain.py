#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python builds mined regtest branches and models UTXOs/undo without sharing Coax's transition code)
"""#12704 light connect/reorg fixtures and native differential checks.

--write writes deterministic fixtures; default checks their exact bytes.
--coax COMPILER also compares native debug/release transitions and persistence.
No upstream programs or scripts are executed.
"""
import argparse
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / "bitcoin/fixtures/light-chain.json"
VECTORS = ROOT / "coax/tests/bitcoin/lightchain_vectors.coax"


def sha256d(data):
    return hashlib.sha256(hashlib.sha256(data).digest()).digest()


def compact(n):
    if n < 253:
        return bytes([n])
    if n <= 65535:
        return b"\xfd" + struct.pack("<H", n)
    if n <= 0xffffffff:
        return b"\xfe" + struct.pack("<I", n)
    return b"\xff" + struct.pack("<Q", n)


def transaction(inputs, outputs):
    result = struct.pack("<I", 1) + compact(len(inputs))
    for point, script in inputs:
        result += point + compact(len(script)) + script + bytes.fromhex("ffffffff")
    result += compact(len(outputs))
    for value, script in outputs:
        result += struct.pack("<q", value) + compact(len(script)) + script
    return result + bytes(4)


def height_script(n):
    if 1 <= n <= 16:
        return bytes([0x50 + n])
    raw = n.to_bytes((n.bit_length() + 7) // 8, "little")
    if raw[-1] & 0x80:
        raw += b"\0"
    return bytes([len(raw)]) + raw


def coinbase(height, tag):
    return transaction([(bytes(32) + b"\xff" * 4, height_script(height) + bytes([tag]))],
                       [(5000000000, b"\x51"), (0, b"\x6a" + bytes([1, tag]))])


def merkle(ids):
    ids = list(ids)
    while len(ids) > 1:
        if len(ids) & 1:
            ids.append(ids[-1])
        ids = [sha256d(ids[i] + ids[i + 1]) for i in range(0, len(ids), 2)]
    return ids[0]


def mined_transactions(parent, height, txs):
    prefix = struct.pack("<I", 4) + parent + merkle(map(sha256d, txs))
    prefix += struct.pack("<II", 1296688602 + height * 600, 0x207fffff)
    nonce = 0
    while True:
        header = prefix + struct.pack("<I", nonce)
        if int.from_bytes(sha256d(header), "little") <= 0x7fffff << (8 * 29):
            return header + compact(len(txs)) + b"".join(txs)
        nonce += 1


def block(parent, height, tag, extra=()):
    return mined_transactions(parent, height, [coinbase(height, tag), *extra])


def make_fixture():
    capture = json.loads((ROOT / "bitcoin/fixtures/block-files-core.json").read_text())
    key = bytes.fromhex(capture["xor_hex"])
    stored = bytes.fromhex(capture["records"][0]["stored_hex"])
    raw = bytes(x ^ key[i % 8] for i, x in enumerate(stored))
    assert raw[:4] == bytes.fromhex("fabfb5da") and len(raw) == 8 + struct.unpack_from("<I", raw, 4)[0]
    genesis = raw[8:]
    assert sha256d(genesis[:80])[::-1].hex() == capture["records"][0]["block_hash"]
    prefix = [genesis]
    for height in range(1, 101):
        prefix.append(block(sha256d(prefix[-1][:80]), height, 1))
    point = sha256d(coinbase(1, 1)) + bytes(4)
    a1 = transaction([(point, b"")], [(3000000000, b"\x51"), (1999999000, b"\x51")])
    a2 = transaction([(sha256d(a1) + bytes(4), b"")], [(2999998000, b"\x51")])
    a101 = block(sha256d(prefix[-1][:80]), 101, 2, [a1, a2])
    a3 = transaction([(sha256d(a2) + bytes(4), b"")], [(2999997000, b"\x51")])
    a102 = block(sha256d(a101[:80]), 102, 2, [a3])
    b1 = transaction([(point, b"")], [(4999999000, b"\x51\x61")])
    b101 = block(sha256d(prefix[-1][:80]), 101, 3, [b1])
    b102 = block(sha256d(b101[:80]), 102, 3)
    b2 = transaction([(sha256d(b1) + bytes(4), b"")], [(4999998000, b"\x51"), (0, b"\x6a")])
    b103 = block(sha256d(b102[:80]), 103, 3, [b2])
    missing = transaction([(bytes([9]) * 32 + bytes(4), b"")], [(1, b"\x51")])
    bad104 = block(sha256d(b103[:80]), 104, 3, [missing])
    return dict(schema=1, network="regtest", profile="proof-of-work-only explorer; no script validation",
                prefix=[b.hex() for b in prefix], branches=[b.hex() for b in (a101, a102, b101, b102, b103, bad104)])


def vector_source(fixture):
    lines = ["// Generated independently by coax/tests/bitcoin/lightchain.py --write.",
             "// 100 mined regtest blocks, mature spends, intra-block spends and two forks."]
    for field, name in (("prefix", "LC_PREFIX"), ("branches", "LC_BRANCH")):
        values = fixture[field]
        lines += [f"{name} :: [{len(values)}] string.[", *[f'  "{x}",' for x in values], "];", ""]
    return "\n".join(lines)


def transactions(raw):
    """Independent reader for this fixture's legacy transactions."""
    at = 80

    def take(n):
        nonlocal at
        result = raw[at:at+n]
        assert len(result) == n
        at += n
        return result

    def size():
        n = take(1)[0]
        return n if n < 253 else int.from_bytes(take({253: 2, 254: 4, 255: 8}[n]), "little")

    count = size()
    result = []
    for _ in range(count):
        start = at
        take(4)
        ins = []
        for _ in range(size()):
            ins.append(take(36))
            take(size()); take(4)
        outs = []
        for _ in range(size()):
            amount = int.from_bytes(take(8), "little")
            outs.append((amount, take(size())))
        take(4)
        result.append((sha256d(raw[start:at]), ins, outs))
    assert at == len(raw)
    return result


def connect(coins, raw, height):
    result = dict(coins)
    for index, (txid, ins, outs) in enumerate(transactions(raw)):
        if index:
            for point in ins:
                del result[point]
        for n, (amount, script) in enumerate(outs):
            if script[:1] == b"\x6a" or len(script) > 10000:
                continue
            point = txid + struct.pack("<I", n)
            assert point not in result
            result[point] = struct.pack("<QIB", amount, height, index == 0) + script
    return result


def chacha_block(key, counter):
    initial = list(struct.unpack("<4I", b"expand 32-byte k")) + list(struct.unpack("<8I", key)) + [counter, 0, 0, 0]
    x = initial[:]

    def quarter(a, b, c, d):
        for p, q, r, shift in ((a, b, d, 16), (c, d, b, 12), (a, b, d, 8), (c, d, b, 7)):
            x[p] = (x[p] + x[q]) & 0xffffffff
            v = x[r] ^ x[p]
            x[r] = ((v << shift) | (v >> (32-shift))) & 0xffffffff

    for _ in range(10):
        for q in ((0, 4, 8, 12), (1, 5, 9, 13), (2, 6, 10, 14), (3, 7, 11, 15),
                  (0, 5, 10, 15), (1, 6, 11, 12), (2, 7, 8, 13), (3, 4, 9, 14)):
            quarter(*q)
    return struct.pack("<16I", *[(a+b) & 0xffffffff for a, b in zip(x, initial)])


def muhash(coins):
    product, prime = 1, 2**3072 - 1103717
    for point, value in coins.items():
        amount, height, cb = struct.unpack("<QIB", value[:13])
        script = value[13:]
        record = point + struct.pack("<IQ", 2*height+cb, amount) + compact(len(script)) + script
        key = hashlib.sha256(record).digest()
        number = int.from_bytes(b"".join(chacha_block(key, i) for i in range(6)), "little")
        product = product * number % prime
    return hashlib.sha256(product.to_bytes(384, "little")).digest()


class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


class IntView(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_int64)), ("count", C.c_int64)]


class Result(C.Structure):
    _fields_ = [(n, C.c_int64) for n in ("code", "used", "n", "disconnected", "connected")]


class Open(C.Structure):
    _fields_ = [("code", C.c_int64), ("used", C.c_int64), ("seq", C.c_uint64),
                ("frames", C.c_int64), ("keep", C.c_int64), ("cut", C.c_bool), ("mask", C.c_uint64)]


def view(data):
    if isinstance(data, bytes):
        data = (C.c_uint8 * len(data)).from_buffer_copy(data)
    return View(data, len(data))


def ints(data):
    storage = (C.c_int64 * len(data))(*data)
    return IntView(storage, len(data))


def table_map(raw):
    result = {}
    at, previous = 0, b""
    while at < len(raw):
        kn, vn = struct.unpack_from("<HI", raw, at)
        at += 6
        key, value = raw[at:at+kn], raw[at+kn:at+kn+vn]
        assert len(value) == vn and key > previous
        result[key] = value
        previous = key
        at += kn + vn
    assert at == len(raw)
    return result


def command(args):
    return subprocess.run([str(a) for a in args], cwd=ROOT, text=True, capture_output=True,
                          check=True, timeout=300).stdout


def exercise(lib, fixture, directory):
    lib.lc_init_vartime.argtypes = [C.c_int64, View, View, C.c_int64, View, View]
    lib.lc_init_vartime.restype = Result
    lib.lc_native_headers.argtypes = [View, View]; lib.lc_native_headers.restype = C.c_int64
    lib.lc_native_reorg.argtypes = [View, View, C.c_int64, C.c_int64, IntView, View, IntView, View, View, C.c_int64]
    lib.lc_native_reorg.restype = Result
    lib.lc_disconnect_vartime.argtypes = [View, C.c_int64, View, View]; lib.lc_disconnect_vartime.restype = Result
    lib.lc_native_open.argtypes = [View, View]; lib.lc_native_open.restype = Open
    lib.lc_native_frame.argtypes = [View, C.c_uint64, View]; lib.lc_native_frame.restype = C.c_int64
    lib.lc_native_muhash.argtypes = [View, C.c_int64, View]; lib.lc_native_muhash.restype = C.c_int64
    lib.lc_native_status.argtypes = [View, C.c_int64]; lib.lc_native_status.restype = C.c_int64
    lib.lc_native_size.restype = C.c_int64
    storage = (C.c_uint8 * lib.lc_native_size())()
    state = View(storage, 1)
    prefix = list(map(bytes.fromhex, fixture["prefix"]))
    branches = list(map(bytes.fromhex, fixture["branches"]))
    by_hash = {sha256d(b[:80]): b for b in prefix + branches}
    assert lib.lc_native_headers(state, view(b"".join(b[:80] for b in prefix + branches))) == 107
    capacity = 4 << 20
    live, stage, batch, frame, reopened = [(C.c_uint8 * capacity)() for _ in range(5)]
    used = seq = 0
    coins = {}
    path = directory / "light.kv"
    header = b"OAKV\x01\x00\x00\x00" + bytes(8)
    path.write_bytes(header + sha256d(header)[:4])
    last = None

    def commit(r, expected, expected_height):
        nonlocal used, seq, coins, last
        assert r.code == 0, r.code
        staged = bytes(stage[:r.used])
        values = table_map(staged)
        assert {k[1:]: v for k, v in values.items() if k[:1] == b"C"} == expected
        meta = values[b"L"]
        assert struct.unpack_from("<III", meta) == (1, 5, expected_height)
        undo_keys = set()
        block_headers = [k for k in values if k[:1] == b"H"]
        assert len(block_headers) == expected_height
        for key in block_headers:
            raw = by_hash[key[1:]]
            head = values[key]
            assert len(head) == 80 and struct.unpack_from("<I", head)[0] == 1
            assert head[8:40] == raw[4:36] and struct.unpack_from("<I", head, 76)[0] == 4 * len(raw)
            count = struct.unpack_from("<I", head, 40)[0]
            txs = transactions(raw)
            expected_count = sum((len(ins) if i else 0) + sum(len(script) <= 10000 and script[:1] != b"\x6a" for _, script in outs)
                                 for i, (_, ins, outs) in enumerate(txs))
            assert count == expected_count
            encoded = raw[4:36] + key[1:]
            for i in range(count):
                ukey = b"U" + key[1:] + struct.pack("<I", i)
                undo_keys.add(ukey)
                inverse = values[ukey]
                encoded += struct.pack("<I", len(inverse)) + inverse
            assert sha256d(encoded) == head[44:76]
        assert {k for k in values if k[:1] == b"U"} == undo_keys
        digest = (C.c_uint8 * 32)()
        assert lib.lc_native_muhash(view(stage), r.used, view(digest)) == 0
        assert bytes(digest) == muhash(expected)
        seq += 1
        n = lib.lc_native_frame(view(bytes(batch[:r.n])), seq, view(frame))
        rawframe = bytes(frame[:n])
        assert n == r.n + 8 and rawframe[:4] == struct.pack("<I", r.n)
        assert rawframe[4:8] == sha256d(rawframe[8:])[:4]
        before = path.read_bytes()
        with path.open("ab") as f:
            f.write(rawframe); f.flush(); os.fsync(f.fileno())
        # Publish only after fsync. Replay must expose exactly that same tip.
        C.memmove(live, stage, r.used)
        used, coins = r.used, expected
        opened = lib.lc_native_open(view(path.read_bytes()), view(reopened))
        assert opened.code == 0 and opened.seq == seq and not opened.cut
        assert bytes(reopened[:opened.used]) == staged
        last = before, rawframe, staged

    def prepare(indices, bodies, target=None, small=0):
        offsets = [0]
        for b in bodies:
            offsets.append(offsets[-1] + len(b))
        before = bytes(live[:used])
        r = lib.lc_native_reorg(state, view(live), used, indices[-1] if target is None else target,
                               ints(indices), view(b"".join(bodies)), ints(offsets), view(stage), view(batch), small)
        assert bytes(live[:used]) == before
        assert all(lib.lc_native_status(state, i) == 2 for i in range(107))
        return r

    genesis = sha256d(prefix[0][:80])
    commit(lib.lc_init_vartime(5, view(genesis), view(live), 0, view(stage), view(batch)), {}, 0)
    for height, raw in enumerate(prefix[1:], 1):
        commit(prepare([height], [raw]), connect(coins, raw, height), height)
    shared_coins = dict(coins)
    for i in (0, 1):
        commit(prepare([101+i], [branches[i]]), connect(coins, branches[i], 101+i), 101+i)
    original = bytes(live[:used]); a_coins = dict(coins)
    assert prepare([103, 104, 105, 106], branches[2:]).code == 4
    for small in (1, 2):
        assert prepare([103, 104, 105], branches[2:5], small=small).code == 2
    changed = bytearray(branches[4]); changed[-1] ^= 1
    assert prepare([103, 104, 105], [*branches[2:4], bytes(changed)]).code == 3
    assert bytes(live[:used]) == original
    b_coins = dict(shared_coins)
    for height, raw in zip((101, 102, 103), branches[2:5]):
        b_coins = connect(b_coins, raw, height)
    r = prepare([103, 104, 105], branches[2:5])
    assert (r.disconnected, r.connected) == (2, 3)
    commit(r, b_coins, 103)
    # Every physical cut of the reorg frame exposes the old or complete new tip.
    before, rawframe, after = last
    old_open = lib.lc_native_open(view(before), view(reopened))
    old_state = bytes(reopened[:old_open.used])
    crash = directory / "crash.kv"
    for cut in range(len(rawframe) + 1):
        crash.write_bytes(before + rawframe[:cut])
        got = lib.lc_native_open(view(crash.read_bytes()), view(reopened))
        assert got.code == 0
        assert bytes(reopened[:got.used]) == (after if cut == len(rawframe) else old_state), cut
    assert prepare([101, 102], branches[:2]).code == 7
    # Step back once, then the earlier arrival wins the equal-work tie.
    commit(lib.lc_disconnect_vartime(view(live), used, view(stage), view(batch)),
           connect(connect(shared_coins, branches[2], 101), branches[3], 102), 102)
    r = prepare([101, 102], branches[:2]); commit(r, a_coins, 102)
    assert bytes(live[:used]) == original
    return len(rawframe) + 1


def native_checks(compiler, fixture):
    with tempfile.TemporaryDirectory(prefix="coax-lightchain-") as temporary:
        directory = Path(temporary)
        for release in (False, True):
            mode = ["--release"] if release else []
            cfile, shared = directory / "light.c", directory / ("release.dylib" if release else "debug.dylib")
            cfile.write_text(command([compiler, "c", "coax/tests/bitcoin/lightchain-native.coax", "--library", *mode]))
            command(["cc", "-std=c11", "-O2", "-shared", "-fPIC", cfile, "-o", shared, "-lm"])
            cuts = exercise(C.CDLL(str(shared)), fixture, directory)
            print(f"{'release' if release else 'debug'}: native UTXOs and MuHash match the independent model; {cuts} physical reorg cuts are atomic", flush=True)


def replay(raw):
    """Independent chapter-62 log reader for the tool's unmasked frames."""
    assert raw[:8] == b"OAKV\x01\x00\x00\x00" and raw[8:16] == bytes(8)
    assert raw[16:20] == sha256d(raw[:16])[:4]
    at, sequence, values = 20, 0, {}
    lengths = []
    while at < len(raw):
        n = struct.unpack_from("<I", raw, at)[0]
        body = raw[at+8:at+8+n]
        assert len(body) == n and raw[at+4:at+8] == sha256d(body)[:4]
        seq, count = struct.unpack_from("<QI", body)
        assert seq == sequence + 1
        sequence = seq
        pos = 12
        for _ in range(count):
            kind, kn = struct.unpack_from("<BH", body, pos)
            pos += 3
            if kind == 1:
                vn = struct.unpack_from("<I", body, pos)[0]; pos += 4
                key, value = body[pos:pos+kn], body[pos+kn:pos+kn+vn]
                assert len(key) == kn and len(value) == vn
                values[key] = value; pos += kn + vn
            else:
                assert kind == 2
                values.pop(body[pos:pos+kn], None); pos += kn
        assert pos == n
        at += 8 + n
        lengths.append(n)
    return values, lengths


def cli_checks(tool, fixture):
    prefix = list(map(bytes.fromhex, fixture["prefix"]))
    branches = list(map(bytes.fromhex, fixture["branches"]))
    with tempfile.TemporaryDirectory(prefix="coax-light-cli-") as temporary:
        directory = Path(temporary)
        state, headers = directory / "state.kv", directory / "headers.bin"
        headers.write_bytes(b"".join(b[:80] for b in prefix + branches))
        paths = []
        for i, raw in enumerate(prefix + branches):
            path = directory / f"{i}.blk"; path.write_bytes(raw); paths.append(path)

        def call(*args, status=0):
            result = subprocess.run([str(tool), *map(str, args)], cwd=ROOT, text=True,
                                    capture_output=True, timeout=120)
            assert result.returncode == status, (args[:4], result.returncode, result.stdout, result.stderr)
            return result.stdout

        def check(expected, height):
            values, lengths = replay(state.read_bytes())
            assert {k[1:]: v for k, v in values.items() if k[:1] == b"C"} == expected
            assert struct.unpack_from("<III", values[b"L"]) == (1, 5, height)
            return lengths

        call("init", "regtest", state)
        call("init", "regtest", state, status=2)
        call("apply", "regtest", state, headers, 2000000000, *paths[1:101])
        common = {}
        for height, raw in enumerate(prefix[1:], 1):
            common = connect(common, raw, height)
        check(common, 100)
        common_file = state.read_bytes()
        call("apply", "regtest", state, headers, 2000000000, *paths[101:103])
        a = connect(connect(common, branches[0], 101), branches[1], 102)
        check(a, 102)
        before = state.read_bytes()
        assert call("apply", "regtest", state, headers, 2000000000, *paths[103:107], status=1) == "refused 4\n"
        assert state.read_bytes() == before
        call("apply", "regtest", state, headers, 2000000000, *paths[103:106])
        b = connect(connect(connect(common, branches[2], 101), branches[3], 102), branches[4], 103)
        check(b, 103)
        assert "height 103" in call("tip", "regtest", state)
        call("tip", "main", state, status=1)
        saved = state.read_bytes()
        with state.open("ab") as f:
            f.write(b"\x01\x02"); f.flush(); os.fsync(f.fileno())
        call("tip", "regtest", state, status=1)
        assert "cut true" in call("recover", "regtest", state)
        assert state.read_bytes() == saved
        call("disconnect", "regtest", state)
        call("apply", "regtest", state, headers, 2000000000, *paths[101:103])
        check(a, 102)
        damaged = directory / "damaged.kv"
        content = bytearray(saved); content[24] ^= 1; damaged.write_bytes(content)
        call("recover", "regtest", damaged, status=1)
        assert damaged.read_bytes() == content
        # A consensus-sized block whose coin/undo batch crosses the old 1 MiB
        # one-shot checksum cap. It still has less than 4,000,000 weight units.
        point = sha256d(coinbase(1, 1)) + bytes(4)
        many = transaction([(point, b"")], [(1, b"\x51" * 100)] * 5000)
        large = block(sha256d(prefix[-1][:80]), 101, 4, [many])
        assert 4 * len(large) < 4000000
        large_path = directory / "large.blk"; large_path.write_bytes(large)
        headers.write_bytes(headers.read_bytes() + large[:80])
        state.write_bytes(common_file)
        call("apply", "regtest", state, headers, 2000000000, large_path)
        lengths = check(connect(common, large, 101), 101)
        assert max(lengths) > 1048576
        assert "height 101" in call("tip", "regtest", state)
        # The complete block's weight includes its header and count. A large
        # unspendable coinbase output lets us test the exact 4M boundary with
        # no unrelated input lookup or storage-capacity refusal.
        null = bytes(32) + b"\xff" * 4
        script_n = 999000
        sample = transaction([(null, height_script(101) + b"\x05")], [(5000000000, b"\x6a" + bytes(script_n-1))])
        script_n += 1000000 - (81 + len(sample))
        for extra, status in ((0, 0), (1, 1)):
            coin = transaction([(null, height_script(101) + b"\x05")], [(5000000000, b"\x6a" + bytes(script_n+extra-1))])
            boundary = mined_transactions(sha256d(prefix[-1][:80]), 101, [coin])
            assert len(boundary) == 1000000 + extra
            boundary_path = directory / f"weight-{extra}.blk"; boundary_path.write_bytes(boundary)
            headers.write_bytes(headers.read_bytes() + boundary[:80])
            state.write_bytes(common_file)
            output = call("apply", "regtest", state, headers, 2000000000, boundary_path, status=status)
            if status:
                assert output == "refused 3\n" and state.read_bytes() == common_file
            else:
                check(common, 101)
        print(f"file tool: connect, reorg, refusal, durable reopen and recovery match; {max(lengths)}-byte batch reopens beyond 1 MiB", flush=True)
        print("file tool: 4,000,000 weight accepted; 4,000,004 refused with the log unchanged", flush=True)


def sanitized_checks(compiler, fixture):
    with tempfile.TemporaryDirectory(prefix="coax-light-sanitized-") as temporary:
        directory = Path(temporary)
        cfile, binary = directory / "light.c", directory / "light-sanitized"
        cfile.write_text(command([compiler, "c", "bitcoin/examples/live/light-chain.coax", "--library"]))
        platform = ROOT / "coax/platform"
        command(["cc", "-std=c11", "-D_DARWIN_C_SOURCE", "-O1", "-g", "-fsanitize=address,undefined",
                 "-fno-omit-frame-pointer", "-I", platform, cfile, platform / "tool-native.c",
                 platform / "store-file-native.c", "-o", binary, "-lpthread", "-lm"])
        cli_checks(binary, fixture)
        print("ASan/UBSan: complete file-tool journey passed", flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--coax")
    parser.add_argument("--cli-tool", help="also check the built Coax file tool")
    parser.add_argument("--sanitize-with", help="build and check the file tool under ASan/UBSan with this compiler")
    args = parser.parse_args()
    fixture = make_fixture()
    encoded = json.dumps(fixture, indent=2) + "\n"
    vectors = vector_source(fixture)
    if args.write:
        FIXTURE.write_text(encoded)
        VECTORS.write_text(vectors)
    else:
        assert FIXTURE.read_text() == encoded
        assert VECTORS.read_text() == vectors
    print("light-chain fixtures: 100 mined blocks and 6 branch blocks reproduce exactly", flush=True)
    if args.coax:
        native_checks(Path(args.coax).resolve(), fixture)
    if args.cli_tool:
        cli_checks(Path(args.cli_tool).resolve(), fixture)
    if args.sanitize_with:
        sanitized_checks(Path(args.sanitize_with).resolve(), fixture)


if __name__ == "__main__":
    main()
