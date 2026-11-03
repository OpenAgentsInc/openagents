#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python parses block transactions, models archived positions and active-chain preference, and reads actual durable files without sharing Coax's index implementation)
"""Chapter 91's transaction index against an independent file/chain model.

No downloaded scripts or programs are executed. The light chapter's own
Python fixture writer supplies deterministic mined blocks; this file has
its own witness-aware reader, position index and reorg model.
"""
import argparse
import ctypes as C
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / "bitcoin/fixtures/tx-index.json"
BASE_PATH = ROOT / "bitcoin/fixtures/light-chain.json"
spec = importlib.util.spec_from_file_location("light_fixture_writer", Path(__file__).with_name("lightchain.py"))
light = importlib.util.module_from_spec(spec)
spec.loader.exec_module(light)  # Our own independent fixture writer, not upstream code.


def digest(raw):
    return hashlib.sha256(hashlib.sha256(raw).digest()).digest()


class Reader:
    def __init__(self, raw, at=0):
        self.raw, self.at = raw, at

    def take(self, n):
        result = self.raw[self.at:self.at+n]
        assert len(result) == n
        self.at += n
        return result

    def compact(self):
        tag = self.take(1)[0]
        if tag < 253:
            return tag
        value = int.from_bytes(self.take({253: 2, 254: 4, 255: 8}[tag]), "little")
        assert value >= {253: 253, 254: 65536, 255: 2**32}[tag]
        return value

    def tx(self):
        start = self.at
        version = self.take(4)
        witness = self.raw[self.at:self.at+2] == b"\x00\x01"
        if witness:
            self.take(2)
        vin_start = self.at
        ins = []
        for _ in range(self.compact()):
            ins.append(self.take(36))
            self.take(self.compact()); self.take(4)
        outs = []
        for _ in range(self.compact()):
            value = int.from_bytes(self.take(8), "little")
            outs.append((value, self.take(self.compact())))
        end_outputs = self.at
        if witness:
            for _ in ins:
                for _ in range(self.compact()):
                    self.take(self.compact())
        locktime = self.take(4)
        raw = self.raw[start:self.at]
        stripped = version + self.raw[vin_start:end_outputs] + locktime
        return dict(id=digest(stripped), raw=raw, offset=start, n=len(raw), ins=ins, outs=outs, witness=witness)


def transactions(block):
    r = Reader(block, 80)
    txs = [r.tx() for _ in range(r.compact())]
    assert r.at == len(block)
    return txs


def merkle(ids):
    ids = list(ids)
    while len(ids) > 1:
        if len(ids) % 2:
            ids.append(ids[-1])
        ids = [digest(ids[i]+ids[i+1]) for i in range(0, len(ids), 2)]
    return ids[0]


def mine(parent, height, txs):
    root = merkle(Reader(tx).tx()["id"] for tx in txs)
    prefix = struct.pack("<I", 4) + parent + root + struct.pack("<II", 1296688602+height*600, 0x207fffff)
    for nonce in range(100000):
        header = prefix + struct.pack("<I", nonce)
        if int.from_bytes(digest(header), "little") <= 0x7fffff << 232:
            return header + light.compact(len(txs)) + b"".join(txs)
    raise AssertionError("mining budget")


def make_fixture():
    base = json.loads(BASE_PATH.read_text())
    prefix = list(map(bytes.fromhex, base["prefix"]))
    a101, a102 = list(map(bytes.fromhex, base["branches"][:2]))
    shared = transactions(a101)[1]
    b101 = mine(digest(prefix[-1][:80]), 101, [light.coinbase(101, 7), shared["raw"]])
    legacy = light.transaction([(shared["id"]+bytes(4), b"")], [(2999999000, b"\x51")])
    witness = legacy[:4]+b"\x00\x01"+legacy[4:-4]+b"\x01\x01\x51"+legacy[-4:]
    root = digest(bytes(32)+digest(witness))
    commitment = bytes.fromhex("6a24aa21a9ed")+digest(root+bytes(32))
    cb = light.transaction([(bytes(32)+b"\xff"*4, light.height_script(102)+b"\x07")], [(5000000000,b"\x51"),(0,commitment)])
    cb = cb[:4]+b"\x00\x01"+cb[4:-4]+b"\x01\x20"+bytes(32)+cb[-4:]
    b102 = mine(digest(b101[:80]), 102, [cb,witness])
    b103 = mine(digest(b102[:80]), 103, [light.coinbase(103,7)])
    missing = light.transaction([(b"\x09"*32+bytes(4),b"")], [(1,b"\x51")])
    bad104 = mine(digest(b103[:80]),104,[light.coinbase(104,7),missing])
    txs = [light.coinbase(104,8)]
    point = transactions(prefix[2])[0]["id"]+bytes(4)
    for i in range(252):
        raw = light.transaction([(point,b"")], [(5000000000-1000*(i+1),b"\x51")])
        txs.append(raw); point = digest(raw)+bytes(4)
    wide104 = mine(digest(b103[:80]),104,txs)
    assert wide104[80:83] == bytes.fromhex("fdfd00")
    return dict(schema=1, base_fixture_sha256=hashlib.sha256(BASE_PATH.read_bytes()).hexdigest(),
                branches=[x.hex() for x in (a101,a102,b101,b102,b103,bad104,wide104)])


