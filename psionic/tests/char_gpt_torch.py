#!/usr/bin/env python3
"""Independent PyTorch baseline and oracle for char-gpt-shakespeare-v1 (#12627).

Firm reason: 2 (an independent oracle and comparator; written from the frozen
specification in docs/verification/psionic-train-shakespeare-2026-11-03/,
not from the Coax code; never shipped, never needed at run time).

PyTorch is not a repository dependency. Run it through Nix:

  nix shell --impure --expr 'let p = (builtins.getFlake "nixpkgs").legacyPackages.${builtins.currentSystem}; in p.python3.withPackages (ps: [ps.torch])' \\
    -c python3 -I psionic/tests/char_gpt_torch.py MODE ...

Modes:
  train DATA OUT [--budget S] [--threads N] [--device cpu|cuda]
      The baseline run in FP32 (on the CPU by default; with cuda, TF32 off). Resumes from OUT/state.pt; stops
      after S seconds of steps (default 780) at a step boundary and saves.
  oracle DATA OUT --config tiny|full [--dtype f64|f32] [--steps K]
      Writes the specification's initial parameters (f32), the first K
      steps' losses, gradient norms, gradients of step 1 and parameters
      after K AdamW steps, computed in the given dtype.
  compare ORACLE_DIR COAX_DIR OUT.json [--rtol R]
      Compares a Coax dump with an oracle dump and writes the gaps.
"""
import json
import math
import os
import struct
import sys
import time
from pathlib import Path

import torch

M64 = (1 << 64) - 1
SHA = "86c4e6aa9db7c042ec79f339dcb96d42b0075e16b8fc2e86bf0ca57e2dc565ed"
FULL = dict(vocab=65, d=384, layers=6, heads=6, context=256, hidden=1536, rows=64, steps=750, warmup=100)
TINY = dict(vocab=65, d=128, layers=2, heads=2, context=64, hidden=512, rows=4, steps=750, warmup=100)
PEAK, FLOOR, B1, B2, EPS, DECAY, CLIP = 1e-3, 1e-4, 0.9, 0.99, 1e-8, 0.1, 1.0


def cr_bits(seed, counter):
    z = (seed + 0x9E3779B97F4A7C15 * (counter + 1)) & M64
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & M64
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & M64
    return z ^ (z >> 31)


def load(path):
    import hashlib
    raw = Path(path).read_bytes()
    if hashlib.sha256(raw).hexdigest() != SHA:
        raise SystemExit("corpus hash differs from the frozen workload")
    vocab = sorted(set(raw))
    index = {b: i for i, b in enumerate(vocab)}
    ids = torch.tensor([index[b] for b in raw], dtype=torch.long)
    cut = len(raw) * 9 // 10
    return ids[:cut], ids[cut:]


def shapes(c):
    d, v, t, h = c["d"], c["vocab"], c["context"], c["hidden"]
    out = [("tok", (v, d)), ("pos", (t, d))]
    for l in range(c["layers"]):
        out += [(f"qkv{l}", (d, 3 * d)), (f"o{l}", (d, d)), (f"up{l}", (d, h)), (f"down{l}", (h, d))]
    return out + [("head", (d, v))]


def init(c):
    params = []
    for k, (name, shape) in enumerate(shapes(c)):
        n = shape[0] * shape[1]
        std = 0.02 / math.sqrt(2 * c["layers"]) if name.startswith(("o", "down")) else 0.02
        vals = []
        for i in range(n):
            u1 = ((cr_bits(1000 + k, 2 * i) >> 11) + 1) / 9007199254740992.0
            u2 = (cr_bits(1000 + k, 2 * i + 1) >> 11) / 9007199254740992.0
            vals.append(std * (math.sqrt(-2.0 * math.log(u1)) * math.cos(2.0 * math.pi * u2)))
        # Rounded once from binary64 to FP32, as the specification says.
        params.append(torch.tensor(vals, dtype=torch.float64).to(torch.float32).reshape(shape))
    return [(name, p) for (name, _), p in zip(shapes(c), params)]


def rows(c, train, step):
    span = len(train) - c["context"]
    starts = [cr_bits(7, c["rows"] * step + r) % span for r in range(c["rows"])]
    x = torch.stack([train[s:s + c["context"]] for s in starts])
    y = torch.stack([train[s + 1:s + 1 + c["context"]] for s in starts])
    return x.to(DEVICE), y.to(DEVICE)


def rms(x):
    return x / torch.sqrt((x * x).mean(-1, keepdim=True) + 1e-5)


