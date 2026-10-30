# 175. A frozen exact core with a learned interface

[Chapter 174](ch174-can-gradient-descent-learn-exact-execution.md) found that
a student can be exact far past its training length, but only when its
features can express the rule and its training data pins the rule down. Both
had to be arranged by hand. This chapter splits the work the other way:
- **The core is compiled.** Chapter 168 compiles the machine's arithmetic and
  its branch target into a block. Chapter 176 pins the block by digest and
  admits it as exact.
- **Only the routing is learned.** Each token a row writes is copied from one
  of a fixed set of interface sources: values read straight from the state,
  and the core's outputs. A controller learns which source each token of each
  instruction comes from.

Every value a run writes is therefore a source's value. A run with the right
routes is exact at any length. A run the core cannot serve stops as refused,
not wrong. As in chapter 174, each run is judged only by chapter 173's
replay of its own rows.

```shell
coax/bin/coax run psionic/examples/core-interface.coax
coax/bin/coax test psionic/lib/core-interface.coax
python3 -I psionic/tests/core_interface_oracle.py
```

```jai
core: chapter 168's block for inputs second, top, ip and arg, with outputs second + top,
  second - top, second * top, and ip + 1 when top is 0, else arg; exact within 2^26
  chapter 176's admission: exact (pin, 4 cases, source recompiled to the pin)
interface: each token is copied from one of 12 sources:
  zero, arg, top, second, local, depth+1, depth-1, outputs+1, add, sub, mul, branch
controller: per instruction and token, a softmax over the sources that sees only the instruction,
trained from zero by 200 full-batch steps on -log P(a source that gives the machine's token)
controller A: trained on count and sum at n = 1..4: 8 traces admitted by chapter 186, 220 rows
  loss 0.0062
  routes, with sources training never told apart after a slash:
    push arg, depth+1; get local, arg; set top/add, arg; add add, depth-1/outputs+1; sub sub, top/depth-1; brif top/add, branch; out top/add, outputs+1; ret zero/arg/top/second/local/add/sub/mul, zero/arg/top/second/local/add/sub/mul;
    never seen: mul load store
  count: n=4 exact, n=8 exact, n=16 exact, n=32 exact
  sum: n=4 exact, n=8 exact, n=16 exact, n=32 exact
  stride: n=4 token 27 of 140, n=8 token 27 of 268, n=16 token 27 of 524, n=32 token 27 of 1036
    first miss at n=32: step 6 (sub) wrote 2 for y, where the machine has 1
  squares: n=4 token 18 of 172, n=8 token 18 of 332, n=16 token 18 of 652, n=32 token 18 of 1292
    first miss at n=32: step 4 (mul) wrote 0 for x, where the machine has 1024
  fact: n=4 token 26 of 188, n=8 token 26 of 348, n=11 token 26 of 468, n=12 token 26 of 508
    first miss at n=12: step 6 (mul) wrote 0 for x, where the machine has 12
controller B: trained on count, sum, stride and fact at n = 1..4: 16 traces admitted by chapter 186, 440 rows
  loss 0.006
  routes, with sources training never told apart after a slash:
    push arg, depth+1; get local, arg; set top/add, arg; add add, depth-1/outputs+1; sub sub, depth-1; mul mul, depth-1/outputs+1; brif top/add, branch; out top/add, outputs+1; ret zero/arg/top/second/local/add/sub/mul, zero/arg/top/second/local/add/sub/mul;
    never seen: load store
  count: n=4 exact, n=8 exact, n=16 exact, n=32 exact
  sum: n=4 exact, n=8 exact, n=16 exact, n=32 exact
  stride: n=4 exact, n=8 exact, n=16 exact, n=32 exact
  squares: n=4 exact, n=8 exact, n=16 exact, n=32 exact
  fact: n=4 exact, n=8 exact, n=11 exact, n=12 refused at step 86 of 127
    at n=12, step 86 (mul) needs a core output past 2^26
of 20 runs each: A 8 exact, 0 refused, 12 wrong; B 19 exact, 1 refused, 0 wrong
```

In short:
- **Controller B is exact on 19 of its 20 runs.** These include the
  held-out `squares` family and every length up to eight times training. The
  20th, `fact 12`, needs a product past the core's range of 2^26, and is
  refused at that step instead of answered wrongly.