def pins():
    directory = ROOT / "coax/tests/vectors"
    manifest = json.loads((directory/"manifest.json").read_text())
    for name in ("txindex.cpp", "txindex.h", "txindex_key.h", "txindex_tests.cpp"):
        entry, = [e for e in manifest["files"] if e["path"] == "upstream/bitcoin-core/"+name]
        data = (directory/entry["path"]).read_bytes()
        assert len(data) == entry["bytes"] and hashlib.sha256(data).hexdigest() == entry["sha256"]
        assert entry["revision"] == "git:ddea9196493d3b8fd841a36a6c53efd85dac7a05"


class View(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_uint8)), ("count", C.c_int64)]


class IntView(C.Structure):
    _fields_ = [("data", C.POINTER(C.c_int64)), ("count", C.c_int64)]


class Result(C.Structure):
    _fields_ = [(name, C.c_int64) for name in ("code","used","n","disconnected","connected")]


class Hit(C.Structure):
    _fields_ = [(name,C.c_int64) for name in ("code","height","seq","file","pos","offset","n")]+[("active",C.c_bool),("block",C.c_uint8*32)]


class Open(C.Structure):
    _fields_ = [("code",C.c_int64),("used",C.c_int64),("seq",C.c_uint64),("frames",C.c_int64),("keep",C.c_int64),("cut",C.c_bool),("mask",C.c_uint64)]


def view(data):
    if isinstance(data, bytes):
        data = (C.c_uint8*len(data)).from_buffer_copy(data)
    return View(data,len(data))


def ints(data):
    return IntView((C.c_int64*len(data))(*data),len(data))


def command(args):
    return subprocess.run(list(map(str,args)),cwd=ROOT,capture_output=True,text=True,check=True,timeout=300).stdout


def table_map(raw):
    result, at, last = {}, 0, b""
    while at < len(raw):
        kn,vn = struct.unpack_from("<HI",raw,at); at += 6
        key,value = raw[at:at+kn],raw[at+kn:at+kn+vn]; at += kn+vn
        assert len(value) == vn and key > last
        result[key],last = value,key
    assert at == len(raw)
    return result


class Model:
    def __init__(self, prefix):
        self.prefix = prefix
        self.archive, self.active = {}, []
        self.block_bytes = b""

    def positions(self, blocks):
        result, pending = [], b""
        for raw in blocks:
            h = digest(raw[:80])
            if h in self.archive:
                result.append(self.archive[h]["pos"])
            else:
                result.append(len(self.block_bytes)+len(pending)+8)
                pending += bytes.fromhex("fabfb5da")+struct.pack("<I",len(raw))+raw
        return result,pending

    def connect(self, blocks, positions, active):
        for raw,pos in zip(blocks,positions):
            h = digest(raw[:80])
            if h not in self.archive:
                self.archive[h] = dict(seq=len(self.archive),pos=pos,raw=raw,txs=transactions(raw))
        self.active = list(active)

    def lookup(self, txid, only_active=False):
        active = set(map(lambda b:digest(b[:80]),self.active))
        candidates = []
        for h,b in self.archive.items():
            if only_active and h not in active:
                continue
            for tx in b["txs"]:
                if tx["id"] == txid:
                    candidates.append((h in active,b["seq"],h,b,tx))
        return max(candidates,key=lambda c:c[:2]) if candidates else None

    def expected(self):
        result = {}
        for h,b in self.archive.items():
            # Height comes from the independently reconstructed ancestry.
            height = self.heights[h]
            result[b"J"+h] = struct.pack("<6I",height,len(b["txs"]),b["seq"],0,b["pos"],len(b["raw"]))
            for tx in b["txs"]:
                result[b"T"+tx["id"]+h] = struct.pack("<II",tx["offset"],tx["n"])
        tip = digest((self.active[-1] if self.active else self.prefix[0])[:80])
        result[b"I"] = struct.pack("<4I",1,5,len(self.active),len(self.archive))+tip
        coins = {}
        for height,raw in enumerate(self.active,1):
            for i,tx in enumerate(transactions(raw)):
                if i:
                    for point in tx["ins"]:
                        del coins[point]
                for n,(amount,script) in enumerate(tx["outs"]):
                    if script[:1] != b"\x6a" and len(script) <= 10000:
                        point = tx["id"]+struct.pack("<I",n)
                        assert point not in coins
                        coins[point] = struct.pack("<QIB",amount,height,i==0)+script
        return result,coins


