# 232. Exact replay as a verification class

[Chapter 224](ch224-a-machine-whose-state-is-a-trace.md) made a run's trace
its receipt: four integers per step, and one digest over all of them.
[Chapter 225](ch225-attention-as-exact-memory.md) and
[chapter 228](ch228-a-harness-that-catches-the-compiler.md) produced the same
digest from executors whose memory and arithmetic are attention and ReLU
layers. Part XXIII's whole claim is that such an executor is *exactly* the
interpreter, so the check is exact too: replay the program and compare.

This chapter makes that check a verification class with three verdicts:
- **exact:** every step was replayed and matched, and so did all three
  digests;
- **failed:** something didn't match, and the report names what. When the
  rows were given, it names the first divergent step and which of its four
  tokens differ;
- **unverifiable:** nothing failed, but the verifier didn't check enough to
  say exact. Only a window was replayed, the run was over its budget, or a
  window was asked for without the rows.

```shell
coax/bin/coax run psionic/examples/exact-replay.coax
coax/bin/coax test psionic/lib/replay.coax
python3 -I psionic/tests/replay_oracle.py
```

```jai
claim: 53 steps, output 17
whole run, with rows: exact (none), 53 steps replayed
whole run, digests only: exact (none), 53 steps replayed
window 40..<53: unverifiable (window), 13 steps replayed
whole run, budget 20: unverifiable (over_budget), 0 steps replayed
program digest changed: failed (program), 0 steps replayed
trace digest changed: failed (rows), 0 steps replayed
output digest changed: failed (outputs), 53 steps replayed
store forged, whole run: failed (row) at step 6, field x, 6 steps replayed
store forged, window 40..<53: failed (row) at step 47, field x, 7 steps replayed
store forged, window 48..<53: unverifiable (window), 5 steps replayed
claim of 52 steps: failed (running) at step 51, 52 steps replayed
```

The [example](../examples/exact-replay.coax) takes its claim from chapter
225's executor, whose memory is attention, and checks it with chapter 224's
interpreter. Then it changes each digest, forges one store, and shortens the
claim.

## A claim is three digests and a step count

<!-- source: psionic/lib/replay.coax digests -->
```jai
rx_word :: (sha:[]u8,v:int) {
    b:[4]u8;w:=cast(u32)(v&0xffffffff);
    for i:0..<4 {b[i]=cast(u8)((w>>cast(u32)(i*8))&0xff);}
    #assert sha256_update_checked(sha,b[..],false)==SHA_OK;
}
rx_program_digest :: (program:[]TmOp)->[32]u8 {
    sha:[112]u8;d:[32]u8;#assert sha256_init(sha[..])==SHA_OK;
    for op:program {
        c:[1]u8;c[0]=cast(u8)op.code;
        #assert sha256_update_checked(sha[..],c[..],false)==SHA_OK;
        rx_word(sha[..],op.arg);
    }
    #assert sha256_final_checked(sha[..],d[..],false)==SHA_OK;
    return d;
}
// The first n integers, each as four little-endian bytes: a trace's rows
// give chapter 224's trace digest, and a run's outputs its output digest.
rx_ints_digest :: (v:[]int,n:int)->[32]u8 {
    sha:[112]u8;d:[32]u8;#assert sha256_init(sha[..])==SHA_OK;
    for i:0..<n {rx_word(sha[..],v[i]);}
    #assert sha256_final_checked(sha[..],d[..],false)==SHA_OK;
    return d;
}
```

A claim names the program, the trace and the outputs, each by SHA-256, plus
the number of steps:
- **the program digest** covers each instruction's code byte, then its
  argument as four little-endian bytes;
- **the trace digest** is chapter 224's;
- **the output digest** covers each output as four little-endian bytes.

A verifier needs the program itself, and can be given the trace's rows too.
The digests alone are enough to check the whole run. The rows are needed to
check a window, and to name where a run went wrong.

<!-- source: psionic/lib/replay.coax claim -->
```jai
// The claimant's side: run the program and name the run by its digests.
rx_claim :: (program:[]TmOp,budget:int,trace:[]int,outputs:[]int)->RxMade {
    r:=tm_run(program,budget,trace,outputs,false);
    m:=RxMade.{status=r.status};
    if r.status!=TmStatus.ok {return m;}
    m.claim.program=rx_program_digest(program);
    m.claim.trace=r.digest;
    m.claim.outputs=rx_ints_digest(outputs,r.outputs);
    m.claim.steps=r.steps;
    return m;
}
```

## Folding the rows before a window

To replay steps `a` to `b`, the verifier needs the machine as it was just
before step `a`: its stack, locals, memory and output count. It doesn't
compute that. It folds the claimed rows instead, taking each row's `x` as the
value its step produced.