- **Controller A shows what learning still can't do.** It saw only `count`
  and `sum`, where every `sub` subtracts 1 at depth 2. Its routes can't tell
  `top` from `depth − 1` for `sub`'s y, and the tie goes to `top`. That is
  right on `count` and `sum`, and wrong at `stride`'s first `sub`. `mul`
  never appeared, so it routes to `zero`, and `squares` and `fact` fail at
  their first `mul`.
- **The controller shows its own blind spots.** A tie between sources is
  printed beside the route. Some ties are harmless: `set` copies `top` or
  `second + top`, and in training `second` was always 0. They are still
  ties, and a program that sets at depth 2 would need data that separates
  them.

As in chapter 174, the CPU interpreter runs these programs faster than any of
this. What changes is the shape of the risk. A wrong route is a single
inspectable choice per instruction, and the core's arithmetic is never
approximate.

## The core

<!-- source: psionic/lib/core-interface.coax core -->
```jai
// Inputs second, top, ip and arg; outputs second + top, second - top,
// second * top, and ip + 1 when top is 0, arg otherwise.
fc_core_graph :: ()->[11]GgNode {
    return [11]GgNode.[
        GgNode.{op=GgOp.input,k=0},GgNode.{op=GgOp.input,k=1},GgNode.{op=GgOp.input,k=2},GgNode.{op=GgOp.input,k=3},
        GgNode.{op=GgOp.add,a=0,b=1},GgNode.{op=GgOp.sub,a=0,b=1},GgNode.{op=GgOp.mul,a=0,b=1},
        GgNode.{op=GgOp.eq0,a=1},GgNode.{op=GgOp.constant,k=1},GgNode.{op=GgOp.add,a=2,b=8},
        GgNode.{op=GgOp.select,c=7,a=9,b=3}];
}
fc_core_outs :: ()->[4]int {return [4]int.[4,5,6,10];}
fc_core :: ()->FcCore {
    c:FcCore;g:=fc_core_graph();o:=fc_core_outs();
    c.block=wm_module(gg_compile(g[..],o[..],c.weights[..]),WmLabel.exact);
    return c;
}
// Chapter 176's admission: the weights hash to the pin, four cases pass, and
// the source graph recompiles to the same pin.
fc_admit :: (core:FcCore)->WmStatus {
    fresh:=fc_core();pin:=wm_digest(fresh.block,fresh.weights[..]);
    // Each case: second, top, then ip when top is 0 and arg otherwise, then
    // the four outputs. When top isn't 0, ip is given second's value.
    rows:=[28]int.[7,3,9, 10,4,21,9, -2,5,0, 3,-7,-10,0, 6,0,4, 6,6,0,5, 9,-9,1, 0,18,-81,1];
    cases:[4]WmCase;
    for i:0..<4 {
        a:=rows[i*7];b:=rows[i*7+1];z:=rows[i*7+2];
        cases[i].inputs[0]=a;cases[i].inputs[1]=b;
        cases[i].inputs[2]=ifx b==0 then z else a;cases[i].inputs[3]=z;
        for j:0..<4 {cases[i].outputs[j]=rows[i*7+3+j];}
    }
    w:=core.weights;g:=fc_core_graph();o:=fc_core_outs();scratch:[1024]f64;
    return wm_admit(core.block,w[..],pin,cases[..],g[..],o[..],scratch[..]).status;
}
```

The core has eleven gate-graph nodes: four inputs, an add, a subtract, a
product, a zero test, a constant and a select. [Chapter
168](ch168-compile-a-gate-graph-into-a-block.md) compiles them into ReLU
layers. Inside chapter 167's range they are exact.

[Chapter 176](ch176-composable-weight-modules.md)'s admission checks three
things:
- the weights hash to a pin computed from a fresh compile;
- four cases pass, covering a negative difference, a product and both branch
  directions;
- the source graph recompiles to the same pin.

## Sources

<!-- source: psionic/lib/core-interface.coax sources -->
```jai
// Every source's value in a state, from one run of the core; a core output
// outside the core's range is not ok.
fc_values :: (core:FcCore,b:[ES_BASE]int)->FcValues {
    r:FcValues;
    plain:=[8]int.[0,b[1],b[2],b[3],b[4],b[5]+1,b[5]-1,b[6]+1];
    for s:0..<8 {r.ok[s]=true;r.v[s]=plain[s];}
    in:=[4]int.[b[3],b[2],b[7],b[1]];out:[4]f64;w:=core.weights;
    if wm_run(core.block,w[..],in[..],out[..]).status!=GgStatus.ok {return r;}
    for k:0..<4 {
        if out[k]>= -f64(RA_VALUE_MAX) && out[k]<=f64(RA_VALUE_MAX) {r.ok[8+k]=true;r.v[8+k]=cast(int)out[k];}
    }
    return r;
}
fc_value :: (core:FcCore,s:FcSource,b:[ES_BASE]int)->FcValue {
    r:=fc_values(core,b);
    return FcValue.{ok=r.ok[cast(int)s],v=r.v[cast(int)s]};
}
```

