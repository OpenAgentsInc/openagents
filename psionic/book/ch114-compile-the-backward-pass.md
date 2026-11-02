# 114. Compile the backward pass

> **Status: partly implemented.** A tensor program and a gradient mode
> compile to one straight-line training program on the CPU: forward
> commands, backward rules, recomputed values and a plain gradient-descent
> update, with every address fixed ahead of time. Compiled and eager
> training agree bit for bit. Fusing backward operations, symbolic shapes
> from chapter 39, the AdamW update of chapter 124 and measured step time
> are not here yet.

The later `cnn-nchw-f32-v1` extension is supported by the eager tape, not this
compiled-backward profile. `cb_compile` explicitly returns `operation` for
convolution, including a forward-only request. It cannot silently run a
convolution with a missing compiled derivative.

[Chapter 106](ch106-differentiate-any-tensor-program.md) differentiates a
tensor program with a tape. The tape is already planned ahead: it decides
which values to save, places them in an arena and fixes the order of the
backward rules. But it stays an interpreter. Each step looks up the node it
runs, rebuilds the kernel metadata, finds every operand through the
program, and hands registered operators back to the caller. Training also
needs a second piece, [chapter 117](ch117-models-are-compositions-of-modules.md)'s
`mdl_sgd`, to read the gradients and move the parameters.

This chapter compiles the tape into a program of its own:

- **One instruction list.** Forward commands, gradient clears, the output's
  seed, backward rules and parameter updates, in the order they run.
- **Every address resolved.** Each instruction carries chapter 38's kernel
  metadata or explicit references to the values and gradients it reads and
  writes. Running the program needs no plan, no program and no tape.
- **A saved-activation plan.** Each value lives from the instruction that
  makes it to the last instruction that reads it. A value that a backward
  rule reads is saved across the gap between the passes.
- **Recomputation.** A cheap elementwise value can be made again in the
  backward pass instead of being saved. A chooser picks what to recompute
  to meet a memory goal in floats, or refuses the goal.
- **The update compiled in.** With updates on, one run is a whole training
  step: forward, backward and every parameter moved by gradient descent.

The profile is `compiled-backward-f32-v1`: chapter 106's arithmetic. Forward
values are f32 from chapter 38's command kernel; gradients add up in f64 in
exactly the tape's order and are rounded once to f32 when published or
applied. Recomputation runs the same kernel on the same bits. So the
compiled program's gradients and updates are the tape's, bit for bit, with
or without recomputation. The limits are chapter 38's 32 nodes and 192
instructions. The code is [`compiled-backward.coax`](../lib/compiled-backward.coax),
and the [example](../examples/compiled-backward.coax) prints the output
below.

```shell
coax/bin/coax run psionic/examples/compiled-backward.coax
coax/bin/coax test psionic/lib/compiled-backward.coax
```

```text
hand-planned network: ok, 18 instructions, 10 launches
  forward 3; forward 4; forward 5; forward 6; forward 7; clear 7; seed 7; clear 6; backward 7;
  clear 5; backward 6; clear 4; clear 2; backward 5; clear 3; backward 4; clear 1; backward 3;
saved for the backward pass: 3 4
values arena 29 floats (the tape's: 29), gradient arena 33 doubles
recomputing h: 19 instructions, values arena 28 floats
  forward 3; forward 4; forward 5; forward 6; forward 7; clear 7; seed 7; clear 6; backward 7;
  clear 5; backward 6; recompute 4; clear 4; clear 2; backward 5; clear 3; backward 4; clear 1;
  backward 3;

chapter 117's block: 27 nodes, 84 parameters
  tape: 30 steps, values 144 floats, gradients 148 doubles
  compiled: 57 instructions, 30 launches, 6 updates, values 144 floats, gradients 140 doubles, 1696 bytes
  saved: 2 6 10 13 23
  goal 127 floats: ok, 34 launches, values 113 floats, 1572 bytes
  recomputed: 4 6 11 13
  saved: 2 10 23
  goal 112 floats: goal, the best plan found holds 113 floats

training, learning rate 0.05, dropout refreshed each step:
  step 0: loss 5.965327 (tape), 5.965327 (compiled), 5.965327 (recomputing); losses and parameters bit for bit: equal
  step 1: loss 3.124088 (tape), 3.124088 (compiled), 3.124088 (recomputing); losses and parameters bit for bit: equal
  step 2: loss 2.803049 (tape), 2.803049 (compiled), 2.803049 (recomputing); losses and parameters bit for bit: equal
  step 3: loss 2.791902 (tape), 2.791902 (compiled), 2.791902 (recomputing); losses and parameters bit for bit: equal
  step 4: loss 2.53129 (tape), 2.53129 (compiled), 2.53129 (recomputing); losses and parameters bit for bit: equal

refusals:
  registered operator with no VJP: operation
  registered operator with a VJP (the caller's kernel): external
  recomputing a matrix product: choice
  a values goal of 27 floats for the network: goal
```