def forward(c, p, x):
    b, t = x.shape
    hd = c["d"] // c["heads"]
    z = p["tok"][x] + p["pos"][:t]
    mask = torch.ones(t, t, dtype=torch.bool, device=x.device).tril()
    for l in range(c["layers"]):
        q, k, v = (rms(z) @ p[f"qkv{l}"]).split(c["d"], dim=-1)
        q, k, v = (a.view(b, t, c["heads"], hd).transpose(1, 2) for a in (q, k, v))
        s = (q @ k.transpose(-1, -2)) * (1.0 / math.sqrt(hd))
        s = s.masked_fill(~mask, float("-inf"))
        o = (torch.softmax(s, dim=-1) @ v).transpose(1, 2).reshape(b, t, c["d"])
        z = z + o @ p[f"o{l}"]
        z = z + torch.relu(rms(z) @ p[f"up{l}"]).square() @ p[f"down{l}"]
    return rms(z) @ p["head"]


def loss_of(c, p, x, y):
    logits = forward(c, p, x)
    return torch.nn.functional.cross_entropy(logits.reshape(-1, c["vocab"]), y.reshape(-1))


def rate(c, s):
    if s < c["warmup"]:
        return PEAK * (s + 1) / c["warmup"]
    f = (s - c["warmup"]) / (c["steps"] - c["warmup"])
    return FLOOR + 0.5 * (PEAK - FLOOR) * (1.0 + math.cos(math.pi * f))


def held_out(c, p, val):
    t = c["context"]
    windows = (len(val) - 1) // t
    total, count = 0.0, 0
    with torch.no_grad():
        for w0 in range(0, windows, 32):
            starts = range(w0 * t, min(windows, w0 + 32) * t, t)
            x = torch.stack([val[s:s + t] for s in starts])
            y = torch.stack([val[s + 1:s + 1 + t] for s in starts])
            x, y = x.to(DEVICE), y.to(DEVICE)
            logits = forward(c, p, x).double()
            total += torch.nn.functional.cross_entropy(logits.reshape(-1, c["vocab"]), y.reshape(-1), reduction="sum").item()
            count += y.numel()
    return total / count, count


DEVICE = "cpu"


def make(c, dtype):
    named = init(c)
    p = {n: t.to(dtype).to(DEVICE).clone().requires_grad_(True) for n, t in named}
    opt = torch.optim.AdamW([p[n] for n, _ in named], lr=PEAK, betas=(B1, B2), eps=EPS, weight_decay=DECAY, foreach=False)
    return named, p, opt


def step(c, p, opt, names, x, y, s):
    for g in opt.param_groups:
        g["lr"] = rate(c, s)
    opt.zero_grad(set_to_none=True)
    loss = loss_of(c, p, x, y)
    loss.backward()
    norm = torch.nn.utils.clip_grad_norm_([p[n] for n in names], CLIP, foreach=False).item()
    if not math.isfinite(norm):
        return loss.item(), norm, False
    opt.step()
    return loss.item(), norm, True


def train(args):
    data, out = args[0], Path(args[1])
    budget = float(args[args.index("--budget") + 1]) if "--budget" in args else 780.0
    global DEVICE
    torch.set_num_threads(int(args[args.index("--threads") + 1]) if "--threads" in args else 16)
    DEVICE = args[args.index("--device") + 1] if "--device" in args else "cpu"
    torch.backends.cuda.matmul.allow_tf32 = False
    torch.backends.cudnn.allow_tf32 = False
    torch.manual_seed(0)
    out.mkdir(parents=True, exist_ok=True)
    c = FULL
    train_ids, val = load(data)
    named, p, opt = make(c, torch.float32)
    names = [n for n, _ in named]
    state = out / "state.pt"
    s0 = 0
    if state.exists():
        saved = torch.load(state, weights_only=False, map_location=DEVICE)
        with torch.no_grad():
            for n in names:
                p[n].copy_(saved["params"][n])
        opt.load_state_dict(saved["opt"])
        s0 = saved["step"]
    log = open(out / "log.jsonl", "a")
    if s0 == 0:
        v, count = held_out(c, p, val)
        log.write(json.dumps(dict(step=0, held_out=v, predictions=count)) + "\n")
    start = time.monotonic()
    s = s0
    while s < c["steps"] and time.monotonic() - start < budget:
        x, y = rows(c, train_ids, s)
        t0 = time.monotonic()
        loss, norm, ok = step(c, p, opt, names, x, y, s)
        s += 1
        if DEVICE == "cuda":
            torch.cuda.synchronize()
        rec = dict(step=s, loss=loss, norm=norm, rate=rate(c, s - 1), applied=ok, seconds=time.monotonic() - t0)
        if s % 250 == 0 or s == c["steps"]:
            rec["held_out"], _ = held_out(c, p, val)
        log.write(json.dumps(rec) + "\n")
        log.flush()
    torch.save(dict(step=s, params={n: p[n].detach() for n in names}, opt=opt.state_dict()), state)
    peak = torch.cuda.max_memory_allocated() if DEVICE == "cuda" else 0
    print(json.dumps(dict(step=s, done=s >= c["steps"], seconds=time.monotonic() - start, device=DEVICE, peak_bytes=peak)))


