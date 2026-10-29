#!/usr/bin/env python3
"""Independent oracle for chapters 165 and 166: a separate interpreter for the
trace machine, seeded programs, trace digests, and exact-memory read streams.

Written from the chapter's specification, not translated from the Coax unit.
Tooling only: `--header` prints a C header for psionic/tests/trace.c.
"""
import hashlib
import random
import struct
import sys

CODES = ["push", "get", "set", "add", "sub", "mul", "load", "store", "brif", "out", "ret"]
C = {name: i for i, name in enumerate(CODES)}
STATUS = ["ok", "shape", "code", "argument", "underflow", "overflow", "local", "address",
          "branch", "arithmetic", "budget", "capacity", "fell", "alias", "digest"]
S = {name: i for i, name in enumerate(STATUS)}
I32 = (-2**31, 2**31 - 1)
STACK, LOCALS, CELLS, OUTPUTS, PROGRAM = 32, 8, 64, 64, 256
POP = {"add": 2, "sub": 2, "mul": 2, "store": 2, "set": 1, "load": 1, "brif": 1, "out": 1}


def admit(program):
    if not 1 <= len(program) <= PROGRAM:
        return "shape"
    for code, arg in program:
        if code == "push" and not I32[0] <= arg <= I32[1]:
            return "argument"
        if code in ("get", "set") and not 0 <= arg < LOCALS:
            return "local"
        if code == "brif" and not 0 <= arg < len(program):
            return "branch"
    return "ok"


def run(program, budget):
    """Returns (status, steps, outputs, trace) for the machine as specified."""
    status = admit(program)
    if status != "ok":
        return status, 0, [], []
    stack, local, cells, out, trace = [], [0] * LOCALS, [0] * CELLS, [], []
    ip = 0
    while True:
        if ip == len(program):
            return "fell", len(trace), [], []
        if len(trace) >= budget:
            return "budget", len(trace), [], []
        code, arg = program[ip]
        if len(stack) < POP.get(code, 0):
            return "underflow", len(trace), [], []
        if code in ("push", "get") and len(stack) >= STACK:
            return "overflow", len(trace), [], []
        nxt = ip + 1
        if code == "push":
            stack.append(arg); x, y = arg, len(stack)
        elif code == "get":
            stack.append(local[arg]); x, y = local[arg], arg
        elif code == "set":
            x = stack.pop(); local[arg] = x; y = arg
        elif code in ("add", "sub", "mul"):
            b, a = stack.pop(), stack.pop()
            v = a + b if code == "add" else a - b if code == "sub" else a * b
            if not I32[0] <= v <= I32[1]:
                return "arithmetic", len(trace), [], []
            stack.append(v); x, y = v, len(stack)
        elif code == "load":
            a = stack[-1]
            if not 0 <= a < CELLS:
                return "address", len(trace), [], []
            stack[-1] = cells[a]; x, y = cells[a], a
        elif code == "store":
            v, a = stack.pop(), stack.pop()
            if not 0 <= a < CELLS:
                return "address", len(trace), [], []
            cells[a] = v; x, y = v, a
        elif code == "brif":
            x = stack.pop()
            if x != 0:
                nxt = arg
            y = nxt
        elif code == "out":
            if len(out) >= OUTPUTS:
                return "capacity", len(trace), [], []
            x = stack.pop(); out.append(x); y = len(out)
        else:
            x, y, nxt = 0, 0, -1
        trace.append((ip, C[code], x, y))
        if nxt < 0:
            return "ok", len(trace), out, trace
        ip = nxt


def digest(trace):
    h = hashlib.sha256()
    for row in trace:
        h.update(struct.pack("<4i", *row))
    return h.digest()


