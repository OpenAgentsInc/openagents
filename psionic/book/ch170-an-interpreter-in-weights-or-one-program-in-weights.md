# 170. An interpreter in weights, or one program in weights

[Chapter 168](ch168-compile-a-gate-graph-into-a-block.md) compiles a gate
graph into a block of weights whose forward pass reproduces it exactly. This
chapter compiles an interpreter, in two ways:
- **Universal:** the interpreter, compiled once. The program is part of its
  input, so the same weights run any program.
- **One program:** the same interpreter with one program's instructions
  folded in before compiling. This is the first Futamura projection:
  specializing an interpreter to a program gives a compiled program. Its
  block grows with the program's length.

Both run beside the native interpreter, and every run must agree three ways.

```shell
coax/bin/coax run psionic/examples/interpreter-weights.coax
coax/bin/coax test psionic/lib/interpreter-weights.coax
python3 -I psionic/tests/interpreter_oracle.py
```

```jai
universal interpreter, any program of 1 to 5 instructions: 110 nodes, 7 layers, width 37, 25086 weights
one program of 1 instructions: 4 nodes, 0 layers, width 4, 0 weights
one program of 2 instructions: 8 nodes, 2 layers, width 4, 156 weights
one program of 3 instructions: 16 nodes, 3 layers, width 7, 567 weights
one program of 4 instructions: 23 nodes, 4 layers, width 8, 1008 weights
one program of 5 instructions: 33 nodes, 6 layers, width 10, 2070 weights
the triangle program, [addr, addi -1, jnz 0, halt]: 26 nodes, 5 layers, width 9, 1458 weights
triangle from 100: native 300 steps, r 5050; universal block 300 steps, r 5050; own block 300 steps, r 5050
200 seeded programs agree three ways: 941 steps, 183 halted, 17 out of budget; checksum 880508489
```

The [example](../examples/interpreter-weights.coax) prints each block's size,
runs the triangle program from 100 all three ways, then runs 200 seeded
programs of every length three ways and asserts that every run agrees.

## The machine

<!-- source: psionic/lib/interpreter-weights.coax machine -->
```jai
// A program has 1 to 5 instructions and ends with halt; every jump lands
// inside it, so the instruction pointer never leaves the program.
iw_admit :: (p:[]IwIns)->IwStatus {
    if p.count<1 || p.count>IW_SLOTS {return IwStatus.shape;}
    for i:0..<p.count {
        if p[i].arg< -IW_VALUE_MAX || p[i].arg>IW_VALUE_MAX {return IwStatus.range;}
        if p[i].op==IwOp.jnz && (p[i].arg<0 || p[i].arg>=p.count) {return IwStatus.target;}
    }
    if p[p.count-1].op!=IwOp.halt {return IwStatus.halt;}
    return IwStatus.ok;
}
iw_in_range :: (s:IwState)->bool {
    return s.acc>= -IW_VALUE_MAX && s.acc<=IW_VALUE_MAX && s.r>= -IW_VALUE_MAX && s.r<=IW_VALUE_MAX;
}
// The native interpreter: one step of an admitted program.
iw_step :: (p:[]IwIns,s:IwState)->IwNext {
    x:=IwNext.{state=s};ins:=p[s.ip];x.state.ip=s.ip+1;
    if ins.op==IwOp.addi {x.state.acc=s.acc+ins.arg;}
    if ins.op==IwOp.addr {x.state.r=s.r+s.acc;}
    if ins.op==IwOp.jnz && s.acc!=0 {x.state.ip=ins.arg;}
    if ins.op==IwOp.halt {x.state.ip=s.ip;}
    x.halted=x.state.ip==s.ip;
    if !iw_in_range(x.state) {x.status=IwStatus.range;}
    return x;
}
iw_run :: (p:[]IwIns,s:IwState,budget:int)->IwRun {
    r:=IwRun.{state=s};
    a:=iw_admit(p);if a!=IwStatus.ok {r.status=a;return r;}
    if !iw_in_range(s) || s.ip<0 || s.ip>=p.count {r.status=IwStatus.shape;return r;}
    for i:0..<budget {
        x:=iw_step(p,r.state);
        if x.status!=IwStatus.ok {r.status=x.status;return r;}
        if x.halted {return r;}
        r.state=x.state;r.steps+=1;
    }
    r.status=IwStatus.budget;return r;
}
```

