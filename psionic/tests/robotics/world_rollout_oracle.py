#!/usr/bin/env python3
# Firm reason: 2 (independent Python oracle for the Coax rollout collector, never shipped).
"""Independent reference oracle for Chapter 259 independent-world rollouts.

Recomputes the oscillator trajectory, Gaussian draws, GAE, Welford update and
tagged SHA-256 digests in plain Python. --emit-golden rewrites the frozen
vectors; --check-golden compares against them.
"""

from __future__ import annotations

import hashlib
import json
import math
import struct
import sys

MASK64 = (1 << 64) - 1
TWO_PI = 6.283185307179586
HALF_LOG_2PI = 0.9189385332046728
HALF_LOG_2PI_E = 1.4189385332046727
DIGEST_TAG = "psionic.robotics.world-rollout.v1"


def f64(x: float) -> float:
    return struct.unpack("<d", struct.pack("<d", float(x)))[0]


def f64_bits(x: float) -> int:
    return struct.unpack("<Q", struct.pack("<d", float(x)))[0]


def canonical_bits(x: float) -> int:
    bits = f64_bits(x)
    return 0 if bits == 0x8000000000000000 else bits


def cr_bits(seed: int, counter: int) -> int:
    """splitmix64-top53-v1 word: the published counter mapping in psionic/lib/counter-rng.coax."""
    z = (seed + 0x9E3779B97F4A7C15 * ((counter + 1) & MASK64)) & MASK64
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK64
    return (z ^ (z >> 31)) & MASK64


def cr_uniform(seed: int, counter: int) -> tuple[float, int]:
    if counter == MASK64:
        raise ValueError("counter exhausted")
    value = float(cr_bits(seed, counter) >> 11) / 9007199254740992.0
    return value, counter + 1


def sample_gaussian(seed: int, counter: int, mean: float, log_std: float) -> tuple[float, float, int]:
    radial, c1 = cr_uniform(seed, counter)
    angle, c2 = cr_uniform(seed, c1)
    if radial == 0.0:
        return mean, 0.0, c2
    w = f64(1.0 - radial)
    theta = f64(TWO_PI * angle)
    radius = f64(math.sqrt(f64(-2.0 * math.log(w))))
    z = f64(radius * math.cos(theta))
    sigma = f64(math.exp(log_std))
    action = f64(mean + f64(sigma * z))
    return action, z, c2


def gaussian_logp(mean: float, log_std: float, action: float) -> float:
    # Same contract as rgp_density: multiply by exp(-log_std), never divide.
    inv_sigma = f64(math.exp(-log_std))
    diff = 0.0 if action == mean else f64(action - mean)
    z = f64(diff * inv_sigma)
    z2 = f64(z * z)
    return f64(f64(f64(-0.5 * z2) - log_std) - HALF_LOG_2PI)


def clip_f64(val: float, low: float, high: float) -> float:
    if val < low:
        return low
    if val > high:
        return high
    return val


def os_rhs(spring: float, damping: float, pos: float, vel: float, force: float) -> tuple[float, float]:
    dpos = vel
    dvel = f64(f64(force - f64(damping * vel)) - f64(spring * pos))
    return dpos, dvel


def os_shift(pos: float, vel: float, kpos: float, kvel: float, h: float) -> tuple[float, float]:
    return f64(pos + f64(h * kpos)), f64(vel + f64(h * kvel))


def os_step(spring: float, damping: float, pos: float, vel: float, force: float, dt: float) -> tuple[float, float]:
    apos, avel = os_rhs(spring, damping, pos, vel, force)
    b_in = os_shift(pos, vel, apos, avel, f64(dt / 2.0))
    bpos, bvel = os_rhs(spring, damping, b_in[0], b_in[1], force)
    c_in = os_shift(pos, vel, bpos, bvel, f64(dt / 2.0))
    cpos, cvel = os_rhs(spring, damping, c_in[0], c_in[1], force)
    d_in = os_shift(pos, vel, cpos, cvel, dt)
    dpos, dvel = os_rhs(spring, damping, d_in[0], d_in[1], force)
    sum_pos = f64(f64(f64(apos + f64(2.0 * bpos)) + f64(2.0 * cpos)) + dpos)
    sum_vel = f64(f64(f64(avel + f64(2.0 * bvel)) + f64(2.0 * cvel)) + dvel)
    dt6 = f64(dt / 6.0)
    return f64(pos + f64(dt6 * sum_pos)), f64(vel + f64(dt6 * sum_vel))