The eight plain sources read the state chapter 174's features describe. The
four core sources come from one run of the block per row. A core output
outside the range, or an input outside it, is not available. A route that
needs it ends the run as refused.

## The controller

<!-- source: psionic/lib/core-interface.coax controller -->
```jai
// The route for (code, token) is the source with the largest logit, the
// first on a tie: a code never seen routes to zero.
fc_route :: (c:FcController,code:int,t:int)->FcSource {
    at:=(code*2+t)*FC_SOURCES;best:=0;
    for s:1..<FC_SOURCES {if c.logits[at+s]>c.logits[at+best] {best=s;}}
    return cast(FcSource)best;
}
fc_softmax :: (c:FcController,code:int,t:int,p:[]f64) {
    at:=(code*2+t)*FC_SOURCES;top:=c.logits[at+cast(int)fc_route(c,code,t)];total:=0.0;
    for s:0..<FC_SOURCES {p[s]=exp(c.logits[at+s]-top);total+=p[s];}
    for s:0..<FC_SOURCES {p[s]=p[s]/total;}
}
// The other sources whose logit equals the route's, as a bit mask: training
// that never told them apart.
fc_ties :: (c:FcController,code:int,t:int)->int {
    at:=(code*2+t)*FC_SOURCES;best:=cast(int)fc_route(c,code,t);m:=0;
    // The route is the first largest logit, so ties can only come after it.
    for s:best+1..<FC_SOURCES {if c.logits[at+s]==c.logits[at+best] {m|=1<<s;}}
    return m;
}
// The probability of the chosen route.
fc_confidence :: (c:FcController,code:int,t:int)->f64 {
    p:[FC_SOURCES]f64;fc_softmax(c,code,t,p[..]);
    return p[cast(int)fc_route(c,code,t)];
}
// The probability of the sources in a bit mask.
fc_mass :: (p:[]f64,mask:int)->f64 {
    mass:=0.0;
    for v,s:p {if ((mask>>s)&1)==1 {mass+=v;}}
    return mass;
}
// Each logit set starts at zero and takes `epochs` full-batch steps down the
// mean of -log(the probability of the sources whose value is the machine's
// token). A row no source can give is counted as unroutable and left out.
fc_train :: (core:FcCore,ex:[]EsExample,epochs:int,rate:f64)->FcFit {
    f:FcFit;
    if ex.count<1 || epochs<0 {f.status=FcStatus.shape;return f;}
    if ex.count>FC_EXAMPLES_MAX {f.status=FcStatus.capacity;return f;}
    // Which sources give each token, as bit masks.
    mask:[2*FC_EXAMPLES_MAX]int;routable:[FC_EXAMPLES_MAX]bool;
    for e,i:ex {
        if e.code<0 || e.code>=ES_CODES {f.status=FcStatus.shape;return f;}
        v:=fc_values(core,e.base);
        for t:0..<2 {
            want:=ifx t==0 then e.x else e.y;
            for s:0..<FC_SOURCES {if v.ok[s] && v.v[s]==want {mask[2*i+t]|=1<<s;}}
        }
        routable[i]=mask[2*i]!=0 && mask[2*i+1]!=0;
        if !routable[i] {f.unroutable+=1;}
    }
    p:[FC_SOURCES]f64;grad:[FC_SOURCES]f64;
    for _,code:f.controller.seen {
        m:=0;
        for e,i:ex {if e.code==code && routable[i] {m+=1;}}
        if m==0 {continue;}
        for t:0..<2 {
            at:=(code*2+t)*FC_SOURCES;
            for epoch:0..<epochs {
                fc_softmax(f.controller,code,t,p[..]);
                for s:0..<FC_SOURCES {grad[s]=0;}
                for e,i:ex {
                    k:=mask[2*i+t];
                    if e.code!=code || !routable[i] {continue;}
                    mass:=fc_mass(p[..],k);
                    for s:0..<FC_SOURCES {
                        grad[s]+=p[s];
                        if ((k>>s)&1)==1 {grad[s]-=p[s]/mass;}
                    }
                }
                for s:0..<FC_SOURCES {f.controller.logits[at+s]-=rate*grad[s]/f64(m);}
            }
        }
    }
    total:=0.0;n:=0;
    for e,i:ex {
        if !routable[i] {continue;}
        f.controller.seen[e.code]=true;n+=1;
        for t:0..<2 {
            fc_softmax(f.controller,e.code,t,p[..]);
            total+=-log(fc_mass(p[..],mask[2*i+t]));
        }
    }
    f.loss=total/f64(2*n);
    return f;
}
```