def loop_sum(n):
    # local0 = n; local1 = 0; loop: local1 += local0; local0 -= 1; brif loop; out local1; ret
    return [("push", n), ("set", 0), ("push", 0), ("set", 1),
            ("get", 1), ("get", 0), ("add", 0), ("set", 1),
            ("get", 0), ("push", 1), ("sub", 0), ("set", 0),
            ("get", 0), ("brif", 4), ("get", 1), ("out", 0), ("ret", 0)]


def memory_fill(n):
    # for i in n..1: mem[i] = i*i; then out mem[n] + mem[1]
    return [("push", n), ("set", 0),
            ("get", 0), ("get", 0), ("get", 0), ("mul", 0), ("store", 0),
            ("get", 0), ("push", 1), ("sub", 0), ("set", 0), ("get", 0), ("brif", 2),
            ("push", n), ("load", 0), ("push", 1), ("load", 0), ("add", 0), ("out", 0),
            ("push", 0), ("load", 0), ("out", 0), ("ret", 0)]


def random_program(rng):
    n = rng.randint(1, 40)
    program = []
    for _ in range(n):
        code = rng.choice(CODES + ["push"] * 6 + ["get"] * 2)
        if code == "push":
            arg = rng.choice([rng.randint(-9, 9), rng.randint(0, 70), I32[1], I32[0]])
        elif code in ("get", "set"):
            arg = rng.randint(0, 7) if rng.random() < 0.97 else 8
        elif code == "brif":
            arg = rng.randint(0, n)
        else:
            arg = 0
        program.append((code, arg))
    if rng.random() < 0.7:
        program.append(("ret", 0))
    return program


def cases():
    rng = random.Random(165)
    out = []
    for n in (1, 2, 5, 17, 100):
        out.append((loop_sum(n), 4096))
    for n in (1, 3, 10, 63):
        out.append((memory_fill(n), 4096))
    out.append((loop_sum(1000), 50))  # stops on its step budget
    while len(out) < 300:
        program = random_program(rng)
        if len(program) <= 64:
            out.append((program, rng.choice([8, 64, 512])))
    return out


def memory_streams():
    rng = random.Random(166)
    streams = []
    for _ in range(40):
        keys = [rng.randint(-6, 6) for _ in range(rng.randint(0, 48))]
        values = [rng.randint(-10**6, 10**6) for _ in keys]
        queries = []
        for q in range(-7, 8):
            latest = None
            for k, v in zip(keys, values):
                if k == q:
                    latest = v
            queries.append((q, latest))
        streams.append((keys, values, queries))
    return streams


def header():
    lines = ["/* Generated by psionic/tests/trace_oracle.py; do not edit. */",
             "#include <stdint.h>",
             "typedef struct { int n; int code[64]; int64_t arg[64]; int budget; int status; int steps;",
             "  int outputs; int64_t out[64]; uint8_t digest[32]; } trace_case;",
             "static const trace_case trace_cases[] = {"]
    for program, budget in cases():
        status, steps, outputs, trace = run(program, budget)
        d = digest(trace) if status == "ok" else bytes(32)
        lines.append("  {%d, {%s}, {%s}, %d, %d, %d, %d, {%s}, {%s}}," % (
            len(program), ",".join(str(C[c]) for c, _ in program),
            ",".join("%dLL" % a for _, a in program), budget, S[status], steps,
            len(outputs), ",".join("%dLL" % v for v in outputs) or "0",
            ",".join(str(b) for b in d)))
    lines.append("};")
    lines.append("typedef struct { int count; int64_t key[48]; int64_t value[48]; int64_t query[15];"
                 " int found[15]; int64_t expect[15]; } memory_stream;")
    lines.append("static const memory_stream memory_streams[] = {")
    for keys, values, queries in memory_streams():
        lines.append("  {%d, {%s}, {%s}, {%s}, {%s}, {%s}}," % (
            len(keys), ",".join("%dLL" % k for k in keys) or "0", ",".join("%dLL" % v for v in values) or "0",
            ",".join("%dLL" % q for q, _ in queries), ",".join("1" if v is not None else "0" for _, v in queries),
            ",".join("%dLL" % (v or 0) for _, v in queries)))
    lines.append("};")
    return "\n".join(lines)


