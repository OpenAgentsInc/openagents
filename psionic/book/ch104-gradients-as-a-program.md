# 104. Gradients as a program

In [chapter 103](ch103-learning-with-arrays.md) every gradient was derived by
hand. A residual was multiplied by whatever each parameter had multiplied, and
for the hidden layer we worked out tanh's slope ourselves. That stops working
once models stop being small. This chapter writes a model as a list of
operations and gets every gradient from one generic backward pass over that
list. It then checks the result three ways:
- each operation's rule against its derivative;
- random programs against 60-digit central differences;
- chapter 103's models, trained end to end, against chapter 103's oracle.

```shell
coax/bin/coax run psionic/examples/gradient-tape.coax
psionic/check.sh --only tape
```

```jai
x * x at x = 3: value 9.0, gradient 6.0 (node 0 is read twice)
5 + a at a = 5, two nodes reading slot 0: value 20.0, gradient 4.0
a * b = 6.0 at (2, 3), backward twice without zeroing: gradient (6.0, 4.0), not (3, 2)
one batch of 16 rows: logistic program 278 nodes, hidden-layer program 1602 nodes
tape against chapter 103's hand-written gradient: largest difference 2.7755575615628914e-17
logistic trained through the tape, 3600 steps: test log loss 0.1593
hidden layer trained through the tape, 3600 steps: test log loss 0.1434
```

The [example](../examples/gradient-tape.coax) runs in about a second in
release mode.

## A model is a list of operations

A program is an array of nodes. Each node is one of 13 operations:
- **Leaves:** a parameter, which reads a slot of the parameter array, or a
  constant.
- **Two operands:** `add`, `sub`, `mul`, `div`.
- **One operand:** `neg`, `square`, `exp`, `log`, `tanh`, `sigmoid`,
  `softplus`.

Operands always name earlier nodes. So a program has no cycles, runs in array
order, and its result is its last node. Admission checks that before anything
runs:

<!-- source: psionic/lib/tape.coax admit -->
```jai
// Operands name earlier nodes, so every program is acyclic and runs in order.
tp_admit :: (nodes:[]TpNode,params:int)->TpStatus #export_c {
    if nodes.count<1 || nodes.count>TP_NODES_MAX || params<0 || params>TP_PARAMS_MAX {return TpStatus.shape;}
    for n,i:nodes {
        if cast(int)n.op>cast(int)TpOp.softplus {return TpStatus.shape;}
        arity:=tp_arity(n.op);
        if n.op==TpOp.param && (n.slot<0 || n.slot>=params) {return TpStatus.shape;}
        if n.op==TpOp.constant && !tb_in(n.value,-TP_BOUND,TP_BOUND) {return TpStatus.number;}
        if arity>=1 && (n.a<0 || n.a>=i) {return TpStatus.shape;}
        if arity==2 && (n.b<0 || n.b>=i) {return TpStatus.shape;}
    }
    return TpStatus.ok;
}
```

## Forward saves every value

The forward pass computes each node in order and keeps every value. Every
value must stay within ±10¹⁰⁰. An operation with no value there is refused as
`number` rather than producing an infinity or NaN: `log` of a number that
isn't positive, `exp` past 230, division by zero, an overflowing product. The
values are computed in staging and published only when the whole program
succeeds.

