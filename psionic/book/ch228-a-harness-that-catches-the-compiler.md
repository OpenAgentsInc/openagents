# 228. A harness that catches the compiler

[Chapter 227](ch227-compile-a-gate-graph-into-a-block.md)'s compiler passes
its checks. That shows it works on the cases someone thought of. It doesn't
show that a broken compiler would be noticed. This chapter builds a harness
that runs every program three independent ways, plants four kinds of compiler
bug on purpose, and requires each one to be caught and placed in the part of
the compiler that caused it.

```shell
coax/bin/coax run psionic/examples/catch-the-compiler.coax
psionic/tests/check_gates.sh
```

```jai
the honest compiler: 99590 graphs over 100 seeds, 0 disagreements
reuse a slot a layer early: caught in 100 of 100 seeds by graph 6 at the latest; schedule 100, emission 0
run a gate in its operand's layer: caught in 100 of 100 seeds by graph 4 at the latest; schedule 100, emission 0
never clear a reused slot: caught in 100 of 100 seeds by graph 5 at the latest; schedule 100, emission 0
clear with the wrong sign: caught in 100 of 100 seeds by graph 9 at the latest; schedule 0, emission 100
first catch for a missing clear, seed 1: graph 0, node 4, in the schedule
```

The [example](../examples/catch-the-compiler.coax) runs about 100,000
programs in two seconds. It also found a real bug, described below.

## Three ways to run a program

1. **The evaluator** (chapter 227) says what the program means, in integers.
2. **The plan interpreter** executes the compiler's own schedule in integers.
   It reads the residual slot by slot as each layer starts, adds each gate's
   value to the gate's slot, and clears what the plan says to clear. Gates
   compute chapter 226's identities, which is what the weights mean, so a
   select on a stale condition of 5 gives `b + 5(a − b)` here exactly as in
   the block.
3. **The compiled weights**, run in binary64.

When the evaluator and the interpreter disagree, the schedule or slot plan is
wrong. When the interpreter and the weights disagree, the emitted weights are.