<!-- source: psionic/lib/replay.coax fold -->
```jai
// Applies the claimed rows before `upto` to a fresh machine. No value is
// computed: each row's x is taken as the value its step produced. Only shape
// is checked, so the state stays inside the machine's bounds: the row's ip
// and code, the stack depth, the address of a store, the output count and
// where control goes next. ok is false at the first row that fails.
rx_fold :: (program:[]TmOp,rows:[]int,upto:int)->RxFold {
    f:RxFold;
    while f.step<upto {
        o:=f.step*TM_TOKENS;
        if f.ip>=program.count || rows[o]!=f.ip {return f;}
        op:=program[f.ip];
        if rows[o+1]!=cast(int)op.code {return f;}
        x:=rows[o+2];y:=rows[o+3];d:=f.state.depth;s:=f.state;next:=f.ip+1;
        if op.code==TmCode.push || op.code==TmCode.get {
            if d>=TM_STACK_MAX {return f;}
            s.stack[d]=x;s.depth=d+1;
        }
        else if op.code==TmCode.set {
            if d<1 {return f;}
            s.locals[op.arg]=x;s.depth=d-1;
        }
        else if op.code==TmCode.add || op.code==TmCode.sub || op.code==TmCode.mul {
            if d<2 {return f;}
            s.stack[d-2]=x;s.depth=d-1;
        }
        else if op.code==TmCode.load {
            if d<1 {return f;}
            s.stack[d-1]=x;
        }
        else if op.code==TmCode.store {
            if d<2 {return f;}
            a:=s.stack[d-2];if a<0 || a>=TM_CELLS {return f;}
            f.cells[a]=x;s.depth=d-2;
        }
        else if op.code==TmCode.brif {
            if d<1 || (y!=next && y!=op.arg) {return f;}
            s.depth=d-1;next=y;
        }
        else if op.code==TmCode.out {
            if d<1 || s.outputs>=TM_OUTPUTS {return f;}
            s.depth=d-1;s.outputs+=1;
        }
        else {return f;}
        f.state=s;f.ip=next;f.step+=1;
    }
    f.ok=true;
    return f;
}
```

The fold checks only shape, enough to keep the machine inside its bounds:
- the row is at the instruction where control went, with that instruction's
  code;
- the stack is deep enough, and not full before a push;
- a store's address is one of the 64 cells;
- a branch goes to the next instruction or its target;
- there are at most 64 outputs, and no `ret` before the window.

A row that breaks the shape fails as `structure`, at its step.

Values before the window are trusted, so a window can miss a lie told
before it. In the example, the forged store at step 6 writes 99 where 16
belongs. The window from step 40 catches it at step 47, where memory 4 is
loaded: the replay reads 99, but the claimed row says 16. The window from
step 48 starts after that load, and finds nothing. That is why a clean window
is `unverifiable`, never `exact`.

## The check

<!-- source: psionic/lib/replay.coax check -->
```jai
// Checks steps a..<b of a claim. rows are the claimed trace (all of it) or
// empty; without rows only the whole run can be checked, by its digests.
// budget bounds the steps the verifier executes; folded rows don't count.
rx_check :: (program:[]TmOp,claim:RxClaim,rows:[]int,a:int,b:int,budget:int)->RxReport {
    r:=RxReport.{step=-1};
    U:=RxVerdict.unverifiable;F:=RxVerdict.failed;
    if claim.steps<1 || claim.steps>TM_STEPS_MAX {return rx_end(r,F,RxReason.steps,-1);}
    if a<0 || b<=a || b>claim.steps || budget<1 || (rows.count!=0 && rows.count!=claim.steps*TM_TOKENS) {
        r.status=RxStatus.shape;return r;
    }
    if !rx_same(rx_program_digest(program),claim.program) {return rx_end(r,F,RxReason.program,-1);}
    if tm_admit(program)!=TmStatus.ok {return rx_end(r,F,RxReason.inadmissible,-1);}
    if b-a>budget {return rx_end(r,U,RxReason.over_budget,-1);}
    whole:=a==0 && b==claim.steps;given:=rows.count==claim.steps*TM_TOKENS;
    if !given && !whole {return rx_end(r,U,RxReason.no_rows,-1);}
    if given && !rx_same(rx_ints_digest(rows,rows.count),claim.trace) {return rx_end(r,F,RxReason.rows,-1);}
    f:=rx_fold(program,rows,a);
    if !f.ok {return rx_end(r,F,RxReason.structure,f.step);}
    // Replay: the trace and outputs are hashed as they are produced, so the
    // whole run needs no storage beyond the machine.
    trace:[112]u8;outs:[112]u8;
    #assert sha256_init(trace[..])==SHA_OK && sha256_init(outs[..])==SHA_OK;
    ip:=f.ip;s:=f.state;row:[4]int;
    for step:a..<b {
        if ip>=program.count {return rx_end(r,F,RxReason.refused,step);}
        t:=tm_step(program,ip,s,f.cells[..],row[..]);
        if t.status!=TmStatus.ok {return rx_end(r,F,RxReason.refused,step);}
        if given {
            field:=rx_field(row[..],rows[step*TM_TOKENS..<step*TM_TOKENS+TM_TOKENS]);
            if field!=RxField.none {e:=rx_end(r,F,RxReason.row,step);e.field=field;return e;}
        }
        r.checked+=1;
        if program[ip].code==TmCode.store {f.cells[row[3]]=row[2];}
        if t.state.outputs>s.outputs {rx_word(outs[..],row[2]);}
        for k:0..<TM_TOKENS {rx_word(trace[..],row[k]);}
        if t.next<0 && step<claim.steps-1 {return rx_end(r,F,RxReason.halted,step);}
        if t.next>=0 && step==claim.steps-1 {return rx_end(r,F,RxReason.running,step);}
        s=t.state;ip=t.next;
    }
    if !whole {return rx_end(r,U,RxReason.window,-1);}
    d:[32]u8;
    #assert sha256_final_checked(trace[..],d[..],false)==SHA_OK;
    if !rx_same(d,claim.trace) {return rx_end(r,F,RxReason.trace,-1);}
    #assert sha256_final_checked(outs[..],d[..],false)==SHA_OK;
    if !rx_same(d,claim.outputs) {return rx_end(r,F,RxReason.outputs,-1);}
    return rx_end(r,RxVerdict.exact,RxReason.none,-1);
}
```