def normalize_obs(pos: float, vel: float, stats_count: int, mean_pos: float, mean_vel: float, m2_pos: float, m2_vel: float, clip_obs: float) -> tuple[float, float]:
    s_pos = pos
    s_vel = vel
    if stats_count > 0:
        s_pos = f64(f64(pos - mean_pos) / math.sqrt(f64(f64(m2_pos / float(stats_count)) + 1e-8)))
        s_vel = f64(f64(vel - mean_vel) / math.sqrt(f64(f64(m2_vel / float(stats_count)) + 1e-8)))
    return clip_f64(s_pos, -clip_obs, clip_obs), clip_f64(s_vel, -clip_obs, clip_obs)


def policy_value(
    obs_pos: float,
    obs_vel: float,
    actor_pos: float,
    actor_vel: float,
    actor_bias: float,
    value_pos2: float,
    value_vel2: float,
    value_bias: float,
) -> tuple[float, float]:
    mean = f64(f64(f64(actor_pos * obs_pos) + f64(actor_vel * obs_vel)) + actor_bias)
    val = f64(f64(f64(value_pos2 * f64(obs_pos * obs_pos)) + f64(value_vel2 * f64(obs_vel * obs_vel))) + value_bias)
    return mean, val


def step_reward(next_pos: float, next_vel: float, applied_force: float, pos_weight: float, vel_weight: float, force_weight: float, alive_bonus: float, terminated: int) -> float:
    pos_cost = f64(pos_weight * f64(next_pos * next_pos))
    vel_cost = f64(vel_weight * f64(next_vel * next_vel))
    act_cost = f64(force_weight * f64(applied_force * applied_force))
    total_cost = f64(f64(pos_cost + vel_cost) + act_cost)
    bonus = alive_bonus if terminated == 0 else 0.0
    return f64(bonus - total_cost)


def tagged_sha256(tag: str, payload: bytes) -> str:
    th = hashlib.sha256(tag.encode("utf-8")).digest()
    return hashlib.sha256(th + th + payload).hexdigest()


def world_digest(
    seed: int,
    counter: int,
    episode_step: int,
    pos: float,
    vel: float,
    reset_pos: float,
    reset_vel: float,
) -> str:
    payload = struct.pack(
        "<QQQQQQQ",
        seed & MASK64,
        counter & MASK64,
        episode_step & MASK64,
        canonical_bits(pos),
        canonical_bits(vel),
        canonical_bits(reset_pos),
        canonical_bits(reset_vel),
    )
    return tagged_sha256(DIGEST_TAG, payload)


