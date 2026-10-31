#!/usr/bin/env python3
"""Firm reason2: compare qualified mode traces and refuse cross-executable restore."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser=argparse.ArgumentParser();parser.add_argument("directory",type=Path);args=parser.parse_args()
    work=args.directory
    modes=["debug","release","sanitized"]
    records={mode:json.loads((work/f"{mode}.reference.json").read_text()) for mode in modes}
    builds=[]
    normalized=[]
    for mode in modes:
        record=records[mode];context=record["identity_context"]
        build=sha(work/mode);assert build==context["binary_sha256"];builds.append(build)
        ids=(work/f"{mode}.ids").read_bytes();assert len(ids)==800
        for index,identity in enumerate(context["identities"]):
            assert identity["build"]==build
            assert ids[160*index:160*(index+1)]==b"".join(bytes.fromhex(identity[key]) for key in ["program","numeric","target","build","run"])
        for suffix,expected in record["payload_sha256"].items():assert sha(work/f"{mode}.{suffix}")==expected
        raw=(work/f"{mode}.snapshots").read_bytes();assert len(raw)==24*1312
        assert (work/f"{mode}.mid").read_bytes()==raw[3*1312:4*1312]
        normalized.append(b"".join(raw[index*1312:index*1312+112]+raw[index*1312+144:index*1312+1280] for index in range(24)))
        assert (work/f"{mode}.trace").read_bytes()==(work/"debug.trace").read_bytes()
    assert len(set(builds))==3 and normalized[0]==normalized[1]==normalized[2]
    cross=[]
    for destination in modes:
        for origin in modes:
            if destination==origin:continue
            output=subprocess.check_output([str((work/destination).resolve()),"--cross",str((work/f"{destination}.ids").resolve()),str((work/f"{origin}.mid").resolve())],text=True)
            assert output=="cross-build restore refused atomically\n"
            cross.append(dict(destination=destination,origin=origin,status="build mismatch, destination unchanged"))
    result=dict(schema="psionic-oscillator-replay-modes-v1",modes=modes,build_sha256=dict(zip(modes,builds)),
                all980_input_state_tick_records_bit_identical=True,all24_snapshot_bodies_equal_except_build_id=True,
                snapshot_digest_differences_expected=True,cross_build_refusals=cross,
                mode_reference_sha256={mode:sha(work/f"{mode}.reference.json") for mode in modes})
    (work/"modes.json").write_text(json.dumps(result,indent=2)+"\n")
    print("replay modes:980 exact transition records;24 snapshots differ only in build/digest;6 cross-build restores refused")


if __name__=="__main__":main()
