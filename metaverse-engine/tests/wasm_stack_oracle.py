#!/usr/bin/env python3
# Firm reason: 2 (independent Wasm binary, affine stack-pointer dataflow and conservative callgraph mathematics; no compiler, build or browser orchestration).
"""Bound the linear-memory ABI stack of a scalar Wasm module, or refuse.

Inputs are the actual Wasm and a direct `llvm-objdump -d` of those bytes.
Every decoded instruction's byte span must exactly cover its original body.
This deliberately supports a small scalar profile and fails closed outside it.
It is not a verifier for arbitrary memory accesses or the engine's native stack.
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import re
import sys


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


class Reader:
    def __init__(self, data, offset=0):
        self.data, self.i = data, offset

    def byte(self):
        require(self.i < len(self.data), "truncated binary")
        value = self.data[self.i]
        self.i += 1
        return value

    def raw(self, count):
        require(0 <= count <= len(self.data) - self.i, "truncated binary extent")
        value = self.data[self.i:self.i + count]
        self.i += count
        return value

    def integer(self, signed=False):
        value = shift = 0
        while True:
            octet = self.byte()
            value |= (octet & 127) << shift
            shift += 7
            require(shift <= 70, "oversized LEB integer")
            if not octet & 128:
                if signed and octet & 64:
                    value -= 1 << shift
                return value

    def text(self):
        return self.raw(self.integer()).decode("utf-8")

    def done(self):
        require(self.i == len(self.data), "unconsumed binary section")


def limits(reader):
    flags = reader.integer()
    require(flags == 1, "memory/table needs explicit nonshared maximum")
    lower, upper = reader.integer(), reader.integer()
    require(lower <= upper, "invalid limits")
    return lower, upper


def module(data):
    require(data[:8] == b"\0asm\1\0\0\0", "not scalar Wasm v1")
    stream = Reader(data, 8)
    sections = {}
    while stream.i < len(data):
        kind = stream.byte()
        size = stream.integer()
        offset = stream.i
        body = stream.raw(size)
        if kind:
            require(kind not in sections, "duplicate section")
            sections[kind] = (offset, body)
    types, functions, imports, exports, globals_, tables = [], [], {}, {}, [], []
    bodies, memories, elements, start = [], [], [], None
    for kind, (offset, body) in sections.items():
        r = Reader(body)
        if kind == 1:
            for _ in range(r.integer()):
                require(r.byte() == 0x60, "unknown type form")
                params = tuple(r.byte() for _ in range(r.integer()))
                results = tuple(r.byte() for _ in range(r.integer()))
                require(all(x in (0x7f, 0x7e, 0x7d, 0x7c) for x in params + results), "non-scalar type")
                require(len(results) <= 1, "multi-value result unsupported")
                types.append((params, results))
        elif kind == 2:
            for _ in range(r.integer()):
                name = r.text() + "." + r.text()
                tag = r.byte()
                if tag == 0:
                    imports[len(functions)] = name
                    functions.append(r.integer())
                elif tag == 2:
                    memories.append(limits(r))
                else:
                    raise ValueError("imported table/global or unknown import refused")
        elif kind == 3:
            functions.extend(r.integer() for _ in range(r.integer()))
        elif kind == 4:
            for _ in range(r.integer()):
                require(r.byte() == 0x70, "non-function table")
                tables.append(limits(r))
        elif kind == 5:
            memories.extend(limits(r) for _ in range(r.integer()))
        elif kind == 6:
            for _ in range(r.integer()):
                typ, mutable = r.byte(), r.byte()
                require(typ == 0x7f and mutable in (0, 1) and r.byte() == 0x41, "unsupported global")
                value = r.integer(signed=True)
                require(r.byte() == 0x0b, "nonconstant global initializer")
                globals_.append((typ, mutable, value))
        elif kind == 7:
            for _ in range(r.integer()):
                name, tag, index = r.text(), r.byte(), r.integer()
                require(name not in exports, "duplicate export")
                require(tag in (0, 2, 3), "exported mutable table refused")
                exports[name] = (tag, index)
        elif kind == 8:
            start = r.integer()
        elif kind == 9:
            for _ in range(r.integer()):
                require(r.integer() == 0 and r.byte() == 0x41, "unsupported element mode")
                base = r.integer(signed=True)
                require(base >= 0 and r.byte() == 0x0b, "invalid element offset")
                targets = [r.integer() for _ in range(r.integer())]
                elements.append((base, targets))
        elif kind == 10:
            for _ in range(r.integer()):
                header = offset + r.i
                size = r.integer()
                body_start = r.i
                local_types = []
                for _ in range(r.integer()):
                    count, typ = r.integer(), r.byte()
                    require(count <= 100000 and typ in (0x7f, 0x7e, 0x7d, 0x7c), "unsupported locals")
                    local_types.extend([typ] * count)
                opcode_start = offset + r.i
                r.raw(size - (r.i - body_start))
                bodies.append({"header": header, "start": opcode_start, "end": offset + r.i, "locals": local_types})
        elif kind in (11, 12):
            # Data bytes do not alter code, tables or the stack-pointer global.
            r.i = len(body)
        else:
            raise ValueError("unknown semantic section " + str(kind))
        r.done()
    require(len(memories) == 1 and len(globals_) >= 1, "one memory and internal SP required")
    require(globals_[0][:2] == (0x7f, 1), "global0 must be internal mutable i32 SP")
    require(all(not g[1] for g in globals_[1:]), "additional mutable globals refused")
    require(not any(tag == 3 and index == 0 for tag, index in exports.values()), "exported SP refused")
    require(len(bodies) + len(imports) == len(functions), "function/code count mismatch")
    require(all(index < len(types) for index in functions), "invalid function type")
    require(len(tables) <= 1 and all(a == b for a, b in tables), "dynamic-size table refused")
    table = [None] * (tables[0][0] if tables else 0)
    for base, targets in elements:
        require(base + len(targets) <= len(table), "element table overflow")
        table[base:base + len(targets)] = targets
    require(all(x is None or x < len(functions) for x in table), "invalid table function")
    return {"types": types, "functions": functions, "imports": imports, "exports": exports,
            "globals": globals_, "table": table, "bodies": bodies, "memory": memories[0], "start": start}


def disassembly(text, data, m):
    found, current = [], None
    for line in text.splitlines():
        header = re.fullmatch(r"([0-9a-f]+) <(.*)>:", line)
        if header:
            if header[2] == "CODE":
                continue
            current = {"header": int(header[1], 16), "name": header[2], "ins": []}
            found.append(current)
        elif re.match(r"^\s+[0-9a-f]+:", line):
            parts = line.split("\t")
            require(current is not None and len(parts) >= 2, "unrecognized instruction line")
            location, raw = parts[0].strip().split(":", 1)
            raw = bytes.fromhex(raw)
            position = int(location, 16)
            require(raw and data[position:position + len(raw)] == raw, "disassembly byte mismatch")
            op = parts[1].strip()
            arg = parts[2].split("#")[0].strip() if len(parts) > 2 else ""
            current["ins"].append((position, raw, op, arg))
    require(len(found) == len(m["bodies"]), "disassembly body count mismatch")
    for index, (f, body) in enumerate(zip(found, m["bodies"])):
        require(f["header"] == body["header"], "disassembly body order mismatch")
        at = body["start"]
        for position, raw, _, _ in f["ins"]:
            require(position == at, "disassembly byte gap/overlap")
            at += len(raw)
        require(at == body["end"], "disassembly incomplete body")
        f["id"] = len(m["imports"]) + index
    return {f["id"]: f for f in found}


# Unknown is None; constants are ('c',integer), SP-derived values ('s',frozenset).
def join_value(a, b):
    if a == b:
        return a
    if a is not None and b is not None and a[0] == b[0] == "s":
        offsets = a[1] | b[1]
        if len(offsets) <= 64:
            return "s", offsets
    return None


def pointer(offsets):
    return "s", frozenset(offsets)


def analyze_function(f, m):
    ins, fid = f["ins"], f["id"]
    budget, calls, indirect = 0, set(), 0
    for i, (_, _, op, arg) in enumerate(ins):
        if op == "global.get" and arg == "0":
            following = ins[i + 1:i + 3]
            require(len(following) == 2 and following[0][2] == "i32.const" and following[1][2] in ("i32.sub", "i32.add"), "unknown SP read in function " + str(fid))
            amount = int(following[0][3]) * (1 if following[1][2] == "i32.sub" else -1)
            require(amount >= 0, "non-decrement SP read")
            budget += amount
        if op == "call":
            calls.add(int(arg))
        if op == "call_indirect":
            indirect += 1
        require(not op.startswith(("table.", "return_call", "ref.", "throw", "try", "catch")) and op not in ("memory.grow", "call_ref"), "dynamic/unknown effect " + op)
    if indirect:
        require(m["table"], "indirect call without fixed table")
        calls.update(x for x in m["table"] if x is not None)
    require(all(0 <= x < len(m["functions"]) for x in calls), "invalid direct call")
    # Match the structured labels first. A branch's target is a lexical label,
    # never a guessed textual destination in the disassembler's comment.
    end, otherwise, scopes, opened = {}, {}, {}, []
    for i, (_, _, op, arg) in enumerate(ins):
        scopes[i] = tuple(opened)
        if op in ("block", "loop", "if"):
            require(arg in ("", "i32", "i64", "f32", "f64"), "unsupported block signature")
            require(op != "loop" or not arg, "loop result/parameter unsupported")
            opened.append(i)
        elif op == "else":
            require(opened and ins[opened[-1]][2] == "if" and opened[-1] not in otherwise, "unmatched else")
            otherwise[opened[-1]] = i
        elif op == "end":
            if opened:
                end[opened.pop()] = i
            else:
                require(i == len(ins) - 1, "early function end")
    require(not opened and ins and ins[-1][2] == "end", "unmatched structured control")
    # State = operand expressions, SP-derived locals, current globalSP offsets.
    states, queue, queued, heights = {}, collections.deque(), set(), {}
    initial = ((), {}, frozenset((0,)))

    def feed(pc, state):
        stack, locals_, sp = state
        require(sp and min(sp) >= -budget and max(sp) <= 0, "SP exceeds constant-site budget in function " + str(fid))
        if pc == len(ins):
            require(sp == frozenset((0,)), "function exit without SP restoration " + str(fid))
            return
        old = states.get(pc)
        if old is None:
            merged = state
        else:
            require(len(old[0]) == len(stack), "inconsistent operand-stack height")
            merged_sp = old[2] | sp
            require(len(merged_sp) <= 64, "unbounded SP alternatives")
            merged = (tuple(join_value(a, b) for a, b in zip(old[0], stack)),
                      {k: v for k in old[1].keys() & locals_.keys() if (v := join_value(old[1][k], locals_[k])) is not None},
                      merged_sp)
        if old != merged:
            states[pc] = merged
            if pc not in queued:
                queue.append(pc)
                queued.add(pc)

    feed(0, initial)
    steps, peak, writes = 0, 0, set()
    unary = {"eqz", "abs", "neg", "ceil", "floor", "trunc", "nearest", "sqrt", "clz", "ctz", "popcnt", "extend8_s", "extend16_s", "extend32_s"}
    binary = {"add", "sub", "mul", "div", "div_s", "div_u", "rem_s", "rem_u", "and", "or", "xor", "shl", "shr_s", "shr_u", "rotl", "rotr", "eq", "ne", "lt", "gt", "le", "ge", "lt_s", "lt_u", "gt_s", "gt_u", "le_s", "le_u", "ge_s", "ge_u", "min", "max", "copysign"}
    while queue:
        pc = queue.popleft()
        queued.remove(pc)
        steps += 1
        require(steps <= max(100000, len(ins) * 300), "dataflow convergence limit")
        stack = list(states[pc][0])
        locals_ = dict(states[pc][1])
        sp = states[pc][2]
        _, _, op, arg = ins[pc]

        def pop():
            require(stack, "operand stack underflow at " + str((fid, pc, op)))
            return stack.pop()

        def normalize(height, arity):
            require(len(stack) >= height + arity, "label stack underflow")
            values = stack[-arity:] if arity else []
            return tuple(stack[:height] + values), dict(locals_), sp

        def branch(depth):
            labels = scopes[pc]
            require(0 <= depth <= len(labels), "branch depth out of range")
            if depth == len(labels):
                require(sp == frozenset((0,)), "branch return without SP restoration")
                return
            label = labels[-depth - 1]
            require(label in heights, "label has no entry height")
            is_loop = ins[label][2] == "loop"
            arity = int(bool(ins[label][3])) if not is_loop else 0
            feed(label + 1 if is_loop else end[label] + 1, normalize(heights[label], arity))

        if op in ("block", "loop", "if"):
            if op == "if":
                pop()
            if pc in heights:
                require(heights[pc] == len(stack), "block entry stack mismatch")
            heights[pc] = len(stack)
            feed(pc + 1, (tuple(stack), locals_, sp))
            if op == "if":
                feed(otherwise.get(pc, end[pc]) + 1, (tuple(stack), locals_, sp))
            continue
        if op == "else":
            label = scopes[pc][-1]
            feed(end[label] + 1, normalize(heights[label], int(bool(ins[label][3]))))
            continue
        if op == "end":
            if not scopes[pc]:
                require(sp == frozenset((0,)), "normal return without SP restoration " + str(fid))
                continue
            label = scopes[pc][-1]
            feed(pc + 1, normalize(heights[label], int(bool(ins[label][3]))))
            continue
        if op == "br":
            branch(int(arg))
            continue
        if op == "br_if":
            pop()
            branch(int(arg))
        elif op == "br_table":
            pop()
            require(re.fullmatch(r"\{\d+(?:, \d+)*\}", arg), "unknown br_table format")
            for depth in (int(x) for x in arg[1:-1].split(", ")):
                branch(depth)
            continue
        elif op == "return":
            require(sp == frozenset((0,)), "explicit return without SP restoration " + str(fid))
            continue
        elif op == "unreachable":
            continue
        elif op.endswith(".const"):
            stack.append(("c", int(arg)) if op == "i32.const" else None)
        elif op == "local.get":
            stack.append(locals_.get(int(arg)))
        elif op in ("local.set", "local.tee"):
            value, index = pop(), int(arg)
            if value is not None and value[0] == "s":
                locals_[index] = value
            else:
                locals_.pop(index, None)
            if op == "local.tee":
                stack.append(value)
        elif op == "global.get":
            index = int(arg)
            require(index < len(m["globals"]), "invalid global")
            stack.append(pointer(sp) if index == 0 else ("c", m["globals"][index][2]))
        elif op == "global.set":
            value = pop()
            require(arg == "0" and value is not None and value[0] == "s", "dynamic/unknown SP write " + str(fid))
            sp = value[1]
            writes.add(pc)
            peak = max(peak, -min(sp))
        elif op == "drop":
            pop()
        elif op.endswith("select"):
            pop()
            b, a = pop(), pop()
            stack.append(join_value(a, b))
        elif op in ("call", "call_indirect"):
            if op == "call_indirect":
                pop()
                require(re.fullmatch(r"\d+", arg), "nondefault indirect table")
                typ = int(arg)
            else:
                typ = m["functions"][int(arg)]
            require(typ < len(m["types"]), "invalid call type")
            params, results = m["types"][typ]
            for _ in params:
                pop()
            stack.extend([None] * len(results))
        elif re.fullmatch(r"[if](?:32|64)\.load(?:8_[su]|16_[su]|32_[su])?", op):
            pop()
            stack.append(None)
        elif re.fullmatch(r"[if](?:32|64)\.store(?:8|16|32)?", op):
            pop()
            pop()
        elif op in ("memory.fill", "memory.copy"):
            for _ in range(3):
                pop()
        elif op == "memory.size":
            stack.append(None)
        elif "." in op and op.split(".", 1)[1] in binary:
            b, a = pop(), pop()
            result = None
            if op in ("i32.add", "i32.sub") and a is not None and b is not None:
                sign = 1 if op == "i32.add" else -1
                if a[0] == "s" and b[0] == "c":
                    result = pointer(x + sign * b[1] for x in a[1])
                elif op == "i32.add" and a[0] == "c" and b[0] == "s":
                    result = pointer(x + a[1] for x in b[1])
                elif a[0] == b[0] == "c":
                    value = (a[1] + sign * b[1]) & 0xffffffff
                    result = "c", value if value < 0x80000000 else value - 0x100000000
            stack.append(result)
        elif "." in op and (op.split(".", 1)[1] in unary or re.fullmatch(r"[if](32|64)\.(?:wrap|extend|trunc|trunc_sat|convert|demote|promote|reinterpret)_[if](32|64)(?:_[su])?", op)):
            pop()
            stack.append(None)
        elif op == "nop":
            pass
        else:
            raise ValueError("unknown opcode " + op)
        feed(pc + 1, (tuple(stack), locals_, sp))
    return {"id": fid, "name": f["name"], "constant_site_sum_bytes": budget,
            "observed_SP_decrement_bytes": peak, "calls": sorted(calls), "indirect_sites": indirect,
            "SP_write_sites": len(writes), "reachable_instruction_states": len(states), "dataflow_steps": steps}


def graph_bounds(rows, imports):
    index, low, pending, active, components = {}, {}, [], set(), []

    def visit(v):
        index[v] = low[v] = len(index)
        pending.append(v)
        active.add(v)
        for w in rows[v]["calls"]:
            if w not in rows:
                continue
            if w not in index:
                visit(w)
                low[v] = min(low[v], low[w])
            elif w in active:
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            component = []
            while True:
                w = pending.pop()
                active.remove(w)
                component.append(w)
                if w == v:
                    break
            components.append(component)
    for v in rows:
        if v not in index:
            visit(v)
    owner = {v: i for i, c in enumerate(components) for v in c}
    recursive = [c for c in components if len(c) > 1 or c[0] in rows[c[0]]["calls"]]
    require(all(rows[v]["constant_site_sum_bytes"] == 0 for c in recursive for v in c), "positive-stack recursion refused")
    memo = {}

    def bound(component):
        if component in memo:
            return memo[component]
        nodes = components[component]
        local = max(rows[v]["constant_site_sum_bytes"] for v in nodes)
        children = {owner[w] for v in nodes for w in rows[v]["calls"] if w in owner and owner[w] != component}
        child = max((bound(c) for c in children), default=(0, []), key=lambda x: x[0])
        result = local + child[0], nodes + child[1]
        memo[component] = result
        return result
    return {v: bound(owner[v]) for v in rows}, recursive


def audit(wasm, disasm, usage, grid):
    data, decoded = wasm.read_bytes(), disasm.read_bytes()
    m = module(data)
    bodies = disassembly(decoded.decode(), data, m)
    rows = {fid: analyze_function(f, m) for fid, f in bodies.items()}
    bounds, recursive = graph_bounds(rows, m["imports"])
    stack_bytes = m["globals"][0][2]
    maximum = max((b[0] for b in bounds.values()), default=0)
    require(0 <= maximum < stack_bytes, "linear-memory stack reservation exceeded")
    su_rows = []
    for source in usage:
        for line in source.read_text().splitlines():
            name, size, kind = line.rsplit("\t", 2)
            require(kind == "static", "dynamic C allocation refused: " + line)
            su_rows.append({"source": str(source), "function": name, "bytes": int(size)})
    exports = {}
    for name, (tag, index) in m["exports"].items():
        if tag == 0:
            b = bounds.get(index, (0, []))
            exports[name] = {"id": index, "bound_bytes": b[0], "path": b[1]}
    result = {"schema": "aethyr.wasm.linear-stack-oracle.v2", "wasm_sha256": digest(data),
              "disassembly_sha256": digest(decoded), "oracle_sha256": digest(Path(__file__).read_bytes()),
              "defined_functions": len(rows), "function_exports": len(exports), "imports": m["imports"],
              "memory_min_pages": m["memory"][0], "memory_max_pages": m["memory"][1],
              "stack_reservation_bytes": stack_bytes, "all_defined_function_bound_bytes": maximum,
              "stack_margin_bytes": stack_bytes - maximum, "immutable_table": m["table"],
              "recursive_components": recursive, "export_bounds": exports, "functions": list(rows.values()),
              "C_stack_usage_rows": len(su_rows), "C_stack_usage_max_bytes": max((r["bytes"] for r in su_rows), default=None),
              "C_stack_usage_sha256": {str(p): digest(p.read_bytes()) for p in usage},
              "method": "Affine entry-relative SP dataflow proves every normal return restores SP and every reachable write remains within the sum of all constant decrement sites. Loops reach a fixed point or refuse. Sum all sites per function plus the largest child; indirect calls include every fixed-table target. Positive-allocation SCCs refuse.",
              "premises": ["LLVM disassembler correctly decodes the byte-verified instruction spans and operands.", "Host imports do not synchronously re-enter positive-stack functions. The Grid host's storage-capacity callback uses only the separately verified zero-stack constant inbox-capacity getter.", "This bounds one normal-entry invocation from the initialized SP, followed by normal restored returns. It does not authorize resuming an instance after an arbitrary trap left its SP unrestored.", "This bounds the linear-memory ABI stack, not the Wasm engine's separate execution stack or arbitrary invalid pointer writes.", "Imported memory is not concurrently mutated; fixed internal function table is neither imported, exported nor mutated."]}
    if grid:
        def getter(name):
            tag, fid = m["exports"].get(name, (None, None))
            require(tag == 0 and fid in bodies, "missing Grid getter " + name)
            ins = bodies[fid]["ins"]
            require(len(ins) == 2 and ins[0][2] == "i32.const" and ins[1][2] == "end", "nonconstant Grid getter " + name)
            return int(ins[0][3])
        require(usage and su_rows, "Grid audit requires static C stack-usage diagnostics")
        size, vertices = getter("coaxial_state_size"), getter("coaxial_max_vertices")
        stride, uniforms = getter("coaxial_vertex_floats"), getter("coaxial_uniform_floats")
        tag, global_index = m["exports"].get("__heap_base", (None, None))
        require(tag == 3 and global_index > 0, "missing immutable heap base")
        heap = m["globals"][global_index][2]
        align = lambda x, n: ((x + n - 1) // n) * n
        # Match the published host arena formula using unbounded integers.
        base = align(max(heap, m["memory"][0] * 65536), 65536)
        capacity = align(max(size, 1024), 16)
        need = base + capacity + vertices * stride * 4 + uniforms * 4 + 64
        pages = align(need, 65536) // 65536
        require(stack_bytes == 4194304 and m["memory"][1] == 1024 and vertices == 400000 and stride == 12 and uniforms == 112, "Grid reservation/capacity changed")
        require(pages <= 1024, "Grid arena exceeds 64 MiB")
        require(all(name in exports for name in ("coaxial_grid_probe", "coaxial_grid_pose", "coaxial_grid_mark", "coaxial_grid_replay")), "missing Grid journey/replay exports")
        inbox_capacity = getter("coaxial_inbox_cap")
        require(exports["coaxial_inbox_cap"]["bound_bytes"] == 0, "host capacity callback must use no ABI stack")
        result["grid_arena"] = {"state_bytes": size, "max_vertices": vertices, "vertex_floats": stride,
                                "uniform_floats": uniforms, "heap_base": heap, "arena_base": base,
                                "state_capacity_bytes": capacity, "required_end_bytes": need,
                                "required_pages": pages, "maximum_pages": 1024,
                                "rounded_arena_margin_bytes": 67108864 - pages * 65536,
                                "synchronous_host_callback": "coaxial_inbox_cap",
                                "synchronous_callback_stack_bytes": 0, "inbox_capacity_bytes": inbox_capacity}
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--wasm", type=Path, required=True)
    p.add_argument("--disasm", type=Path, required=True)
    p.add_argument("--usage", type=Path, nargs="*", default=[])
    p.add_argument("--grid", action="store_true")
    p.add_argument("--receipt", type=Path, required=True)
    args = p.parse_args()
    try:
        result = audit(args.wasm, args.disasm, args.usage, args.grid)
        args.receipt.write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps({k: result[k] for k in ("wasm_sha256", "defined_functions", "function_exports", "all_defined_function_bound_bytes", "stack_margin_bytes")}))
        return 0
    except (ValueError, IndexError, KeyError, OSError) as error:
        print("Wasm stack oracle refused: " + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