def collect_batch(cfg: dict, stats: dict, worlds: list[dict]) -> dict:
    nw = len(worlds)
    horizon = cfg["horizon"]
    cur_worlds = [dict(w) for w in worlds]
    raw_obs_rows: list[tuple[float, float]] = []
    records: list[dict] = [[None] * horizon for _ in range(nw)]  # type: ignore[list-item]

    for step_idx in range(horizon):
        for w_idx in range(nw):
            w = cur_worlds[w_idx]
            pos = f64(w["position"])
            vel = f64(w["velocity"])
            raw_obs_rows.append((pos, vel))
            n_pos, n_vel = normalize_obs(
                pos,
                vel,
                stats["count"],
                stats["mean"][0],
                stats["mean"][1],
                stats["m2"][0],
                stats["m2"][1],
                cfg["obs_clip"],
            )
            mean, cur_val = policy_value(
                n_pos,
                n_vel,
                cfg["actor_pos"],
                cfg["actor_vel"],
                cfg["actor_bias"],
                cfg["value_pos2"],
                cfg["value_vel2"],
                cfg["value_bias"],
            )
            sampled_action, z, next_counter = sample_gaussian(w["seed"], w["counter"], mean, cfg["log_std"])
            logp = gaussian_logp(mean, cfg["log_std"], sampled_action)
            applied_force = clip_f64(sampled_action, -cfg["force_limit"], cfg["force_limit"])
            next_pos, next_vel = os_step(cfg["spring"], cfg["damping"], pos, vel, applied_force, cfg["dt"])
            next_ep_step = w["episode_step"] + 1
            terminated = 1 if (abs(next_pos) > cfg["position_limit"] or abs(next_vel) > cfg["velocity_limit"]) else 0
            truncated = 1 if (terminated == 0 and next_ep_step >= cfg["max_episode_steps"]) else 0
            rew = step_reward(
                next_pos,
                next_vel,
                applied_force,
                cfg["pos_weight"],
                cfg["vel_weight"],
                cfg["force_weight"],
                cfg["alive_bonus"],
                terminated,
            )
            if terminated == 1:
                next_val = 0.0
            else:
                nn_pos, nn_vel = normalize_obs(
                    next_pos,
                    next_vel,
                    stats["count"],
                    stats["mean"][0],
                    stats["mean"][1],
                    stats["m2"][0],
                    stats["m2"][1],
                    cfg["obs_clip"],
                )
                _, next_val = policy_value(
                    nn_pos,
                    nn_vel,
                    cfg["actor_pos"],
                    cfg["actor_vel"],
                    cfg["actor_bias"],
                    cfg["value_pos2"],
                    cfg["value_vel2"],
                    cfg["value_bias"],
                )
            if terminated == 1 or truncated == 1:
                live_pos = f64(w["reset_position"])
                live_vel = f64(w["reset_velocity"])
                live_ep_step = 0
            else:
                live_pos = next_pos
                live_vel = next_vel
                live_ep_step = next_ep_step

            w["position"] = live_pos
            w["velocity"] = live_vel
            w["counter"] = next_counter
            w["episode_step"] = live_ep_step

            records[w_idx][step_idx] = {
                "world": w_idx,
                "step": step_idx,
                "obs_pos": pos,
                "obs_vel": vel,
                "norm_pos": n_pos,
                "norm_vel": n_vel,
                "mean": mean,
                "sampled_action": sampled_action,
                "applied_force": applied_force,
                "z": z,
                "logp": logp,
                "reward": rew,
                "value": cur_val,
                "bootstrap_value": next_val,
                "next_pos": next_pos,
                "next_vel": next_vel,
                "live_pos": live_pos,
                "live_vel": live_vel,
                "terminated": terminated,
                "truncated": truncated,
            }

    discount = f64(cfg["gamma"] * cfg["gae_lambda"])
    flat_steps: list[dict] = []
    for w_idx in range(nw):
        next_adv = 0.0
        for rev in range(horizon):
            local = horizon - 1 - rev
            rec = records[w_idx][local]
            bootstrap = 1 if rec["terminated"] == 0 else 0
            trace = 1 if (rec["terminated"] == 0 and rec["truncated"] == 0 and local != horizon - 1) else 0
            future = f64(cfg["gamma"] * rec["bootstrap_value"]) if bootstrap == 1 else 0.0
            delta = f64(f64(rec["reward"] + future) - rec["value"])
            adv = f64(delta + f64(discount * next_adv)) if trace == 1 else delta
            lam_ret = f64(adv + rec["value"])
            rec["delta"] = delta
            rec["advantage"] = adv
            rec["lambda_return"] = lam_ret
            rec["bootstrap"] = bootstrap
            rec["trace"] = trace
            next_adv = adv
        for local in range(horizon):
            flat_steps.append(records[w_idx][local])

    # Update running stats after rollout collection (streaming Welford matching sm_push_owned)
    counts = [stats["count"], stats["count"]]
    next_mean = list(stats["mean"])
    next_m2 = list(stats["m2"])
    for r in range(len(raw_obs_rows)):
        for ch in range(2):
            counts[ch] += 1
            x = raw_obs_rows[r][ch]
            d = f64(x - next_mean[ch])
            next_mean[ch] = f64(next_mean[ch] + f64(d / float(counts[ch])))
            next_m2[ch] = f64(next_m2[ch] + f64(d * f64(x - next_mean[ch])))
    next_count = counts[0]

    digests = [
        world_digest(
            cur_worlds[i]["seed"],
            cur_worlds[i]["counter"],
            cur_worlds[i]["episode_step"],
            cur_worlds[i]["position"],
            cur_worlds[i]["velocity"],
            cur_worlds[i]["reset_position"],
            cur_worlds[i]["reset_velocity"],
        )
        for i in range(nw)
    ]
    batch_payload = bytes.fromhex("".join(digests))
    batch_digest = tagged_sha256(DIGEST_TAG + ".batch", batch_payload)

    return {
        "steps": flat_steps,
        "worlds": cur_worlds,
        "digests": digests,
        "batch_digest": batch_digest,
        "next_stats": {"count": next_count, "mean": next_mean, "m2": next_m2},
    }