The machine is small on purpose. Chapter 168's blocks take at most 16
inputs and 128 nodes, and the universal interpreter needs the state and the
whole program as inputs. Chapter 165's stack machine doesn't fit. This one
has three registers (`ip`, `acc` and `r`), four instructions, and programs
of up to five instructions. It can still loop: the triangle program sums
1 to n by counting `acc` down.

Its values stay within 2²⁴, a quarter of chapter 167's exact range. The
interpreter computes every slot's effect before choosing one, so a sum the
current instruction doesn't make is still computed, and must stay in range
too.

## The universal interpreter

<!-- source: psionic/lib/interpreter-weights.coax universal -->
```jai
iw_put :: (g:[]GgNode,n:int,e:GgNode)->int {g[n]=e;return n+1;}
// One step for any program of up to `slots` instructions (1 to 5). Inputs:
// ip, acc, r, then each slot's op and arg. Each slot computes its own
// instruction's effect, so a slot whose instruction is known folds to just
// that effect; the slot the ip names is then chosen. Within a slot the ip is
// known, so the next ip is a constant or the jump target. An op is 0 to 3,
// so halt needs no comparison of its own. Outputs: ip, acc, r.
iw_universal :: (g:[]GgNode,outs:[]int,slots:int)->int {
    n:=0;IN:=GgOp.input;SUB:=GgOp.sub;ADD:=GgOp.add;EQ:=GgOp.eq0;SEL:=GgOp.select;
    for k:0..<3+2*slots {n=iw_put(g,n,GgNode.{op=IN,k=k});}
    // Constants 0 to 5: the ops compared against and every slot's next ip.
    C:=n;for j:0..<IW_SLOTS+1 {n=iw_put(g,n,GgNode.{op=GgOp.constant,k=j});}
    z:=n;n=iw_put(g,n,GgNode.{op=EQ,a=1});
    at:[IW_SLOTS]int;acc:[IW_SLOTS]int;reg:[IW_SLOTS]int;ip:[IW_SLOTS]int;
    // The last slot is chosen when no other is.
    for i:0..<slots-1 {d:=n;n=iw_put(g,n,GgNode.{op=SUB,b=C+i});at[i]=n;n=iw_put(g,n,GgNode.{op=EQ,a=d});}
    for i:0..<slots {
        op:=3+2*i;arg:=4+2*i;is:[3]int;
        for k:0..<3 {d:=n;n=iw_put(g,n,GgNode.{op=SUB,a=op,b=C+k});is[k]=n;n=iw_put(g,n,GgNode.{op=EQ,a=d});}
        v:=n;n=iw_put(g,n,GgNode.{op=SEL,a=arg,b=C,c=is[0]});acc[i]=n;n=iw_put(g,n,GgNode.{op=ADD,a=1,b=v});
        v=n;n=iw_put(g,n,GgNode.{op=SEL,a=1,b=C,c=is[1]});reg[i]=n;n=iw_put(g,n,GgNode.{op=ADD,a=2,b=v});
        jump:=n;n=iw_put(g,n,GgNode.{op=SEL,a=C+i+1,b=arg,c=z});
        // addi and addr move on, halt stays: i plus their two bits. A jnz
        // takes jump instead.
        v=n;n=iw_put(g,n,GgNode.{op=ADD,a=is[0],b=is[1]});
        stay:=n;n=iw_put(g,n,GgNode.{op=ADD,a=C+i,b=v});
        ip[i]=n;n=iw_put(g,n,GgNode.{op=SEL,a=jump,b=stay,c=is[2]});
    }
    chosen:=[3]int.[ip[slots-1],acc[slots-1],reg[slots-1]];
    for j:0..<slots-1 {
        i:=slots-2-j;
        v:=n;n=iw_put(g,n,GgNode.{op=SEL,a=ip[i],b=chosen[0],c=at[i]});chosen[0]=v;
        v=n;n=iw_put(g,n,GgNode.{op=SEL,a=acc[i],b=chosen[1],c=at[i]});chosen[1]=v;
        v=n;n=iw_put(g,n,GgNode.{op=SEL,a=reg[i],b=chosen[2],c=at[i]});chosen[2]=v;
    }
    for k:0..<3 {outs[k]=chosen[k];}
    return n;
}
// The universal block's inputs for a state and a program, padded with halts.
iw_inputs :: (p:[]IwIns,s:IwState,inputs:[]int) {
    inputs[0]=s.ip;inputs[1]=s.acc;inputs[2]=s.r;
    for i:0..<IW_SLOTS {
        inputs[3+2*i]=cast(int)IwOp.halt;inputs[4+2*i]=0;
        if i<p.count {inputs[3+2*i]=cast(int)p[i].op;inputs[4+2*i]=p[i].arg;}
    }
}
```

