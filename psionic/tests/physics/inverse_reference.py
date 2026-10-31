#!/usr/bin/env python3
"""Firm reason2: independent analytic-sensitivity, fit, grid and rollout oracle.

Uses derivatives of the augmented matrix polynomial, not TpNodes, Coax loss
calls, finite-difference fitting, an external AD package or the product's RK
stage construction. Python is development tooling only.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[3]
EXPERIMENT = ROOT / "psionic/fixtures/physics/inverse/experiment.json"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def random_word(seed, counter):
    mask = (1 << 64) - 1
    z = (seed + 0x9E3779B97F4A7C15 * (counter + 1)) & mask
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & mask
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & mask
    return z ^ (z >> 31)


def uniform(seed, counter):
    return (random_word(seed, counter) >> 11) / 9007199254740992


def step(state, k, c, u):
    """state=(q,v,dq/dk,dv/dk,dq/dc,dv/dc). Differentiate each matrix power."""
    q, v, qk, vk, qc, vc = state
    term = [q, v, 1.0]
    dk = [qk, vk, 0.0]
    dc = [qc, vc, 0.0]
    total, total_k, total_c = term[:], dk[:], dc[:]
    for order in range(1, 5):
        h = .05 / order
        # d(Az)/dk = (dA/dk)z + A(dz/dk), and similarly for damping.
        next_k = [h * dk[1], h * (-term[0] - k * dk[0] - c * dk[1]), 0.0]
        next_c = [h * dc[1], h * (-term[1] - k * dc[0] - c * dc[1]), 0.0]
        term = [h * term[1], h * (-k * term[0] - c * term[1] + u * term[2]), 0.0]
        dk, dc = next_k, next_c
        total = [a + b for a, b in zip(total, term)]
        total_k = [a + b for a, b in zip(total_k, dk)]
        total_c = [a + b for a, b in zip(total_c, dc)]
    return total[:2] + total_k[:2] + total_c[:2]


def data_for(mode, experiment):
    train, truth = experiment["training"], experiment["parameters"]
    amplitude = 0.0 if mode < 2 or mode == 6 else (.002 if mode < 4 else .01)
    stride = 4 if mode % 2 else 1
    initial, forces, observed = [], [], []
    for episode in range(3):
        q, v = (.5, 0.0) if mode == 6 else (train["initial_position"][episode], train["initial_velocity"][episode])
        initial.append([q, v])
        state = [q, v, 0.0, 0.0, 0.0, 0.0]
        for tick in range(24):
            index = episode * 24 + tick
            u = 1.2 if mode == 6 else [0, .5, -.5][(tick // 6 + episode) % 3]
            state = step(state, truth["truth_spring"], truth["truth_damping"], u)
            forces.append(u)
            observed.append([state[0] + amplitude * (2 * uniform(11100, 2 * index) - 1),
                             state[1] + amplitude * (2 * uniform(11100, 2 * index + 1) - 1)])
    return dict(initial=initial, forces=forces, observed=observed, stride=stride)


def objective(data, k, c):
    loss, gk, gc, observations = 0.0, 0.0, 0.0, 0
    for episode, initial in enumerate(data["initial"]):
        state = initial[:] + [0.0] * 4
        for tick in range(24):
            index = episode * 24 + tick
            state = step(state, k, c, data["forces"][index])
            if (tick + 1) % data["stride"] == 0:
                eq = state[0] - data["observed"][index][0]
                ev = state[1] - data["observed"][index][1]
                loss += eq * eq + ev * ev
                gk += eq * state[2] + ev * state[3]
                gc += eq * state[4] + ev * state[5]
                observations += 1
    return .5 * loss / observations, gk / observations, gc / observations


def fit(data, mode, experiment):
    config = experiment["gradient_fit"]
    k, c = (2.4, .9) if mode == 6 else (config["initial_spring"], config["initial_damping"])
    projected = 0
    for iteration in range(config["updates"]):
        _, gk, gc = objective(data, k, c)
        raw_k, raw_c = k - config["rate"] * gk, c - config["rate"] * gc
        k, c = max(.5, min(4, raw_k)), max(.1, min(1, raw_c))
        projected += int(k != raw_k) + int(c != raw_c)
    return dict(spring=k, damping=c, loss=objective(data, k, c)[0],
                forward=256, reverse=255, projected=projected)


def grid(data, experiment):
    side = experiment["grid"]["side"]
    best = None
    for row in range(side):
        for column in range(side):
            k = .5 + 3.5 * (row / (side - 1))
            c = .1 + .9 * (column / (side - 1))
            loss = objective(data, k, c)[0]
            if best is None or loss < best["loss"]:
                best = dict(spring=k, damping=c, loss=loss, forward=256, reverse=0, projected=0)
    return best


def forecast(k, c, experiment):
    config = experiment["heldout"]
    result = dict(rows=384, q_square=0.0, v_square=0.0, max_q=0.0, max_v=0.0)
    for episode in range(3):
        initial = [config["initial_position"][episode], config["initial_velocity"][episode]]
        truth, own = initial[:] + [0.0] * 4, initial[:] + [0.0] * 4
        for tick in range(128):
            phase = (tick + 5 * episode) % 20
            u = .4 if phase < 5 else (-.2 if phase < 12 else 0)
            truth = step(truth, 2.4, .55, u)
            own = step(own, k, c, u)
            q, v = own[0] - truth[0], own[1] - truth[1]
            result["q_square"] += q * q
            result["v_square"] += v * v
            result["max_q"] = max(result["max_q"], abs(q))
            result["max_v"] = max(result["max_v"], abs(v))
    return result



def check_transcripts(gradients, fits):
    points = [(1, .9), (2.4, .55), (3.5, .2), (.5, .1), (4, 1)]
    expected = {(mode, k, c) for mode in range(7) for k, c in points}
    assert len(gradients) == 35
    assert {(row["mode"], row["spring"], row["damping"]) for row in gradients} == expected
    sweep = [.01, .001, .0001, .00001, .000001, .0000001]
    for row in gradients:
        interior = (row["spring"], row["damping"]) in points[:3]
        assert len(row["fd"]) == (12 if interior else 0)
        wanted = {(axis, h) for axis in range(2) for h in sweep} if interior else set()
        assert {(entry["axis"], entry["h"]) for entry in row["fd"]} == wanted
    assert len(fits) == 7
    assert [row["mode"] for row in fits] == list(range(7))
    for row in fits:
        assert row["nodes"] == (3255 if row["mode"] % 2 else 3687)
        assert row["observations"] == (18 if row["mode"] % 2 else 72)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("prefix", type=Path)
    parser.add_argument("--cache", type=Path, required=True)
    args = parser.parse_args()
    experiment = json.loads(EXPERIMENT.read_text())
    budgets = experiment["budgets"]
    data = [data_for(mode, experiment) for mode in range(7)]
    expected_data = sum((sum(d["initial"], []) + d["forces"] + sum(d["observed"], []) for d in data), [])
    binary = Path(str(args.prefix) + ".data.f64").read_bytes()
    actual_data = struct.unpack("=" + "d" * (len(binary) // 8), binary)
    assert len(actual_data) == len(expected_data) == 1554
    data_error = max(abs(a - b) for a, b in zip(actual_data, expected_data))
    assert data_error <= 1e-11, data_error
    gradients = json.loads(Path(str(args.prefix) + ".gradients.json").read_text())
    actual_fits = json.loads(Path(str(args.prefix) + ".fits.json").read_text())
    check_transcripts(gradients, actual_fits)
    loss_error = gradient_error = 0.0
    fd_error = {}
    for row in gradients:
        loss, gk, gc = objective(data[row["mode"]], row["spring"], row["damping"])
        error = abs(loss - row["loss"])
        assert error <= budgets["independent_loss_absolute"], error
        loss_error = max(loss_error, error)
        for a, b in zip([gk, gc], row["gradient"]):
            error = abs(a - b)
            assert error <= budgets["analytic_gradient_absolute"], error
            gradient_error = max(gradient_error, error)
        for estimate in row["fd"]:
            error = abs(estimate["value"] - row["gradient"][estimate["axis"]])
            fd_error[str(estimate["h"])] = max(error, fd_error.get(str(estimate["h"]), 0.0))
    identity = dict(oracle_sha256=sha(Path(__file__)), experiment_sha256=sha(EXPERIMENT))
    if args.cache.exists():
        cache = json.loads(args.cache.read_text())
        assert cache["identity"] == identity, "independent oracle identity changed"
        expected_fits = cache["fits"]
    else:
        expected_fits = [dict(mode=mode, gradient=fit(d, mode, experiment), grid=grid(d, experiment))
                         for mode, d in enumerate(data)]
        args.cache.write_text(json.dumps(dict(identity=identity, fits=expected_fits), indent=2) + "\n")
    assert len(expected_fits) == 7 and [row["mode"] for row in expected_fits] == list(range(7))
    parameter_error = fit_loss_error = forecast_error = 0.0
    for row, expected in zip(actual_fits, expected_fits):
        assert row["mode"] == expected["mode"]
        for method in ["gradient", "grid"]:
            for key in ["spring", "damping"]:
                error = abs(row[method][key] - expected[method][key])
                assert error <= budgets["independent_fit_parameter_absolute"], (row["mode"], method, key, error)
                parameter_error = max(parameter_error, error)
            error = abs(row[method]["loss"] - expected[method]["loss"])
            assert error <= budgets["independent_loss_absolute"], error
            fit_loss_error = max(fit_loss_error, error)
            for key in ["forward", "reverse", "projected"]:
                assert row[method][key] == expected[method][key]
            heldout = forecast(row[method]["spring"], row[method]["damping"], experiment)
            for key, value in heldout.items():
                error = abs(row[method][key] - value)
                assert error <= 1e-11, (key, error)
                forecast_error = max(forecast_error, error)
    record = dict(schema="psionic-inverse-reference-v1", identity=identity,
                  independent_data_values=1554, analytic_gradient_cases=35,
                  finite_difference_estimates=252, fitted_and_grid_variants=7,
                  data_max_absolute=data_error, analytic_loss_max_absolute=loss_error,
                  analytic_gradient_max_absolute=gradient_error,
                  parameter_fit_max_absolute=parameter_error, fit_loss_max_absolute=fit_loss_error,
                  heldout_metric_max_absolute=forecast_error,
                  finite_difference_max_absolute_by_step=fd_error,
                  native=json.loads(Path(str(args.prefix) + ".native.json").read_text()),
                  fits=actual_fits,
                  payload_sha256={suffix:sha(Path(str(args.prefix) + "." + suffix))
                                  for suffix in ["data.f64", "states.f64", "gradients.json", "fits.json"]})
    Path(str(args.prefix) + ".reference.json").write_text(json.dumps(record, indent=2) + "\n")
    print(f"independent inverse: 1554 data,35 analytic gradients,7 fits/grids; max errors "
          f"{data_error:.9g}/{gradient_error:.9g}/{parameter_error:.9g}/{forecast_error:.9g}")


if __name__ == "__main__":
    main()