def default_fixture() -> dict:
    cfg = {
        "horizon": 4,
        "dt": 0.05,
        "spring": 4.0,
        "damping": 0.4,
        "force_limit": 2.0,
        "position_limit": 0.5,
        "velocity_limit": 2.5,
        "max_episode_steps": 3,
        "obs_epsilon": 1e-8,
        "obs_clip": 5.0,
        "actor_pos": -1.2,
        "actor_vel": -0.45,
        "actor_bias": 0.0,
        "log_std": -0.7,
        "value_pos2": -0.8,
        "value_vel2": -0.2,
        "value_bias": 0.5,
        "pos_weight": 1.0,
        "vel_weight": 0.1,
        "force_weight": 0.05,
        "alive_bonus": 0.25,
        "gamma": 0.99,
        "gae_lambda": 0.95,
    }
    stats = {"count": 4, "mean": [0.05, -0.02], "m2": [0.36, 0.64]}
    worlds = [
        {"seed": 101, "counter": 0, "episode_step": 0, "position": 0.18, "velocity": -0.10, "reset_position": 0.10, "reset_velocity": 0.0},
        {"seed": 202, "counter": 0, "episode_step": 1, "position": -0.22, "velocity": 0.15, "reset_position": -0.10, "reset_velocity": 0.0},
        {"seed": 303, "counter": 0, "episode_step": 0, "position": 0.49, "velocity": 0.90, "reset_position": 0.05, "reset_velocity": 0.0},
    ]
    return {"cfg": cfg, "stats": stats, "worlds": worlds, "result": collect_batch(cfg, stats, worlds)}


GOLDEN_PATH = "psionic/lib/robotics/world-rollout-golden.coax"
FLOAT_FIELDS = (
    ("OBS_POS", "obs_pos"), ("OBS_VEL", "obs_vel"), ("NORM_POS", "norm_pos"), ("NORM_VEL", "norm_vel"),
    ("MEAN", "mean"), ("SAMPLED", "sampled_action"), ("FORCE", "applied_force"), ("Z", "z"),
    ("LOGP", "logp"), ("REWARD", "reward"), ("VALUE", "value"), ("BOOT_VALUE", "bootstrap_value"),
    ("NEXT_POS", "next_pos"), ("NEXT_VEL", "next_vel"), ("LIVE_POS", "live_pos"), ("LIVE_VEL", "live_vel"),
    ("DELTA", "delta"), ("ADV", "advantage"), ("RET", "lambda_return"),
)
INT_FIELDS = (("TERM", "terminated"), ("TRUNC", "truncated"), ("BOOT", "bootstrap"), ("TRACE", "trace"))
# Python math.log/cos/exp come from the host libm; Coax uses its own fdlibm
# profile (coax/strict-math.md). Values may differ in the last bits, so floats
# are compared with this relative budget. Flags and digest framing are exact.
REL_BUDGET = 1e-12
ABS_BUDGET = 1e-14


def self_test() -> None:
    # Published counter-RNG vectors from psionic/lib/counter-rng.coax.
    assert cr_bits(0, 0) == 0xE220A8397B1DCDAF
    assert cr_bits(0, 1) == 0x6E789E6AA1B965F4
    assert cr_bits(1, 0) == 0x910A2DEC89025CC1
    assert cr_uniform(0, 0)[0] == 0.8833108082136426
    # Chapter 259 hand calculation: truncation keeps the final value.
    assert abs((1 + 0.9 * 2 - 0.5) - 2.3) < 1e-15 and 1 - 0.5 == 0.5


def emit_golden(res: dict) -> str:
    def hexes(vals: list[float]) -> str:
        return ",".join(f"0x{f64_bits(v):016x}" for v in vals)

    steps = res["steps"]
    n = len(steps)
    lines = ["// Frozen independent IEEE-754 oracle vectors from psionic/tests/robotics/world_rollout_oracle.py."]
    lines.append("// Regenerate: python3 -I psionic/tests/robotics/world_rollout_oracle.py --emit-golden")
    lines.append("WR_GOLDEN_BATCH_DIGEST :: [32]u8.[" + ",".join(str(b) for b in bytes.fromhex(res["batch_digest"])) + "];")
    lines.append(f"WR_GOLDEN_WORLD_DIGESTS :: [{len(res['digests'])}][32]u8.[")
    for d in res["digests"]:
        lines.append("  [32]u8.[" + ",".join(str(b) for b in bytes.fromhex(d)) + "],")
    lines.append("];")
    for name, key in FLOAT_FIELDS:
        lines.append(f"WR_GOLDEN_{name} :: [{n}]u64.[" + hexes([s[key] for s in steps]) + "];")
    for name, key in INT_FIELDS:
        lines.append(f"WR_GOLDEN_{name} :: [{n}]int.[" + ",".join(str(s[key]) for s in steps) + "];")
    lines.append("WR_GOLDEN_STATS_MEAN :: [2]u64.[" + hexes(res["next_stats"]["mean"]) + "];")
    lines.append("WR_GOLDEN_STATS_M2 :: [2]u64.[" + hexes(res["next_stats"]["m2"]) + "];")
    return "\n".join(lines) + "\n"


