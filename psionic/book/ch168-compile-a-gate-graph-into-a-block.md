# 168. Compile a gate graph into a block

[Chapter 167](ch167-arithmetic-and-branches-from-relu.md) built exact
gadgets one layer at a time. A program needs many of them, wired together and
spread across layers. This chapter is a compiler: a small language of gates,
an integer evaluator that defines what a program means, a scheduler that packs
gates into layers and reuses residual slots, and the weights it emits. The
emitted block's forward pass must reproduce the evaluator exactly.

```shell
coax/bin/coax run psionic/examples/gate-block.coax
psionic/tests/check_gates.sh
```

```jai
one arithmetic step: 3 layers, residual width 6, units 13 / 4 / 6, 414 weights
add 7 and -3: block 4.0 (evaluator 4)
subtract 7 and -3: block 10.0 (evaluator 10)
multiply 7 and -3: block -21.0 (evaluator -21)
multiply 4097 and 4097: binary64 16785409.0 (evaluator 16785409), the same weights in binary32 16785408.0
multiply 10000 and 10000: refused by the evaluator at node 9 (range)
```

The [example](../examples/gate-block.coax) compiles one arithmetic step of
[chapter 165's machine](ch165-a-machine-whose-state-is-a-trace.md): op 0 adds,
1 subtracts and 2 multiplies. It then runs the step as 414 weights in three
layers.

## The block, and why it isn't chapter 6's

A transformer block is attention and a feed-forward layer, each added to a
residual stream. [Chapter 6](ch006-attention-without-training.md)'s block also
has two parts that can't be exact:
- **RMSNorm** divides by a square root.
- **SwiGLU's SiLU** is a smooth curve, not a ReLU.

The exact profile keeps the residual stream and the gated feed-forward layer,
removes the normalization, uses chapter 167's ReLU gating, and computes in
binary64, as chapter 172 requires. These blocks have no attention sublayer yet;
chapter 170 adds the memory reads.

## The language

| Node | Meaning | Costs a layer? |
| --- | --- | --- |
| `input k` | the `k`-th input | no |
| `constant k` | `k` | no |
| `add a b`, `sub a b`, `scale a k` | linear combinations | no |
| `mul a b`, `relu a`, `step a`, `eq0 a` | chapter 167's gadgets | yes |
| `select c a b` | `c ? a : b`, where `c` must be a `step` or `eq0` | yes |

Operands are earlier nodes, so a graph is acyclic by construction. Admission
checks indices, arities, the select type rule, input numbering and constant
ranges before any work starts.

<!-- source: psionic/lib/gate-graph.coax admit -->
```jai
gg_arity :: (op:GgOp)->int {
    if op==GgOp.input || op==GgOp.constant {return 0;}
    if op==GgOp.add || op==GgOp.sub || op==GgOp.mul {return 2;}
    if op==GgOp.select {return 3;}
    return 1;
}
gg_bit :: (op:GgOp)->bool {return op==GgOp.step || op==GgOp.eq0;}
gg_admit :: (nodes:[]GgNode,outputs:[]int)->GgResult #export_c {
    if nodes.count<1 || nodes.count>GG_NODES_MAX || outputs.count<1 || outputs.count>GG_OUTPUTS_MAX {return GgResult.{status=GgStatus.shape};}
    inputs:=0;
    for i:0..<nodes.count {
        n:=nodes[i];
        if cast(int)n.op>cast(int)GgOp.constant {return GgResult.{status=GgStatus.operand,node=i};}
        arity:=gg_arity(n.op);
        if arity>=1 && (n.a<0 || n.a>=i) {return GgResult.{status=GgStatus.operand,node=i};}
        if arity>=2 && (n.b<0 || n.b>=i) {return GgResult.{status=GgStatus.operand,node=i};}
        if arity==3 && (n.c<0 || n.c>=i) {return GgResult.{status=GgStatus.operand,node=i};}
        if arity==3 && !gg_bit(nodes[n.c].op) {return GgResult.{status=GgStatus.type,node=i};}
        // Inputs are numbered 0, 1, 2, ... in the order they appear.
        if n.op==GgOp.input {
            if n.k!=inputs || inputs>=GG_INPUTS_MAX {return GgResult.{status=GgStatus.operand,node=i};}
            inputs+=1;
        }
        if (n.op==GgOp.scale || n.op==GgOp.constant) && (n.k< -RA_VALUE_MAX || n.k>RA_VALUE_MAX) {return GgResult.{status=GgStatus.range,node=i};}
    }
    for o:outputs {if o<0 || o>=nodes.count {return GgResult.{status=GgStatus.operand,node=o};}}
    return GgResult.{};
}
```

## What a program means

The evaluator is the reference. It works on integers and refuses the first
node whose value leaves ±2²⁶. That's the range in which chapter 167's gadgets
are exact in binary64, so a program the evaluator accepts is one the block can
run exactly.

<!-- source: psionic/lib/gate-graph.coax eval -->
```jai
// The reference: integers, every value within RA_VALUE_MAX, refused at the
// first node that leaves it.
gg_eval :: (nodes:[]GgNode,inputs:[]int,outputs:[]int,values:[]int)->GgResult #export_c {
    admitted:=gg_admit(nodes,outputs);if admitted.status!=GgStatus.ok {return admitted;}
    v:[128]int;count:=0;
    for n:nodes {if n.op==GgOp.input {count+=1;}}
    if inputs.count!=count || values.count<outputs.count {return GgResult.{status=GgStatus.capacity};}
    for i:0..<nodes.count {
        n:=nodes[i];x:=0;
        if n.op==GgOp.input {x=inputs[n.k];}
        if n.op==GgOp.add {x=v[n.a]+v[n.b];}
        if n.op==GgOp.sub {x=v[n.a]-v[n.b];}
        if n.op==GgOp.scale {x=v[n.a]*n.k;}
        if n.op==GgOp.mul {x=v[n.a]*v[n.b];}
        if n.op==GgOp.constant {x=n.k;}
        if n.op==GgOp.relu && v[n.a]>=1 {x=v[n.a];}
        if n.op==GgOp.step && v[n.a]>=1 {x=1;}
        if n.op==GgOp.eq0 && v[n.a]==0 {x=1;}
        if n.op==GgOp.select {x=v[n.b];if v[n.c]==1 {x=v[n.a];}}
        if x< -RA_VALUE_MAX || x>RA_VALUE_MAX {return GgResult.{status=GgStatus.range,node=i};}
        v[i]=x;
    }
    for i:0..<outputs.count {values[i]=v[outputs[i]];}
    return GgResult.{};
}
```

## Scheduling

Linear nodes cost nothing. Each node's value is a linear form over base nodes
(inputs, constants and gates), and the compiler folds linear nodes into the
forms that gates read. A gate runs one layer after its deepest operand. Every
gate in a layer reads the residual before any of them writes it, so gates in
the same layer can't see each other.

Each base node owns a residual slot: slot 0 holds 1 (constants are multiples
of it), inputs come next, and each gate gets a slot in its layer. The slot
stays live until the last gate that reads it. In that layer one extra unit
subtracts the value back to zero, and the slot can go straight to a gate
written in the same layer, because reads happen first. Outputs and constants
are never cleared. A chain of eight ReLUs runs in a residual of width 2.

<!-- source: psionic/lib/gate-graph.coax schedule -->
```jai
// A node's form is a sum over "base" nodes: inputs and nonlinear gates, each of
// which owns a residual slot. Linear nodes (add, sub, scale) cost no layer; a
// gate runs in the layer after its latest operand. A base node's slot is
// cleared in the layer of its last reader and may be handed to a gate written
// in that same layer, because every unit reads the residual before any unit
// writes it.
gg_linear :: (op:GgOp)->bool {return op==GgOp.add || op==GgOp.sub || op==GgOp.scale || op==GgOp.constant;}
gg_gate :: (op:GgOp)->bool {return op!=GgOp.input && !gg_linear(op);}
// The larger of two layers, without a comparison whose tie case is invisible.
gg_max :: (a:int,b:int)->int {return (a+b+cast(int)abs(f64(a-b)))/2;}
gg_form_of :: (nodes:[]GgNode,forms:[]RaForm,i:int)->RaForm {
    n:=nodes[i];
    if n.op==GgOp.add {return ra_plus(forms[n.a],forms[n.b]);}
    if n.op==GgOp.sub {return ra_plus(forms[n.a],ra_neg(forms[n.b]));}
    if n.op==GgOp.scale {f:=forms[n.a];for t:0..<f.count {f.coefs[t]*=n.k;}return f;}
    // A constant is its own base node, pinned to slot 0 (which holds 1), so its
    // form is k times that node.
    if n.op==GgOp.constant {return ra_term(i,n.k);}
    return ra_term(i,1);
}
gg_coef_sum :: (f:RaForm)->f64 {
    s:f64=0;
    for t:0..<f.count {s+=abs(f64(f.coefs[t]));}
    return s;
}
```

<!-- source: psionic/lib/gate-graph.coax compile -->
```jai
// Node 0 is always an input (it has no earlier node to read), so loops over
// gates start at node 1.
gg_layer_of :: (f:RaForm,layer:[]int)->int {
    deepest:=0;
    for t:0..<f.count {deepest=gg_max(deepest,layer[f.slots[t]]);}
    return deepest;
}
gg_operand :: (n:GgNode,which:int)->int {
    if which==1 {return n.b;}
    if which==2 {return n.c;}
    return n.a;
}
gg_compile :: (nodes:[]GgNode,outputs:[]int,weights:[]f64)->GgPlan #export_c {
    admitted:=gg_admit(nodes,outputs);
    if admitted.status!=GgStatus.ok {return GgPlan.{status=admitted.status,node=admitted.node};}
    forms:[128]RaForm;layer:[128]int;last:[128]int;slot:[128]int;kept:[128]bool;
    plan:GgPlan;plan.outputs=outputs.count;
    // Forms over base nodes, the coefficient bound, and layers: a gate runs one
    // layer after its deepest operand; a linear node sits at its deepest term.
    for i:0..<nodes.count {
        n:=nodes[i];forms[i]=gg_form_of(nodes,forms[..],i);
        if forms[i].count==-1 {return GgPlan.{status=GgStatus.terms,node=i};}
        if gg_coef_sum(forms[i])>f64(GG_COEF_SUM_MAX) {return GgPlan.{status=GgStatus.range,node=i};}
        if n.op==GgOp.input {plan.inputs+=1;}
        if gg_gate(n.op) {
            deepest:=0;
            for which:0..<gg_arity(n.op) {deepest=gg_max(deepest,gg_layer_of(forms[gg_operand(n,which)],layer[..]));}
            layer[i]=deepest+1;
            if layer[i]>GG_LAYERS_MAX {return GgPlan.{status=GgStatus.layers,node=i};}
            plan.layers=gg_max(plan.layers,layer[i]);
        }
        // Constants live in slot 0 and are never cleared.
        if n.op==GgOp.constant {kept[i]=true;}
    }
    // The last layer that reads each base node (0: never read by a gate);
    // outputs are read after the last layer, so their nodes are kept.
    for i:1..<nodes.count {
        n:=nodes[i];
        if gg_gate(n.op) {
            for which:0..<gg_arity(n.op) {
                f:=forms[gg_operand(n,which)];
                for t:0..<f.count {last[f.slots[t]]=gg_max(last[f.slots[t]],layer[i]);}
            }
        }
    }
    for o:outputs {f:=forms[o];for t:0..<f.count {kept[f.slots[t]]=true;}}
    // Slots: 0 holds the constant one, then inputs in order, then gates by layer.
    // A slot is freed in the layer of its node's last reader; 0 means none chosen.
    free:[64]bool;plan.width=1;
    for i:0..<nodes.count {if nodes[i].op==GgOp.input {slot[i]=plan.width;plan.width+=1;plan.values+=1;}}
    for layer_index:0..<plan.layers {
        L:=layer_index+1;
        for i:0..<nodes.count {if last[i]==L && !kept[i] {free[slot[i]]=true;}}
        for i:1..<nodes.count {
            if layer[i]==L && gg_gate(nodes[i].op) {
                plan.values+=1;chosen:=0;
                for s:1..<plan.width {if chosen==0 && free[s] {chosen=s;}}
                if chosen==0 {
                    if plan.width>=RA_WIDTH_MAX {return GgPlan.{status=GgStatus.width,node=i};}
                    chosen=plan.width;plan.width+=1;
                }
                free[chosen]=false;slot[i]=chosen;
            }
        }
    }
    // Units per layer: each gate's units, plus one to clear each freed slot.
    for i:0..<nodes.count {
        if gg_gate(nodes[i].op) {plan.units[layer[i]-1]+=gg_units(nodes[i].op);}
        if last[i]>=1 && !kept[i] {plan.units[last[i]-1]+=1;}
    }
    for L:0..<plan.layers {
        if plan.units[L]>RA_UNITS_MAX {return GgPlan.{status=GgStatus.units,node=L};}
        plan.weights+=ra_weight_count(plan.width,plan.units[L]);
    }
    if weights.count<plan.weights {return GgPlan.{status=GgStatus.capacity};}
    for k:0..<plan.weights {weights[k]=0;}
    // Emit: forms over base nodes become forms over their slots.
    base:=0;
    for layer_index:0..<plan.layers {
        L:=layer_index+1;units:=plan.units[layer_index];w:=weights[base..<base+ra_weight_count(plan.width,units)];u:=0;
        for i:1..<nodes.count {
            n:=nodes[i];
            if layer[i]==L && gg_gate(n.op) {
                a:=gg_slots(forms[n.a],slot[..]);t:=slot[i];
                if n.op==GgOp.mul {u=ra_mul(w,plan.width,units,u,a,gg_slots(forms[n.b],slot[..]),t);}
                if n.op==GgOp.relu {u=ra_relu(w,plan.width,units,u,a,t);}
                if n.op==GgOp.step {u=ra_step(w,plan.width,units,u,a,t);}
                if n.op==GgOp.eq0 {u=ra_eq0(w,plan.width,units,u,a,t);}
                if n.op==GgOp.select {u=ra_select(w,plan.width,units,u,gg_slots(forms[n.c],slot[..]),a,gg_slots(forms[n.b],slot[..]),t);}
            }
        }
        for i:0..<nodes.count {
            if last[i]==L && !kept[i] {#assert ra_unit(w,plan.width,units,u,ra_term(slot[i],1),ra_one(),slot[i],-1);u+=1;}
        }
        #assert u==units;
        base+=ra_weight_count(plan.width,units);
    }
    for k:0..<outputs.count {plan.out[k]=gg_slots(forms[outputs[k]],slot[..]);}
    return plan;
}
gg_units :: (op:GgOp)->int {
    if op==GgOp.relu {return 1;}
    if op==GgOp.eq0 {return 5;}
    return 2;
}
gg_slots :: (f:RaForm,slot:[]int)->RaForm {r:=f;for t:0..<f.count {r.slots[t]=slot[f.slots[t]];}return r;}
```

Each limit the compiler meets is a typed refusal:
- **`layers`:** more than 32 layers.
- **`width`:** more than 64 live slots.
- **`units`:** more than 128 units in a layer.
- **`terms`:** a linear form with more than eight terms.
- **`range`:** coefficients summing past 2²⁷, which could push a partial sum
  past 2⁵³.
- **`capacity`:** a weight buffer too small for the plan.

## Running the block

<!-- source: psionic/lib/gate-graph.coax run -->
```jai
// The compiled block: slot 0 is one, inputs fill slots 1 onward, every other
// slot starts at zero; each layer is chapter 167's gated layer; outputs are
// read as linear forms over the final residual.
gg_run :: (plan:GgPlan,weights:[]f64,inputs:[]int,outputs:[]f64,single:bool,overlap:bool)->GgStatus #export_c {
    if plan.status!=GgStatus.ok {return plan.status;}
    if weights.count<plan.weights || inputs.count!=plan.inputs || outputs.count<plan.outputs {return GgStatus.capacity;}
    if overlap {return GgStatus.alias;}
    for v:inputs {if v< -RA_VALUE_MAX || v>RA_VALUE_MAX {return GgStatus.range;}}
    x:[64]f64;x[0]=1;
    for k:0..<inputs.count {x[k+1]=f64(inputs[k]);}
    base:=0;
    for L:0..<plan.layers {
        n:=ra_weight_count(plan.width,plan.units[L]);
        // The layer stages its output, so it may write the residual it reads.
        #assert ra_forward(plan.width,plan.units[L],weights[base..<base+n],x[0..<plan.width],x[0..<plan.width],single,false)==RaStatus.ok;
        base+=n;
    }
    for k:0..<plan.outputs {
        f:=plan.out[k];sum:f64=0;
        for t:0..<f.count {sum=ra_round(sum+ra_round(f64(f.coefs[t])*x[f.slots[t]],single),single);}
        outputs[k]=sum;
    }
    return GgStatus.ok;
}
```

The example shows the format boundary from chapter 172 again. The same 414
weights multiply 4097 by 4097 exactly in binary64 and round it in binary32.

## How it is checked

- **Coax checks:**
  - Maximum, absolute value, a product plus an offset, equality and select
    against the evaluator.
  - Every operation at its boundary inputs.
  - Gates whose deepest operand comes in any position.
  - Slot reuse: within a layer, an input read by every layer, and an output
    still read after a later gate needs a slot.
  - A constant read early that must not clear slot 0.
  - Every admission and compiler limit, at exactly the limit and one past it.
- **The independent oracle** generates 300 seeded graphs with up to 22 nodes
  and inputs up to ±8,192. It predicts each graph's values, or the node where
  a value leaves the range or a form exceeds eight terms. In the native
  harness, 291 graphs compile and reproduce the oracle exactly, and the other
  9 are refused at the predicted node.
- **Mutation testing** of `gate-graph.coax` and the chapter 167 unit it loads
  kills all 666 mutants. The first run left 88 survivors. Most came from
  choices the checks could not observe:
  - a maximum written as "update if greater", whose tie case is invisible;
  - "never read" written as −1;
  - loops over node 0, which is always an input;
  - an extra iteration past the last layer.

  Each became observable code. The last two survivors showed that nothing
  tested a constant read by an early gate, which would have cleared slot 0
  under every later layer.

## What this does not do yet

This is straight-line code, one step at a time, with no memory. Chapter 169
attacks the compiler with a differential harness and planted bugs. Chapter 170
adds chapter 166's attention reads and loops the block over a trace, so the
whole of chapter 165's machine runs as weights.
