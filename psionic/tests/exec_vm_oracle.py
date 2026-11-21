#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: reads the serialized F64 safetensors dense weights off disk and runs pure dense matrix multiplications and parabolic attention in Python outside Coax)
"""Independent dense-matmul & parabolic-attention oracle for exec-vm-v1 (W1.1, #13913).

Materializes the compiled Ch 226 stack+memory VM artifact onto disk via
`coax/web/tool.sh psionic/tools/exec-vm-artifact.coax`, reads the ACTUAL
serialized F64 safetensors file off disk, verifies its SHA-256 artifact digest
and tensor inventory, executes all 54 programs (squares, sum, fact, memfill +
50 seeded programs) plus typed refusal cases STRICTLY via pure dense matrix-
vector multiplications + parabolic hard-max / uniform prefix-sum attention +
ReGLU gating in Python (zero fallback to any interpreter), and verifies bit-
identical step traces and SHA-256 trace digests against BOTH Coax's loaded
dense-weight rollout AND an independent Python Ch 226 reference interpreter.
"""
import hashlib
import json
import math
import operator
import pathlib
import struct
import subprocess
import sys
import tempfile

# Ch 226 opcodes and status codes (independent copy from spec).
OP_PUSH, OP_GET, OP_SET, OP_ADD, OP_SUB, OP_MUL, OP_LOAD, OP_STORE, OP_BRIF, OP_OUT, OP_RET = range(11)
ST_OK = 0
ST_UNDERFLOW = 4
ST_OVERFLOW = 5
ST_ADDRESS = 7
ST_ARITHMETIC = 9
ST_CAPACITY = 11
ST_FELL = 12

VALUE_MAX = 1 << 26
EXACT_MAX = float(1 << 53)
TM_STACK_MAX = 32
TM_LOCALS = 8
TM_CELLS = 64
TM_OUTPUTS = 64

# Persistent KV-slot indices for prompt ROM encoding only (step execution uses ONLY loaded W_Q/W_K/W_V/W_O/FFN/W_out).
S_ONE = 0
S_PROG_K1, S_PROG_K2, S_PROG_KEY, S_PROG_OP, S_PROG_ARG, S_PROG_VAL = 1, 2, 3, 4, 5, 6
S_STEP_K2 = 7


def dot(row, vec):
    return sum(map(operator.mul, row, vec))


def matvec(mat, vec):
    return [sum(map(operator.mul, row, vec)) for row in mat]


