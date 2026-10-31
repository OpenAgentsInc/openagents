#!/usr/bin/env python3
"""Firm reason2: independent stdlib matrix-polynomial/data/SGD/rollout oracle.

No simulator/model imports, subprocess downloads, third-party code or runtime
requirement. The teacher is evaluated as a fourth-order augmented matrix
polynomial, rather than reproducing the four-stage product implementation.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[3]
EXPERIMENT = ROOT / "psionic/fixtures/physics/oscillator/experiment.json"
CONTRACT = ROOT / "psionic/fixtures/physics/oscillator/verification-contract.json"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_doubles(path):
    raw = path.read_bytes()
    assert len(raw) % 8 == 0
    return list(struct.unpack("=" + "d" * (len(raw) // 8), raw))


def teacher(q, v, k, c, u, dt=.05):
    total = [q, v, 1.0]
    term = total[:]
    for order in range(1, 5):
        term = [dt / order * term[1],
                dt / order * (-k * term[0] - c * term[1] + u * term[2]), 0.0]
        total = [a + b for a, b in zip(total, term)]
    return total[:2]


def force(identity, tick):
    if identity < 100:
        return [0, .5, -.5][(tick // 8 + identity) % 3]
    if identity < 200:
        return .25 * ((tick // 8 + identity) % 5 - 2)
    phase = (tick + 3 * identity) % 20
    return .5 if phase < 5 else (-.25 if phase < 10 else 0)


def dataset(experiment):
    x, y, groups = [], [], []
    for name, group in experiment["groups"].items():
        for index, identity in enumerate(group["ids"]):
            k, c = group["spring"][index], group["damping"][index]
            q, v = group["position"][index], group["velocity"][index]
            for tick in range(32 if name == "train" else 128):
                u = force(identity, tick)
                q1, v1 = teacher(q, v, k, c, u)
                x.append([q, v, u, k, c])
                y.append([(q1 - q) / .05, (v1 - v) / .05])
                groups.append(identity)
                q, v = q1, v1
    return x, y, groups


def scale(values):
    mean = math.fsum(values) / len(values)
    return mean, math.sqrt(math.fsum((x - mean) ** 2 for x in values) / len(values))


def random_word(seed, counter):
    mask = (1 << 64) - 1
    z = (seed + 0x9E3779B97F4A7C15 * (counter + 1)) & mask
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & mask
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & mask
    return z ^ (z >> 31)


def uniform(seed, counter):
    return (random_word(seed, counter) >> 11) / 9007199254740992


def forward(parameters, x, hidden):
    if not hidden:
        return math.fsum([parameters[5]] + [parameters[j] * x[j] for j in range(5)]), []
    h = [math.tanh(math.fsum([parameters[40 + k]] +
                            [parameters[5 * k + j] * x[j] for j in range(5)]))
         for k in range(8)]
    return math.fsum([parameters[56]] + [parameters[48 + k] * h[k] for k in range(8)]), h


def train(x, y, axis, hidden, config):
    count = 57 if hidden else 6
    parameters = [0.0] * count
    if hidden:
        seed = config["mlp_init_seeds"][axis]
        for slot in range(40):
            parameters[slot] = (2 * uniform(seed, slot) - 1) / math.sqrt(5)
        for slot in range(48, 56):
            parameters[slot] = (2 * uniform(seed, slot) - 1) / math.sqrt(8)
    order = list(range(256))
    counter = 0
    for epoch in range(config["epochs"]):
        for i in range(255, 0, -1):
            j = random_word(config["shuffle_seeds"][axis], counter) % (i + 1)
            counter += 1
            order[i], order[j] = order[j], order[i]
        for start in range(0, 256, config["batch"]):
            batch = order[start:start + config["batch"]]
            terms = [[] for _ in range(count)]
            for row in batch:
                prediction, h = forward(parameters, x[row], hidden)
                residual = prediction - y[row][axis]
                if not hidden:
                    for j in range(5):
                        terms[j].append(residual * x[row][j])
                    terms[5].append(residual)
                else:
                    terms[56].append(residual)
                    for k in range(8):
                        terms[48 + k].append(residual * h[k])
                        back = residual * parameters[48 + k] * (1 - h[k] * h[k])
                        terms[40 + k].append(back)
                        for j in range(5):
                            terms[5 * k + j].append(back * x[row][j])
            for slot in range(count):
                gradient = math.fsum(terms[slot]) / len(batch)
                weight = slot < 5 if not hidden else (slot < 40 or 48 <= slot < 56)
                if weight:
                    gradient += config["decay"] * parameters[slot]
                parameters[slot] -= config["rate"] * gradient
    return parameters


def fitted(x, y, experiment):
    means, sd = zip(*(scale([row[j] for row in x[:256]]) for j in range(5)))
    target_means, target_sd = zip(*(scale([row[j] for row in y[:256]]) for j in range(2)))
    xn = [[(row[j] - means[j]) / sd[j] for j in range(5)] for row in x[:256]]
    yn = [[(row[j] - target_means[j]) / target_sd[j] for j in range(2)] for row in y[:256]]
    affine = [train(xn, yn, axis, False, experiment["training"]) for axis in range(2)]
    mlp = [train(xn, yn, axis, True, experiment["training"]) for axis in range(2)]
    return list(means) + list(sd) + list(target_means) + list(target_sd) + sum(affine, []) + sum(mlp, [])


def predict(fit, kind, q, v, k, c, u):
    if kind == 1:
        return q, v
    x = [(value - fit[j]) / fit[5 + j] for j, value in enumerate([q, v, u, k, c])]
    delta = fit[10:12]
    for axis in range(2):
        if kind == 2:
            output, _ = forward(fit[14 + 6 * axis:20 + 6 * axis], x, False)
            delta[axis] += fit[12 + axis] * output
        if kind == 3:
            output, _ = forward(fit[26 + 57 * axis:83 + 57 * axis], x, True)
            delta[axis] += fit[12 + axis] * output
    out = q + .05 * delta[0], v + .05 * delta[1]
    return out if abs(out[0]) <= 2 and abs(out[1]) <= 4 else None


def evaluate(fit, kind, split, experiment):
    name = ["train", "validation", "test"][split]
    group = experiment["groups"][name]
    metric = dict(split=split, model=kind, episodes=len(group["ids"]), completed=0,
                  one_rows=0, roll_rows=0, one_refused=0, roll_refused=0,
                  first_episode=-1, first_tick=-1,
                  one_q=0., one_v=0., roll_q=0., roll_v=0., max_q=0., max_v=0.)
    for index, identity in enumerate(group["ids"]):
        q, v = group["position"][index], group["velocity"][index]
        k, c = group["spring"][index], group["damping"][index]
        own_q, own_v = q, v
        alive = True
        for tick in range(32 if split == 0 else 128):
            u = force(identity, tick)
            nq, nv = teacher(q, v, k, c, u)
            one = predict(fit, kind, q, v, k, c, u)
            if one is None:
                metric["one_refused"] += 1
            else:
                metric["one_rows"] += 1
                metric["one_q"] += (one[0] - nq) ** 2
                metric["one_v"] += (one[1] - nv) ** 2
            if alive:
                own = predict(fit, kind, own_q, own_v, k, c, u)
                if own is None:
                    alive = False
                    metric["roll_refused"] += 1
                    if metric["first_episode"] < 0:
                        metric["first_episode"], metric["first_tick"] = identity, tick
                else:
                    own_q, own_v = own
                    metric["roll_rows"] += 1
                    metric["roll_q"] += (own_q - nq) ** 2
                    metric["roll_v"] += (own_v - nv) ** 2
                    metric["max_q"] = max(metric["max_q"], abs(own_q - nq))
                    metric["max_v"] = max(metric["max_v"], abs(own_v - nv))
            q, v = nq, nv
        metric["completed"] += int(alive)
    return metric


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("prefix", type=Path)
    parser.add_argument("--cache", type=Path, required=True)
    args = parser.parse_args()
    experiment, contract = json.loads(EXPERIMENT.read_text()), json.loads(CONTRACT.read_text())
    x, y, groups = dataset(experiment)
    actual = load_doubles(Path(str(args.prefix) + ".data.f64"))
    expected = sum(x, []) + sum(y, [])
    assert len(actual) == len(expected) == 8960
    data_error = max(abs(a - b) for a, b in zip(actual, expected))
    assert data_error <= contract["independent_dataset_absolute"], data_error
    assert list(struct.unpack("=1280q", Path(str(args.prefix) + ".groups.i64").read_bytes())) == groups
    identity = {"oracle_sha256": digest(Path(__file__)), "experiment_sha256": digest(EXPERIMENT),
                "contract_sha256": digest(CONTRACT)}
    if args.cache.exists():
        cache = json.loads(args.cache.read_text())
        assert cache["identity"] == identity, "oracle cache identity changed"
        expected_fit = cache["fit"]
    else:
        expected_fit = fitted(x, y, experiment)
        args.cache.write_text(json.dumps({"identity": identity, "fit": expected_fit}, indent=2) + "\n")
    actual_fit = load_doubles(Path(str(args.prefix) + ".fit.f64"))
    assert len(actual_fit) == len(expected_fit) == 140
    fit_error = max(abs(a - b) for a, b in zip(actual_fit, expected_fit))
    assert fit_error <= contract["independent_full_training_parameter_absolute"], fit_error
    actual_metrics = json.loads(Path(str(args.prefix) + ".metrics.json").read_text())
    worst_metric = 0.0
    for actual in actual_metrics:
        expected = evaluate(actual_fit, actual["model"], actual["split"], experiment)
        for key in expected:
            if isinstance(expected[key], float):
                error = abs(actual[key] - expected[key])
                assert error <= contract["independent_metric_absolute"], (key, actual, expected, error)
                worst_metric = max(worst_metric, error)
            else:
                assert actual[key] == expected[key], (key, actual, expected)
    record = dict(schema="psionic-oscillator-reference-v1", identity=identity,
                  independent_data_values=8960, independent_group_ids=1280,
                  independent_trained_parameter_and_scale_values=140, metric_records=12,
                  dataset_max_absolute=data_error, fit_max_absolute=fit_error,
                  metric_max_absolute=worst_metric,
                  native=json.loads(Path(str(args.prefix) + ".native.json").read_text()),
                  metrics=actual_metrics,
                  native_payload_sha256={suffix: digest(Path(str(args.prefix) + "." + suffix))
                                         for suffix in ["data.f64", "groups.i64", "fit.f64", "metrics.json", "steps.f64"]})
    Path(str(args.prefix) + ".reference.json").write_text(json.dumps(record, indent=2) + "\n")
    print(f"independent oscillator: 8960 values, 1280 groups, 140 scales/parameters, 12 metrics; "
          f"max errors {data_error:.9g}/{fit_error:.9g}/{worst_metric:.9g}")


if __name__ == "__main__":
    main()
