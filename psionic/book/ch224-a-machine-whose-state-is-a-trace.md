# 224. A machine whose state is a trace

Part XXIII builds a transformer that executes programs exactly, because its
weights are constructed rather than trained. Before any weights, we need the
thing it will imitate: a small machine, a reference interpreter, and a record
of every step precise enough that two runs can be compared by one digest.

Each instruction the machine executes writes four integers to its trace. A
model that executes the same program must emit the same integers in the same
order, so the trace is the receipt: anyone holding the program can replay it
and check the digest.

```shell
coax/build.sh
coax/bin/coax run psionic/examples/exact-trace.coax
psionic/tests/check_trace.sh
```

```jai
machine: 53 steps, output 17, trace digest d7be7b683aee9a03
memory by attention: 53 steps, output 17, trace digest d7be7b683aee9a03
read key 7 from [7,3,7,9]: 30 (latest write, position 2)
read key 8: refused as missing
endless loop: refused after 64 steps, nothing published
```

The [example](../examples/exact-trace.coax) prints those five lines. This chapter covers the first line and the last; [chapter 225](ch225-attention-as-exact-memory.md)
covers the rest. Nothing here needs model weights, a GPU or a network.

## The machine

The machine has a stack of at most 32 integers, 8 locals, 64 memory cells that
start at zero, and an output list. Every value is a signed 32-bit integer, held
in a 64-bit `int` so overflow can be checked rather than wrapped.

| Instruction | Effect | Trace `x` | Trace `y` |
| --- | --- | --- | --- |
| `push n` | push `n` | `n` | new depth |
| `get i` | push local `i` | the value | `i` |
| `set i` | pop into local `i` | the value | `i` |
| `add`, `sub`, `mul` | pop `b`, pop `a`, push `a op b` | the result | new depth |
| `load` | replace address `a` on top with memory `a` | the value | `a` |
| `store` | pop value `v`, pop address `a`, write memory `a` | `v` | `a` |
| `brif t` | pop `x`; jump to `t` if `x` is not zero | `x` | next instruction |
| `out` | pop and append to the outputs | the value | outputs so far |
| `ret` | halt | 0 | 0 |

A trace row is `(ip, code, x, y)`: where the step ran, what it was, and the
two values that let a reader reconstruct the change of state without
re-running the step. The digest is SHA-256 over the rows, each token as four
little-endian bytes.

The three-instruction program `push 7; out; ret` has this trace:

| ip | code | x | y |
| --- | --- | --- | --- |
| 0 | 0 (`push`) | 7 | 1 |
| 1 | 9 (`out`) | 7 | 1 |
| 2 | 10 (`ret`) | 0 | 0 |

## Admit the program first

Everything that can be checked without running is checked before the first
step: the length, every instruction code, push arguments that fit in 32 bits,
local indexes and branch targets.

<!-- source: psionic/lib/trace-machine.coax admit -->
```jai
tm_fits :: (v:int)->bool {return v>=TM_I32_MIN && v<=TM_I32_MAX;}
tm_admit :: (program:[]TmOp)->TmStatus #export_c {
    if program.count<1 || program.count>TM_PROGRAM_MAX {return TmStatus.shape;}
    for op:program {
        if cast(int)op.code>cast(int)TmCode.ret {return TmStatus.code;}
        if op.code==TmCode.push && !tm_fits(op.arg) {return TmStatus.argument;}
        if (op.code==TmCode.get || op.code==TmCode.set) && (op.arg<0 || op.arg>=TM_LOCALS) {return TmStatus.local;}
        if op.code==TmCode.brif && (op.arg<0 || op.arg>=program.count) {return TmStatus.branch;}
    }
    return TmStatus.ok;
}
```

## One step

`tm_step` takes the state by value and returns the next one, so a refused step
leaves the caller's state untouched. Memory isn't part of that state. A `load`
reads whatever memory the caller passes in, and a `store` only reports its
address and value in the trace row, for the caller to apply. That separation
is what lets chapter 225 serve memory from somewhere else. Stack underflow and
overflow, an address outside memory, arithmetic that leaves 32 bits and a full
output list are each a typed refusal.