def dump(path, tensors):
    with open(path, "wb") as f:
        for t in tensors:
            f.write(t.detach().to(torch.float64).contiguous().numpy().tobytes())


def oracle(args):
    data, out = args[0], Path(args[1])
    c = dict(FULL if args[args.index("--config") + 1] == "full" else TINY)
    dtype = torch.float64 if ("--dtype" not in args or args[args.index("--dtype") + 1] == "f64") else torch.float32
    k = int(args[args.index("--steps") + 1]) if "--steps" in args else 3
    torch.set_num_threads(16)
    out.mkdir(parents=True, exist_ok=True)
    train_ids, _ = load(data)
    named, p, opt = make(c, dtype)
    names = [n for n, _ in named]
    with open(out / "init.f32", "wb") as f:
        for _, t in named:
            f.write(t.contiguous().numpy().tobytes())
    record = dict(config=c, dtype=str(dtype), steps=[])
    for s in range(k):
        x, y = rows(c, train_ids, s)
        loss, norm, ok = step(c, p, opt, names, x, y, s)
        if s == 0:
            # Gradients of step 1 before clipping: recompute them cleanly.
            q = {n: t.to(dtype).clone().requires_grad_(True) for n, t in named}
            loss_of(c, q, x, y).backward()
            dump(out / "grad1.f64", [q[n].grad for n in names])
        record["steps"].append(dict(loss=loss, norm=norm, applied=ok, rate=rate(c, s)))
    dump(out / "params.f64", [p[n] for n in names])
    (out / "oracle.json").write_text(json.dumps(record, indent=1) + "\n")
    print(json.dumps(record["steps"]))


def read64(path):
    raw = Path(path).read_bytes()
    return torch.frombuffer(bytearray(raw), dtype=torch.float64)


def compare(args):
    a, b, out = Path(args[0]), Path(args[1]), Path(args[2])
    rtol = float(args[args.index("--rtol") + 1]) if "--rtol" in args else 1e-4
    result = dict(rtol=rtol, items={})
    ok = True
    ra, rb = json.loads((a / "oracle.json").read_text()), json.loads((b / "oracle.json").read_text())
    for i, (x, y) in enumerate(zip(ra["steps"], rb["steps"])):
        for key in ("loss", "norm"):
            gap = abs(x[key] - y[key]) / max(abs(x[key]), 1e-30)
            result["items"][f"step{i + 1}.{key}"] = dict(oracle=x[key], coax=y[key], relative=gap)
            ok &= gap <= rtol
    ia = torch.frombuffer(bytearray((a / "init.f32").read_bytes()), dtype=torch.float32)
    ib = torch.frombuffer(bytearray((b / "init.f32").read_bytes()), dtype=torch.float32)
    same = ia.shape == ib.shape and bool(torch.equal(ia.view(torch.int32), ib.view(torch.int32)))
    differ = int((ia.view(torch.int32) != ib.view(torch.int32)).sum()) if ia.shape == ib.shape else -1
    result["items"]["init"] = dict(values=ia.numel(), bit_differences=differ, max_abs=float((ia - ib).abs().max()) if ia.shape == ib.shape else None)
    ok &= ia.shape == ib.shape and float((ia - ib).abs().max()) <= 1e-9
    for name in ("grad1.f64", "params.f64"):
        x, y = read64(a / name), read64(b / name)
        if x.shape != y.shape:
            result["items"][name] = dict(error="sizes differ", oracle=x.numel(), coax=y.numel())
            ok = False
            continue
        scale = x.abs().max().item()
        gap = (x - y).abs()
        rel = (gap / (x.abs() + y.abs() + 1e-3 * scale)).max().item()
        result["items"][name] = dict(values=x.numel(), max_abs=gap.max().item(), max_scaled_relative=rel, scale=scale,
                                     cosine=float(torch.nn.functional.cosine_similarity(x, y, dim=0)))
        ok &= rel <= rtol
    result["init_bits_equal"] = same
    result["passed"] = bool(ok)
    out.write_text(json.dumps(result, indent=1) + "\n")
    print(json.dumps(dict(passed=result["passed"], out=str(out))))
    if not ok:
        sys.exit(1)


if __name__ == "__main__":
    mode, rest = sys.argv[1], sys.argv[2:]
    {"train": train, "oracle": oracle, "compare": compare}[mode](rest)