<!-- source: psionic/lib/tape.coax forward -->
```jai
// One operation's value. Where it has none in the domain (log of a number
// that is not positive, exp past 230), the result is outside it and refused.
tp_apply :: (op:TpOp,a:f64,b:f64)->f64 {
    if op==TpOp.add {return a+b;}
    if op==TpOp.sub {return a-b;}
    if op==TpOp.mul {return a*b;}
    if op==TpOp.div {return a/b;}
    if op==TpOp.neg {return -a;}
    if op==TpOp.square {return a*a;}
    if op==TpOp.exp {if a>TP_EXP_MAX {return 2*TP_BOUND;} return exp(a);}
    if op==TpOp.log {if !(a>0) {return 2*TP_BOUND;} return log(a);}
    if op==TpOp.tanh {return tb_tanh(a);}
    if op==TpOp.sigmoid {return tb_sigmoid(a);}
    return tb_softplus(a);
}
// Every node's value, in order; the last node is the program's result.
tp_forward :: (nodes:[]TpNode,params:[]f64,values:[]f64,overlap:bool)->TpStatus #export_c {
    s:=tp_admit(nodes,params.count);
    if s!=TpStatus.ok {return s;}
    if values.count<nodes.count {return TpStatus.capacity;}
    if overlap {return TpStatus.alias;}
    for p:params {if !tb_in(p,-TP_BOUND,TP_BOUND) {return TpStatus.number;}}
    v:[4096]f64;
    for n,i:nodes {
        x:=n.value;
        if n.op==TpOp.param {x=params[n.slot];}
        if tp_arity(n.op)==1 {x=tp_apply(n.op,v[n.a],0);}
        if tp_arity(n.op)==2 {x=tp_apply(n.op,v[n.a],v[n.b]);}
        if !tb_in(x,-TP_BOUND,TP_BOUND) {return TpStatus.number;}
        v[i]=x;
    }
    for i:0..<nodes.count {values[i]=v[i];}
    return TpStatus.ok;
}
```

## Backward walks the list in reverse

The backward pass runs on these saved values:
1. Seed the result's adjoint, `∂result/∂result`, with 1.
2. Visit the nodes from last to first. By the time a node is reached, every
   node that reads it has already added to its adjoint. Pass that adjoint to
   the node's operands, multiplied by the operation's partial derivative.
3. A parameter node adds its adjoint into its slot's gradient.

<!-- source: psionic/lib/tape.coax backward -->
```jai
// d out / d a and d out / d b from saved values only: the operands and the output.
tp_partial_a :: (op:TpOp,a:f64,b:f64,out:f64)->f64 {
    if op==TpOp.add || op==TpOp.sub {return 1;}
    if op==TpOp.mul {return b;}
    if op==TpOp.div {return 1/b;}
    if op==TpOp.neg {return -1;}
    if op==TpOp.square {return 2*a;}
    if op==TpOp.exp {return out;}
    if op==TpOp.log {return 1/a;}
    if op==TpOp.tanh {return 1-out*out;}
    if op==TpOp.sigmoid {return out*(1-out);}
    return tb_sigmoid(a);
}
tp_partial_b :: (op:TpOp,a:f64,b:f64,out:f64)->f64 {
    if op==TpOp.add {return 1;}
    if op==TpOp.sub {return -1;}
    if op==TpOp.mul {return a;}
    return -out/b;
}
// Adds d result / d parameter into `grads`, one entry per slot. The adjoint of
// a node is the sum over everything that reads it, so a node read twice, or a
// slot read by two nodes, accumulates. The caller zeroes `grads` between steps.
tp_backward :: (nodes:[]TpNode,values:[]f64,grads:[]f64,overlap:bool)->TpStatus #export_c {
    s:=tp_admit(nodes,grads.count);
    if s!=TpStatus.ok {return s;}
    if values.count<nodes.count {return TpStatus.shape;}
    if overlap {return TpStatus.alias;}
    for i:0..<nodes.count {if !tb_in(values[i],-TP_BOUND,TP_BOUND) {return TpStatus.number;}}
    adj:[4096]f64;staged:[4096]f64;
    adj[nodes.count-1]=1;
    i:=nodes.count-1;
    while i>=0 {
        n:=nodes[i];g:=adj[i];
        if !tb_in(g,-TP_BOUND,TP_BOUND) {return TpStatus.number;}
        if n.op==TpOp.param {staged[n.slot]+=g;}
        arity:=tp_arity(n.op);
        if arity>=1 {
            b:=0.0;if arity==2 {b=values[n.b];}
            adj[n.a]+=g*tp_partial_a(n.op,values[n.a],b,values[i]);
            if arity==2 {adj[n.b]+=g*tp_partial_b(n.op,values[n.a],b,values[i]);}
        }
        i-=1;
    }
    for k:0..<grads.count {if !tb_in(grads[k]+staged[k],-TP_BOUND,TP_BOUND) {return TpStatus.number;}}
    for k:0..<grads.count {grads[k]+=staged[k];}
    return TpStatus.ok;
}
```

