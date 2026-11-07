# Firm reason: 2 (independent oracle for actual allocation lifetimes and flat-corridor walking observations).
"""Check SDK traces against RCM1 and an independent binary32 1-D walk.

This does not execute Coax, infer physical GPU backing, or qualify recovery.
The supported reference has flat ground, no obstructing actors on x=32,
64 Hz steps, 60 m/s² acceleration, 7.5 m/s speed and held world edges.
"""
from collections import Counter, defaultdict
import copy
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

from streaming_assets_oracle import png, words


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def state_wire_vector(flag):
    """Independent v1 wire vector: zero scalars, one cell/body/decoded pair,
    one command per outbox, and every Boolean set to the supplied bit.

    This raw encoding fixture tests the byte contract, not world admission.
    Offsets are semantic wire words, independent of C/Wasm struct layout.
    """
    v = int(flag)
    header = [1398231857, 1, v, v] + [0] * 14
    schedule = [0] * 13
    player = [0] * 33 + [v, 0, v, v, 0, v, v, 0, v, v, 0, 0, 0, v, 0, 0] + [0] * 13 + [v, 0, 0, 0]
    profile, surface = [0] * 26, [0] * 10
    cells = [1, 0] + [0] * 7 + [0, 0, 0, 0, 0, 1] + [0] * 8
    entities = [0] * 8
    for i in range(12):
        entities += [0, v, v, v, 0, 0, 0, v, v, 6 if i == 0 else 0, 1 if i == 0 else 0] + [0] * 8
        if i == 0:
            entities += [0] * 14
    stream = [v, v] + [0] * 10 + [0] * 8
    budget = [v] + [0] * 10 + [0] * (7 * 6 + 9)
    owners = ([v, 0, 0, 0, 0, v, 0, 0, 0, 0, 0, 0, v, v] + [0] * (8 + 15 + 3)) * 64
    claims, prefetch = [v, 0, 0] * 64, [0] * 3
    inbox = [v, 0, 0, 0, 0, 0, 0, v] + [v, 0, 0] * 64
    outboxes = [1, 1, 1] + [0] * (11 + 11 + 4 + 11 + 8)
    words = header + schedule + player + profile + surface + cells + entities + stream + budget + owners + claims + prefetch + inbox + outboxes
    data = struct.pack("<%dq" % len(words), *words)
    return {"flag": v, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def inspect_native(scene_dir, receipt_path, receipt, report_path):
    report = json.loads(report_path.read_bytes())
    version = report["schema"]
    assert version in ("aethyr.stream-walk-native-replay.v1", "aethyr.stream-walk-native-replay.v2")
    full_state = version.endswith(".v2")
    if full_state:
        assert report["state_schema"] == receipt["profile"]["stateSchema"] == "aethyr.stream-walk-state.v1"
    assert report["input_sha256"] == hashlib.sha256(receipt_path.read_bytes()).hexdigest()
    assert report["frames"] == len(receipt["frames"])
    assert report["ticks"] == receipt["cleanup"]["info"][0]
    assert report["attempts"] == receipt["cleanup"]["info"][20]
    assert report.get("tier", 0) == receipt.get("profile", {}).get("tier", 0)
    assert 0 < report["cpu_peak"] <= 32768
    observed = bytearray()
    all_frames = [(f[13], f[3:6], f[14] if full_state else "", f[2]) for f in receipt["frames"]]
    all_frames += [(f["records"], f["position"], "".join(f["stepDigests"]) if full_state else "", f["tick"])
                   for f in receipt["cleanup"]["closingFrames"]]
    previous_tick = 0
    for records, position, marks, tick in all_frames:
        if full_state:
            assert len(marks) == (tick - previous_tick) * 64
            assert all(c in "0123456789abcdef" for c in marks)
            observed.extend(marks.encode("ascii"))
        previous_tick = tick
        for record in records:
            observed.extend(struct.pack("<23q", *map(int, record)))
        for value in position:
            observed.extend(struct.pack("<q", struct.unpack("<I", struct.pack("<f", value))[0]))
    assert report["observation_sha256"] == hashlib.sha256(observed).hexdigest()
    # These are deliberately different inputs, not repetitions of the passing
    # native route. Every observer must notice a changed expectation/history.
    with tempfile.TemporaryDirectory(prefix="aethyr-stream-replay-oracle-") as temp:
        temp = Path(temp)
        changed_frame = min(100, len(receipt["frames"]) - 1)
        completed_frame, completed_record = next((i, k) for i, frame in enumerate(receipt["frames"])
                                                for k, record in enumerate(frame[13]) if int(record[1]) == 0)
        faults = ("position", "generation", "missing_completion")
        if full_state:
            faults += ("intermediate_state", "missing_step", "reordered_steps")
        for fault in faults:
            changed = copy.deepcopy(receipt)
            if fault == "position":
                changed["frames"][changed_frame][5] += 1
            elif fault == "generation":
                r = changed["frames"][completed_frame][13][completed_record]
                r[3] = str(int(r[3]) + 1)
            elif fault == "missing_completion":
                changed["frames"][completed_frame][13].pop(completed_record)
            else:
                # Change only intermediate observations; the input, accepted
                # history, frame endpoint and final actor state stay exact.
                marks = changed["frames"][changed_frame][14]
                assert len(marks) == 256
                if fault == "intermediate_state":
                    changed["frames"][changed_frame][14] = marks[:64] + ("0" if marks[64] != "0" else "1") + marks[65:]
                elif fault == "missing_step":
                    changed["frames"][changed_frame][14] = marks[64:]
                else:
                    assert marks[:64] != marks[64:128]
                    changed["frames"][changed_frame][14] = marks[64:128] + marks[:64] + marks[128:]
            source, output = temp / (fault + ".json"), temp / (fault + ".result.json")
            source.write_text(json.dumps(changed))
            # A mismatched expectation deliberately stops the observer with
            # live test allocations. Leak qualification belongs to its full
            # positive route; memory errors must still fail these negatives.
            env = dict(os.environ, ASAN_OPTIONS="detect_leaks=0:abort_on_error=1")
            result = subprocess.run([str(report_path.parent / "replay"), str(scene_dir), str(source), str(output)],
                                    capture_output=True, timeout=30, env=env)
            assert result.returncode == 1 and not output.exists() and not result.stderr, (fault, result.stdout, result.stderr)
            if fault == "intermediate_state":
                expected_tick = changed["frames"][changed_frame][2] - 2
                assert ("canonical state differs at paid tick %d" % expected_tick).encode() in result.stdout
    return {"observation_sha256": report["observation_sha256"], "cpu_peak": report["cpu_peak"],
            "negative_histories_rejected": len(faults), "canonical_paid_steps": previous_tick if full_state else 0}


def compare_tiers(low_path, high_path):
    low, high = [json.loads(path.read_bytes()) for path in (low_path, high_path)]
    assert high["profile"]["tier"] == 1
    assert len(low["frames"]) == len(high["frames"]) == 3206
    assert all(a[:9] == b[:9] for a, b in zip(low["frames"], high["frames"]))
    assert [leg["cells"] for leg in low["legs"]] == [leg["cells"] for leg in high["legs"]]
    # The immutable content and attempts agree even though two live SDK
    # schedules need not deliver completion callbacks in the same order.
    def contents(receipt):
        return Counter(tuple(record[2:5] + record[7:15]) for frame in receipt["frames"]
                       for record in frame[13] if int(record[1]) == 0)
    assert contents(low) == contents(high)
    assert low["before"]["info"][9] < high["before"]["info"][9] <= 32768
    assert low["before"]["info"][23] < high["before"]["info"][23]
    result = {"matching_gameplay_frames": 3206, "matching_final_actor_cells": 12,
              "retained_peaks": [r["before"]["info"][9] for r in (low, high)],
              "requested_cpu_peaks": [r["before"]["info"][15] for r in (low, high)],
              "final_dynamic_gpu_bytes": [r["last"]["gpuBytes"] for r in (low, high)],
              "final_decoded_image_bytes": [r["before"]["info"][23] for r in (low, high)]}
    print(json.dumps(result, sort_keys=True))


def inspect_upload_failure(scene_dir, report_path):
    scene = json.loads((scene_dir / "scene.json").read_bytes())
    report = json.loads(report_path.read_bytes())
    assert report["injected"] and report["failed"] == 1 and report["failedOwners"] == [1]
    assert not report["staleReady"] and report["retired"] and report["retried"]
    manifest_sha = report["profile"]["manifestSHA"]
    manifest = (scene_dir / (manifest_sha + ".rcm")).read_bytes()
    assert hashlib.sha256(manifest).hexdigest() == manifest_sha
    costs = [struct.unpack_from("<13q", manifest, 48 + row * 440 + 44) for row in range(64)]
    groups = list(report["allocations"]) + report["cleanup"]["closingFrames"]
    live, attempts, total, peak, events = {}, set(), 0, 0, 0
    for group in groups:
        for freed, row, generation, epoch, field, n in group.get("events", group.get("allocations", [])):
            assert 0 <= row < 64 and generation > 0 and epoch == 1 and 0 <= field < 5
            assert n == (costs[row][0], costs[row][1], costs[row][3], costs[row][4], costs[row][5])[field] > 0
            key = row, generation, epoch, field
            if not freed:
                assert freed == 0 and key not in live
                live[key] = n
                total += n
                attempts.add(key[:3])
            else:
                assert freed == 1 and live.pop(key) == n
                total -= n
            assert 0 <= total <= 32768
            peak = max(peak, total)
            events += 1
    assert not live and total == 0 and peak == report["cleanup"]["info"][15]
    assert len(attempts) == report["cleanup"]["info"][20] == report["cleanup"]["info"][21]
    records = list(report["records"]) + [r for f in report["cleanup"]["closingFrames"] for r in f["records"]]
    states, bad = defaultdict(dict), []
    for record in records:
        r = list(map(int, record))
        tick, kind, row, generation, epoch, decoded, status = r[:7]
        key = row, generation, epoch
        assert key in attempts and kind in (0, 1, 2) and kind not in states[key]
        assert struct.pack("<8I", *r[7:15]) == words(scene["resources"][row]["sha256"])
        if kind == 0:
            assert struct.pack("<8I", *r[15:23]) == words(scene["resources"][row]["sha256"])
            assert (decoded, status) in ((0, 0), (5, 6))
            if status == 6:
                bad.append(key)
        else:
            assert decoded == status == 0
            assert struct.pack("<8I", *r[15:23]) == words(hashlib.sha256(b"").hexdigest())
        states[key][kind] = tick
    assert len(bad) == 1 and bad[0][0] == 1 and set(states[bad[0]]) == {0, 1}
    assert all(set(v) == ({0, 1} if k == bad[0] else {0, 1, 2}) for k, v in states.items())
    assert any(k[0] == 1 and k[1] > bad[0][1] and v.get(2, 0) > states[bad[0]][1] for k, v in states.items())
    assert report["before"]["loadFailures"] == 1 and not report["cleanup"]["gpuErrors"]
    assert all(report["cleanup"]["info"][i] == 0 for i in (4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 22, 23))
    assert report["cleanup"]["transport"]["storage"][:2] == [1, 8388608]
    print(json.dumps({"failed_owner": 1, "attempts": len(attempts), "requested_cpu_peak": peak,
                      "physical_events": events, "failed_generation_never_ready": True,
                      "replacement_ready_after_physical_retirement": True}, sort_keys=True))


def inspect(scene_dir, receipt_path, native_report=None):
    scene = json.loads((scene_dir / "scene.json").read_bytes())
    receipt = json.loads(receipt_path.read_bytes())
    allocations = json.loads(Path(str(receipt_path) + ".allocations.json").read_bytes())
    stages = json.loads(Path(str(receipt_path) + ".stages.json").read_bytes())
    tier = receipt.get("profile", {}).get("tier", 0)
    assert tier in (0, 1)
    descriptor = next(m for m in scene["manifests"] if m["backend"] == 0 and m["tier"] == tier)
    if "profile" in receipt:
        assert receipt["profile"]["manifestSHA"] == descriptor["sha256"]
        assert receipt["profile"]["sceneSHA"] == hashlib.sha256((scene_dir / "scene.json").read_bytes()).hexdigest()
    manifest = (scene_dir / (descriptor["sha256"] + ".rcm")).read_bytes()
    assert hashlib.sha256(manifest).hexdigest() == descriptor["sha256"]
    costs = [struct.unpack_from("<13q", manifest, 48 + row * 440 + 44) for row in range(64)]
    categories = [manifest[48 + row * 440 + 5] for row in range(64)]
    live, row_generations, visited, requests = {}, {}, set(), set()
    total, peak, events = 0, 0, 0
    owner_live, owner_peaks = Counter(), Counter()
    category_live, category_peaks = Counter(), Counter()
    caps = (8192, 512, 4096, 2048, 8192, 4096, 2048)
    for group in allocations:
        assert group["observedAfterTick"] >= 1
        for freed, row, generation, epoch, field, n in group["events"]:
            assert 0 <= row < 64 and generation > 0 and epoch == 1 and 0 <= field < 5
            expected = (costs[row][0], costs[row][1], costs[row][3], costs[row][4], costs[row][5])[field]
            assert n == expected > 0
            key = row, generation, epoch, field
            category = 6 if field >= 3 else categories[row]
            if not freed:
                assert freed == 0 and key not in live
                assert row not in row_generations or row_generations[row] == generation
                live[key] = n
                row_generations[row] = generation
                requests.add((row, generation, epoch))
                visited.add(row)
                total += n
                owner_live[row] += n
                category_live[category] += n
            else:
                assert freed == 1 and live.pop(key) == n
                total -= n
                owner_live[row] -= n
                category_live[category] -= n
                if not any(k[0] == row for k in live):
                    del row_generations[row]
            assert total >= 0 and all(0 <= category_live[c] <= caps[c] for c in range(7))
            peak = max(peak, total)
            owner_peaks[row] = max(owner_peaks[row], owner_live[row])
            category_peaks[category] = max(category_peaks[category], category_live[category])
            events += 1
    assert visited == set(range(64)) and not live and total == 0 and not any(owner_live.values())
    assert len(requests) == receipt["cleanup"]["info"][20] == receipt["cleanup"]["info"][21]
    assert peak == receipt["cleanup"]["info"][15]
    stage_keys = {(s["owner"], int(s["generation"]), int(s["epoch"])) for s in stages}
    assert stage_keys == requests and len(stage_keys) == len(stages)
    for stage in stages:
        assert stage["status"] == 0 and stage["source"] == scene["resources"][stage["owner"]]["sha256"]
        times = [stage[k] for k in ("allocationMs", "sourceMs", "decodeMs", "uploadMs")]
        assert all(t >= 0 for t in times) and sum(times) <= stage["totalMs"] + 1e-5
    position, velocity, carry, tick = f32(56), f32(0), 0, 0
    frame_times, records = [], []
    for sample in receipt["frames"]:
        full_state = receipt.get("profile", {}).get("stateSchema") == "aethyr.stream-walk-state.v1"
        assert isinstance(sample, list) and len(sample) == (15 if full_state else 14)
        ns, direction, observed_tick, x, y, z, active, drawn, updated, declared, cpu, gpu, elapsed, accepted = sample[:14]
        assert ns in (15625000, 62500000) and direction in (-1, 0, 1)
        carry += ns
        steps = carry // 15625000
        assert steps <= 8
        if full_state:
            assert len(sample[14]) == steps * 64 and all(c in "0123456789abcdef" for c in sample[14])
        carry -= steps * 15625000
        for _ in range(steps):
            difference = f32(f32(direction * 7.5) - velocity)
            change = min(abs(difference), 0.9375)
            velocity = f32(velocity + (change if difference > 0 else -change if difference < 0 else 0))
            candidate = f32(position + f32(velocity * 0.015625))
            if 0 <= candidate < 768:
                position = candidate
            else:
                velocity = f32(0)
            tick += 1
        assert (x, y, z, observed_tick) == (32, 0, position, tick)
        assert 0 <= updated <= active <= 4 and 0 <= drawn <= active and 0 <= cpu + gpu <= declared <= 32768
        assert elapsed >= 0
        frame_times.append(elapsed)
        records.extend(accepted)
    for frame in receipt["cleanup"]["closingFrames"]:
        assert frame["position"] == [32, 0, position]
        records.extend(frame["records"])
    outcomes = Counter()
    seen_records = defaultdict(set)
    for r in records:
        applied, kind, row, generation, epoch, decoded, status = map(int, r[:7])
        assert 1 <= applied <= receipt["cleanup"]["info"][0] and decoded == 0 and status == 0
        assert (row, generation, epoch) in requests
        source = scene["resources"][row]["sha256"]
        assert struct.pack("<8I", *map(int, r[7:15])) == words(source)
        if kind == 0:
            assert struct.pack("<8I", *map(int, r[15:23])) == words(source)
        else:
            assert kind in (1, 2)
            assert struct.pack("<8I", *map(int, r[15:23])) == words(hashlib.sha256(b"").hexdigest())
        assert kind not in seen_records[row, generation, epoch]
        seen_records[row, generation, epoch].add(kind)
        outcomes[kind] += 1
    assert all(kinds == {0, 1, 2} for kinds in seen_records.values())
    assert set(seen_records) == requests
    assert receipt["legs"][0]["last"]["position"][2] > 704 and position < 64
    for leg in receipt["legs"]:
        assert [c[3] for c in leg["cells"]] == list(range(1, 13))
    assert receipt["legs"][1]["cells"][0][2] > receipt["legs"][0]["cells"][0][2] > 0
    cleanup = receipt["cleanup"]
    assert all(cleanup["info"][i] == 0 for i in (4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 22, 23))
    assert not cleanup["gpuErrors"] and cleanup["loadFailures"] == 0
    assert cleanup["wasmBytesBeforeDispose"] <= cleanup["wasmReservedBytes"] == 8388608
    assert cleanup["transport"]["storage"][:2] == [1, 8388608]
    assert all(cleanup["transport"][k] == 0 for k in ("owners", "loading", "published", "readers", "sdkPending"))
    width, height, pixels, _ = png(Path(str(receipt_path) + ".png").read_bytes())
    assert (width, height) == (640, 360) and len(set(struct.iter_unpack("4B", pixels))) > 1
    frame_times.sort()
    percentile = lambda p: frame_times[min(len(frame_times) - 1, int((len(frame_times) - 1) * p))]
    result = {"tier": tier, "frames": len(frame_times), "fixed_ticks": tick, "final_z": position, "physical_events": events,
              "attempts": len(requests), "requested_cpu_peak": peak, "owner_cpu_peaks": dict(sorted(owner_peaks.items())),
              "category_cpu_peaks": dict(sorted(category_peaks.items())), "outcomes": dict(outcomes),
              "frame_ms": {"p50": percentile(.5), "p95": percentile(.95), "p99": percentile(.99)}}
    if native_report:
        result["native"] = inspect_native(scene_dir, receipt_path, receipt, native_report)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    if sys.argv[1] == "--state-wire-vectors":
        print(json.dumps([state_wire_vector(False), state_wire_vector(True)], sort_keys=True))
    elif sys.argv[1] == "--native-only":
        receipt = Path(sys.argv[3])
        print(json.dumps(inspect_native(Path(sys.argv[2]), receipt, json.loads(receipt.read_bytes()), Path(sys.argv[4])), sort_keys=True))
    elif sys.argv[1] == "--compare-tiers":
        compare_tiers(Path(sys.argv[2]), Path(sys.argv[3]))
    elif sys.argv[1] == "--upload-failure":
        inspect_upload_failure(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        inspect(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]) if len(sys.argv) == 4 else None)