<!-- source: psionic/lib/trace-machine.coax step -->
```jai
// One instruction: it returns the next state and instruction pointer (or a
// refusal) and writes its four trace tokens (ip, code, x, y) into `row`.
// Memory is not part of the state: `load` reads the caller's `cells`, and a
// `store` is only reported in its row, for the caller to apply.
tm_step :: (program:[]TmOp,ip:int,s:TmState,cells:[]int,row:[]int)->TmStepResult {
    op:=program[ip];next:=ip+1;r:=TmStepResult.{state=s};
    need:=0;
    if op.code==TmCode.add || op.code==TmCode.sub || op.code==TmCode.mul || op.code==TmCode.store {need=2;}
    if op.code==TmCode.set || op.code==TmCode.load || op.code==TmCode.brif || op.code==TmCode.out {need=1;}
    if r.state.depth<need {r.status=TmStatus.underflow;return r;}
    pushes:=op.code==TmCode.push || op.code==TmCode.get;
    if pushes && r.state.depth>=TM_STACK_MAX {r.status=TmStatus.overflow;return r;}
    x:=0;y:=0;d:=r.state.depth;
    if op.code==TmCode.push {x=op.arg;r.state.stack[d]=x;r.state.depth=d+1;y=d+1;}
    else if op.code==TmCode.get {x=r.state.locals[op.arg];r.state.stack[d]=x;r.state.depth=d+1;y=op.arg;}
    else if op.code==TmCode.set {x=r.state.stack[d-1];r.state.depth=d-1;r.state.locals[op.arg]=x;y=op.arg;}
    else if op.code==TmCode.add || op.code==TmCode.sub || op.code==TmCode.mul {
        a:=r.state.stack[d-2];b:=r.state.stack[d-1];v:=a+b;
        if op.code==TmCode.sub {v=a-b;}
        if op.code==TmCode.mul {v=a*b;}
        if !tm_fits(v) {r.status=TmStatus.arithmetic;return r;}
        r.state.stack[d-2]=v;r.state.depth=d-1;x=v;y=d-1;
    }
    else if op.code==TmCode.load {
        a:=r.state.stack[d-1];if a<0 || a>=TM_CELLS {r.status=TmStatus.address;return r;}
        x=cells[a];r.state.stack[d-1]=x;y=a;
    }
    else if op.code==TmCode.store {
        a:=r.state.stack[d-2];v:=r.state.stack[d-1];if a<0 || a>=TM_CELLS {r.status=TmStatus.address;return r;}
        r.state.depth=d-2;x=v;y=a;
    }
    else if op.code==TmCode.brif {
        x=r.state.stack[d-1];r.state.depth=d-1;if x!=0 {next=op.arg;}y=next;
    }
    else if op.code==TmCode.out {
        if r.state.outputs>=TM_OUTPUTS {r.status=TmStatus.capacity;return r;}
        x=r.state.stack[d-1];r.state.depth=d-1;r.state.outputs+=1;y=r.state.outputs;
    }
    else {next=-1;}
    row[0]=ip;row[1]=cast(int)op.code;row[2]=x;row[3]=y;
    r.next=next;return r;
}
```

## Run, then publish

A program may loop forever, so the caller supplies a step budget. The first
pass executes and hashes while staging outputs locally. Only when the program
has reached `ret`, inside its budget, with room for the whole trace, are the
outputs published and a second pass written into the caller's trace. A
refused run writes nothing.