def main():
    if sys.argv[1:] == ["--header"]:
        print(header())
    elif sys.argv[1:] == ["--coax"]:
        sys.stdout.write(coax_fixture())
    elif sys.argv[1:] == ["--memory"]:
        sys.stdout.write(coax_fixture(memory=True))
    else:
        counts = {}
        for program, budget in cases():
            status = run(program, budget)[0]
            counts[status] = counts.get(status, 0) + 1
        print("trace oracle: %d programs; statuses %s" % (sum(counts.values()), counts))


def targeted():
    """Programs aimed at every boundary of the machine, for the Coax fixtures."""
    MAX, MIN = I32[1], I32[0]
    P = lambda *ops: [o if isinstance(o, tuple) else (o, 0) for o in ops]
    outs = lambda n: P(*([op for i in range(n) for op in (("push", i * 3 - 7), "out")] + ["ret"]))
    t = [
        (P(("push", 3), ("push", 4), "add", "out", ("push", 9), ("push", 2), "sub", "out",
           ("push", -6), ("push", 7), "mul", "out", "ret"), 64),
        (P(("push", 11), ("set", 3), ("get", 3), ("get", 3), "add", ("set", 5), ("get", 5), "out", ("get", 7), "out", "ret"), 64),
        (P(("push", 0), ("push", 5), "store", ("push", 63), ("push", 7), "store", ("push", 0), "load", "out",
           ("push", 63), "load", "out", ("push", 1), "load", "out", "ret"), 64),
        (P(("push", 64), ("push", 1), "store", "ret"), 64),
        (P(("push", -1), ("push", 1), "store", "ret"), 64),
        (P(("push", 64), "load", "ret"), 64),
        (P(("push", -1), "load", "ret"), 64),
        (P(("set", 0), "ret"), 64), (P("load", "ret"), 64), (P(("brif", 0), "ret"), 64), (P("out", "ret"), 64),
        (P(("push", 1), "store", "ret"), 64), (P(("push", 1), "add", "ret"), 64), (P("add", "ret"), 64),
        (P(("push", 1), "sub", "ret"), 64), (P(("push", 1), "mul", "ret"), 64),
        (P(*([("push", i) for i in range(32)] + [("get", 0), "ret"])), 64),
        (P(*([("push", i) for i in range(32)] + ["ret"])), 64),
        (P(*([("push", i) for i in range(31)] + [("get", 0), "add", "out", "ret"])), 64),
        (P(("push", 0), ("brif", 4), ("push", 5), "out", ("push", 2), ("brif", 7), ("push", 6), ("push", 8), "out", "ret"), 64),
        (P(("push", -3), ("brif", 3), ("push", 1), ("push", 4), "out", "ret"), 64),
        (outs(64), 256), (outs(65), 256), (outs(3), 16),
        (P("ret"), 1), (P("ret"), 65536), (P(("push", 1), "out", "ret"), 3), (P(("push", 1), "out", "ret"), 2),
        (P(("push", MAX), "out", ("push", MIN), "out", "ret"), 8),
        (P(("push", MAX), ("push", 1), "add", "ret"), 8), (P(("push", MIN), ("push", 1), "sub", "ret"), 8),
        (P(("push", MIN), ("push", -1), "mul", "ret"), 8), (P(("push", -65536), ("push", 32768), "mul", "out", "ret"), 8),
        (P(("push", 2), ("push", -3), "sub", "out", ("push", -2), ("push", -3), "add", "out", "ret"), 16),
        (P(("push", 7)), 8), (P(("push", 1), ("brif", 0), "ret"), 40),
        (P(("push", 3), ("push", 1000000), "store", ("push", 3), "load", ("push", 3), ("push", -5), "store",
           ("push", 3), "load", "add", "out", "ret"), 64),
    ]
    picked = [c for c in cases()[9:] if run(*c)[0] == "ok" and run(*c)[1] <= 120][:12]
    return t + picked