Decoding only the current instruction would be smaller: choose the op and
argument the `ip` names, then decode that op once. But a specializer could
fold almost nothing out of it, because the chosen op depends on the `ip`,
which is never known in advance. So each slot decodes its own instruction,
and a slot is chosen afterwards. Once the instructions are known, each slot
folds down to the one thing its instruction does. This is the classic
**binding-time improvement**: writing an interpreter so that a specializer
can use what it is given.

A program shorter than five instructions is padded with halts. Since a
program ends in halt and every jump lands inside it, the padding is never
reached.

## The specializer

<!-- source: psionic/lib/interpreter-weights.coax specialize -->
```jai
// A partial evaluator over gate graphs. Inputs with known[k] take value[k];
// every node whose operands are all known becomes a constant, a select with
// a known condition becomes the operand it picks, and adding zero or scaling
// by one disappears. The remaining inputs keep their order. Nodes no output
// reads are then removed, except inputs, so the block's interface is fixed
// by which inputs were left unknown.
IwFold :: struct {known:bool;value:int;node:int;}
iw_const :: (t:[]GgNode,n:int,v:int)->int {
    for i:0..<n {if t[i].op==GgOp.constant && t[i].k==v {return i;}}
    return -1;
}
iw_ref :: (t:[]GgNode,n:int,f:IwFold)->int {
    if !f.known {return f.node;}
    return iw_const(t,n,f.value);
}
iw_specialize :: (g:[]GgNode,outs:[]int,known:[]bool,value:[]int,r:[]GgNode,routs:[]int)->IwSpec {
    s:IwSpec;a:=gg_admit(g,outs);
    if a.status!=GgStatus.ok {s.status=a.status;s.node=a.node;return s;}
    t:[256]GgNode;n:=0;f:[128]IwFold;
    for i:0..<g.count {
        e:=g[i];x:=f[e.a];y:=f[e.b];c:=f[e.c];v:=IwFold.{known=true};emit:=false;
        if e.op==GgOp.input {
            if e.k>=known.count || e.k>=value.count {s.status=GgStatus.capacity;return s;}
            v.known=known[e.k];v.value=value[e.k];
            if !v.known {v.node=n;t[n]=GgNode.{op=GgOp.input,k=s.inputs};n+=1;s.inputs+=1;}
        }
        if e.op==GgOp.constant {v.value=e.k;}
        if e.op==GgOp.add {
            if x.known && y.known {v.value=x.value+y.value;}
            else if x.known && x.value==0 {v=y;}
            else if y.known && y.value==0 {v=x;}
            else {emit=true;}
        }
        if e.op==GgOp.sub {
            if x.known && y.known {v.value=x.value-y.value;}
            else if y.known && y.value==0 {v=x;}
            else {emit=true;}
        }
        if e.op==GgOp.scale {
            if x.known {v.value=x.value*e.k;}
            else if e.k==1 {v=x;}
            else {emit=true;}
        }
        if e.op==GgOp.mul {
            if x.known && y.known {v.value=x.value*y.value;}
            else if x.known || y.known {
                k:=ifx x.known then x.value else y.value;o:=ifx x.known then y else x;
                // Times zero is the default: known, 0.
                if k==1 {v=o;} else if k!=0 {e=GgNode.{op=GgOp.scale,k=k};x=o;emit=true;}
            }
            else {emit=true;}
        }
        if e.op==GgOp.relu || e.op==GgOp.step || e.op==GgOp.eq0 {
            if x.known {
                if e.op==GgOp.relu && x.value>=1 {v.value=x.value;}
                if e.op==GgOp.step && x.value>=1 {v.value=1;}
                if e.op==GgOp.eq0 && x.value==0 {v.value=1;}
            }
            else {emit=true;}
        }
        if e.op==GgOp.select {
            if c.known {v=ifx c.value==1 then x else y;}
            else if x.known && y.known && x.value==y.value {v=x;}
            else {emit=true;}
        }
        if v.known && (v.value< -RA_VALUE_MAX || v.value>RA_VALUE_MAX) {s.status=GgStatus.range;s.node=i;return s;}
        if emit {
            // Known operands become constants; each is placed once, before its first reader.
            ops:=[3]IwFold.[x,y,c];refs:[3]int;
            for j:0..<gg_arity(e.op) {
                refs[j]=iw_ref(t[..],n,ops[j]);
                if refs[j]<0 {refs[j]=n;t[n]=GgNode.{op=GgOp.constant,k=ops[j].value};n+=1;}
            }
            t[n]=GgNode.{op=e.op,a=refs[0],b=refs[1],c=refs[2],k=e.k};v=IwFold.{node=n};n+=1;
        }
        f[i]=v;
    }
    // The outputs, then the nodes they read.
    o:[16]int;
    for j:0..<outs.count {
        o[j]=iw_ref(t[..],n,f[outs[j]]);
        if o[j]<0 {o[j]=n;t[n]=GgNode.{op=GgOp.constant,k=f[outs[j]].value};n+=1;}
    }
    live:[256]bool;
    for j:0..<outs.count {live[o[j]]=true;}
    for q:0..<n {
        i:=n-1-q;
        if t[i].op==GgOp.input {live[i]=true;}
        if live[i] {
            k:=gg_arity(t[i].op);
            if k>=1 {live[t[i].a]=true;}
            if k>=2 {live[t[i].b]=true;}
            if k>=3 {live[t[i].c]=true;}
        }
    }
    at:[256]int;
    for i:0..<n {
        if !live[i] {continue;}
        if s.count==r.count {s.status=GgStatus.capacity;return s;}
        e:=t[i];e.a=at[e.a];e.b=at[e.b];e.c=at[e.c];r[s.count]=e;at[i]=s.count;s.count+=1;
    }
    for j:0..<outs.count {routs[j]=at[o[j]];}
    return s;
}
```