<!-- source: psionic/lib/compiler-harness.coax interpret -->
```jai
// Execute the compiler's own schedule with integers: slot 0 holds 1, inputs
// sit in their slots, and in each layer every gate reads the residual as it
// was at the start of the layer, adds its value to its slot, and every slot the
// plan clears there loses its start-of-layer value. Gates compute chapter 226's
// identities, which is what the weights mean, not the graph's ideal meaning:
// a select on a stale condition of 5 gives b + 5(a - b) here as in the block,
// and the evaluator alone decides what was intended. A node's reported value is
// what its gate wrote, or its linear form over the values already reported;
// the outputs are read from the final residual, as the block reads them.
ch_interpret :: (plan:GgPlan,nodes:[]GgNode,inputs:[]int,values:[]int,read:[]int)->GgResult #export_c {
    if plan.status!=GgStatus.ok {return GgResult.{status=plan.status};}
    if nodes.count!=plan.nodes {return GgResult.{status=GgStatus.shape};}
    if inputs.count!=plan.inputs || values.count<nodes.count || read.count<plan.outputs {return GgResult.{status=GgStatus.capacity};}
    forms:[128]RaForm;v:[128]int;s:[64]int;s[0]=1;
    for i:0..<nodes.count {forms[i]=gg_form_of(nodes,forms[..],i);}
    for i:0..<nodes.count {if nodes[i].op==GgOp.input {s[plan.slot[i]]=inputs[nodes[i].k];}}
    for layer_index:0..<plan.layers {
        L:=layer_index+1;start:=s;
        // Node 0 is always an input, so gates start at node 1.
        for i:1..<nodes.count {
            n:=nodes[i];
            if gg_gate(n.op) && plan.layer[i]==L {
                a:=ch_form(forms[n.a],plan,start[..]);x:int;
                if n.op==GgOp.mul {x=a*ch_form(forms[n.b],plan,start[..]);}
                if n.op==GgOp.relu {x=ch_relu(a);}
                if n.op==GgOp.step {x=ch_relu(a)-ch_relu(a-1);}
                if n.op==GgOp.eq0 {x=1-(ch_relu(a)-ch_relu(a-1))-(ch_relu(-a)-ch_relu(-a-1));}
                if n.op==GgOp.select {b:=ch_form(forms[n.b],plan,start[..]);x=b+(a-b)*ch_relu(ch_form(forms[n.c],plan,start[..]));}
                s[plan.slot[i]]+=x;v[i]=x;
            }
        }
        for i:0..<nodes.count {if plan.clear[i]==L {s[plan.slot[i]]-=start[plan.slot[i]];}}
    }
    for i:0..<nodes.count {
        n:=nodes[i];
        if n.op==GgOp.input {v[i]=inputs[n.k];}
        if n.op==GgOp.constant {v[i]=n.k;}
        if n.op==GgOp.add {v[i]=v[n.a]+v[n.b];}
        if n.op==GgOp.sub {v[i]=v[n.a]-v[n.b];}
        if n.op==GgOp.scale {v[i]=v[n.a]*n.k;}
        values[i]=v[i];
    }
    for k:0..<plan.outputs {
        f:=plan.out[k];sum:=0;
        for t:0..<f.count {sum+=f.coefs[t]*s[f.slots[t]];}
        read[k]=sum;
    }
    return GgResult.{};
}
ch_relu :: (x:int)->int {return (x+cast(int)abs(f64(x)))/2;}
// A form over base nodes, read through the plan's slots. Constants are k times slot 0.
ch_form :: (f:RaForm,plan:GgPlan,s:[]int)->int {
    sum:=0;
    for t:0..<f.count {sum+=f.coefs[t]*s[plan.slot[f.slots[t]]];}
    return sum;
}
```

## Watching without changing what you watch

The first version made every node an output, so that any disagreement could
name its node. Every planted reuse bug then went uncaught. Outputs are kept
alive, so a program with every node as an output never reuses a slot, and
three of the four bugs never happened.

The harness now has the evaluator and the interpreter report every node
directly, without making it an output, and compares the weights only on each
program's two real outputs. The interpreter also reads those outputs from its
final residual, as the block does, so a stale value hiding in an output's slot
still counts against the schedule, not the weights.

<!-- source: psionic/lib/compiler-harness.coax run -->
```jai
// One graph, three ways. The evaluator and the plan interpreter both report
// every node, so a schedule bug names its first node; the block is compared on
// the graph's real outputs only. Making every node an output would keep every
// slot alive, and no slot would ever be reused: the probe would hide the bugs.
ch_run :: (nodes:[]GgNode,outputs:[]int,inputs:[]int,fault:GgFault)->ChRun #export_c {
    if nodes.count<1 || nodes.count>CH_GRAPH_NODES {return ChRun.{status=GgStatus.shape};}
    every:[16]int;for i:1..<nodes.count {every[i]=i;}  // every[0] is already 0
    want:[16]int;
    e:=gg_eval(nodes,inputs,every[0..<nodes.count],want[..]);if e.status!=GgStatus.ok {return ChRun.{status=e.status,node=e.node};}
    weights:[16384]f64;plan:=gg_compile_planted(nodes,outputs,weights[..],fault);
    if plan.status!=GgStatus.ok {return ChRun.{status=plan.status,node=plan.node};}
    planned:[128]int;read:[16]int;
    #assert ch_interpret(plan,nodes,inputs,planned[..],read[..]).status==GgStatus.ok;
    first:=ch_first_difference(planned[0..<nodes.count],want[0..<nodes.count]);
    if first!=-1 {return ChRun.{where=ChWhere.schedule,node=first};}
    for k:0..<outputs.count {if read[k]!=want[outputs[k]] {return ChRun.{where=ChWhere.schedule,node=outputs[k]};}}
    got:[16]f64;
    #assert gg_run(plan,weights[..],inputs,got[..],false,false)==GgStatus.ok;
    for k:0..<outputs.count {if got[k]!=f64(read[k]) {return ChRun.{where=ChWhere.emission,node=outputs[k]};}}
    return ChRun.{};
}
// The first index where two equally long lists differ, or -1.
ch_first_difference :: (a:[]int,b:[]int)->int {
    for i:0..<a.count {if a[i]!=b[i] {return i;}}
    return -1;
}
```