def coax_fixture(memory=False):
    """One self-contained #test per run: no shared helper for mutation to weaken.
    With `memory`, the same runs go through em_run with exactly enough key and
    value scratch for the program's stores."""
    name = "memory" if memory else "trace"
    lines = ["// Generated by psionic/tests/trace_oracle.py --%s; an independent interpreter's runs." % ("memory" if memory else "coax")]
    for i, (program, budget) in enumerate(targeted()):
        status, steps, outputs, trace = run(program, budget)
        d = digest(trace) if status == "ok" else bytes(32)
        n = len(program)
        stores = max(1, sum(1 for row in trace if row[1] == C["store"]) if status == "ok" else 64)
        call = "em_run(p[..],%d,%s,%s,keys[..],values[..],false)" if memory else "tm_run(p[..],%d,%s,%s,false)"
        lines.append('#test "independent %s fixture %d" {' % (name, i))
        if memory:
            lines.append("    keys:[%d]int;values:[%d]int;" % (stores, stores))
        lines.append("    p:=[%d]TmOp.[%s];" % (n, ",".join("TmOp.{code=TmCode.%s,arg=%d}" % (c, a) for c, a in program)))
        lines.append("    d:=[32]u8.[%s];trace:[%d]int;outputs:[65]int;" % (",".join(str(b) for b in d), max(4, 4 * steps + 4)))
        lines.append("    for i:0..<trace.count {trace[i]=-9;} for i:0..<65 {outputs[i]=-9;}")
        lines.append("    r:=" + call % (budget, "trace[..]", "outputs[..]") + ";")
        lines.append("    #assert r.status==TmStatus.%s;#assert r.steps==%d;" % (status, steps))
        lines.append("    for i:0..<32 {#assert r.digest[i]==d[i];}")
        if status != "ok":
            lines.append("    for v:trace {#assert v==-9;} for v:outputs {#assert v==-9;}")
        else:
            lines.append("    #assert r.outputs==%d;#assert p[r.ip].code==TmCode.ret;" % len(outputs))
            for j, v in enumerate(outputs):
                lines.append("    #assert outputs[%d]==%d;" % (j, v))
            lines.append("    #assert outputs[%d]==-9;#assert trace[%d]==-9;" % (len(outputs), 4 * steps))
            if steps <= 40:
                for j, row in enumerate(trace):
                    lines.append("    #assert trace[%d]==%d;#assert trace[%d]==%d;#assert trace[%d]==%d;#assert trace[%d]==%d;" % (
                        4*j, row[0], 4*j+1, row[1], 4*j+2, row[2], 4*j+3, row[3]))
                A = lambda b, t, o, want: lines.append("    #assert " + call % (b, t, o) + ".status==TmStatus.%s;" % want)
                A(budget, "trace[0..<%d]" % (4*steps), "outputs[0..<%d]" % max(1, len(outputs)), "ok")
                A(budget, "trace[0..<%d]" % (4*steps-1), "outputs[..]", "capacity")
                if len(outputs) > 1:
                    A(budget, "trace[..]", "outputs[0..<%d]" % (len(outputs)-1), "capacity")
                A(steps, "trace[..]", "outputs[..]", "ok")
                if steps > 1:
                    A(steps-1, "trace[..]", "outputs[..]", "budget")
            if memory and any(row[1] == C["store"] for row in trace):
                lines.append("    #assert em_run(p[..],%d,trace[..],outputs[..],keys[0..<%d],values[..],false).status==TmStatus.capacity;" % (budget, stores-1))
                lines.append("    #assert em_run(p[..],%d,trace[..],outputs[..],keys[..],values[0..<%d],false).status==TmStatus.capacity;" % (budget, stores-1))
        lines.append("}")
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    main()