The partial evaluator knows nothing about the machine. It folds any gate
graph whose inputs are partly known:
- a node whose operands are all known becomes a constant;
- a select with a known condition becomes the operand it picks;
- adding zero, subtracting zero, scaling by one and multiplying by one
  disappear, and multiplying by zero is a constant;
- a product with one known operand becomes a scale;
- nodes no output reads are removed, but inputs are kept, so the block's
  interface doesn't depend on what folded away.

A program's block comes from the interpreter for exactly that program's
length, so a three-instruction program has three slots, not five. The first
version specialized the five-slot interpreter for every program. The padding
halts kept every slot, and blocks of one to five instructions all came out
between 27 and 33 nodes.

## Running the weights

<!-- source: psionic/lib/interpreter-weights.coax weights -->
```jai
// One step through compiled weights: the block's outputs are the next state.
// The machine's range is checked here, as the native step checks it.
iw_weights_step :: (plan:GgPlan,w:[]f64,inputs:[]int,s:IwState)->IwNext {
    x:=IwNext.{state=s};out:[3]f64;
    if gg_run(plan,w,inputs,out[..],false,false)!=GgStatus.ok {x.status=IwStatus.graph;return x;}
    x.state=IwState.{ip=cast(int)out[0],acc=cast(int)out[1],r=cast(int)out[2]};
    x.halted=x.state.ip==s.ip;
    if !iw_in_range(x.state) {x.status=IwStatus.range;}
    return x;
}
// Runs a program through the universal block (specialized false) or its own
// block, until it halts or the budget runs out.
iw_weights_run :: (plan:GgPlan,w:[]f64,p:[]IwIns,s:IwState,budget:int,specialized:bool)->IwRun {
    r:=IwRun.{state=s};
    a:=iw_admit(p);if a!=IwStatus.ok {r.status=a;return r;}
    if !iw_in_range(s) || s.ip<0 || s.ip>=p.count {r.status=IwStatus.shape;return r;}
    inputs:[IW_INPUTS]int;
    for i:0..<budget {
        iw_inputs(p,r.state,inputs[..]);n:=ifx specialized then 3 else IW_INPUTS;
        x:=iw_weights_step(plan,w,inputs[0..<n],r.state);
        if x.status!=IwStatus.ok {r.status=x.status;return r;}
        if x.halted {return r;}
        r.state=x.state;r.steps+=1;
    }
    r.status=IwStatus.budget;return r;
}
```