## Programs from a seed

Each case is a graph of up to 16 nodes, its inputs from −100 to 100, and two
outputs, all from a seed and an index. A catch can then be replayed exactly.
The generator is part of the harness's contract. An independent Python copy,
written from this description, reproduces 900 cases exactly and predicts which
cases are skipped because a value leaves the range or a form grows too long.
Skipped cases are counted and never taken as agreement.

<!-- source: psionic/lib/compiler-harness.coax generate -->
```jai
// A seeded graph of at most 16 nodes: one to three inputs, then gates and
// linear nodes over earlier nodes, with selects only on earlier bits.
ch_next :: (x:int)->int {return (x*1103515245+12345)%2147483648;}
ch_generate :: (seed:int,graph:int,nodes:[]GgNode)->int #export_c {
    x:=ch_next((seed%65536)*65537+graph%65536);
    x=ch_next(x);inputs:=1+x%3;count:=inputs;
    for i:0..<inputs {nodes[i]=GgNode.{op=GgOp.input,k=i};}
    x=ch_next(x);total:=inputs+4+x%(CH_GRAPH_NODES-inputs-3);
    bits:[16]int;nbits:=0;
    while count<total {
        x=ch_next(x);pick:=x%12;
        x=ch_next(x);a:=x%count;x=ch_next(x);b:=x%count;
        n:=GgNode.{op=GgOp.relu,a=a};
        if pick<2 {n=GgNode.{op=GgOp.mul,a=a,b=b};}
        else if pick<4 {n=GgNode.{op=GgOp.step,a=a};}
        else if pick<5 {n=GgNode.{op=GgOp.eq0,a=a};}
        else if pick<7 && nbits>0 {x=ch_next(x);n=GgNode.{op=GgOp.select,a=a,b=b,c=bits[x%nbits]};}
        else if pick<8 {n=GgNode.{op=GgOp.sub,a=a,b=b};}
        else if pick<9 {n=GgNode.{op=GgOp.constant,k=x%7-3};}
        nodes[count]=n;
        if n.op==GgOp.step || n.op==GgOp.eq0 {bits[nbits]=count;nbits+=1;}
        count+=1;
    }
    return count;
}
```

<!-- source: psionic/lib/compiler-harness.coax differential -->
```jai
// A whole case from a seed and an index: the graph, inputs from -100 to 100,
// and two outputs (one chosen node and the last). Reproducible, so any catch
// can be replayed.
ch_case :: (seed:int,graph:int,nodes:[]GgNode)->ChCase #export_c {
    c:ChCase;c.count=ch_generate(seed,graph,nodes);
    x:=ch_next(seed*7919+graph);
    for i:0..<c.count {if nodes[i].op==GgOp.input {x=ch_next(x);c.values[c.inputs]=x%201-100;c.inputs+=1;}}
    x=ch_next(x);c.outputs[0]=x%(c.count-1);c.outputs[1]=c.count-1;
    return c;
}
ch_count_ok :: (count:int)->bool {return count>=1 && count<=CH_RUNS_MAX;}
// Run `count` cases with the given planted fault and stop at the first one
// where the three paths disagree. Cases that cannot be compared (a value out
// of range, a form too long) are skipped and counted, never taken as agreement.
ch_differential :: (seed:int,count:int,fault:GgFault)->ChCatch #export_c {
    if !ch_count_ok(count) {return ChCatch.{status=GgStatus.shape};}
    result:ChCatch;nodes:[16]GgNode;
    for g:0..<count {
        c:=ch_case(seed,g,nodes[..]);
        r:=ch_run(nodes[0..<c.count],c.outputs[..],c.values[0..<c.inputs],fault);
        if r.status!=GgStatus.ok {result.skipped+=1;}
        else {
            result.graphs+=1;
            if r.where!=ChWhere.none {result.caught=true;result.graph=g;result.node=r.node;result.where=r.where;return result;}
        }
    }
    return result;
}
```