The checks run in a fixed order, and the first that fails gives the verdict:
1. a step count outside the machine's 1 to 65,536;
2. the request's shape: the window, the budget and the number of rows;
3. the program digest, then chapter 224's admission;
4. the budget: the verifier executes at most `budget` steps. Folding doesn't
   count, because it computes nothing;
5. the rows, if given, must hash to the trace digest. A claimant can't show
   one set of rows and sign another;
6. the fold up to the window;
7. the replay, step by step. A step the machine refuses fails as `refused`.
   A row that differs fails as `row`, with its first different field. A run
   that halts before its claimed last step fails as `halted`, and one still
   running after it fails as `running`;
8. for the whole run, the replayed trace and outputs must match their
   digests.

The replay hashes as it goes, so even a 65,536-step run needs no storage
beyond the machine.

## Every way to fail

| Change to an honest claim | Verdict | Where it's caught |
| --- | --- | --- |
| A program digest bit | failed, `program` | before any step |
| A trace digest bit, with rows | failed, `rows` | before any step |
| A trace digest bit, digests only | failed, `trace` | after the whole replay |
| An output digest bit | failed, `outputs` | after the whole replay |
| One row's `ip`, `code`, `x` or `y`, rehashed | failed, `row` | its step and field |
| A row out of place before a window | failed, `structure` | its step |
| A branch's `y` changed to the other target | failed, `structure` | the next step, in a later window |
| One step fewer, rehashed | failed, `running` | the last claimed step |
| One `ret` row more, rehashed | failed, `halted` | the real `ret` |
| Rows the machine refuses, hashed honestly | failed, `refused` | the refused step |
| A step count of 0, or over 65,536 | failed, `steps` | before anything |

The Coax checks pin every row of this table.

## Where Part XXIII uses it

- **The mesh's re-execution checks** (chapters 26 and 193) will replay a
  provider's claim. A buyer who can afford the whole run gets `exact` or
  `failed`. A buyer who can't checks a few windows. Each window that passes
  is `unverifiable`, and any window that fails is proof.
- **The GPU kernels' acceptance** (chapters 43 and 44) needs the same
  verdicts for a compiled block run on a device: `exact` only when every
  step is replayed.

Neither chapter is built yet; this is the class they'll call.

## Supported profile

`replay-v1`:
- chapter 224's machine, at most 65,536 steps;
- the three digests above;
- full replay with or without rows, and windows with rows;
- a step budget for the verifier.

## How it is checked

- **Coax checks:** this chapter's 22 checks, with chapter 224's 97 that `replay.coax` loads, all 119 passing in debug and release. Every verdict and reason is covered, each tampered digest and
  row field, and each of the fold's shape checks at its boundary.
- **An independent oracle,**
  [replay_oracle.py](../tests/replay_oracle.py), has its own machine, digests
  (`hashlib`), fold and replay. It rebuilds the example's eleven scenarios
  and prints the same lines, byte for byte.
- **Mutation testing** of `replay.coax`: 1,343 of 1,343 mutants killed.

The [verification record](../../docs/verification/psionic-ch173-2026-10-30.md)
has the commands and results.

## What this does not do yet

- **Windows are chosen by the caller.** Choosing them at random, after the
  claim is fixed, and the odds that a given number of windows catches a
  given lie, are the mesh chapters' work.
- **A folded state isn't committed.** A claim could carry a digest of the
  machine every so many steps. Then a window would start from a committed
  state instead of trusting every row before it.
- **One machine.** The class is written for chapter 224's interpreter, not
  yet for a model's forward pass.
