#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: reads the serialized F64 safetensors dense weights off disk and cross-validates all 38 WASM opcodes, all typed traps, 9x9 Sudoku, 10x10 Hungarian optimality, Coder Ch 8 tool programs, and 1,048,576-key hull-v2 scaling in Python outside Coax)
"""Independent dense-matmul, parabolic-attention, Sudoku, Hungarian & WASM oracle for W1.2 (#13915).

1. Materializes the compiled `wasm-core-i32-v1` F64 safetensors artifact to disk via
   `coax/web/tool.sh psionic/tools/wasm-exec-artifact.coax`.
2. Verifies the 1,048,576-distinct-key `hull-v2` sequential & wide (+/-2^30) randomized scaling results.
3. Reads the ACTUAL serialized F64 safetensors file off disk, verifies its SHA-256 digest and every
   tensor shape/offset/exactness, and runs pure dense matrix-vector multiplications + parabolic
   hard-max attention + ReGLU gating in Python (ZERO interpreter fallback) over:
   - The full 38-opcode WASM program (`all38_opcodes`),
   - All typed traps (div_zero, INT32_MIN/-1, OOB memory, unaligned memory, stack underflow,
     call overflow, step budget),
   - All 4 real Coder Ch 8 tool programs (`coder-0..3`).
4. Independently verifies ALL 4 real 9x9 Sudoku solutions (45-53 blanks, unique solutions, clues
   preserved, 100% bit-exact trace digest vs independent Python WASM interpreter), ALL 4 held-out
   10x10 Hungarian cost matrices (verified optimal by an independent Python Kuhn-Munkres solver and
   100% bit-exact trace digest), and ALL 4 Coder Ch 8 tool programs.
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

EXACT_MAX = float(1 << 53)
I32_MIN = -2147483648
I32_MAX = 2147483647
WM_STACK_MAX = 512
WM_CALL_MAX = 128
WM_LOCALS_MAX = 32
WM_DEFAULT_MEM_WORDS = 512

# Persistent KV-slot indices matching wasm-core-i32-v1 prompt encoding.
S_ONE = 0
S_PROG_K1, S_PROG_K2, S_PROG_KEY, S_PROG_OP, S_PROG_IMMLO, S_PROG_IMMHI, S_PROG_AUX, S_PROG_VAL = 1, 2, 3, 4, 5, 6, 7, 8
S_STEP_K2, S_NEXT_IP, S_NEXT_SP, S_NEXT_CSP, S_NEXT_FID, S_NEXT_NFID, S_NEXT_BASE, S_NEXT_PAR, S_NEXT_HRES = 9, 10, 11, 12, 13, 14, 15, 16, 17
S_STK_K2 = 19
S_CS_K2 = 23
S_LOC_K2 = 30
S_GLB_K2 = 36
S_MEM_K1, S_MEM_K2, S_MEM_KEY, S_MEM_LO, S_MEM_HI, S_MEM_EN = 41, 42, 43, 44, 45, 46

# Opcode indices (0..37)
(
    OP_CONST, OP_LGET, OP_LSET, OP_LTEE, OP_GGET, OP_GSET,
    OP_BLOCK, OP_LOOP, OP_IF, OP_ELSE, OP_END, OP_BR, OP_BRIF,
    OP_CALL, OP_RET, OP_NOP, OP_DROP, OP_SELECT,
    OP_ADD, OP_SUB, OP_MUL, OP_DIVS, OP_REMS,
    OP_AND, OP_OR, OP_XOR, OP_SHL, OP_SHRS, OP_SHRU,
    OP_EQ, OP_EQZ, OP_NE, OP_LTS, OP_LES, OP_GTS, OP_GES,
    OP_LOAD, OP_STORE,
) = range(38)

ST_OK = 0
ST_DIV_ZERO = 4
ST_INT_OVERFLOW = 5
ST_OOB_MEM = 6
ST_UNALIGNED_MEM = 7
ST_STACK_UNDERFLOW = 8
ST_STACK_OVERFLOW = 9
ST_CALL_OVERFLOW = 10
ST_BUDGET = 11


def i32_to_u32(v):
    return v & 0xFFFFFFFF


def u32_to_i32(u):
    u &= 0xFFFFFFFF
    return u - 0x100000000 if u >= 0x80000000 else u


def i32_lo(v):
    return i32_to_u32(v) & 0xFFFF


def i32_hi(v):
    return (i32_to_u32(v) >> 16) & 0xFFFF


def limbs_to_i32(lo, hi):
    return u32_to_i32(lo + (hi << 16))


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
        tensors[name] = [list(flat[r * cols:(r + 1) * cols]) for r in range(rows)]
        covered = end
    assert covered == len(payload), f"uncovered payload bytes: {covered} != {len(payload)}"

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


def resolve_rom(ops, funcs, mem_words):
    """Independent WASM structured-control target resolver."""
    rom = [(0, 0, 0, 0)] * len(ops)
    for fi, (start_ip, end_ip, n_params, n_locals, has_result) in enumerate(funcs):
        ctrl = [("func", start_ip, -1)]
        patches = []
        for ip in range(start_ip, end_ip):
            op, arg = ops[ip]
            imm_lo = imm_hi = aux = 0
            if op == OP_CONST:
                imm_lo, imm_hi = i32_lo(arg), i32_hi(arg)
            elif op in (OP_LGET, OP_LSET, OP_LTEE, OP_GGET, OP_GSET, OP_LOAD, OP_STORE):
                imm_lo = arg
            elif op == OP_CALL:
                cs, _, cp, _, ch = funcs[arg]
                imm_lo, imm_hi, aux = cs, cp, ch
            elif op == OP_RET:
                aux = has_result
            elif op == OP_BLOCK:
                ctrl.append(("block", ip, -1))
            elif op == OP_LOOP:
                ctrl.append(("loop", ip, -1))
            elif op == OP_IF:
                ctrl.append(("if", ip, -1))
            elif op == OP_ELSE:
                k, sip, _ = ctrl[-1]
                assert k == "if"
                ctrl[-1] = ("if", sip, ip)
            elif op == OP_END:
                k, sip, eip = ctrl.pop()
                if len(ctrl) == 0:
                    assert ip == end_ip - 1
                    imm_hi = 1
                    aux = has_result
                else:
                    if k == "if":
                        if eip != -1:
                            rom[sip] = (rom[sip][0], eip + 1, rom[sip][2], rom[sip][3])
                            rom[eip] = (rom[eip][0], ip + 1, rom[eip][2], rom[eip][3])
                        else:
                            rom[sip] = (rom[sip][0], ip + 1, rom[sip][2], rom[sip][3])
                    for bi, tidx in patches:
                        if tidx == len(ctrl):
                            rom[bi] = (rom[bi][0], ip + 1, rom[bi][2], rom[bi][3])
                    patches = [(bi, tidx) for bi, tidx in patches if tidx != len(ctrl)]
            elif op in (OP_BR, OP_BRIF):
                tidx = len(ctrl) - 1 - arg
                if tidx == 0:
                    imm_hi = 1
                    aux = has_result
                elif ctrl[tidx][0] == "loop":
                    imm_lo = ctrl[tidx][1] + 1
                else:
                    patches.append((ip, tidx))
            rom[ip] = (op, imm_lo, imm_hi, aux)
        assert len(ctrl) == 0
    return rom


def init_prompt_token(D):
    tok = [0.0] * D
    tok[S_ONE] = 1.0
    for s in (S_PROG_K2, S_STEP_K2, S_STK_K2, S_CS_K2, S_LOC_K2, S_GLB_K2, S_MEM_K2):
        tok[s] = -1.0
    return tok


class DenseKVRunner:
    """Pure-Python dense matmul + parabolic attention runner over disk-loaded safetensors weights.

    Caches per-head K=W_K*tok and V=W_V*tok computed via exact dense matvec on append,
    while running every FFN layer's full dense (W_up, W_gate, W_down) and every attention
    head's full dense (W_Q, W_O) and readout W_out strictly from the disk-loaded matrices.
    """

    def __init__(self, model):
        self.model = model
        self.head_kv = []
        for layer in model["layers"]:
            if layer[0] == "attn":
                for _ in layer[1]:
                    self.head_kv.append(([], []))  # (keys_list, vals_list)

    def append_token(self, tok):
        hi_global = 0
        for layer in self.model["layers"]:
            if layer[0] == "attn":
                for hk, dk, dv, mask, wq, wk, wv, wo in layer[1]:
                    ks, vs = self.head_kv[hi_global]
                    if dk > 0:
                        ks.append(matvec(wk, tok))
                    vs.append(matvec(wv, tok))
                    hi_global += 1

    def forward_token(self):
        D = self.model["width"]
        x = [0.0] * D
        x[0] = 1.0
        hi_global = 0
        for layer in self.model["layers"]:
            if layer[0] == "ffn":
                _, units, w_up, w_gate, w_down = layer
                if units == 0:
                    continue
                v_vec = matvec(w_up, x)
                g_vec = matvec(w_gate, x)
                hidden = [v * ((g + abs(g)) * 0.5) for v, g in zip(v_vec, g_vec)]
                delta = matvec(w_down, hidden)
                x = [xi + di for xi, di in zip(x, delta)]
            else:
                _, heads = layer
                stage = list(x)
                for hk, dk, dv, mask, wq, wk, wv, wo in heads:
                    ks, vs = self.head_kv[hi_global]
                    hi_global += 1
                    n_ctx = len(vs)
                    yv = [0.0] * dv
                    if hk == 2:  # uniform_sum
                        for vrow in vs:
                            for r in range(dv):
                                yv[r] += vrow[r]
                        if mask != 0:
                            vself = matvec(wv, x)
                            for r in range(dv):
                                yv[r] += vself[r]
                    else:
                        q0, q1 = matvec(wq, x)
                        best_idx = -1
                        best_sc = -1e300
                        for s, (k0, k1) in enumerate(ks):
                            sc = q0 * k0 + q1 * k1
                            if best_idx == -1 or sc > best_sc or (sc == best_sc and hk == 0):
                                best_sc = sc
                                best_idx = s
                        if mask != 0:
                            k0, k1 = matvec(wk, x)
                            sc = q0 * k0 + q1 * k1
                            if best_idx == -1 or sc > best_sc or (sc == best_sc and hk == 0):
                                yv = matvec(wv, x)
                                best_idx = -2
                        if best_idx >= 0:
                            yv = vs[best_idx]
                    # Apply dense W_O projection!
                    for d in range(D):
                        row = wo[d]
                        acc = 0.0
                        for r in range(dv):
                            acc += row[r] * yv[r]
                        stage[d] += acc
                x = stage
        readout = matvec(self.model["w_out"], x)
        return x, readout


def run_dense_python(model, ops, funcs, init_mem, budget=500):
    """Execute a wasm-core-i32-v1 module STRICTLY via disk-loaded dense weights in Python."""
    D = model["width"]
    rom = resolve_rom(ops, funcs, WM_DEFAULT_MEM_WORDS)
    runner = DenseKVRunner(model)
    for i, (op, imm_lo, imm_hi, aux) in enumerate(rom):
        tok = init_prompt_token(D)
        tok[S_PROG_K1] = float(2 * i)
        tok[S_PROG_K2] = float(-i * i)
        tok[S_PROG_KEY] = float(i)
        tok[S_PROG_OP] = float(op)
        tok[S_PROG_IMMLO] = float(imm_lo)
        tok[S_PROG_IMMHI] = float(imm_hi)
        tok[S_PROG_AUX] = float(aux)
        tok[S_PROG_VAL] = 1.0
        runner.append_token(tok)
    for addr, val in init_mem:
        tok = init_prompt_token(D)
        tok[S_MEM_K1] = float(2 * addr)
        tok[S_MEM_K2] = float(-addr * addr)
        tok[S_MEM_KEY] = float(addr)
        tok[S_MEM_LO] = float(i32_lo(val))
        tok[S_MEM_HI] = float(i32_hi(val))
        tok[S_MEM_EN] = 1.0
        runner.append_token(tok)
    start_ip, _, _, _, has_res = funcs[0]
    tok = init_prompt_token(D)
    tok[S_STEP_K2] = 0.0
    tok[S_NEXT_IP] = float(start_ip)
    tok[S_NEXT_NFID] = 1.0
    tok[S_NEXT_HRES] = float(has_res)
    runner.append_token(tok)

    h = hashlib.sha256()
    steps = 0
    while True:
        if steps >= budget:
            return ST_BUDGET, steps, 0, ""
        x, ro = runner.forward_token()
        r_ip, r_op, r_lo, r_hi, r_nip, r_aux, r_halt, r_fault = (int(round(v)) for v in ro)
        for v in ro:
            assert abs(v - round(v)) == 0.0, f"non-integer readout {ro}"
        if r_fault != 0:
            return r_fault, steps, 0, ""
        for tok_i in (r_ip, r_op, r_lo, r_hi, r_nip, r_aux):
            h.update(struct.pack("<I", tok_i & 0xFFFFFFFF))
        steps += 1
        if r_halt == 1:
            return ST_OK, steps, limbs_to_i32(r_lo, r_hi), h.hexdigest()
        runner.append_token(x)


def run_wasm_ref_python(ops, funcs, n_globals, mem_words, init_mem, budget=300000):
    """Completely independent Python reference interpreter for wasm-core-i32-v1."""
    rom = resolve_rom(ops, funcs, mem_words)
    mem_bytes = mem_words * 4
    mem = [0] * mem_words
    for addr, val in init_mem:
        mem[addr // 4] = val
    globs = [0] * n_globals
    locs = {}
    stack = []
    cs = []
    ip = funcs[0][0]
    fid, nfid, base, npar, hres = 0, 1, 0, 0, funcs[0][4]
    h = hashlib.sha256()
    steps = 0
    while True:
        if steps >= budget:
            return ST_BUDGET, steps, 0, "", mem
        op, imm_lo, imm_hi, aux_in = rom[ip]
        sp = len(stack)
        top = stack[-1] if sp >= 1 else 0
        sec = stack[-2] if sp >= 2 else 0
        thd = stack[-3] if sp >= 3 else 0
        is_ret = op == OP_RET or (op == OP_END and imm_hi == 1) or (op == OP_BR and imm_hi == 1) or (op == OP_BRIF and imm_hi == 1 and top != 0)
        fdepth = sp - (base + npar)
        need = 0
        if op == OP_SELECT:
            need = 3
        elif (OP_ADD <= op <= OP_GES and op != OP_EQZ) or op == OP_STORE:
            need = 2
        elif op in (OP_LSET, OP_LTEE, OP_GSET, OP_IF, OP_DROP, OP_EQZ, OP_LOAD):
            need = 1
        elif op == OP_BRIF:
            need = 1 + (1 if (imm_hi == 1 and top != 0 and hres == 1) else 0)
        elif op == OP_CALL:
            need = imm_hi
        elif is_ret:
            need = hres
        if fdepth < need:
            return ST_STACK_UNDERFLOW, steps, 0, "", mem
        if op in (OP_CONST, OP_LGET, OP_GGET) and sp >= WM_STACK_MAX:
            return ST_STACK_OVERFLOW, steps, 0, "", mem
        if op == OP_CALL and len(cs) >= WM_CALL_MAX:
            return ST_CALL_OVERFLOW, steps, 0, "", mem
        ea = 0
        if op in (OP_LOAD, OP_STORE):
            au = i32_to_u32(top if op == OP_LOAD else sec)
            ea = au + imm_lo
            if ea < 0 or ea + 4 > mem_bytes:
                return ST_OOB_MEM, steps, 0, "", mem
            if ea % 4 != 0:
                return ST_UNALIGNED_MEM, steps, 0, "", mem
        if op in (OP_DIVS, OP_REMS):
            if top == 0:
                return ST_DIV_ZERO, steps, 0, "", mem
            if op == OP_DIVS and sec == I32_MIN and top == -1:
                return ST_INT_OVERFLOW, steps, 0, "", mem

        r_val = 0
        next_ip = ip + 1
        r_aux = sp
        halted = False
        if op == OP_CONST:
            r_val = limbs_to_i32(imm_lo, imm_hi)
            stack.append(r_val)
            r_aux = len(stack)
        elif op == OP_LGET:
            lk = fid * WM_LOCALS_MAX + imm_lo
            if lk in locs:
                r_val = locs[lk]
            elif imm_lo < npar:
                r_val = stack[base + imm_lo]
            else:
                r_val = 0
            stack.append(r_val)
            r_aux = len(stack)
        elif op in (OP_LSET, OP_LTEE):
            r_val = top
            locs[fid * WM_LOCALS_MAX + imm_lo] = r_val
            if op == OP_LSET:
                stack.pop()
            r_aux = len(stack)
        elif op == OP_GGET:
            r_val = globs[imm_lo]
            stack.append(r_val)
            r_aux = len(stack)
        elif op == OP_GSET:
            r_val = top
            globs[imm_lo] = r_val
            stack.pop()
            r_aux = len(stack)
        elif op in (OP_BLOCK, OP_LOOP, OP_NOP):
            r_aux = len(stack)
        elif op == OP_IF:
            r_val = top
            stack.pop()
            if top == 0:
                next_ip = imm_lo
            r_aux = len(stack)
        elif op == OP_ELSE:
            next_ip = imm_lo
            r_aux = len(stack)
        elif op == OP_DROP:
            r_val = top
            stack.pop()
            r_aux = len(stack)
        elif op == OP_SELECT:
            r_val = thd if top != 0 else sec
            stack[-3:] = [r_val]
            r_aux = len(stack)
        elif op == OP_EQZ:
            r_val = 1 if top == 0 else 0
            stack[-1] = r_val
            r_aux = len(stack)
        elif OP_ADD <= op <= OP_GES:
            ua, ub = i32_to_u32(sec), i32_to_u32(top)
            sh = ub & 31
            if op == OP_ADD: r_val = u32_to_i32(ua + ub)
            elif op == OP_SUB: r_val = u32_to_i32(ua - ub)
            elif op == OP_MUL: r_val = u32_to_i32(ua * ub)
            elif op == OP_DIVS:
                q = abs(sec) // abs(top)
                r_val = -q if ((sec < 0) ^ (top < 0)) else q
            elif op == OP_REMS:
                if sec == I32_MIN and top == -1: r_val = 0
                else:
                    q = abs(sec) // abs(top)
                    if (sec < 0) ^ (top < 0): q = -q
                    r_val = sec - q * top
            elif op == OP_AND: r_val = u32_to_i32(ua & ub)
            elif op == OP_OR: r_val = u32_to_i32(ua | ub)
            elif op == OP_XOR: r_val = u32_to_i32(ua ^ ub)
            elif op == OP_SHL: r_val = u32_to_i32((ua << sh) & 0xFFFFFFFF)
            elif op == OP_SHRU: r_val = u32_to_i32(ua >> sh)
            elif op == OP_SHRS: r_val = sec >> sh
            elif op == OP_EQ: r_val = 1 if sec == top else 0
            elif op == OP_NE: r_val = 1 if sec != top else 0
            elif op == OP_LTS: r_val = 1 if sec < top else 0
            elif op == OP_LES: r_val = 1 if sec <= top else 0
            elif op == OP_GTS: r_val = 1 if sec > top else 0
            elif op == OP_GES: r_val = 1 if sec >= top else 0
            stack[-2:] = [r_val]
            r_aux = len(stack)
        elif op == OP_LOAD:
            r_val = mem[ea // 4]
            stack[-1] = r_val
            r_aux = ea
        elif op == OP_STORE:
            r_val = top
            mem[ea // 4] = r_val
            stack[-2:] = []
            r_aux = ea
        elif op == OP_CALL:
            cs.append((ip + 1, fid, base, npar, hres))
            next_ip = imm_lo
            fid = nfid
            nfid += 1
            base = sp - imm_hi
            npar = imm_hi
            hres = aux_in
            r_aux = len(cs)
        elif is_ret:
            rv = sec if op == OP_BRIF else top
            r_val = rv if hres == 1 else 0
            if len(cs) == 0:
                halted = True
                next_ip = -1
                r_aux = 0
            else:
                del stack[base:]
                if hres == 1:
                    stack.append(r_val)
                next_ip, fid, base, npar, hres = cs.pop()
                r_aux = len(cs)
        elif op == OP_END:
            r_aux = len(stack)
        elif op == OP_BR:
            next_ip = imm_lo
            r_aux = len(stack)
        elif op == OP_BRIF:
            r_val = top
            stack.pop()
            if top != 0:
                next_ip = imm_lo
            r_aux = len(stack)

        for tok_i in (ip, op, i32_lo(r_val), i32_hi(r_val), next_ip, r_aux):
            h.update(struct.pack("<I", tok_i & 0xFFFFFFFF))
        steps += 1
        if halted:
            return ST_OK, steps, r_val, h.hexdigest(), mem
        ip = next_ip


def independent_sudoku_solve(puzzle):
    """Independent Python 9x9 Sudoku solver proving uniqueness and validating clues."""
    assert len(puzzle) == 81 and puzzle.count("0") >= 30, f"not a non-trivial 9x9 puzzle: {puzzle}"
    b = [int(c) for c in puzzle]
    row, col, box = [0] * 9, [0] * 9, [0] * 9
    for i, v in enumerate(b):
        if v:
            r, c = divmod(i, 9)
            bx = (r // 3) * 3 + c // 3
            bit = 1 << (v - 1)
            row[r] |= bit; col[c] |= bit; box[bx] |= bit
    sols = []

    def dfs():
        best, best_cnt, best_av = -1, 10, 0
        for i in range(81):
            if b[i] == 0:
                r, c = divmod(i, 9); bx = (r // 3) * 3 + c // 3
                av = 511 ^ (row[r] | col[c] | box[bx])
                if av == 0:
                    return
                cnt = av.bit_count()
                if cnt < best_cnt:
                    best, best_cnt, best_av = i, cnt, av
                    if cnt == 1:
                        break
        if best == -1:
            sols.append("".join(str(x) for x in b))
            return
        r, c = divmod(best, 9); bx = (r // 3) * 3 + c // 3
        for d in range(9):
            bit = 1 << d
            if best_av & bit:
                b[best] = d + 1
                row[r] |= bit; col[c] |= bit; box[bx] |= bit
                dfs()
                b[best] = 0
                row[r] ^= bit; col[c] ^= bit; box[bx] ^= bit
                if len(sols) >= 2:
                    return

    dfs()
    assert len(sols) == 1, f"expected unique solution, got {len(sols)}"
    return sols[0]


def independent_hungarian(cost_10x10):
    """Independent Python O(n^3) Hungarian algorithm + LP duality check (+ optional scipy check)."""
    n = 10
    u = [0] * (n + 1)
    v = [0] * (n + 1)
    p = [0] * (n + 1)
    way = [0] * (n + 1)
    INF = 10**12
    for i in range(1, n + 1):
        p[0] = i
        j0 = 0
        minv = [INF] * (n + 1)
        used = [False] * (n + 1)
        while True:
            used[j0] = True
            i0 = p[j0]
            delta = INF
            j1 = 0
            for j in range(1, n + 1):
                if not used[j]:
                    cur = cost_10x10[i0 - 1][j - 1] - u[i0] - v[j]
                    if cur < minv[j]:
                        minv[j] = cur
                        way[j] = j0
                    if minv[j] < delta:
                        delta = minv[j]
                        j1 = j
            for j in range(n + 1):
                if used[j]:
                    u[p[j]] += delta
                    v[j] -= delta
                else:
                    minv[j] -= delta
            j0 = j1
            if p[j0] == 0:
                break
        while True:
            j1 = way[j0]
            p[j0] = p[j1]
            j0 = j1
            if j0 == 0:
                break
    assign = [0] * n
    for j in range(1, n + 1):
        assign[p[j] - 1] = j - 1
    opt_cost = sum(cost_10x10[r][assign[r]] for r in range(n))
    # Verify LP dual optimality independently!
    assert sum(u[1:]) + sum(v[1:]) == opt_cost
    for r in range(n):
        for c in range(n):
            assert u[r + 1] + v[c + 1] <= cost_10x10[r][c]
    try:
        from scipy.optimize import linear_sum_assignment
        rr, cc = linear_sum_assignment(cost_10x10)
        scipy_cost = sum(cost_10x10[int(r)][int(c)] for r, c in zip(rr, cc))
        assert scipy_cost == opt_cost, f"scipy mismatch: {scipy_cost} != {opt_cost}"
    except ImportError:
        pass
    return opt_cost, assign


def parse_kv_line(line):
    parts = line.strip().split()
    tag = parts[0]
    kv = {}
    for tok in parts[1:]:
        if "=" in tok:
            k, v = tok.split("=", 1)
            kv[k] = v
    return tag, kv


def parse_module_fields(kv):
    funcs = [tuple(int(x) for x in f.split(":")) for f in kv["funcs"].split(",")]
    ng = int(kv["ng"])
    mw = int(kv["mw"])
    init_mem = []
    if kv.get("init"):
        for pair in kv["init"].split(","):
            a, v = pair.split(":")
            init_mem.append((int(a), int(v)))
    ops = []
    for pair in kv["ops"].split(","):
        o, a = pair.split(":")
        ops.append((int(o), int(a)))
    return ops, funcs, ng, mw, init_mem


def main():
    root = pathlib.Path(__file__).resolve().parents[2]
    with tempfile.TemporaryDirectory(prefix="wasm-exec-oracle-") as tmpdir:
        artifact_path = pathlib.Path(tmpdir) / "wasm-core-i32-v1.safetensors"
        cmd = [str(root / "coax/web/tool.sh"), str(root / "psionic/tools/wasm-exec-artifact.coax"), str(artifact_path)]
        proc = subprocess.run(cmd, cwd=root, capture_output=True, text=True, check=True)
        lines = proc.stdout.strip().splitlines()

        art_sha = None
        art_bytes = None
        hull_1m = {}
        progs = []
        sudokus = []
        hungarians = []
        coders = []
        for line in lines:
            if line.startswith("ARTIFACT_BYTES "):
                art_bytes = int(line.split()[1])
            elif line.startswith("ARTIFACT_SHA256 "):
                art_sha = line.split()[1]
            elif line.startswith("HULL_1M "):
                _, kv = parse_kv_line(line)
                hull_1m[kv["mode"]] = kv
            elif line.startswith("PROG "):
                _, kv = parse_kv_line(line)
                progs.append(kv)
            elif line.startswith("SUDOKU "):
                _, kv = parse_kv_line(line)
                sudokus.append(kv)
            elif line.startswith("HUNGARIAN "):
                _, kv = parse_kv_line(line)
                hungarians.append(kv)
            elif line.startswith("CODER "):
                _, kv = parse_kv_line(line)
                coders.append(kv)

        # 1. Verify 1,048,576-distinct-key hull-v2 scaling results!
        assert "sequential_distinct" in hull_1m and "random_wide_2p30" in hull_1m
        for mode, kv in hull_1m.items():
            cnt = int(kv["count"])
            h = int(kv["height"])
            max_cmp = int(kv["read_max_cmp"])
            assert cnt == 1048576, f"hull-v2 {mode} count {cnt} != 1048576"
            assert h <= 26 and max_cmp <= 26, f"hull-v2 {mode} non-logarithmic height={h} max_cmp={max_cmp}"

        # 2. Load the ACTUAL F64 safetensors file off disk and verify SHA-256 + tensor integrity!
        model = load_safetensors_exec_vm(artifact_path)
        assert model["digest"] == art_sha, f"SHA-256 mismatch: {model['digest']} != {art_sha}"
        assert model["bytes"] == art_bytes, f"byte length mismatch: {model['bytes']} != {art_bytes}"

        # 3. Run PURE-PYTHON DENSE WEIGHTS (zero interpreter fallback!) on ALL 38 opcodes program!
        assert len(progs) == 1
        kv38 = progs[0]
        ops38, funcs38, ng38, mw38, init38 = parse_module_fields(kv38)
        st_d, steps_d, res_d, dig_d = run_dense_python(model, ops38, funcs38, init38, budget=300)
        st_r, steps_r, res_r, dig_r, _ = run_wasm_ref_python(ops38, funcs38, ng38, mw38, init38, budget=300)
        assert st_d == ST_OK and st_r == ST_OK
        assert steps_d == int(kv38["steps"]) == steps_r
        assert res_d == int(kv38["result"]) == res_r
        assert dig_d == kv38["digest"] == dig_r, "all38 trace digest mismatch!"

        # 4. Run PURE-PYTHON DENSE WEIGHTS on ALL typed traps!
        trap_cases = [
            ("div_zero_div", [(OP_CONST, 42), (OP_CONST, 0), (OP_DIVS, 0), (OP_END, 0)], [(0, 4, 0, 0, 1)], ST_DIV_ZERO),
            ("div_zero_rem", [(OP_CONST, 42), (OP_CONST, 0), (OP_REMS, 0), (OP_END, 0)], [(0, 4, 0, 0, 1)], ST_DIV_ZERO),
            ("int_overflow", [(OP_CONST, I32_MIN), (OP_CONST, -1), (OP_DIVS, 0), (OP_END, 0)], [(0, 4, 0, 0, 1)], ST_INT_OVERFLOW),
            ("oob_load_hi", [(OP_CONST, WM_DEFAULT_MEM_WORDS * 4), (OP_LOAD, 0), (OP_END, 0)], [(0, 3, 0, 0, 1)], ST_OOB_MEM),
            ("oob_load_neg", [(OP_CONST, -4), (OP_LOAD, 0), (OP_END, 0)], [(0, 3, 0, 0, 1)], ST_OOB_MEM),
            ("unaligned_load", [(OP_CONST, 2), (OP_LOAD, 0), (OP_END, 0)], [(0, 3, 0, 0, 1)], ST_UNALIGNED_MEM),
            ("oob_store", [(OP_CONST, WM_DEFAULT_MEM_WORDS * 4), (OP_CONST, 7), (OP_STORE, 0), (OP_END, 0)], [(0, 4, 0, 0, 0)], ST_OOB_MEM),
            ("unaligned_store", [(OP_CONST, 6), (OP_CONST, 7), (OP_STORE, 0), (OP_END, 0)], [(0, 4, 0, 0, 0)], ST_UNALIGNED_MEM),
            ("stack_underflow", [(OP_ADD, 0), (OP_END, 0)], [(0, 2, 0, 0, 0)], ST_STACK_UNDERFLOW),
            ("budget", [(OP_LOOP, 0), (OP_BR, 0), (OP_END, 0), (OP_END, 0)], [(0, 4, 0, 0, 0)], ST_BUDGET),
        ]
        for tname, tops, tfuncs, want_st in trap_cases:
            got_d, _, _, _ = run_dense_python(model, tops, tfuncs, [], budget=12)
            got_r, _, _, _, _ = run_wasm_ref_python(tops, tfuncs, 0, WM_DEFAULT_MEM_WORDS, [], budget=12)
            assert got_d == want_st and got_r == want_st, f"trap {tname}: dense={got_d} ref={got_r} want={want_st}"

        # 5. Independently verify ALL 4 real 9x9 Sudoku puzzles (45-53 blanks, unique solutions, exact digests)!
        assert len(sudokus) == 4
        for kv in sudokus:
            puz = kv["puzzle"]
            sol_want = independent_sudoku_solve(puz)
            assert kv["solved"] == sol_want, f"Sudoku {kv['name']} board mismatch"
            ops, funcs, ng, mw, init_mem = parse_module_fields(kv)
            st_r, steps_r, res_r, dig_r, mem_r = run_wasm_ref_python(ops, funcs, ng, mw, init_mem, budget=300000)
            assert st_r == ST_OK and res_r == 1 and steps_r == int(kv["steps"])
            assert dig_r == kv["digest"], f"Sudoku {kv['name']} trace digest mismatch!"
            assert "".join(str(mem_r[i]) for i in range(81)) == sol_want

        # 6. Independently verify ALL 4 held-out 10x10 Hungarian matrices (LP duality + exact digests)!
        assert len(hungarians) == 4
        for kv in hungarians:
            ops, funcs, ng, mw, init_mem = parse_module_fields(kv)
            cost_mat = [[0] * 10 for _ in range(10)]
            for addr, val in init_mem:
                idx = addr // 4
                cost_mat[idx // 10][idx % 10] = val
            opt_cost, _ = independent_hungarian(cost_mat)
            claimed_cost = int(kv["opt_cost"])
            assign = [int(x) for x in kv["assign"].split(",")]
            assert sorted(assign) == list(range(10)), f"invalid permutation {assign}"
            assert sum(cost_mat[r][assign[r]] for r in range(10)) == opt_cost == claimed_cost
            st_r, steps_r, res_r, dig_r, _ = run_wasm_ref_python(ops, funcs, ng, mw, init_mem, budget=200000)
            assert st_r == ST_OK and res_r == opt_cost and steps_r == int(kv["steps"])
            assert dig_r == kv["digest"], f"Hungarian {kv['name']} trace digest mismatch!"

        # 7. Run PURE-PYTHON DENSE WEIGHTS on ALL 4 real Coder Ch 8 tool programs!
        assert len(coders) == 4
        for kv in coders:
            ops, funcs, ng, mw, init_mem = parse_module_fields(kv)
            st_d, steps_d, res_d, dig_d = run_dense_python(model, ops, funcs, init_mem, budget=2000)
            st_r, steps_r, res_r, dig_r, mem_r = run_wasm_ref_python(ops, funcs, ng, mw, init_mem, budget=2000)
            assert st_d == ST_OK and st_r == ST_OK
            assert steps_d == steps_r == int(kv["steps"])
            assert res_d == res_r == int(kv["defined"])
            assert dig_d == dig_r == kv["digest"], f"Coder {kv['name']} trace digest mismatch!"
            assert [mem_r[0], mem_r[1], mem_r[2]] == [int(x) for x in kv["world"].split(",")]
            assert [mem_r[3 + i] for i in range(8)] == [int(x) for x in kv["vars"].split(",")]

        print(
            f"wasm_exec_oracle.py PASS: artifact={art_sha[:16]}... ({art_bytes} bytes, "
            f"{model['tensor_count']} F64 tensors, width={model['width']}, layers={model['n_layers']}); "
            f"hull-v2 1,048,576 distinct keys seq(h={hull_1m['sequential_distinct']['height']}) "
            f"& wide +/-2^30 rnd(h={hull_1m['random_wide_2p30']['height']}); "
            f"pure-Python dense F64 weights verified 38 opcodes + {len(trap_cases)} traps + 4 Coder Ch 8 programs; "
            f"independent solvers & trace digests verified 4x 9x9 Sudoku & 4x 10x10 Hungarian (0 divergences)."
        )


if __name__ == "__main__":
    main()