def inputs(fixture):
    prefix = list(map(bytes.fromhex,json.loads(BASE_PATH.read_text())["prefix"]))
    branches = list(map(bytes.fromhex,fixture["branches"]))
    heights = {digest(b[:80]):i for i,b in enumerate(prefix)}
    for raw in branches:
        heights[digest(raw[:80])] = heights[raw[4:36]]+1
    return prefix,branches,heights


def native(compiler, fixture, mode):
    with tempfile.TemporaryDirectory(prefix="coax-txindex-") as temporary:
        directory = Path(temporary); cfile = directory/"index.c"; binary = directory/"index.dylib"
        flags = ["--release"] if mode == "release" else []
        cfile.write_text(command([compiler,"c","coax/tests/bitcoin/txindex-native.coax","--library",*flags]))
        command(["cc","-std=c11","-O2","-shared","-fPIC",cfile,"-o",binary,"-lm"])
        lib = C.CDLL(str(binary)); prefix,branches,heights = inputs(fixture)
        lib.txi_native_size.restype = C.c_int64
        state_buffer = (C.c_uint8*lib.txi_native_size())(); state = View(state_buffer,1)
        lib.txi_native_headers.argtypes = [View,View]; lib.txi_native_headers.restype = C.c_int64
        lib.txi_init_vartime.argtypes = [C.c_int64,View,View,C.c_int64,View,View]; lib.txi_init_vartime.restype = Result
        lib.txi_native_reorg.argtypes = [View,View,C.c_int64,IntView,View,IntView,IntView,View,View]; lib.txi_native_reorg.restype = Result
        lib.txi_disconnect_vartime.argtypes = [View,C.c_int64,View,View]; lib.txi_disconnect_vartime.restype = Result
        lib.txi_find_vartime.argtypes = [View,C.c_int64,View,C.c_bool]; lib.txi_find_vartime.restype = Hit
        lib.txi_native_check.argtypes = [View,Hit,View,View,View]; lib.txi_native_check.restype = C.c_int64
        lib.lc_native_frame.argtypes = [View,C.c_uint64,View]; lib.lc_native_frame.restype = C.c_int64
        lib.lc_native_open.argtypes = [View,View]; lib.lc_native_open.restype = Open
        assert lib.txi_native_headers(state,view(b"".join(b[:80] for b in prefix+branches))) == len(prefix+branches)
        buffers = [(C.c_uint8*(4<<20))() for _ in range(5)]
        live,stage,batch,frame,reopened = buffers
        model = Model(prefix); model.heights = heights
        used = sequence = 0
        path,blocks_path = directory/"index.kv",directory/"blk00000.dat"
        header = b"OAKV\x01\x00\x00\x00"+bytes(8); path.write_bytes(header+digest(header)[:4])
        blocks_path.write_bytes(b"")
        queries = 0

        def lookup(txid, active=False):
            nonlocal queries
            want = model.lookup(txid,active)
            got = lib.txi_find_vartime(view(live),used,view(txid),active)
            if want is None:
                assert got.code == 8,got.code
                return
            is_active,seq,h,b,tx = want
            assert (got.code,got.active,got.seq,bytes(got.block),got.height,got.file,got.pos,got.offset,got.n) == (0,is_active,seq,h,heights[h],0,b["pos"],tx["offset"],tx["n"])
            rawfile = blocks_path.read_bytes()
            header = rawfile[got.pos:got.pos+80]; raw = rawfile[got.pos+got.offset:got.pos+got.offset+got.n]
            assert raw == tx["raw"]
            assert lib.txi_native_check(state,got,view(txid),view(header),view(raw)) == 0
            queries += 1

        def commit(r, bodies, positions, pending, active):
            nonlocal used,sequence
            assert r.code == 0,r.code
            model.connect(bodies,positions,active)
            values = table_map(bytes(stage[:r.used])); expected,coins = model.expected()
            assert {k:v for k,v in values.items() if k[:1] in (b"I",b"J",b"T")} == expected
            assert {k[1:]:v for k,v in values.items() if k[:1] == b"C"} == coins
            before = path.read_bytes()
            # Publish block data first, then the single coins/index/tip frame.
            with blocks_path.open("ab") as f:
                f.write(pending); f.flush(); os.fsync(f.fileno())
            model.block_bytes += pending
            sequence += 1
            size = lib.lc_native_frame(view(bytes(batch[:r.n])),sequence,view(frame)); rawframe = bytes(frame[:size])
            assert size == r.n+8
            with path.open("ab") as f:
                f.write(rawframe); f.flush(); os.fsync(f.fileno())
            C.memmove(live,stage,r.used); used = r.used
            opened = lib.lc_native_open(view(path.read_bytes()),view(reopened))
            assert opened.code == 0 and not opened.cut and bytes(reopened[:opened.used]) == bytes(live[:used])
            return before,rawframe,bytes(live[:used])

        def prepare(indices,bodies):
            offsets = [0]
            for b in bodies:offsets.append(offsets[-1]+len(b))
            positions,pending = model.positions(bodies); saved = bytes(live[:used])
            r = lib.txi_native_reorg(state,view(live),used,ints(indices),view(b"".join(bodies)),ints(offsets),ints(positions),view(stage),view(batch))
            assert bytes(live[:used]) == saved
            return r,positions,pending

        commit(lib.txi_init_vartime(5,view(digest(prefix[0][:80])),view(live),0,view(stage),view(batch)),[],[],b"",[])
        for height,b in enumerate(prefix[1:],1):
            r,pos,pending = prepare([height],[b]); commit(r,[b],pos,pending,prefix[1:height+1])
        r,pos,pending = prepare([101,102],branches[:2]); commit(r,branches[:2],pos,pending,prefix[1:]+branches[:2])
        a_values = {k:v for k,v in table_map(bytes(live[:used])).items() if k[:1] not in (b"I",b"J",b"T")}
        shared = transactions(branches[0])[1]["id"]; unique = transactions(branches[0])[2]["id"]
        lookup(shared); lookup(unique)
        assert prepare([103,104,105,106],branches[2:6])[0].code == 4
        r,pos,pending = prepare([103,104,105],branches[2:5]); assert (r.disconnected,r.connected) == (2,3)
        before,rawframe,after = commit(r,branches[2:5],pos,pending,prefix[1:]+branches[2:5])
        lookup(shared); lookup(unique); lookup(unique,True)
        witness_tx = transactions(branches[3])[1]; lookup(witness_tx["id"])
        lookup(digest(witness_tx["raw"]))  # wtxid is not a txid key.
        old = lib.lc_native_open(view(before),view(reopened)); old_values = bytes(reopened[:old.used]); crash = directory/"crash.kv"
        for cut in range(len(rawframe)+1):
            crash.write_bytes(before+rawframe[:cut]); reopened_state = lib.lc_native_open(view(crash.read_bytes()),view(reopened))
            assert reopened_state.code == 0 and bytes(reopened[:reopened_state.used]) == (after if cut == len(rawframe) else old_values),cut
        r,pos,pending = prepare([107],[branches[6]]); commit(r,[branches[6]],pos,pending,prefix[1:]+branches[2:5]+[branches[6]])
        assert transactions(branches[6])[0]["offset"] == 83
        for b in model.archive.values():
            for tx in b["txs"]:lookup(tx["id"])
        for active in (True,False):
            lookup(transactions(prefix[0])[0]["id"],active); lookup(b"\x09"*32,active)
        # Detach wide104 and B103, then the older arrival wins the tie to A.
        for _ in range(2):
            commit(lib.txi_disconnect_vartime(view(live),used,view(stage),view(batch)),[],[],b"",model.active[:-1])
        r,pos,pending = prepare([101,102],branches[:2]); assert not pending
        commit(r,branches[:2],pos,pending,prefix[1:]+branches[:2]); lookup(shared)
        assert {k:v for k,v in table_map(bytes(live[:used])).items() if k[:1] not in (b"I",b"J",b"T")} == a_values
        print(f"{mode}: {queries} verified file lookups; active/stale preference, shared txids, witnesses, CompactSize 253 and {len(rawframe)+1} atomic reorg cuts pass",flush=True)