def load_safetensors_exec_vm(path):
    """Independently parse an F64 safetensors file off disk into dense 2D matrices."""
    raw = pathlib.Path(path).read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    assert len(raw) >= 16, "safetensors file too short"
    hlen = struct.unpack_from("<Q", raw, 0)[0]
    assert hlen >= 8 and hlen % 8 == 0 and 8 + hlen <= len(raw), f"bad header length {hlen}"
    header = json.loads(raw[8:8 + hlen].decode("utf-8"))
    meta = header["__metadata__"]
    assert meta["format"] == "exec-vm-v1", f"unexpected format {meta['format']}"
    assert meta["dtype"] == "F64", f"unexpected dtype {meta['dtype']}"
    width = int(meta["width"])
    n_layers = int(meta["layers"])
    n_inputs = int(meta["inputs"])
    n_outputs = int(meta["outputs"])
    kv_slots = int(meta["kv_slots"])
    layer_spec = meta["layer_spec"]

    payload = raw[8 + hlen:]
    tensors = {}
    covered = 0
    for name, info in header.items():
        if name == "__metadata__":
            continue
        assert info["dtype"] == "F64", f"tensor {name} dtype {info['dtype']}"
        rows, cols = info["shape"]
        begin, end = info["data_offsets"]
        assert begin == covered, f"non-contiguous offset on {name}: {begin} != {covered}"
        assert end - begin == rows * cols * 8, f"byte size mismatch on {name}"
        flat = struct.unpack_from(f"<{rows * cols}d", payload, begin)
        for v in flat:
            assert math.isfinite(v) and abs(v) <= EXACT_MAX, f"non-exact weight in {name}: {v}"
        # Materialize explicit 2D row-major matrix [rows][cols]
        mat = [list(flat[r * cols:(r + 1) * cols]) for r in range(rows)]
        tensors[name] = mat
        covered = end
    assert covered == len(payload), f"uncovered payload bytes: {covered} != {len(payload)}"

    # Reconstruct alternating Attention + FFN layer list strictly from __metadata__ + dense tensors.
    layers = []
    for li, spec in enumerate(layer_spec.split("|")):
        if spec.startswith("F:"):
            units = int(spec[2:])
            if units > 0:
                w_up = tensors[f"L{li}.ffn.W_up"]
                w_gate = tensors[f"L{li}.ffn.W_gate"]
                w_down = tensors[f"L{li}.ffn.W_down"]
                assert len(w_up) == units and len(w_up[0]) == width
                assert len(w_gate) == units and len(w_gate[0]) == width
                assert len(w_down) == width and len(w_down[0]) == units
            else:
                w_up = w_gate = w_down = []
            layers.append(("ffn", units, w_up, w_gate, w_down))
        elif spec.startswith("A:"):
            parts = spec[2:].split(";")
            hcount = int(parts[0])
            assert len(parts) == 1 + hcount
            heads = []
            for hi in range(hcount):
                hk, dk, dv, mask = (int(x) for x in parts[1 + hi].split(":"))
                wq = tensors[f"L{li}.attn.H{hi}.W_Q"] if dk > 0 else []
                wk = tensors[f"L{li}.attn.H{hi}.W_K"] if dk > 0 else []
                wv = tensors[f"L{li}.attn.H{hi}.W_V"]
                wo = tensors[f"L{li}.attn.H{hi}.W_O"]
                assert len(wv) == dv and len(wv[0]) == width
                assert len(wo) == width and len(wo[0]) == dv
                if dk > 0:
                    assert len(wq) == dk and len(wq[0]) == width
                    assert len(wk) == dk and len(wk[0]) == width
                heads.append((hk, dk, dv, mask, wq, wk, wv, wo))
            layers.append(("attn", heads))
        else:
            raise AssertionError(f"unknown layer spec {spec}")

    w_out = tensors["readout.W_out"]
    assert len(w_out) == n_outputs and len(w_out[0]) == width
    return {
        "digest": digest,
        "bytes": len(raw),
        "width": width,
        "n_layers": n_layers,
        "n_inputs": n_inputs,
        "n_outputs": n_outputs,
        "kv_slots": kv_slots,
        "tensor_count": len(tensors),
        "layers": layers,
        "w_out": w_out,
    }


def dense_forward_token(model, kv_ctx):
    """Pure dense matrix-vector product + parabolic/uniform attention + ReGLU forward pass."""
    D = model["width"]
    x = [0.0] * D
    x[0] = 1.0
    for layer in model["layers"]:
        if layer[0] == "ffn":
            _, units, w_up, w_gate, w_down = layer
            if units == 0:
                continue
            v_vec = matvec(w_up, x)
            g_vec = matvec(w_gate, x)
            hidden = [v * ((g + abs(g)) * 0.5) for v, g in zip(v_vec, g_vec)]
            for h in hidden:
                assert abs(h) <= EXACT_MAX
            delta = matvec(w_down, hidden)
            x = [xi + di for xi, di in zip(x, delta)]
            for xi in x:
                assert abs(xi) <= EXACT_MAX
        else:
            _, heads = layer
            stage = list(x)
            for hk, dk, dv, mask, wq, wk, wv, wo in heads:
                n_attend = len(kv_ctx) if mask == 0 else len(kv_ctx) + 1
                yv = [0.0] * dv
                if hk == 2:  # uniform_sum
                    for s in range(n_attend):
                        src = kv_ctx[s] if s < len(kv_ctx) else x
                        vs = matvec(wv, src)
                        for r in range(dv):
                            yv[r] += vs[r]
                else:
                    q = matvec(wq, x)
                    if n_attend > 0:
                        best = 0
                        best_sc = None
                        for s in range(n_attend):
                            src = kv_ctx[s] if s < len(kv_ctx) else x
                            ks = matvec(wk, src)
                            sc = dot(q, ks)
                            assert abs(sc) <= EXACT_MAX
                            if best_sc is None:
                                best_sc = sc
                                best = 0
                            elif hk == 0:  # parabolic_hardmax: latest tie wins
                                if sc >= best_sc:
                                    best_sc = sc
                                    best = s
                            else:  # parabolic_oldest
                                if sc > best_sc:
                                    best_sc = sc
                                    best = s
                        best_src = kv_ctx[best] if best < len(kv_ctx) else x
                        yv = matvec(wv, best_src)
                delta = matvec(wo, yv)
                for i in range(D):
                    stage[i] += delta[i]
            x = stage
    readout = matvec(model["w_out"], x)
    return x, readout