In short:

- **The hand-planned network compiles to 18 instructions.** Ten of them
  launch work; the rest clear and seed gradients. Its values arena is 29
  floats, the same plan chapter 106's tape makes.
- **Recomputation lowers memory without changing a bit.** On chapter 117's
  residual block, recomputing four values cuts the values arena from 144
  floats to 113 for four more launches (30 to 34), and five training steps
  give the same losses and parameters, bit for bit, as the tape with
  `mdl_sgd`.
- **Goals and programs the compiler cannot meet are refused.** A goal below
  the best plan found returns `goal`; a registered operator without a VJP
  returns `operation`; one with a VJP returns `external`, because a compiled
  program runs every kernel itself.

## Plan a network by hand

Take the smallest network with a saved activation. `X` is a 4x2 batch of
data, `W1` is 2x3 and `W2` is 3x1; both weights want a gradient.

```text
node 3  z    = X @ W1      4x3, 12 floats
node 4  h    = relu(z)     4x3, 12 floats
node 5  y    = h @ W2      4x1,  4 floats
node 6  s    = sum(y, 0)   1,    1 float
node 7  loss = sum(s, 0)   scalar, 1 float
```

**What each rule reads.** Chapter 106's `ad_reads` table says: the sums
read nothing; `y = h @ W2` reads `h` for W2's gradient and `W2` for h's;
`relu` reads `z`; `z = X @ W1` reads only `X`, because `X` wants no
gradient. `X` and `W2` are inputs, always present. So `z` and `h` are saved.

**The schedule.** Five forward commands come first, instructions 0 to 4.
Then the output's gradient is cleared and seeded with ones (5 and 6). Each
backward rule runs in reverse node order, and a gradient is cleared just
before its first contribution:

```text
 7 clear s     8 backward loss
 9 clear y    10 backward s
11 clear h    12 clear W2     13 backward y
14 clear z    15 backward h
16 clear W1   17 backward z
```