def cli_checks(tool, fixture):
    prefix,branches,heights = inputs(fixture)
    with tempfile.TemporaryDirectory(prefix="coax-txindex-cli-") as temporary:
        directory = Path(temporary); state = directory/"index.kv"; store = directory/"blk00000.dat"; headers = directory/"headers.bin"
        headers.write_bytes(b"".join(b[:80] for b in prefix+branches))
        paths = []
        for i,b in enumerate(prefix+branches):
            path = directory/f"{i}.blk"; path.write_bytes(b); paths.append(path)
        model = Model(prefix); model.heights = heights

        def call(*args, status=0):
            result = subprocess.run([str(tool),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=60,
                                    env={**os.environ,"ASAN_OPTIONS":"halt_on_error=1","UBSAN_OPTIONS":"halt_on_error=1:print_stacktrace=1"})
            assert result.returncode == status,(args,result.returncode,result.stdout,result.stderr)
            assert "runtime error:" not in result.stderr and "ERROR: AddressSanitizer" not in result.stderr,result.stderr
            return result.stdout

        def snapshot():
            values,_ = light.replay(state.read_bytes()); expected,coins = model.expected()
            assert {k:v for k,v in values.items() if k[:1] in (b"I",b"J",b"T")} == expected
            assert {k[1:]:v for k,v in values.items() if k[:1] == b"C"} == coins
            assert store.read_bytes() == model.block_bytes

        def apply(indices,active,status=0):
            bodies = [(prefix+branches)[i] for i in indices]
            positions,pending = model.positions(bodies)
            before = state.read_bytes(),store.read_bytes() if store.exists() else b""
            output = call("apply","regtest",state,store,headers,2000000000,*[paths[i] for i in indices],status=status)
            if status:
                assert state.read_bytes() == before[0] and store.read_bytes() == before[1]
                return output
            model.connect(bodies,positions,active); model.block_bytes += pending; snapshot()
            return output

        def get(txid,active=False):
            want = model.lookup(txid,active); extra = ["--active"] if active else []
            output = call("get","regtest",state,store,txid[::-1].hex(),*extra,status=0 if want else 1)
            if want is None:
                assert output == "refused 8\n",output
            else:
                is_active,seq,h,b,tx = want
                height = heights[h]; confirmations = len(model.active)-height+1 if is_active else 0
                assert output == f"profile pow-only, active {str(is_active).lower()}, height {height}, confirmations {confirmations}\nblock {h[::-1].hex()}\nraw {tx['raw'].hex()}\n",output
            return output

        call("init","regtest",state)
        assert not store.exists()
        apply(list(range(1,101)),prefix[1:])
        apply([101,102],prefix[1:]+branches[:2])
        a_state = state.read_bytes()
        shared = transactions(branches[0])[1]["id"]; unique = transactions(branches[0])[2]["id"]
        get(shared); get(unique)
        assert apply([103,104,105,106],model.active,status=1) == "refused 4\n"
        apply([103,104,105],prefix[1:]+branches[2:5]); get(shared); get(unique); get(unique,True)
        witness = transactions(branches[3])[1]; get(witness["id"]); get(digest(witness["raw"]))
        get(transactions(prefix[0])[0]["id"]); get(bytes(32))
        saved_state,saved_store = state.read_bytes(),store.read_bytes()
        # A physical state-frame cut after block append recovers to A. All B
        # bytes remain harmlessly unreferenced until a complete state commit.
        state.write_bytes(saved_state[:-7]); call("tip","regtest",state,status=1)
        call("recover","regtest",state,store)
        assert state.read_bytes() == a_state and store.read_bytes() == saved_store
        output = call("get","regtest",state,store,shared[::-1].hex())
        assert "block "+digest(branches[0][:80])[::-1].hex()+"\n" in output
        state.write_bytes(saved_state)
        # Incomplete unreferenced block bytes can be removed; committed ones
        # cannot. Neither failure is permission to shorten committed state.
        store.write_bytes(saved_store+bytes.fromhex("fabfb5"))
        call("apply","regtest",state,store,headers,2000000000,paths[107],status=1)
        call("recover","regtest",state,store); assert store.read_bytes() == saved_store
        store.write_bytes(saved_store[:-1]); before = store.read_bytes()
        assert call("recover","regtest",state,store,status=1) == "refused 12\n"
        assert store.read_bytes() == before and state.read_bytes() == saved_state
        store.write_bytes(saved_store)
        # A header or transaction corruption refuses before any raw bytes are
        # printed. The index is a hint; both identities are checked on read.
        candidate = model.lookup(witness["id"]); _,_,h,b,tx = candidate
        for offset in (b["pos"],b["pos"]+tx["offset"]):
            bad = bytearray(saved_store); bad[offset] ^= 1; store.write_bytes(bad)
            assert call("get","regtest",state,store,witness["id"][::-1].hex(),status=1) == "refused 11\n"
        store.write_bytes(saved_store)
        # Damage an interior frame: a final checksum failure may be an
        # interrupted append, while a frame followed by later commits may not.
        bad = bytearray(saved_state); bad[24] ^= 1; state.write_bytes(bad)
        call("recover","regtest",state,store,status=1); assert state.read_bytes() == bad and store.read_bytes() == saved_store
        state.write_bytes(saved_state)
        assert call("get","signet",state,store,shared[::-1].hex(),status=1) == "refused 1\n"
        # Complete orphan records are retained. New positions follow their
        # actual physical end, instead of overwriting or guessing offsets.
        orphan = branches[0]; record = bytes.fromhex("fabfb5da")+struct.pack("<I",len(orphan))+orphan
        store.write_bytes(saved_store+record); model.block_bytes = store.read_bytes()
        apply([107],prefix[1:]+branches[2:5]+[branches[6]])
        wide = transactions(branches[6]); get(wide[0]["id"]); get(wide[-1]["id"])
        for _ in range(2):
            call("disconnect","regtest",state); model.active.pop(); snapshot()
        before = store.read_bytes()
        apply([101,102],prefix[1:]+branches[:2]); assert store.read_bytes() == before
        get(shared); get(shared,True); get(witness["id"]); get(witness["id"],True)
        # An old light-only database is not silently advertised as indexed.
        meta = struct.pack("<III",1,5,0)+digest(prefix[0][:80]); body = struct.pack("<QI",1,1)+b"\x01"+struct.pack("<HI",1,len(meta))+b"L"+meta
        header = b"OAKV\x01\x00\x00\x00"+bytes(8)
        old = directory/"light-only.kv"; old.write_bytes(header+digest(header)[:4]+struct.pack("<I",len(body))+digest(body)[:4]+body)
        assert call("tip","regtest",old,status=1) == "refused 1\n"
        print("file tool: independent index/UTXO bytes, active/stale/shared/witness lookups, reorgs, CompactSize 253, orphan storage, recovery and corruption refusals pass",flush=True)


def sanitized(compiler,fixture):
    with tempfile.TemporaryDirectory(prefix="coax-txindex-sanitized-") as temporary:
        directory = Path(temporary); cfile = directory/"tool.c"; binary = directory/"tx-index"
        cfile.write_text(command([compiler,"c","bitcoin/examples/live/tx-index.coax","--library"]))
        platform = ROOT/"coax/platform"
        command(["cc","-std=c11","-D_DARWIN_C_SOURCE","-O1","-g","-fsanitize=address,undefined","-fno-omit-frame-pointer","-I",platform,cfile,platform/"tool-native.c",platform/"store-file-native.c","-o",binary,"-lpthread","-lm"])
        cli_checks(binary,fixture)
        print("ASan/UBSan: complete transaction-index file-tool trace passed",flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write",action="store_true")
    parser.add_argument("--coax")
    parser.add_argument("--mode",choices=("debug","release","all"),default="all")
    parser.add_argument("--cli-tool")
    parser.add_argument("--sanitize-with")
    args = parser.parse_args(); fixture = make_fixture(); encoded = json.dumps(fixture,indent=2)+"\n"
    if args.write:FIXTURE.write_text(encoded)
    else:assert FIXTURE.read_text() == encoded
    pins(); print("tx-index fixtures and four Core source pins reproduce exactly",flush=True)
    if args.coax:
        for mode in (("debug","release") if args.mode == "all" else (args.mode,)):
            native(Path(args.coax).resolve(),fixture,mode)
    if args.cli_tool:cli_checks(Path(args.cli_tool).resolve(),fixture)
    if args.sanitize_with:sanitized(Path(args.sanitize_with).resolve(),fixture)


if __name__ == "__main__":
    main()