def encode_program_kv(program, width):
    """Encode program instructions as prompt tokens H[0..<len(program)]."""
    kv = []
    for i, (code, arg) in enumerate(program):
        tok = [0.0] * width
        tok[S_ONE] = 1.0
        tok[S_PROG_K1] = float(2 * i)
        tok[S_PROG_K2] = float(-i * i)
        tok[S_PROG_KEY] = float(i)
        tok[S_PROG_OP] = float(code)
        tok[S_PROG_ARG] = float(arg)
        tok[S_PROG_VAL] = 1.0
        tok[S_STEP_K2] = -1.0
        kv.append(tok)
    return kv


def run_dense_vm(model, program, budget=400):
    """Autoregressive execution STRICTLY via dense_forward_token on disk-loaded weights."""
    kv = encode_program_kv(program, model["width"])
    trace = []
    outputs = []
    sha = hashlib.sha256()
    for step in range(budget):
        res_vec, ro = dense_forward_token(model, kv)
        r_ip, r_code, r_x, r_y, r_halt, r_fault = (int(v) for v in ro)
        for v in ro:
            assert v == float(int(v)), f"non-integer readout {v}"
        if r_fault != 0:
            return r_fault, step, trace, outputs, None
        row = (r_ip, r_code, r_x, r_y)
        trace.append(row)
        sha.update(struct.pack("<4I", *(t & 0xFFFFFFFF for t in row)))
        if r_code == OP_OUT:
            outputs.append(r_x)
        kv.append(res_vec)
        if r_halt == 1:
            return ST_OK, step + 1, trace, outputs, sha.hexdigest()
    return 10, budget, trace, outputs, None  # budget


def run_reference_ch226(program, budget=400):
    """Independent Python Ch 226 reference interpreter."""
    stack = []
    locals_ = [0] * TM_LOCALS
    cells = [0] * TM_CELLS
    outputs = []
    trace = []
    sha = hashlib.sha256()
    ip = 0
    for step in range(budget):
        if ip == len(program):
            return ST_FELL, step, trace, outputs, None
        code, arg = program[ip]
        need = 2 if code in (OP_ADD, OP_SUB, OP_MUL, OP_STORE) else (1 if code in (OP_SET, OP_LOAD, OP_BRIF, OP_OUT) else 0)
        if len(stack) < need:
            return ST_UNDERFLOW, step, trace, outputs, None
        if code in (OP_PUSH, OP_GET) and len(stack) >= TM_STACK_MAX:
            return ST_OVERFLOW, step, trace, outputs, None
        nxt = ip + 1
        x = y = 0
        if code == OP_PUSH:
            x = arg
            stack.append(x)
            y = len(stack)
        elif code == OP_GET:
            x = locals_[arg]
            stack.append(x)
            y = arg
        elif code == OP_SET:
            x = stack.pop()
            locals_[arg] = x
            y = arg
        elif code in (OP_ADD, OP_SUB, OP_MUL):
            b = stack.pop()
            a = stack.pop()
            v = a + b if code == OP_ADD else (a - b if code == OP_SUB else a * b)
            if abs(v) > VALUE_MAX:
                return ST_ARITHMETIC, step, trace, outputs, None
            stack.append(v)
            x = v
            y = len(stack)
        elif code == OP_LOAD:
            a = stack[-1]
            if a < 0 or a >= TM_CELLS:
                return ST_ADDRESS, step, trace, outputs, None
            x = cells[a]
            stack[-1] = x
            y = a
        elif code == OP_STORE:
            v = stack.pop()
            a = stack.pop()
            if a < 0 or a >= TM_CELLS:
                return ST_ADDRESS, step, trace, outputs, None
            cells[a] = v
            x = v
            y = a
        elif code == OP_BRIF:
            x = stack.pop()
            if x != 0:
                nxt = arg
            y = nxt
        elif code == OP_OUT:
            if len(outputs) >= TM_OUTPUTS:
                return ST_CAPACITY, step, trace, outputs, None
            x = stack.pop()
            outputs.append(x)
            y = len(outputs)
        elif code == OP_RET:
            nxt = -1
        row = (ip, code, x, y)
        trace.append(row)
        sha.update(struct.pack("<4I", *(t & 0xFFFFFFFF for t in row)))
        if nxt < 0:
            return ST_OK, step + 1, trace, outputs, sha.hexdigest()
        ip = nxt
    return 10, budget, trace, outputs, None