**Lifetimes**, as inclusive instruction ranges: `z` is made at 0 and last
read by h's rule at 15, so it lives 0 to 15. `h` lives 1 to 13 (y's rule).
`y` lives 2 to 3 (s's forward command), `s` 3 to 4, and the loss 4 to the
end, 18.

**Placement.** Chapter 106's first fit puts each value at the lowest offset
that overlaps no value alive at the same time. `z` goes at 0 and `h` at 12.
`y` goes at 24. `s` is alive with `z`, `h` and `y` at instruction 3, so it
goes at 28. The loss starts at 4, when `y` is dead, so it takes 24. The
arena ends at 28 + 1 = **29 floats**.

**Recomputing h.** Mark `h` for recomputation. Its forward copy now dies at
2, after y's forward command reads it. A recompute instruction is inserted
just before the first backward reader, y's rule: it becomes instruction 11,
and the copy lives 11 to 14. `z` must now stay alive for the recompute at
11 and for h's rule, which moves to 16. Placement changes: `h` still goes at
12, `y` at 24, but `s` (3 to 4) now fits at 12, where `h` was, and the loss
at 13. The copy (11 to 14) overlaps `z` (0 to 12) and the loss (13 to 14),
so it goes at 14 and ends at 26. `y` at 24 still ends at 28, so the arena is
**28 floats**: one float saved. In a network this small, every value is
alive at the forward peak anyway. The residual block below has more room.

The first check asserts every number in this section.

## The instructions

<!-- source: psionic/lib/compiled-backward.coax types -->
```jai
CbStatus :: enum u8 {ok;program;operation;external;mode;choice;goal;capacity;number;unused;state;}
// forward runs one node's command; recompute runs it again in the backward
// phase into a fresh buffer; clear zeroes a gradient; seed adds the output's
// ones; backward applies one node's rule; update moves one parameter.
CbKind :: enum u8 {forward;recompute;clear;seed;backward;update;}
// Where an operand lives: the caller's inputs or an arena, read through a view.
CbRef :: struct {input:bool;base:int;view:TvView;}
CbInstr :: struct {
    kind:CbKind;node:int;op:TiOp;items:int;meta:[64]int;epsilon:f32;
    shape:TvShape;axis:int;indices:[16]int;dy:int;a:CbRef;b:CbRef;ga:CbRef;gb:CbRef;need_a:bool;need_b:bool;
}
// Which values to recompute instead of saving, and whether to append the update.
CbChoice :: struct {recompute:[32]bool;update:bool;}
// What the compiler keeps from chapter 106's tape, by node: each node's
// root and view, whether it wants a gradient, whether the output reaches it,
// and its input buffer offset when it is an input.
CbFacts :: struct {
    root:[32]int;input:[32]bool;input_base:[32]int;views:[32]TvView;storage:[32]int;
    wants:[32]bool;reaches:[32]bool;
}
CbProgram :: struct {
    status:CbStatus=CbStatus.state;program:TiProgram;mode:AdMode;choice:CbChoice;facts:CbFacts;input_elements:int;
    order:[32]int;rules:int;
    instrs:[CB_INSTRS]CbInstr;count:int;launches:int;updates:int;flag_items:int;
    saved:[32]bool;value_at:[32]int;copy_at:[32]int;grad_at:[32]int;
    born:[32]int;dies:[32]int;copy_born:[32]int;copy_dies:[32]int;grad_born:[32]int;grad_dies:[32]int;
    values:AdArena;grads:AdArena;
}
```

A `CbRef` is everything a rule needs to read or write one operand: whether
it lives in your input buffer or an arena, the base offset of its root, and
its view. Views and reshapes cost nothing: a reference to a view is its
root's base with the view's strides and offset.

## From the tape to a schedule

The compiler does not decide again what chapter 106 already decided. It
builds the tape, refuses what the tape refuses, and keeps only facts: each
node's root and view, whether it wants a gradient, whether the output
reaches it, and the order of the backward rules.

<!-- source: psionic/lib/compiled-backward.coax compile -->
```jai
// Chapter 106's tape decides which nodes want a gradient, which rules run
// and in what order; the compiler keeps those facts and drops the tape.
cb_compile :: (p:TiProgram,mode:AdMode,registry:OrRegistry,choice:CbChoice)->CbProgram {
    c:=CbProgram.{status=CbStatus.ok,program=p,mode=mode,choice=choice};
    t:=ad_tape(p,mode,registry);
    if t.status==AdStatus.program {return cb_fail(c,CbStatus.program);}
    if t.status==AdStatus.operation {return cb_fail(c,CbStatus.operation);}
    if t.status!=AdStatus.ok {return cb_fail(c,CbStatus.mode);}
    c.input_elements=t.plan.input_elements;
    for i:0..<p.count {
        // Chapter 106 admitted it; a compiled program also runs every kernel itself.
        if p.nodes[i].op==TiOp.registered || mode.custom[i] {return cb_fail(c,CbStatus.external);}
        // Convolution has an interpreted formal VJP, but no compiled-backward
        // instruction rule yet. Refuse even forward-only compilation explicitly.
        if p.nodes[i].op==TiOp.conv2d {return cb_fail(c,CbStatus.operation);}
        r:=t.root[i];c.facts.root[i]=r;c.facts.input[i]=p.nodes[r].op==TiOp.input;c.facts.input_base[i]=p.nodes[i].input_base;
        c.facts.views[i]=t.plan.values[i].view;c.facts.storage[i]=t.plan.values[i].storage;
        c.facts.wants[i]=t.wants[i];c.facts.reaches[i]=t.reaches[i];
    }
    for s:t.forward..<t.count {c.order[c.rules]=t.steps[s].node;c.rules+=1;}
    for i:0..<32 {if choice.recompute[i] && (i>=p.count || !cb_recomputable(p.nodes[i],i,c.facts.root[p.output])) {return cb_fail(c,CbStatus.choice);}}
    c=cb_schedule(c);c=cb_lifetimes(c);c=cb_place(c);return cb_resolve(c);
}
```

A registered operator is refused even when it has a VJP. Chapter 106 hands
such a node back to the caller, who runs its kernel. A compiled program has
no caller in the middle of a run, so it admits only built-in operators. The
same goes for a custom derivative.

<!-- source: psionic/lib/compiled-backward.coax schedule -->
```jai
// The instructions, in order: every forward command; then, if any gradient
// is wanted, the output's seed and each backward rule in reverse node order,
// with each gradient cleared just before its first contribution and each
// recomputed value made just before its first backward reader; then the
// updates.
cb_schedule :: (program:CbProgram)->CbProgram {
    c:=program;p:=c.program;f:=c.facts;made:[32]bool;cleared:[32]bool;
    for i:1..<p.count {if cb_material(p.nodes[i].op) {c.instrs[c.count]=cb_instr(CbKind.forward,i);c.count+=1;}}
    if c.rules>0 {
        // No rule reads the output's root, so it is cleared only here.
        out:=f.root[p.output];
        c.instrs[c.count]=cb_instr(CbKind.clear,out);c.instrs[c.count+1]=cb_instr(CbKind.seed,p.output);c.count+=2;
        for s:0..<c.rules {
            i:=c.order[s];n:=p.nodes[i];binary:=ti_binary(n.op);
            reads:=ad_reads(n,false,f.wants[n.a],binary && f.wants[n.b]);
            for o:0..<2 {
                if !reads[o] {continue;}
                x:=ifx o==0 then n.a else n.b;r:=f.root[x];
                if !f.input[x] && c.choice.recompute[r] && !made[r] {
                    from:=c.count;c=cb_make(c,r,made);for k:from..<c.count {made[c.instrs[k].node]=true;}
                }
            }
            for o:0..<2 {
                if o==1 && !binary {continue;}
                x:=ifx o==0 then n.a else n.b;r:=f.root[x];
                if f.wants[x] && !cleared[r] {c.instrs[c.count]=cb_instr(CbKind.clear,r);c.count+=1;cleared[r]=true;}
            }
            c.instrs[c.count]=cb_instr(CbKind.backward,i);c.count+=1;
        }
        if c.choice.update {
            for i:0..<p.count {if p.nodes[i].op==TiOp.input && c.mode.grad[i] && f.reaches[i] {c.instrs[c.count]=cb_instr(CbKind.update,i);c.count+=1;c.updates+=1;}}
        }
    }
    return c;
}
```

## Lifetimes and placement

<!-- source: psionic/lib/compiled-backward.coax lifetimes -->
```jai
// Lifetimes are instruction indices, inclusive. A value lives from its
// forward command to its last reader. A recomputed value's forward copy
// stops at its last forward reader, and the copy covers the readers after
// the forward pass. A value read after the forward pass from its original
// is saved. A copy is made only for a reader, so its readers end it.
cb_lifetimes :: (program:CbProgram)->CbProgram {
    c:=program;p:=c.program;f:=c.facts;
    for i:0..<32 {c.born[i]=-1;c.copy_born[i]=-1;c.grad_born[i]=-1;}
    for k:0..<c.count {
        ins:=c.instrs[k];i:=ins.node;n:=p.nodes[i];binary:=ti_binary(n.op);reads:=[2]bool.[true,binary];
        if ins.kind==CbKind.forward {c.born[i]=k;c.dies[i]=k;}
        else if ins.kind==CbKind.recompute {c.copy_born[i]=k;}
        else if ins.kind==CbKind.backward {reads=ad_reads(n,false,f.wants[n.a],binary && f.wants[n.b]);}
        if ins.kind==CbKind.forward || ins.kind==CbKind.recompute || ins.kind==CbKind.backward {
            for o:0..<2 {
                if !reads[o] {continue;}
                x:=ifx o==0 then n.a else n.b;r:=f.root[x];
                if f.input[x] {continue;}
                late:=ins.kind!=CbKind.forward;
                if late && c.choice.recompute[r] {c.copy_dies[r]=k;}
                else {c.dies[r]=k;if late {c.saved[r]=true;}}
            }
        }
        // A gradient is born at its clear. Every contribution comes before
        // its own rule, which reads it last; an input has no rule, and its
        // gradient lives to the end for the update and for publishing.
        if ins.kind==CbKind.clear {c.grad_born[i]=k;c.grad_dies[i]=c.count;}
        if ins.kind==CbKind.backward {c.grad_dies[i]=k;}
    }
    c.dies[f.root[p.output]]=c.count;
    return c;
}
```

Values and recomputed copies are then placed in instruction order with
chapter 106's `ad_place`, and gradients in the order they are cleared. An
input's gradient lives to the end of the program, because the update reads
it and you may publish it.

<!-- source: psionic/lib/compiled-backward.coax resolve -->
```jai
// Fill each instruction's addresses from the placement. A forward or
// recomputing command gets chapter 38's kernel metadata; a rule gets its
// operands' values and gradients as references.
cb_resolve :: (program:CbProgram)->CbProgram {
    c:=program;p:=c.program;f:=c.facts;
    // Where the backward phase reads each value: the copy when it is recomputed.
    // Node 0 can only be an input.
    late:=c.value_at;for i:1..<32 {if c.choice.recompute[i] {late[i]=c.copy_at[i];}}
    for k:0..<c.count {
        ins:=c.instrs[k];i:=ins.node;n:=p.nodes[i];binary:=ti_binary(n.op);
        ins.items=ad_count(f.views[i].shape);ins.epsilon=n.epsilon;
        if ins.kind==CbKind.forward || ins.kind==CbKind.recompute {
            s:=f.views[i].shape;a:=f.views[n.a].shape;m:=ins.meta;
            m[0]=int(cast(u8)n.op);m[1]=ifx ins.kind==CbKind.forward then c.value_at[i] else c.copy_at[i];m[3]=s.rank;
            for j:0..<4 {m[4+j]=s.dims[j];}
            from:=ifx ins.kind==CbKind.forward then c.value_at else late;
            cb_describe(cb_value_ref(f,from,n.a),8,m[..]);if binary {cb_describe(cb_value_ref(f,from,n.b),19,m[..]);}
            if n.op==TiOp.matmul {m[30]=a.dims[1];} else if n.op==TiOp.sum {m[30]=a.dims[n.axis];} else if n.op==TiOp.layernorm {m[30]=a.dims[a.rank-1];}
            m[31]=n.axis;m[48]=n.index_count;for j:0..<16 {m[32+j]=n.indices[j];}
            ins.meta=m;c.flag_items=cb_larger(c.flag_items,ins.items);
        } else if ins.kind==CbKind.clear {ins.ga=cb_grad_ref(f,c.grad_at,i);}
        else if ins.kind==CbKind.seed {ins.ga=cb_grad_ref(f,c.grad_at,i);}
        else if ins.kind==CbKind.backward {
            ins.op=n.op;ins.axis=n.axis;ins.indices=n.indices;ins.shape=f.views[i].shape;
            ins.dy=c.grad_at[i];ins.need_a=f.wants[n.a];ins.need_b=binary && f.wants[n.b];
            ins.a=cb_value_ref(f,late,n.a);ins.ga=cb_grad_ref(f,c.grad_at,n.a);
            if binary {ins.b=cb_value_ref(f,late,n.b);ins.gb=cb_grad_ref(f,c.grad_at,n.b);}
        } else {ins.a=cb_value_ref(f,c.value_at,i);ins.ga=cb_grad_ref(f,c.grad_at,i);}
        c.instrs[k]=ins;
        if ins.kind==CbKind.forward || ins.kind==CbKind.recompute || ins.kind==CbKind.backward {c.launches+=1;}
    }
    return c;
}
```

## Recomputation and a memory goal

<!-- source: psionic/lib/compiled-backward.coax recomputable -->
```jai
// A value may be recomputed when it is a cheap elementwise node and not the
// output. Its operands come from inputs, from saved values, or from other
// recomputed values, which are made first.
cb_recomputable :: (n:TiNode,i:int,output_root:int)->bool {
    return (n.op==TiOp.relu || n.op==TiOp.add || n.op==TiOp.multiply) && i!=output_root;
}
```

Only ReLU, add and multiply are recomputed: one pass over their output,
reading values that are already there. A matrix product or a layer norm is
never recomputed. When a recomputed value reads another recomputed value,
the compiler makes the operand's copy first.

Recomputing one value alone often helps nothing. In chapter 117's block,
`drop = relu(fc1) * mask` is saved for fc2's weight gradient. Recomputing
`drop` alone needs `relu(fc1)` in the backward pass, so that value would be
saved instead, at the same size. The chooser therefore tries chains: a
value together with every cheap ancestor that nothing else saves, back to
saved values and inputs.

<!-- source: psionic/lib/compiled-backward.coax choose -->
```jai
// Recomputing i alone would keep its operands alive instead, so a candidate
// is i with every cheap ancestor that nothing else saves: the chain back to
// saved values and inputs.
cb_chain :: (c:CbProgram,i:int)->CbChoice {
    next:=c.choice;next.recompute[i]=true;
    for k:0..<i {
        j:=i-k;n:=c.program.nodes[j];
        if !next.recompute[j] || c.choice.recompute[j] {continue;}
        for o:0..<2 {
            if o==1 && !ti_binary(n.op) {continue;}
            x:=ifx o==0 then n.a else n.b;q:=c.facts.root[x];
            if !c.facts.input[x] && !c.saved[q] && cb_recomputable(c.program.nodes[q],q,c.facts.root[c.program.output]) {next.recompute[q]=true;}
        }
    }
    return next;
}
// Meet a values-arena goal in floats: while the plan is over it, add the
// chain that lowers the arena most (the lowest node on a tie).
cb_choose :: (p:TiProgram,mode:AdMode,registry:OrRegistry,goal:int,update:bool)->CbProgram {
    c:=cb_compile(p,mode,registry,CbChoice.{update=update});
    while c.status==CbStatus.ok && c.values.top>goal {
        best:=c;
        for i:1..<p.count {
            if c.choice.recompute[i] || !cb_recomputable(p.nodes[i],i,c.facts.root[p.output]) {continue;}
            d:=cb_compile(p,mode,registry,cb_chain(c,i));
            if d.status==CbStatus.ok && d.values.top<best.values.top {best=d;}
        }
        if best.values.top==c.values.top {return cb_fail(c,CbStatus.goal);}
        c=best;
    }
    return c;
}
```

For the block, the first chain is the norm's affine output, `norm * weight
+ bias` (nodes 4 and 6), recomputed from the saved layer-norm output: 144
floats down to 128. The second is the dropped hidden layer (nodes 11 and
13), recomputed from the saved pre-activation `fc1`: down to 113. No
further chain lowers the arena, so a goal of 112 is refused with the best
plan's size.

## Running the program

<!-- source: psionic/lib/compiled-backward.coax rule -->
```jai
// One node's vector-Jacobian product, from resolved references alone. The
// loops and the order of every f64 addition are chapter 106's.
cb_rule :: (ins:CbInstr,inputs:[]f32,values:[]f32,grads:[]f64) {
    op:=ins.op;axis:=ins.axis;s:=ins.shape;a:=ins.a.view.shape;b:=ins.b.view.shape;
    for j:0..<ins.items {
        g:=grads[ins.dy+j];c:=to_coords(s,j);
        if op==TiOp.add || op==TiOp.multiply {
            ja:=to_broadcast_index(a,s,j);jb:=to_broadcast_index(b,s,j);ga:=g;gb:=g;
            if op==TiOp.multiply {
                if ins.need_a {ga=g*cb_read(ins.b,jb,inputs,values);}
                if ins.need_b {gb=g*cb_read(ins.a,ja,inputs,values);}
            }
            if ins.need_a {cb_add(ins.ga,ja,ga,grads);}
            if ins.need_b {cb_add(ins.gb,jb,gb,grads);}
        } else if op==TiOp.matmul {
            for k:0..<a.dims[1] {
                ja:=c[0]*a.dims[1]+k;jb:=k*b.dims[1]+c[1];
                if ins.need_a {cb_add(ins.ga,ja,g*cb_read(ins.b,jb,inputs,values),grads);}
                if ins.need_b {cb_add(ins.gb,jb,g*cb_read(ins.a,ja,inputs,values),grads);}
            }
        } else if op==TiOp.relu {
            if cb_read(ins.a,j,inputs,values)>0 {cb_add(ins.ga,j,g,grads);}
        } else if op==TiOp.sum {
            point:[4]int;cursor:=0;
            for d:0..<a.rank {if d!=axis {point[d]=c[cursor];cursor+=1;}}
            for k:0..<a.dims[axis] {point[axis]=k;cb_add(ins.ga,to_flat(a,point),g,grads);}
        } else if op==TiOp.gather {
            point:=c;point[axis]=ins.indices[c[axis]];cb_add(ins.ga,to_flat(a,point),g,grads);
        }
    }
    if op==TiOp.layernorm {
        width:=a.dims[a.rank-1];
        for row:0..<ad_count(a)/width {
            base:=row*width;mean:f64=0;
            for k:0..<width {mean+=cb_read(ins.a,base+k,inputs,values);}
            mean/=f64(width);variance:f64=0;
            for k:0..<width {d:=cb_read(ins.a,base+k,inputs,values)-mean;variance+=d*d;}
            sigma:=sqrt(variance/f64(width)+f64(ins.epsilon));m1:f64=0;m2:f64=0;
            for k:0..<width {g:=grads[ins.dy+base+k];m1+=g;m2+=g*(cb_read(ins.a,base+k,inputs,values)-mean)/sigma;}
            m1/=f64(width);m2/=f64(width);
            for k:0..<width {
                xhat:=(cb_read(ins.a,base+k,inputs,values)-mean)/sigma;
                cb_add(ins.ga,base+k,(grads[ins.dy+base+k]-m1-xhat*m2)/sigma,grads);
            }
        }
    }
}
```

This is chapter 106's `ad_rule` with every lookup replaced by a reference
fixed at compile time. The loops, and so the order of every f64 addition,
are the same. One difference: a product reads the other factor only when
that factor's gradient is wanted, so a rule never reads a value that was
not saved.

<!-- source: psionic/lib/compiled-backward.coax run -->
```jai
// Run every instruction. Updates come last: every parameter's gradient is
// checked before any parameter moves, then each moves by rate times its
// gradient rounded to f32, exactly as chapter 117's mdl_sgd does.
cb_run :: (c:CbProgram,rate:f32,inputs:[]f32,values:[]f32,grads:[]f64,flags:[]u8)->CbStatus {
    if c.status!=CbStatus.ok || c.count<1 {return CbStatus.state;}
    if inputs.count<c.input_elements || values.count<c.values.top || grads.count<c.grads.top || flags.count<c.flag_items {return CbStatus.capacity;}
    if c.updates>0 && (!(rate>0) || !tv_finite(rate)) {return CbStatus.number;}
    first:=c.count-c.updates;
    for k:0..<first {
        ins:=c.instrs[k];
        if ins.kind==CbKind.forward || ins.kind==CbKind.recompute {
            for item:0..<ins.items {tik_run(item,ins.meta[..],ins.epsilon,inputs,values,values,flags);}
            for item:0..<ins.items {if flags[item]!=0 {return CbStatus.number;}}
        } else if ins.kind==CbKind.clear {for j:0..<ins.items {grads[ins.ga.base+j]=0;}}
        else if ins.kind==CbKind.seed {for j:0..<ins.items {cb_add(ins.ga,j,1,grads);}}
        else {cb_rule(ins,inputs,values,grads);}
    }
    for k:first..<c.count {ins:=c.instrs[k];for j:0..<ins.items {if !to_fits(grads[ins.ga.base+tv_offset(ins.ga.view,j)]) {return CbStatus.number;}}}
    for k:first..<c.count {
        ins:=c.instrs[k];
        for j:0..<ins.items {
            g:=f32(grads[ins.ga.base+tv_offset(ins.ga.view,j)]);at:=ins.a.base+tv_offset(ins.a.view,j);
            inputs[at]=f32(f64(inputs[at])-f64(rate)*f64(g));
        }
    }
    return CbStatus.ok;
}
```

<!-- source: psionic/lib/compiled-backward.coax publish -->
```jai
// The output's value and an input's gradient, after a run. A program with
// updates has already moved its parameters, but not its gradients.
cb_output :: (c:CbProgram,inputs:[]f32,values:[]f32,out:[]f32)->CbStatus {
    if c.status!=CbStatus.ok {return CbStatus.state;}
    r:=cb_value_ref(c.facts,c.value_at,c.program.output);
    for j:0..<ad_count(r.view.shape) {out[j]=f32(cb_read(r,j,inputs,values));}
    return CbStatus.ok;
}
cb_gradient :: (c:CbProgram,x:int,grads:[]f64,out:[]f32)->CbStatus {
    if c.status!=CbStatus.ok || x<0 || x>=c.program.count || c.program.nodes[x].op!=TiOp.input || !c.mode.grad[x] {return CbStatus.state;}
    if !c.mode.enabled {return CbStatus.mode;}
    if !c.facts.reaches[x] {return CbStatus.unused;}
    r:=cb_grad_ref(c.facts,c.grad_at,x);
    for j:0..<ad_count(r.view.shape) {g:=grads[r.base+tv_offset(r.view,j)];if !to_fits(g) {return CbStatus.number;}out[j]=f32(g);}
    return CbStatus.ok;
}
```

## The example

<!-- source: psionic/examples/compiled-backward.coax example -->
```jai
main :: () {
    // The two-layer network planned by hand in the chapter.
    p:=cb_test_mlp();mode:=cb_test_mode(1,2,-1);
    c:=cb_compile(p,mode,OrRegistry.{},CbChoice.{});t:=ad_tape(p,mode,OrRegistry.{});
    print("hand-planned network: %, % instructions, % launches\n ",c.status,c.count,c.launches);show(c);
    list("saved for the backward pass",c.saved,p.count);
    print("values arena % floats (the tape's: %), gradient arena % doubles\n",c.values.top,t.values.top,c.grads.top);
    h:=CbChoice.{};h.recompute[4]=true;d:=cb_compile(p,mode,OrRegistry.{},h);
    print("recomputing h: % instructions, values arena % floats\n ",d.count,d.values.top);show(d);

    // Chapter 117's residual block, one training step per run.
    m:=mdl_test_block(128);bmode:=mdl_mode(m);
    plain:=cb_compile(m.program,bmode,OrRegistry.{},CbChoice.{update=true});
    small:=cb_choose(m.program,bmode,OrRegistry.{},127,true);
    tight:=cb_choose(m.program,bmode,OrRegistry.{},112,true);
    tape:=ad_tape(m.program,bmode,OrRegistry.{});
    print("\nchapter 117's block: % nodes, % parameters\n",m.program.count,mdl_parameters(m));
    print("  tape: % steps, values % floats, gradients % doubles\n",tape.count,tape.values.top,tape.grads.top);
    print("  compiled: % instructions, % launches, % updates, values % floats, gradients % doubles, % bytes\n",
        plain.count,plain.launches,plain.updates,plain.values.top,plain.grads.top,4*plain.values.top+8*plain.grads.top);
    list("  saved",plain.saved,m.program.count);
    print("  goal 127 floats: %, % launches, values % floats, % bytes\n",small.status,small.launches,small.values.top,4*small.values.top+8*small.grads.top);
    list("  recomputed",small.choice.recompute,m.program.count);
    list("  saved",small.saved,m.program.count);
    print("  goal 112 floats: %, the best plan found holds % floats\n",tight.status,tight.values.top);

    // Five steps three ways from the same start: the tape with mdl_sgd,
    // the compiled program, and the compiled program that recomputes.
    eager:[160]f32;if mdl_init(m,7,eager[..])!=MdlStatus.ok {print("no init\n");return;}
    for j:0..<16 {eager[j]=mdl_test_value(1,j);eager[16+j]=mdl_test_value(2,j);}
    one:=eager;two:=eager;
    tv:[1024]f32;tg:[1024]f64;g:[84]f32;values:[512]f32;grads:[512]f64;flags:[64]u8;
    print("\ntraining, learning rate 0.05, dropout refreshed each step:\n");
    for step:0..<5 {
        s:=cast(u64)step;
        if mdl_refresh(m,s,eager[..])!=MdlStatus.ok || mdl_refresh(m,s,one[..])!=MdlStatus.ok || mdl_refresh(m,s,two[..])!=MdlStatus.ok {print("no refresh\n");return;}
        run:=ad_run(tape,AdCursor.{},eager[..],tv[..],tg[..],flags[..]);l:[1]f32;
        if run.status!=AdStatus.ok || ad_value(tape,run.cursor,26,eager[..],tv[..],l[..])!=AdStatus.ok {print("tape failed\n");return;}
        if mdl_sgd(m,tape,run.cursor,tg[..],g[..],eager[..],0.05)!=MdlStatus.ok {print("no update\n");return;}
        a:=cb_run(plain,0.05,one[..],values[..],grads[..],flags[..]);la:[1]f32;
        if a!=CbStatus.ok || cb_output(plain,one[..],values[..],la[..])!=CbStatus.ok {print("compiled: %\n",a);return;}
        b:=cb_run(small,0.05,two[..],values[..],grads[..],flags[..]);lb:[1]f32;
        if b!=CbStatus.ok || cb_output(small,two[..],values[..],lb[..])!=CbStatus.ok {print("recomputing: %\n",b);return;}
        same:=f32_bits(l[0])==f32_bits(la[0]) && f32_bits(l[0])==f32_bits(lb[0]);
        for j:0..<160 {if f32_bits(eager[j])!=f32_bits(one[j]) || f32_bits(eager[j])!=f32_bits(two[j]) {same=false;}}
        print("  step %: loss % (tape), % (compiled), % (recomputing); losses and parameters bit for bit: %\n",step,r6(f64(l[0])),r6(f64(la[0])),r6(f64(lb[0])),ifx same then "equal" else "DIFFERENT");
    }

    // Programs the compiler refuses.
    q:=p;q.nodes[4]=or_node(0,3,3,false,0.5);registry:=OrRegistry.{count=1};registry.schemas[0].backward=false;
    print("\nrefusals:\n  registered operator with no VJP: %\n",cb_compile(q,mode,registry,CbChoice.{}).status);
    registry.schemas[0].backward=true;
    print("  registered operator with a VJP (the caller's kernel): %\n",cb_compile(q,mode,registry,CbChoice.{}).status);
    bad:=CbChoice.{};bad.recompute[5]=true;
    print("  recomputing a matrix product: %\n",cb_compile(p,mode,OrRegistry.{},bad).status);
    print("  a values goal of 27 floats for the network: %\n",cb_choose(p,mode,OrRegistry.{},27,false).status);
}
```

## How it is checked

- **The hand plan.** The network above: every instruction's kind and node,
  the saved values, every lifetime, every value and gradient offset, the
  29-float arena (equal to the tape's), and the 28-float arena with `h`
  recomputed.
- **Two more hand plans.** A chain `q = relu(x)`, `a = relu(q)` read by
  two products, with a dead `relu` beside them: every instruction,
  lifetime and offset, plain (16 floats) and with `q` and `a` recomputed
  (12 floats, both copies made once, in node order, before the first
  reader). Recomputing `a` alone saves `q` instead and stays at 16;
  recomputing a value no rule reads makes nothing. The chooser takes the
  chain `q`, `a`. In a pair of equal branches it breaks the tie on the
  lower node. Each forward command's metadata equals the tape's own
  `ad_meta`, here and for chapter 106's program with views and a gather.
- **The smallest programs.** One rule (`sum(x)` with its update), no rule
  (one forward command), and an input as the output (no instruction, which
  `cb_run` refuses with `state`).
- **Against the tape, bit for bit.** The hand-planned network with two
  gradient modes and with recomputation; chapter 106's program with a
  strided view, a reshape, a sum, a layer norm, a gather and a product,
  with and without recomputing the add that feeds the layer norm; and
  chapter 117's block over five training steps with dropout refreshed each
  step, plain and recomputing, against `ad_run` and `mdl_sgd`: same loss,
  same parameters.
- **Against central differences.** The network's nine weight gradients
  match central differences of a loss written out by hand in f64, within
  1e-5.
- **Refusals.** A registered operator without a VJP (`operation`), with
  one (`external`), a custom derivative (`external`), a bad dependency
  (`program`), a detached input (`mode`), every non-recomputable node
  (`choice`), and an unmeetable goal (`goal`). At run time: short input,
  arena, gradient and flag buffers (`capacity`), a zero or infinite rate
  and an overflowing forward value (`number`), and a gradient too large for
  f32, `d/da (a * b) * c` with `a = 1e-30` and `b = c = 1e30`, refused
  before any parameter moves.

## What is not here yet

- **Fusion.** Each backward rule is its own instruction. Merging rules
  whose buffers do not overlap is not done.
- **Symbolic shapes.** A compiled program is for one set of shapes;
  chapter 39's guarded specialization is not used.
- **AdamW.** The compiled update is plain gradient descent, matching
  `mdl_sgd`. [Chapter 124](ch124-adamw-and-explicit-precision.md)'s AdamW
  is not compiled in.
- **Step time.** Memory is counted exactly in floats and bytes; step time
  is not measured. Launch counts stand in for the extra work of
  recomputation.
- **Registered operators.** A compiled program cannot call your kernel.

## Sources

- [Chapter plan](PLAN.md)
- [Chapter 106](ch106-differentiate-any-tensor-program.md): the tape, its
  rules and its first-fit arenas.
- [Chapter 117](ch117-models-are-compositions-of-modules.md): the residual
  block and `mdl_sgd`.
- Chen, Xu, Zhang and Guestrin, [Training Deep Nets with Sublinear Memory
  Cost](https://arxiv.org/abs/1604.06174) (2016): recomputation to trade
  work for memory.