## The planted bugs

The compiler takes a fault argument that production code never passes:

| Fault | What it does | Where the harness places it |
| --- | --- | --- |
| Early reuse | hands a slot to a new gate one layer before its last reader | schedule |
| Same layer | runs a gate in its deepest operand's own layer, before that value exists | schedule |
| No clear | reuses a slot without subtracting its old value | schedule |
| Clear sign | emits the clearing unit with `+1` instead of `−1`, so the plan is right but the weights aren't | emission |

They follow the two bugs the pre-reset harness found:
- **Same-step writes:** same-step writes resolved in schedule order instead of
  program order, which is early reuse and same-layer here.
- **Cumulative sums:** cumulative sums reordered across layers. Here that is a
  missing or wrong-signed clear, which corrupts what accumulates in a slot.

Across 100 seeds, every fault is caught in every seed within its first ten
cases, always attributed to the right part. Each catch is checked to be the
first disagreeing case: replaying the cases before it agrees.

## The bug it found

At 1,000 cases per seed, the honest compiler failed an internal assertion on
seed 3, graph 694. A select computes `a − b`, and its operands' forms had
grown, because forms never merged like terms: `x − x` stayed two terms. The
select's form needed nine terms, the gadget couldn't build it, and the
compiler had already accepted the graph. Chapter 227 now merges like terms
and checks each gadget's own forms before scheduling, refusing with `terms`.
The Python oracle predicts that refusal independently. The pinned cases
include seed 3, graph 694.

A second refinement came from attribution rather than a crash. One planted
bug in a hundred was briefly blamed on the weights: a stale select condition
gives different answers under the graph's ideal meaning and under the gadget's
arithmetic. The interpreter now computes the arithmetic, and the evaluator
alone decides what was intended.

## How it is checked

- **Coax checks:**
  - The honest compiler over 40 cases each for seeds 1 and 11, including a
    predicted skip.
  - Every fault caught and attributed, with the catch replayed.
  - The interpreter following a plan rather than the graph, including a
    tampered plan.
  - Linear nodes and wide products three ways; run bounds.
  - Fourteen generated cases pinned by the oracle, filled from garbage so a
    skipped write shows.
- **Chapter 227's own checks** exercise each planted fault directly, so that
  chapter's mutation run covers the fault code as well.
- **The native harness** [harness.c](../tests/harness.c):
  - The interpreter against the independent oracle's 292 graphs.
  - 900 generated cases against the Python generator.
  - 20 seeds of 1,000 cases with the predicted skips (81).
  - Every fault caught in every seed within 20 cases.
  - All of it plain and under ASan/UBSan, in debug and release.
- **Mutation testing** of `compiler-harness.coax`, with chapters 226 and 227
  loaded, kills all 995 mutants. Chapter 227's own file kills 729, and chapter
  226's 295.

The [verification record](../../docs/verification/psionic-ch169-2026-10-29.md)
has the commands and results.

## What this does not do yet

The planted bugs are ones we chose. The harness catches those and any bug
that changes a value on these cases. A bug that only appears beyond 16 nodes,
past the input range, or in binary32 would need cases that reach it.
[Chapter 229](PLAN.md) runs the whole machine as weights, and this harness is
how its compiler will be judged.