def main():
    root = pathlib.Path(__file__).resolve().parents[2]
    with tempfile.TemporaryDirectory(prefix="exec-vm-oracle-") as tmpdir:
        artifact = pathlib.Path(tmpdir) / "exec-vm-ch226.safetensors"
        proc = subprocess.run(
            [str(root / "coax/web/tool.sh"), str(root / "psionic/tools/exec-vm-artifact.coax"), str(artifact)],
            cwd=root,
            capture_output=True,
            text=True,
            check=True,
        )
        coax_bytes = None
        coax_sha = None
        programs = []
        for line in proc.stdout.splitlines():
            if line.startswith("ARTIFACT_BYTES "):
                coax_bytes = int(line.split()[1])
            elif line.startswith("ARTIFACT_SHA256 "):
                coax_sha = line.split()[1].strip()
            elif line.startswith("PROG "):
                parts = line.split()
                name = parts[1]
                kv = dict(p.split("=", 1) for p in parts[2:])
                ops = [tuple(int(z) for z in pair.split(":")) for pair in kv["ops"].split(",")]
                programs.append((name, int(kv["steps"]), int(kv["outputs"]), kv["digest"], ops))

        assert len(programs) == 54, f"expected 54 programs, got {len(programs)}"
        model = load_safetensors_exec_vm(artifact)
        assert model["bytes"] == coax_bytes, f"size mismatch {model['bytes']} != {coax_bytes}"
        assert model["digest"] == coax_sha, f"SHA-256 mismatch {model['digest']} != {coax_sha}"

        total_steps = 0
        for name, coax_steps, coax_outs, coax_digest, ops in programs:
            st_ref, steps_ref, trace_ref, outs_ref, dig_ref = run_reference_ch226(ops)
            st_vm, steps_vm, trace_vm, outs_vm, dig_vm = run_dense_vm(model, ops)
            assert st_ref == ST_OK and st_vm == ST_OK, f"{name}: ref={st_ref} vm={st_vm}"
            assert steps_vm == steps_ref == coax_steps, f"{name}: steps {steps_vm} vs {steps_ref} vs {coax_steps}"
            assert len(outs_vm) == len(outs_ref) == coax_outs and outs_vm == outs_ref, f"{name}: outputs mismatch"
            assert trace_vm == trace_ref, f"{name}: trace step divergence"
            assert dig_vm == dig_ref == coax_digest, f"{name}: digest mismatch {dig_vm} vs {dig_ref} vs {coax_digest}"
            total_steps += steps_vm

        # Verify typed refusals directly through the disk-loaded dense weights in Python!
        refusal_cases = [
            ("add_overflow", [(OP_PUSH, VALUE_MAX), (OP_PUSH, 1), (OP_ADD, 0), (OP_RET, 0)], ST_ARITHMETIC),
            ("mul_overflow", [(OP_PUSH, 8193), (OP_PUSH, 8192), (OP_MUL, 0), (OP_RET, 0)], ST_ARITHMETIC),
            ("stack_underflow", [(OP_PUSH, 1), (OP_ADD, 0), (OP_RET, 0)], ST_UNDERFLOW),
            ("stack_overflow", [(OP_PUSH, i) for i in range(33)] + [(OP_RET, 0)], ST_OVERFLOW),
            ("oob_load_hi", [(OP_PUSH, 64), (OP_LOAD, 0), (OP_RET, 0)], ST_ADDRESS),
            ("oob_load_lo", [(OP_PUSH, -1), (OP_LOAD, 0), (OP_RET, 0)], ST_ADDRESS),
            ("oob_store_hi", [(OP_PUSH, 64), (OP_PUSH, 5), (OP_STORE, 0), (OP_RET, 0)], ST_ADDRESS),
            ("fell_off_end", [(OP_PUSH, 1)], ST_FELL),
        ]
        for rname, rops, expect_st in refusal_cases:
            st_ref, _, _, _, _ = run_reference_ch226(rops)
            st_vm, _, _, _, _ = run_dense_vm(model, rops)
            assert st_ref == expect_st and st_vm == expect_st, f"refusal {rname}: ref={st_ref} vm={st_vm} want={expect_st}"

        print(
            f"exec-vm oracle PASS: loaded {model['bytes']} bytes ({model['tensor_count']} dense F64 tensors, "
            f"width={model['width']}, layers={model['n_layers']}, sha256={model['digest'][:16]}...); "
            f"54/54 programs ({total_steps} total autoregressive dense-matmul steps) bit-identical across "
            f"Python dense-matmul, Coax disk-loaded weights, and Python Ch 226 reference; "
            f"{len(refusal_cases)}/{len(refusal_cases)} typed refusals exact"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