Four properties of the backward pass matter for everything that builds on it:
- **Saved values only.** The partial derivatives read the operands' saved
  values and the node's own output, such as `1 − out²` for tanh or `−out / b`
  for division. They never recompute anything from the parameters. Change a
  parameter after the forward pass and the gradient doesn't move. Change a
  saved value and it does. Both are checked.
- **Accumulation.** A node read twice receives two contributions: `x · x`
  reads node 0 twice and gets `x + x = 2x`. The same holds for parameter
  identity. A slot is one parameter however many nodes read it, so `5 + a`
  has gradient 4 in `a`'s slot.
- **The caller zeroes.** `tp_backward` adds into the gradients it is given,
  so calling it twice doubles them, as the example shows. That is how
  gradients accumulate across a batch split into pieces. It is also a common
  training bug, the first one chapter 105 plants.
- **Bounded adjoints.** Adjoints can grow even when values don't: `p · c · c ·
  c` with `c = 10⁶⁰` has values near 10⁸⁰ and a gradient of 10¹⁸⁰. Each adjoint
  is checked when its node is visited, and each gradient after accumulation.
  Any refusal leaves the caller's gradients unchanged.

## Chapter 103's models as programs

`tp_tabular` writes chapter 103's models, with their mean loss over a set of
rows, as a program:
- nodes 0 to P−1 read the parameter slots in chapter 103's layout;
- each row adds its features and label as constants, then the model's
  operations and its loss.

The loss is `softplus(z) − y·z` for logistic regression and `(z − y)² / 2` for
the line, and the mean divides the sum by the row count. One batch of 16 rows
takes 278 nodes for logistic regression and 1,602 for the hidden layer.

<!-- source: psionic/lib/tape.coax tabular -->
```jai
// Append one node; -1 once storage is full, and every later append stays -1.
tp_push :: (nodes:[]TpNode,n:int,node:TpNode)->int {
    if n<0 || n>=nodes.count {return -1;}
    nodes[n]=node;
    return n+1;
}
// Chapter 103's models and mean loss over `rows`, as a program. Nodes 0 to P-1
// read parameter slots 0 to P-1, in chapter 103's layout; each use of a
// parameter reads its node, so its adjoint collects every use.
tp_tabular :: (m:TbModel,x:[]f64,y:[]f64,rows:[]int,nodes:[]TpNode,overlap:bool)->TpBuild #export_c {
    d:=m.features;
    if !tb_model_ok(m) || x.count!=y.count*d || !tb_row_ok(rows,y.count) {return TpBuild.{status=TpStatus.shape};}
    if overlap {return TpBuild.{status=TpStatus.alias};}
    P:=tb_param_count(m);n:=0;total:int;hd:=m.hidden*d;base:=hd+m.hidden;
    for s:0..<P {n=tp_push(nodes,n,tp_p(s));}
    for r,ri:rows {
        xs:=n;
        for j:0..<d {n=tp_push(nodes,n,tp_c(x[r*d+j]));}
        z:=d;
        if m.hidden==0 {
            for j:0..<d {n=tp_push(nodes,n,tp_b(TpOp.mul,j,xs+j));n=tp_push(nodes,n,tp_b(TpOp.add,z,n-1));z=n-1;}
        } else {
            h:[16]int;
            for k:0..<m.hidden {
                a:=hd+k;
                for j:0..<d {n=tp_push(nodes,n,tp_b(TpOp.mul,k*d+j,xs+j));n=tp_push(nodes,n,tp_b(TpOp.add,a,n-1));a=n-1;}
                n=tp_push(nodes,n,tp_u(TpOp.tanh,a));h[k]=n-1;
            }
            z=P-1;
            for k:0..<m.hidden {n=tp_push(nodes,n,tp_b(TpOp.mul,base+k,h[k]));n=tp_push(nodes,n,tp_b(TpOp.add,z,n-1));z=n-1;}
        }
        n=tp_push(nodes,n,tp_c(y[r]));label:=n-1;
        if m.logistic {
            // softplus(z) - y z
            n=tp_push(nodes,n,tp_u(TpOp.softplus,z));n=tp_push(nodes,n,tp_b(TpOp.mul,label,z));n=tp_push(nodes,n,tp_b(TpOp.sub,n-2,n-1));
        } else {
            // (z - y)^2 / 2
            n=tp_push(nodes,n,tp_b(TpOp.sub,z,label));n=tp_push(nodes,n,tp_u(TpOp.square,n-1));
            n=tp_push(nodes,n,tp_c(0.5));n=tp_push(nodes,n,tp_b(TpOp.mul,n-1,n-2));
        }
        if ri==0 {total=n-1;} else {n=tp_push(nodes,n,tp_b(TpOp.add,total,n-1));total=n-1;}
    }
    n=tp_push(nodes,n,tp_c(f64(rows.count)));n=tp_push(nodes,n,tp_b(TpOp.div,total,n-1));
    if n==-1 {return TpBuild.{status=TpStatus.capacity};}
    return TpBuild.{count=n};
}
```

`tp_gradient` is a drop-in replacement for chapter 103's `tb_gradient`. It
builds the program, runs it forward, zeroes the gradients and runs it
backward:

<!-- source: psionic/lib/tape.coax gradient -->
```jai
// A drop-in for chapter 103's tb_gradient: build the program, run it forward,
// zero the gradients and run it backward. `nodes` and `values` are scratch.
tp_gradient :: (m:TbModel,p:[]f64,x:[]f64,y:[]f64,rows:[]int,g:[]f64,nodes:[]TpNode,values:[]f64,overlap:bool)->TpStatus #export_c {
    n:=tb_param_count(m);
    if p.count!=n || g.count!=n {return TpStatus.shape;}
    b:=tp_tabular(m,x,y,rows,nodes,overlap);
    if b.status!=TpStatus.ok {return b.status;}
    s:=tp_forward(nodes[0..<b.count],p,values,false);
    if s!=TpStatus.ok {return s;}
    z:[4096]f64;
    s=tp_backward(nodes[0..<b.count],values,z[0..<n],false);
    if s!=TpStatus.ok {return s;}
    for k:0..<n {g[k]=z[k];}
    return TpStatus.ok;
}
```

On the first training batch of seed 70, the tape and chapter 103's hand
gradient differ by at most 2.8 × 10⁻¹⁷. The difference comes from summation
order. Chapter 103 sums each gradient with a compensated accumulator. The tape
adds the rows' contributions as the reverse pass reaches them.

## Training through the tape

`tp_train` is chapter 103's trainer with `tp_gradient` where `tb_gradient`
was. Admission, initialization, the shuffle, the batches, decay and staging
are all shared. The admission checks are now a separate procedure,
`tb_admit_train`, which both trainers call.

<!-- source: psionic/lib/tape.coax train -->
```jai
// Chapter 103's trainer with the tape in place of the hand-written gradient:
// the same admission, initialization, shuffle, batches, decay and staging.
tp_train :: (m:TbModel,c:TbConfig,x:[]f64,y:[]f64,rows:[]int,params:[]f64,order:[]int,nodes:[]TpNode,values:[]f64,overlap:bool)->TbTrain #export_c {
    admitted:=tb_admit_train(m,c,x,y,rows,params,order,overlap);
    if admitted!=TbStatus.ok {return TbTrain.{status=admitted};}
    n:=tb_param_count(m);
    p:[161]f64;g:[161]f64;tb_initialize(m,c.init_seed,p[..]);
    o:=order[0..<rows.count];for i:0..<rows.count {o[i]=rows[i];}
    counter:=(cast(u64)0);steps:=0;
    for epoch:0..<c.epochs {
        counter=tb_shuffle(o,c.shuffle_seed,counter);
        start:=0;
        for i:0..<o.count {
            if i+1-start!=c.batch && i+1!=o.count {continue;}
            // After admission, only the program's own storage can fall short.
            if tp_gradient(m,p[0..<n],x,y,o[start..<i+1],g[0..<n],nodes,values,false)!=TpStatus.ok {return TbTrain.{status=TbStatus.capacity};}
            for k:0..<n {
                step:=g[k];if tb_is_weight(m,k) {step+=c.decay*p[k];}
                next:=p[k]-c.rate*step;
                if !tb_in(next,-TB_DOMAIN,TB_DOMAIN) {return TbTrain.{status=TbStatus.update};}
                p[k]=next;
            }
            steps+=1;start=i+1;
        }
    }
    loss:=tb_loss(m,p[0..<n],x,y,rows);
    for k:0..<n {params[k]=p[k];}
    return TbTrain.{steps=steps,loss=loss};
}
```

Trained for chapter 103's 40 epochs, 3,600 steps, both classifiers reach the
same test log loss as in chapter 103: 0.1593 and 0.1434. The native harness
compares the trained parameters themselves with chapter 103's independent
oracle, for logistic regression, the hidden layer and the latency line. The
largest relative difference is 3.4 × 10⁻¹⁴.

## Supported profile

`cpu-f64-tape-v1`:
- binary64;
- 1 to 4,096 nodes and 0 to 4,096 parameter slots;
- every value, adjoint and gradient within ±10¹⁰⁰, and `exp` arguments up to
  230.

`tp_tabular`, `tp_gradient` and `tp_train` accept chapter 103's models under
chapter 103's bounds. A program that doesn't fit the caller's node or value
storage is refused as `capacity`. Every exported procedure that writes caller
storage takes an `overlap` observation, which the native adapter computes from real addresses.

## How it is checked

- **Coax checks:** 69 checks in debug and release. They cover:
  - every operation's forward value by hand;
  - every unary and binary rule, which must equal its derivative exactly;
  - a 19-node program against central differences;
  - fan-out, parameter identity, accumulation without zeroing and
    saved-value semantics;
  - every admission, forward and backward refusal at its bound;
  - node counts and gradients of all three chapter 103 models against
    `tb_gradient`;
  - `tp_train` against `tb_train` within 10⁻¹⁴ for all three models.
- **An independent oracle,** [tape_oracle.py](../tests/tape_oracle.py), uses
  no reverse mode at all. It generates 300 seeded programs that use all 13
  operations, with 9 to 31 nodes, and evaluates them in 60-digit decimal
  arithmetic. Each derivative is a central difference with step 10⁻²⁵. It keeps
  a program only if:
  - every operation stays well inside its domain;
  - binary64 evaluation agrees with the decimal one to 10⁻¹² at every node;
  - its result depends on some parameter.

  So a disagreement would point at the tape, not at cancellation.
- **The native harness** [tape.c](../tests/tape.c) runs the generated C
  through the span adapter, plain and under ASan/UBSan, in debug and release.
  Tolerances were frozen before comparison. Results are within 10⁻¹¹ relative
  and gradients within 10⁻⁹ of max(1, |g|). The worst observed were 1.8 ×
  10⁻¹⁵ and 1.2 × 10⁻¹⁴ across 902 parameter derivatives. The harness also
  trains chapter 103's three seed-70 models through the tape, as above, and
  checks that an operation code outside the enumeration, NaN, infinity and
  overlapping storage are refused.
- **Mutation testing** of `tape.coax` kills all 1,270 mutants. The
  survivors of early runs were:
  - a saved-value check that another check happened to cover;
  - a variable initialized to a value it never kept;
  - a refusal test written as `< 0` for a count that is either −1 or at
    least 2.

  Each was fixed in the code or the checks.

The [verification record](../../docs/verification/psionic-ch71-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **Scalars, not tensors.** Every node holds one number, so a hidden layer of
  eight units is about a hundred nodes per row. Chapter 106 differentiates
  tensor programs over the compiler's IR, and this tape stays the teaching
  version.
- **One result.** The program's last node is the only output differentiated.
- **No second derivatives** and no custom rules.
- **Rebuilt every step.** The program for a batch is written again each time,
  because its constants are the batch's rows.
