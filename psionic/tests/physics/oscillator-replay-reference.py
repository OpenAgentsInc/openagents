#!/usr/bin/env python3
"""Firm reason2: independent canonical bytes/parser, actual-build binding and replay oracle."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[3]
TAG = b"psionic.oscillator.snapshot.v1"
MAX_TICK = 2147483647
SIZE = 1312


def sha(data):
    return hashlib.sha256(data).digest()


def file_sha(path):
    return sha(path.read_bytes()).hex()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def bits(value):
    return struct.unpack("<Q", struct.pack("<d", value))[0]


def scenario(index):
    lengths = [128, 720, 128, 4, 0]
    starts = [0, 0, MAX_TICK - 128, 0, 0]
    captures = [[0, 1, 16, 43, 48, 80, 127, 128], [0, 127, 128, 359, 360, 719, 720],
                [0, 1, 64, 127, 128], [0, 1, 4], [0]]
    return dict(index=index, length=lengths[index], start_tick=starts[index], captures=captures[index],
                mass=1.0, spring=1.0 if index == 3 else 2.0, damping=.5,
                position=0.0 if index == 3 else (.75 if index == 2 else .5),
                velocity=0.0 if index >= 3 else -.25,
                dt=.03125 if index == 2 else (.0009765625 if index == 4 else .05),
                force_recipe="signed-zero-four" if index == 3 else ("constant-half" if index == 2 else "dyadic-triad"),
                acceptance="0:32,16:32,48:32,80:32" if index == 0 else ("each32" if index == 1 else "all-at-zero"))


def force(case, index):
    if case == 3:
        return [-0.0, 0.0, .5, -.5][index]
    if case == 2:
        return .5
    return .25 * ((index // 4) % 3 - 1)


def accepted(case, relative):
    length = scenario(case)["length"]
    if case == 0:
        return {0:(0, 32), 16:(32, 32), 48:(64, 32), 80:(96, 32)}.get(relative, (0, 0))
    if case == 1:
        return (relative, min(32, length - relative)) if relative < length and relative % 32 == 0 else (0, 0)
    return (0, length) if relative == 0 else (0, 0)


def step(q, v, k, c, u, dt):
    def field(q, v):
        return v, (u - c * v) - k * q
    a = field(q, v)
    b = field(q + (dt / 2) * a[0], v + (dt / 2) * a[1])
    cc = field(q + (dt / 2) * b[0], v + (dt / 2) * b[1])
    d = field(q + dt * cc[0], v + dt * cc[1])
    return (q + (dt / 6) * (((a[0] + 2 * b[0]) + 2 * cc[0]) + d[0]),
            v + (dt / 6) * (((a[1] + 2 * b[1]) + 2 * cc[1]) + d[1]))


def simulate(index, ids):
    case = scenario(index)
    q, v = case["position"], case["velocity"]
    queue, snapshots, frames = [], [], []
    for relative in range(case["length"] + 1):
        start, count = accepted(index, relative)
        queue.extend(force(index, i) for i in range(start, start + count))
        if relative in case["captures"]:
            snapshots.append(dict(ids=ids, tick=case["start_tick"] + relative, dt=case["dt"], mass=1.0,
                                  spring=case["spring"], damping=case["damping"], position=q, velocity=v,
                                  pending=len(queue), forces=queue[:], case=index, relative=relative))
        if relative == case["length"]:
            break
        u = queue.pop(0)
        q, v = step(q, v, case["spring"], case["damping"], u, case["dt"])
        frames.append((case["start_tick"] + relative + 1, bits(u), bits(q), bits(v)))
    return snapshots, frames


def tagged(body):
    t = sha(TAG)
    return sha(t + t + body)


def encode(snapshot):
    out = bytearray(SIZE)
    struct.pack_into("<4sHHII", out, 0, b"OSRP", 1, 31, SIZE, 0)
    out[16:176] = snapshot["ids"]
    struct.pack_into("<QddddddQQII", out, 176, snapshot["tick"], snapshot["dt"], snapshot["mass"],
                     snapshot["spring"], snapshot["damping"], snapshot["position"], snapshot["velocity"],
                     snapshot["pending"], snapshot["tick"] + snapshot["pending"], 1, 0)
    for index, value in enumerate(snapshot["forces"]):
        struct.pack_into("<d", out, 256 + 8 * index, value)
    out[1280:] = tagged(out[:1280])
    return bytes(out)


def parse(raw, expected_ids):
    assert len(raw) == SIZE
    assert struct.unpack_from("<4sHHII", raw) == (b"OSRP", 1, 31, SIZE, 0)
    assert raw[1280:] == tagged(raw[:1280])
    assert raw[16:176] == expected_ids
    tick, dt, mass, spring, damping, q, v, pending, end, workers, history = struct.unpack_from("<QddddddQQII", raw, 176)
    assert workers == 1 and history == 0
    assert tick <= MAX_TICK and pending <= 128 and pending <= MAX_TICK - tick and end == tick + pending
    assert all(math.isfinite(value) for value in [dt, mass, spring, damping, q, v])
    assert mass == 1 and .5 <= spring <= 4 and .1 <= damping <= 1
    assert abs(q) <= 2 and abs(v) <= 4 and .0009765625 <= dt <= .05
    active = struct.unpack_from("<" + "d" * pending, raw, 256)
    assert all(math.isfinite(value) and abs(value) <= 2 for value in active)
    assert raw[256 + 8 * pending:1280] == bytes((128 - pending) * 8)
    return dict(tick=tick, pending=pending, position=q, velocity=v, forces=list(active))


def faults(baseline):
    result = []
    def changed(name, status, offset, fmt, value, reseal=True):
        raw = bytearray(baseline)
        struct.pack_into(fmt, raw, offset, value)
        if reseal:
            raw[1280:] = tagged(raw[:1280])
        result.append((name, status, bytes(raw)))
    for bit in range(5):
        changed(f"omitted-inventory-{bit}", 16, 6, "<H", 31 ^ (1 << bit))
    for name, offset, fmt, value in [("version",4,"<H",2),("reserved",12,"<I",1),
                                    ("worker",248,"<I",2),("history",252,"<I",1),
                                    ("length",8,"<I",1313)]:
        changed(name, 16, offset, fmt, value)
    changed("digest", 17, 1000, "<B", 1, False)
    changed("tick-upper", 3, 176, "<Q", MAX_TICK + 1)
    changed("tick-u64", 3, 176, "<Q", (1 << 64) - 1)
    changed("pending-upper", 3, 232, "<Q", 129)
    changed("end-mismatch", 16, 240, "<Q", 65)
    for offset in range(184, 232, 8):
        for word in [0x7ff0000000000000, 0xfff0000000000000, 0x7ff8000000000123]:
            changed(f"nonfinite-scalar-{offset}-{word:x}", 4, offset, "<Q", word)
    for word in [0x7ff0000000000000, 0xfff0000000000000, 0x7ff8000000000123]:
        changed(f"nonfinite-active-{word:x}", 4, 256, "<Q", word)
        changed(f"noncanonical-inactive-{word:x}", 16, 256 + 21 * 8, "<Q", word)
    changed("inactive-negative-zero", 16, 256 + 21 * 8, "<Q", 1 << 63)
    for name, offset, value, status in [("mass",192,2,5),("spring-low",200,.499,5),
                                       ("spring-high",200,4.001,5),("damping-low",208,.099,5),
                                       ("damping-high",208,1.001,5),("q",216,2.001,6),
                                       ("v",224,-4.001,6),("dt-zero",184,0,8),("dt-high",184,.051,8),
                                       ("force-high",256,2.001,7),("inactive-force",256+21*8,1,16)]:
        changed(name, status, offset, "<d", value)
    for lane in range(5):
        raw = bytearray(baseline)
        raw[16+32*lane:48+32*lane] = bytes(32)
        raw[1280:] = tagged(raw[:1280])
        result.append((f"omitted-identity-{lane}", 9, bytes(raw)))
        raw = bytearray(baseline)
        raw[16+32*lane] ^= 1
        raw[1280:] = tagged(raw[:1280])
        result.append((f"different-identity-{lane}", 10+lane, bytes(raw)))
    return result


def prepare(binary, prefix, generated):
    env = json.loads(subprocess.check_output([str(binary.resolve()), "--environment"], text=True))
    assert env["nearest"] and env["control_observed"] and env["little_endian"] and env["pointer_bits"] == 64
    teacher = ["psionic/lib/physics/oscillator-types.coax", "psionic/lib/physics/oscillator.coax"]
    source = {name:file_sha(ROOT/name) for name in teacher}
    qualification = json.loads((ROOT/"psionic/fixtures/physics/oscillator/qualification.json").read_text())
    assert all(qualification["source_sha256"][name] == value for name, value in source.items())
    program = dict(name="oscillator-rk4-f64-v1", source_sha256=source)
    numeric = dict(name="host-observed-f64-rk4-serial-v1", nearest=True, control_register=env["control_register"],
                   contraction="off", fast_math=False, workers=1, solver_transcendentals=False,
                   external_precondition="FP control environment remains unchanged; not restored by snapshot")
    target = {key:env[key] for key in ["machine", "system", "pointer_bits", "little_endian"]}
    build = sha(binary.read_bytes())
    identity = sha(canonical(program)) + sha(canonical(numeric)) + sha(canonical(target)) + build
    ids = [identity + sha(canonical(scenario(index))) for index in range(5)]
    Path(str(prefix)+".ids").write_bytes(b"".join(ids))
    snapshots, frames = [], []
    for index in range(5):
        ss, ff = simulate(index, ids[index])
        snapshots.extend(ss)
        frames.extend(ff)
    baseline = encode(next(s for s in snapshots if s["case"] == 0 and s["relative"] == 43))
    records = faults(baseline)
    for name, _, raw in records:
        rejected = False
        try:
            parse(raw, ids[0])
        except AssertionError:
            rejected = True
        assert rejected, name
    with Path(str(prefix)+".faults").open("wb") as out:
        out.write(struct.pack("<I",len(records)))
        for _, status, raw in records:
            out.write(struct.pack("<I",status)+raw)
    manifest = dict(schema="psionic-oscillator-replay-native-context-v1", binary_sha256=build.hex(),
                    generated_c_sha256=file_sha(generated), oracle_sha256=file_sha(Path(__file__)),
                    environment=env, program=program, numeric=numeric, target=target,
                    identities=[dict(program=value[:32].hex(), numeric=value[32:64].hex(), target=value[64:96].hex(),
                                     build=value[96:128].hex(), run=value[128:].hex()) for value in ids],
                    scenarios=[scenario(i) for i in range(5)], faults=[dict(name=n,status=s) for n,s,_ in records],
                    snapshot_count=len(snapshots), frame_count=len(frames))
    Path(str(prefix)+".context.json").write_text(json.dumps(manifest,indent=2)+"\n")
    print(f"prepared actual build {build.hex()[:12]}: {len(snapshots)} snapshots, {len(frames)} transitions, {len(records)} fault records")


def verify(prefix, binary):
    context=json.loads(Path(str(prefix)+".context.json").read_text())
    assert context["binary_sha256"] == file_sha(binary)
    ids_raw=Path(str(prefix)+".ids").read_bytes();assert len(ids_raw)==800
    expected_snapshots=[];expected_frames=[]
    for index in range(5):
        ss,ff=simulate(index,ids_raw[160*index:160*(index+1)])
        expected_snapshots.extend(ss);expected_frames.extend(ff)
    snapshots=Path(str(prefix)+".snapshots").read_bytes()
    assert len(snapshots)==24*SIZE==len(expected_snapshots)*SIZE
    for index,snapshot in enumerate(expected_snapshots):
        raw=snapshots[index*SIZE:(index+1)*SIZE]
        assert raw==encode(snapshot),(index,snapshot["case"],snapshot["relative"])
        parse(raw,snapshot["ids"])
    expected_trace=b"".join(struct.pack("<qQQQ",*frame) for frame in expected_frames)
    actual_trace=Path(str(prefix)+".trace").read_bytes()
    assert len(expected_frames)==980 and actual_trace==expected_trace
    native=json.loads(Path(str(prefix)+".native.json").read_text())
    assert native["snapshots"]==24 and native["original_transitions"]==980 and native["restores"]==24
    assert native["single_byte_corruptions"]==1312 and native["difference_records"]==7
    assert native["independent_fault_records"]==len(context["faults"]) and native["fp_control_unchanged"]
    differences=json.loads(Path(str(prefix)+".differences.json").read_text())
    expected_differences={"changed-state":(44,"position","psionic/lib/physics/oscillator.coax:os_step:next.position"),
        "changed-clock":(44,"completed_tick","oscillator-replay:completed_tick"),
        "changed-dt":(44,"position","psionic/lib/physics/oscillator.coax:os_step:next.position"),
        "omitted-pending-inputs":(44,"trace_length","oscillator-replay:trace_length"),
        "unaccepted-later-event-not-resupplied":(65,"trace_length","oscillator-replay:trace_length"),
        "altered-later-acceptance":(65,"applied_force","oscillator-replay:accepted_force_order"),
        "coincident-state-distinct-applied-zero":(1,"applied_force","oscillator-replay:accepted_force_order")}
    assert len(differences)==7 and {row["name"] for row in differences}==set(expected_differences)
    for row in differences:assert (row["tick"],row["field"],row["source"])==expected_differences[row["name"]]
    record=dict(schema="psionic-oscillator-replay-reference-v1", snapshots=24, canonical_bytes=len(snapshots),
                independent_transition_records=980, independent_frame_words=3920,
                exact_bytes_and_trace=True, identity_context=context, native=native, differences=differences,
                payload_sha256={suffix:file_sha(Path(str(prefix)+"."+suffix)) for suffix in ["snapshots","trace","ids","faults","differences.json"]})
    Path(str(prefix)+".reference.json").write_text(json.dumps(record,indent=2)+"\n")
    print("independent replay: all24 canonical snapshots and980 complete input/state/tick records match exactly")


def main():
    parser=argparse.ArgumentParser();sub=parser.add_subparsers(dest="command",required=True)
    prepare_args=sub.add_parser("prepare");prepare_args.add_argument("binary",type=Path);prepare_args.add_argument("prefix",type=Path);prepare_args.add_argument("generated",type=Path)
    verify_args=sub.add_parser("verify");verify_args.add_argument("prefix",type=Path);verify_args.add_argument("binary",type=Path)
    args=parser.parse_args()
    if args.command=="prepare":prepare(args.binary,args.prefix,args.generated)
    else:verify(args.prefix,args.binary)


if __name__=="__main__":main()