For each instruction code and token there is one softmax over the twelve
sources. It sees only the code, so it is a learned table of routes. Each row
says which sources would have given the machine's token. Training raises the
probability of that set, and starts from zero.

Two sources that always agree in training get identical gradients, so their
logits stay exactly equal. `fc_ties` reports them, and the first source wins
the tie. A code never seen keeps all logits at zero and routes to `zero`.

## Factorial

<!-- source: psionic/lib/core-interface.coax factorial -->
```jai
// fact n prints n! = 1 * 2 * ... * n, the product kept in local 1.
fc_program :: (n:int,out:[]TmOp)->int {
    if n<1 || n>ES_N_MAX || out.count<17 {return 0;}
    T:=TmCode.push;G:=TmCode.get;S:=TmCode.set;
    p:=[17]TmOp.[es_op(T,n),es_op(S,0),es_op(T,1),es_op(S,1),es_op(G,1),es_op(G,0),es_op(TmCode.mul,0),es_op(S,1),
        es_op(G,0),es_op(T,1),es_op(TmCode.sub,0),es_op(S,0),es_op(G,0),es_op(TmCode.brif,4),
        es_op(G,1),es_op(TmCode.out,0),es_op(TmCode.ret,0)];
    for op,i:p {out[i]=op;}
    return 17;
}
```

Factorial joins chapter 174's families to teach `mul`. Its products grow
fast: `fact 12` is 479,001,600. That fits the machine's 32-bit integers but
not the core's 2^26.

## Rollout and scoring

<!-- source: psionic/lib/core-interface.coax rollout -->
```jai
// As chapter 174's rollout, but every x and y is copied from its routed
// source. A core output outside the core's range ends the run as refused,
// before its row is written.
fc_rollout :: (core:FcCore,c:FcController,program:[]TmOp,budget:int,rows:[]int,outs:[]int)->FcRun {
    r:FcRun;
    if budget<1 || rows.count<budget*TM_TOKENS || outs.count<TM_OUTPUTS || tm_admit(program)!=TmStatus.ok {
        r.status=FcStatus.shape;return r;
    }
    for step:0..<budget {
        f:=rx_fold(program,rows,step);
        if !f.ok || f.ip>=program.count {r.end=FcEnd.broken;r.steps=step;return r;}
        op:=program[f.ip];code:=cast(int)op.code;b:=es_base(program,f);
        v:=fc_values(core,b);x:=cast(int)fc_route(c,code,0);y:=cast(int)fc_route(c,code,1);
        if !v.ok[x] || !v.ok[y] {r.end=FcEnd.refused;r.steps=step;return r;}
        o:=step*TM_TOKENS;rows[o]=f.ip;rows[o+1]=code;rows[o+2]=v.v[x];rows[o+3]=v.v[y];
        if op.code==TmCode.out && r.outputs<TM_OUTPUTS {outs[r.outputs]=v.v[x];r.outputs+=1;}
        if op.code==TmCode.ret {r.end=FcEnd.halted;r.steps=step+1;return r;}
    }
    r.steps=budget;
    return r;
}
```