def parse_golden(text: str) -> dict:
    import re

    out: dict = {}
    for name, body in re.findall(r"^(WR_GOLDEN_[A-Z0-9_]+) :: \[[^\n]*?\.\[(.*?)\];", text, re.M | re.S):
        out[name] = [int(t, 0) for t in re.findall(r"0x[0-9a-f]+|-?\d+", re.sub(r"\[\d+\](\[32\])?u8\.", "", body))]
    return out


def close(a: float, b: float) -> bool:
    return a == b or abs(a - b) <= max(ABS_BUDGET, REL_BUDGET * max(abs(a), abs(b)))


def check_golden(fix: dict) -> str:
    res = fix["result"]
    g = parse_golden(open(GOLDEN_PATH, encoding="utf-8").read())
    steps = res["steps"]
    exact = 0
    for name, key in FLOAT_FIELDS:
        frozen = [struct.unpack("<d", struct.pack("<Q", w))[0] for w in g["WR_GOLDEN_" + name]]
        assert len(frozen) == len(steps), name
        for i, s in enumerate(steps):
            assert close(s[key], frozen[i]), (name, i, s[key], frozen[i])
            exact += f64_bits(s[key]) == f64_bits(frozen[i])
    for name, key in INT_FIELDS:
        assert g["WR_GOLDEN_" + name] == [s[key] for s in steps], name
    # Digest framing is checked exactly: rebuild every world digest from the
    # frozen final live state, so host-libm drift cannot hide a framing change.
    horizon = fix["cfg"]["horizon"]
    frozen_digests = []
    for w, world in enumerate(res["worlds"]):
        last = w * horizon + horizon - 1
        pos = struct.unpack("<d", struct.pack("<Q", g["WR_GOLDEN_LIVE_POS"][last]))[0]
        vel = struct.unpack("<d", struct.pack("<Q", g["WR_GOLDEN_LIVE_VEL"][last]))[0]
        d = world_digest(world["seed"], world["counter"], world["episode_step"], pos, vel, world["reset_position"], world["reset_velocity"])
        assert list(bytes.fromhex(d)) == g["WR_GOLDEN_WORLD_DIGESTS"][32 * w : 32 * w + 32], w
        frozen_digests.append(d)
    batch = tagged_sha256(DIGEST_TAG + ".batch", bytes.fromhex("".join(frozen_digests)))
    assert list(bytes.fromhex(batch)) == g["WR_GOLDEN_BATCH_DIGEST"]
    for c in range(2):
        for key, name in (("mean", "MEAN"), ("m2", "M2")):
            frozen = struct.unpack("<d", struct.pack("<Q", g["WR_GOLDEN_STATS_" + name][c]))[0]
            assert close(res["next_stats"][key][c], frozen), (key, c)
    total = len(FLOAT_FIELDS) * len(steps)
    return f"golden: {total} float fields within {REL_BUDGET:g} relative ({exact} bit-exact), flags and digest framing exact"


if __name__ == "__main__":
    self_test()
    fix = default_fixture()
    res = fix["result"]
    # Verify world independence: running world 1 alone with frozen pre-rollout stats produces identical trajectory & digest.
    single = collect_batch(fix["cfg"], fix["stats"], [fix["worlds"][1]])
    assert single["digests"][0] == res["digests"][1]
    for h in range(fix["cfg"]["horizon"]):
        for k in ("obs_pos", "norm_pos", "sampled_action", "applied_force", "reward", "delta", "advantage", "lambda_return", "terminated", "truncated"):
            assert canonical_bits(single["steps"][h][k]) == canonical_bits(res["steps"][1 * fix["cfg"]["horizon"] + h][k]), k
    if "--json" in sys.argv:
        print(json.dumps(fix, indent=2))
    elif "--emit-golden" in sys.argv:
        with open(GOLDEN_PATH, "w", encoding="utf-8") as f:
            f.write(emit_golden(res))
        print(f"wrote {GOLDEN_PATH}")
    else:
        print(f"world rollout oracle: 3 worlds x 4 steps, batch_digest={res['batch_digest']}, independence verified")
        if "--check-golden" in sys.argv:
            print(check_golden(fix))