<!-- source: psionic/lib/trace-machine.coax run -->
```jai
// Runs a program for at most `budget` steps. The first pass executes and
// hashes without touching the caller's storage; only a run known to succeed
// is replayed into the caller's trace, so a refused run writes nothing.
tm_pass :: (program:[]TmOp,budget:int,trace:[]int,outputs:[]int,publish:bool)->TmResult {
    s:TmState;cells:[64]int;ip:=0;steps:=0;stage_out:[64]int;row:[4]int;bytes:[16]u8;
    sha:[112]u8;ok:=sha256_init(sha[..])==SHA_OK;
    while true {
        if ip==program.count {return TmResult.{status=TmStatus.fell,ip=ip,steps=steps};}
        if steps>=budget {return TmResult.{status=TmStatus.budget,ip=ip,steps=steps};}
        r:=tm_step(program,ip,s,cells[..],row[..]);
        if r.status!=TmStatus.ok {return TmResult.{status=r.status,ip=ip,steps=steps};}
        if program[ip].code==TmCode.store {cells[row[3]]=row[2];}
        if r.state.outputs>s.outputs {stage_out[s.outputs]=row[2];}
        if (steps+1)*TM_TOKENS>trace.count || r.state.outputs>outputs.count {return TmResult.{status=TmStatus.capacity,ip=ip,steps=steps};}
        if publish {for t:0..<TM_TOKENS {trace[steps*TM_TOKENS+t]=row[t];}}
        tm_bytes(row[..],bytes[..]);ok=ok && sha256_update_checked(sha[..],bytes[..],false)==SHA_OK;
        s=r.state;steps+=1;
        if r.next<0 {break;}
        ip=r.next;
    }
    result:=TmResult.{ip=ip,steps=steps,outputs=s.outputs};
    ok=ok && sha256_final_checked(sha[..],result.digest[..],false)==SHA_OK;
    if !ok {return TmResult.{status=TmStatus.digest};}
    for i:0..<s.outputs {outputs[i]=stage_out[i];}
    return result;
}
tm_run :: (program:[]TmOp,budget:int,trace:[]int,outputs:[]int,overlap:bool)->TmResult #export_c {
    admitted:=tm_admit(program);if admitted!=TmStatus.ok {return TmResult.{status=admitted};}
    if budget<1 || budget>TM_STEPS_MAX {return TmResult.{status=TmStatus.budget};}
    if outputs.count<1 {return TmResult.{status=TmStatus.capacity};}
    if overlap {return TmResult.{status=TmStatus.alias};}
    first:=tm_pass(program,budget,trace,outputs,false);
    if first.status!=TmStatus.ok {return first;}
    return tm_pass(program,budget,trace,outputs,true);
}
```

| Refusal | When |
| --- | --- |
| `shape`, `code`, `argument`, `local`, `branch` | Admission, before any step |
| `budget` | The budget is outside 1 to 65,536, or the program has not halted within it |
| `underflow`, `overflow` | The stack has too few values, or already holds 32 |
| `address`, `arithmetic` | A memory address outside 0 to 63, or a result outside 32 bits |
| `fell` | Execution ran past the last instruction without `ret` |
| `capacity`, `alias` | The caller's trace or outputs are too small, or overlap |

## How it is checked

- **Coax checks** run a constant program, a counting loop, memory traffic, every
  refusal (with the caller's buffers unchanged), and the bounds on the stack,
  program length and step budget.
- **49 generated fixtures** in [trace-vectors.coax](../fixtures/trace-vectors.coax)
  come from the independent interpreter below. They aim at every boundary
  (addresses 0, 63, 64 and −1, exactly 32 stack values, 64 and 65 outputs, the
  32-bit limits). Each checks its status, step count, outputs, every trace
  token and the digest, then reruns with exactly enough storage, one token
  short and one step short of its budget.
- **An independent interpreter** in [trace_oracle.py](../tests/trace_oracle.py)
  was written from the table above, not from the Coax code. It runs 300
  programs (a summing loop, a memory-filling loop and seeded random programs)
  and records each one's status, steps, outputs and digest.
- **The native harness** [trace.c](../tests/trace.c) runs all 300 through the
  generated C, plain and under ASan and UBSan, in debug and release. Each
  refused run must leave its buffers untouched.
- **Mutation testing** of `trace-machine.coax` kills all 1,045 mutants. The
  first run left 46 alive, which led to the generated fixtures and to removing
  one condition that could never be observed.

The [verification record](../../docs/verification/psionic-ch165-166-2026-10-29.md)
has the commands and results.

## What this does not do yet

The interpreter is the reference, not the model. Chapter 225 replaces its
memory with an attention read; chapter 226 adds arithmetic and branches as
feed-forward layers; chapter 227 compiles a program into weights. Every later
chapter in this part is judged by one test: does it reproduce this
interpreter's digest, bit for bit?