<!-- source: psionic/lib/core-interface.coax score -->
```jai
// The run is claimed and replayed by chapter 173, as in chapter 174. A
// refused run that matched the machine up to its refusal is reported as
// refused at that step: it stopped, it did not answer wrongly.
fc_score :: (core:FcCore,c:FcController,program:[]TmOp,rows:[]int,truth:[]int,outs:[]int)->FcScore {
    s:FcScore;s.token=-1;
    m:=rx_claim(program,TM_STEPS_MAX,truth,outs);
    if m.status!=TmStatus.ok {s.status=FcStatus.shape;return s;}
    run:=fc_rollout(core,c,program,m.claim.steps,rows,outs);
    if run.status!=FcStatus.ok {s.status=run.status;return s;}
    s.tokens=m.claim.steps*TM_TOKENS;
    s.refused=run.end==FcEnd.refused;s.step=run.steps;
    n:=run.steps*TM_TOKENS;
    claim:=RxClaim.{program=m.claim.program,trace=rx_ints_digest(rows,n),outputs=rx_ints_digest(outs,run.outputs),steps=run.steps};
    r:=rx_check(program,claim,rows[0..<n],0,run.steps,run.steps);
    if r.verdict==RxVerdict.exact {s.exact=true;return s;}
    if r.reason==RxReason.row {
        s.refused=false;s.token=r.step*TM_TOKENS+cast(int)r.field-1;s.step=r.step;
        s.wrote=rows[s.token];s.want=truth[s.token];
    }
    return s;
}
```

The rollout and score follow chapter 174's, except for one more ending. A
run refused by the core, whose rows matched the machine up to that point, is
reported as refused at that step. Any earlier wrong token is still reported
as the miss.

## The example

The [example](../examples/core-interface.coax) admits each controller's
training traces through [chapter
186](ch186-outcomes-become-training-rows.md). Each controller is its own
artifact over a shared exposure ledger. The example then trains each
controller and scores it on 20 runs.

<!-- source: psionic/examples/core-interface.coax example -->
```jai
main :: () {
    core:=fc_core();
    print("core: chapter 168's block for inputs second, top, ip and arg, with outputs second + top,\n");
    print("  second - top, second * top, and ip + 1 when top is 0, else arg; exact within 2^26\n");
    print("  chapter 176's admission: %\n",ifx fc_admit(core)==WmStatus.ok then "exact (pin, 4 cases, source recompiled to the pin)" else "refused");
    print("interface: each token is copied from one of 12 sources:\n  ");
    for s:0..<FC_SOURCES {print("%",SOURCE[s]);print("%",ifx s==FC_SOURCES-1 then "\n" else ", ");}
    print("controller: per instruction and token, a softmax over the sources that sees only the instruction,\n");
    print("trained from zero by 200 full-batch steps on -log P(a source that gives the machine's token)\n");

    entry:[64]TrEntry;
    names:=[2]string.["A","B"];
    sets:=[2][4]int.[[4]int.[0,1,-1,-1],[4]int.[0,1,2,4]];sizes:=[2]int.[2,4];
    tally:[2]Tally;entries:=0;
    for k:0..<2 {
        fams:=sets[k];
        r:=gather(1+k,fams[0..<sizes[k]],entry[..],entries);
        entries=r.entries;
        print("controller %: trained on",names[k]);
        for i:0..<sizes[k] {print("%",ifx i==0 then " " else ifx i==sizes[k]-1 then " and " else ", ");print("%",FAMILY[fams[i]]);}
        print(" at n = 1..4: % traces admitted by chapter 186, % rows\n",r.traces,r.count);
        f:=fc_train(core,r.ex[0..<r.count],200,1);#assert f.status==FcStatus.ok && f.unroutable==0;
        print("  loss %\n",r4(f.loss));
        show_routes(f.controller);
        tally[k]=evaluate(core,f.controller);
    }
    print("of 20 runs each: A % exact, % refused, % wrong; B % exact, % refused, % wrong\n",
        tally[0].exact,tally[0].refused,tally[0].wrong,tally[1].exact,tally[1].refused,tally[1].wrong);
}
```

## How it is checked

- **14 Coax checks:**
  - factorial against chapter 165's interpreter, up to the machine's own
    overflow at 13;
  - the core's admission, its four outputs, both branch directions, and its
    range at exactly 2^26 and one past it;
  - every source by hand;
  - learned routes, ties and confidences;
  - one step of descent by hand;
  - unroutable rows and the shapes training refuses;
  - rollouts that halt, run out of budget, break, run off the end, keep 64
    outputs, and stop at the core's range;
  - scores that are exact, refused, or wrong at a stated token.
- **[`core_interface_oracle.py`](../tests/core_interface_oracle.py)** rebuilds
  the example's output line for line. It has its own machine, an integer core
  with the same range, its own training and its own replay. The admission
  line is the example's report of chapter 176's checks, which the Coax tests
  cover.
- **Mutation:** 3,687 mutants, all killed.