Each step is one forward pass, and its outputs are the next state. A step
that leaves the `ip` where it was halts the machine: either a halt, or a
jump to itself, which changes nothing and would repeat forever. The native
interpreter uses the same rule.

## Sizes and speed

| Path | Nodes | Layers | Width | Weights |
| --- | --- | --- | --- | --- |
| Universal, any program up to 5 instructions | 110 | 7 | 37 | 25,086 |
| One program, 1 instruction | 4 | 0 | 4 | 0 |
| One program, 2 instructions | 8 | 2 | 4 | 156 |
| One program, 3 instructions | 16 | 3 | 7 | 567 |
| One program, 4 instructions | 23 | 4 | 8 | 1,008 |
| One program, 5 instructions | 33 | 6 | 10 | 2,070 |
| The triangle program (4 instructions) | 26 | 5 | 9 | 1,458 |

A single halt compiles to no layers at all: the next state is the input,
wired straight through.

Steps per second, for the triangle program from 5,000 (15,000 steps a run).
Each path is a release build timed from outside, with one warm-up and five
repeats:

| Path | Steps per second | Standard deviation |
| --- | --- | --- |
| Native interpreter | 614 million | 13 million |
| The triangle program's own block | 505,000 | 4,700 |
| Universal block | 33,700 | 2,200 |

These are binary64 forward passes on the CPU, on an Apple M5 Max that other
work was sharing (load average about 50), so they are an upper bound on the
cost. The ratios are what matter here:
- The program's own block is about 17 times smaller than the universal block
  and about 15 times faster.
- The native interpreter is about 1,200 times faster than the fastest block.

A block pays for every weight on every step, even when only one instruction
runs. That cost is the point: the weights do the whole step in parallel,
fixed arithmetic, the shape a GPU runs well, and they compose with other
blocks as chapter 176 shows.

## How it is checked

- **Coax checks:** the chapter's 20 checks, with the 25 from chapters 167 and 168 that `interpreter-weights.coax` loads, all 45 passing in debug and release. They cover:
  - admission, each instruction and the range at its edges, natively;
  - the universal step against the native step, for every op in every slot
    and for seeded programs of every length;
  - each folding rule of the specializer, its refusals, dead-node removal and
    shared constants;
  - each program's block against the universal graph, its evaluator and its
    compiled weights;
  - runs through weights against native runs, including budget, range and
    jumps to self;
  - a step at the edge of the range in binary64, where binary32 rounds wrong.
- **The example** runs 200 seeded programs of every length three ways
  (native, the universal block and each program's own block) and asserts
  they agree on every run.
- **An independent oracle,** [interpreter_oracle.py](../tests/interpreter_oracle.py),
  implements the machine and the seeded generator itself. It derives the
  triangle line, the seeded runs' totals and checksum, and the universal
  graph's node count, and finds each in the example's output.
- **Mutation testing** of `interpreter-weights.coax`: 1,311 of 1,311 mutants killed.

The [verification record](../../docs/verification/psionic-ch170-2026-10-30.md)
has the commands and results.

## What this does not do yet

- **Chapter 165's machine.** Its stack and memory need far more inputs and
  nodes than chapter 168's blocks allow. Chapter 166's exact memory,
  read by attention, is the way to hold them.
- **GPU timings.** The plan asks for steps per second from running weights;
  these are CPU forward passes. A device run of the same blocks belongs with
  chapter 29's fused replay.
- **The second projection.** Specializing the specializer to the interpreter
  would give a compiler from programs to blocks. Here the specializer is
  general and runs each time.
