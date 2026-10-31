# 233. Can gradient descent learn exact execution?

Chapters 224 to 232 built an executor by compiling it: every weight was
written down, so the trace is exact by construction. Verified traces are also
training data whose labels are provably correct, so the obvious next question
is whether a model can *learn* the same execution from those traces.

The attempts before the reset all failed, 0 of 2 exact at best. This chapter
asks again on a machine small enough to answer fully, and reports what it
finds. Two rules keep the answer honest:
- **A student is judged only by running it alone.** Each row it writes is
  folded into the state it reads next, so one wrong token sends it down its
  own path. Training loss, and accuracy from the true states, are printed
  beside every result and never count.
- **The tests are longer and different.** Students train on four short
  programs from two families. They are tested at two, four and eight times
  that length, and on two families they never saw.

The interpreter that wrote the training traces is exact and far faster than
any student here. The point is only whether gradient descent can recover it.

```shell
coax/bin/coax run psionic/examples/students.coax
coax/bin/coax test psionic/lib/students.coax
python3 -I psionic/tests/students_oracle.py
```

```jai
chapter 224's machine; four program families, each counting n down to 1 in local 0:
  count prints n, ..., 1; sum prints 1 + ... + n; stride prints 2n, 2n-2, ..., 2; squares prints n*n, ..., 1
training traces: count and sum at n = 1, 2, 3, 4, each replayed exactly and admitted as train rows
  220 rows of the machine, 880 tokens
  count 4 again with step 3's x changed and its digest redone: failed
held out: count and sum at n = 8, 16, 32, stride and squares at n = 4, 8, 16, 32: 14 heldout rows, never trained on
  row set cd9ed6630597: 22 rows, 8 train, 14 heldout
students: one affine model per instruction and token, trained from zero by full-batch gradient descent;
scored by rollout alone: the state is folded from the student's own rows, chapter 232 replays the run,
and a miss is the first token that differs, of the machine's
linear, 10000 epochs: training loss 0.3044, 424 of 440 tokens right from true states
  count: n=4 token 39 of 140, n=8 token 39 of 268, n=16 token 39 of 524, n=32 token 39 of 1036
    first miss at n=32: step 9 (brif) wrote -104 for y, where the machine has 2
  sum: n=4 token 55 of 188, n=8 token 55 of 348, n=16 token 55 of 668, n=32 token 55 of 1308
    first miss at n=32: step 13 (brif) wrote -102 for y, where the machine has 4
  stride: n=4 token 26 of 140, n=8 token 26 of 268, n=16 token 26 of 524, n=32 token 26 of 1036
    first miss at n=32: step 6 (sub) wrote 63 for x, where the machine has 62
  squares: n=4 token 18 of 172, n=8 token 18 of 332, n=16 token 18 of 652, n=32 token 18 of 1292
    first miss at n=32: step 4 (mul) wrote 0 for x, where the machine has 1024
gated, 10000 epochs: training loss 0.0002, 440 of 440 tokens right from true states
  count: n=4 exact, n=8 exact, n=16 token 199 of 524, n=32 token 39 of 1036
    first miss at n=32: step 9 (brif) wrote 1 for y, where the machine has 2
  sum: n=4 exact, n=8 exact, n=16 exact, n=32 token 55 of 1308
    first miss at n=32: step 13 (brif) wrote 3 for y, where the machine has 4
  stride: n=4 token 26 of 140, n=8 token 26 of 268, n=16 token 26 of 524, n=32 token 26 of 1036
    first miss at n=32: step 6 (sub) wrote 63 for x, where the machine has 62
  squares: n=4 token 18 of 172, n=8 token 18 of 332, n=16 token 18 of 652, n=32 token 18 of 1292
    first miss at n=32: step 4 (mul) wrote 0 for x, where the machine has 1024
gated, 100000 epochs: training loss 0.0, 440 of 440 tokens right from true states
  count: n=4 exact, n=8 exact, n=16 exact, n=32 exact
  sum: n=4 exact, n=8 exact, n=16 exact, n=32 exact
  stride: n=4 token 26 of 140, n=8 token 26 of 268, n=16 token 26 of 524, n=32 token 26 of 1036
    first miss at n=32: step 6 (sub) wrote 63 for x, where the machine has 62
  squares: n=4 token 18 of 172, n=8 token 18 of 332, n=16 token 18 of 652, n=32 token 18 of 1292
    first miss at n=32: step 4 (mul) wrote 0 for x, where the machine has 1024
table, 10000 epochs: training loss 0.0097, 432 of 440 tokens right from true states
  count: n=4 token 103 of 140, n=8 token 2 of 268, n=16 token 2 of 524, n=32 token 2 of 1036
    first miss at n=32: step 0 (push) wrote 2 for x, where the machine has 32
  sum: n=4 token 175 of 188, n=8 token 2 of 348, n=16 token 2 of 668, n=32 token 2 of 1308
    first miss at n=32: step 0 (push) wrote 2 for x, where the machine has 32
  stride: n=4 token 2 of 140, n=8 token 2 of 268, n=16 token 2 of 524, n=32 token 2 of 1036
    first miss at n=32: step 0 (push) wrote 2 for x, where the machine has 64
  squares: n=4 token 18 of 172, n=8 token 2 of 332, n=16 token 2 of 652, n=32 token 2 of 1292
    first miss at n=32: step 0 (push) wrote 2 for x, where the machine has 32
exact runs of 16: linear 0, gated after 10000 epochs 5, after 100000 8, table 0
```

In short:
- **Loss and accuracy say nothing about exactness.** The gated student after
  10,000 epochs has a training loss of 0.0002 and gets all 440 training
  tokens right. It is still wrong on `count` at four and eight times the
  training length.
- **Exactness needs the rule in reach, and training to its end.** The gated
  student has one feature given by hand: each input again, multiplied by
  "the top of the stack is not zero". That is enough to write `brif`. Trained
  to convergence it is exact on all eight `count` and `sum` runs, up to 1,036
  and 1,308 tokens. Without that feature, the linear student can't write a
  branch and is never exact.
- **What training never varies is never learned.** Every `sub` in training
  subtracts 1, so nothing tells the student whether the 1 came from the stack
  or from a constant. The least-norm answer splits it between them, and
  `stride`, which subtracts 2, is off by one at its first `sub`. `mul` never
  appears in training, so `squares` fails at its first `mul`.
- **A lookup table doesn't extrapolate.** The table student learns a weight
  for each value it saw. It can't fit `brif` even in training, and for a
  value it never saw, such as `push 32`, it writes what the other features
  suggest: 2.

So a student can be exact far beyond its training length, but only when its
features can express the rule and its training data pins the rule down. Here
both were arranged by hand. That is the case for chapter 234: keep the exact
part compiled, and learn only the routing around it.

## Program families

<!-- source: psionic/lib/students.coax families -->
```jai
es_op :: (code:TmCode,arg:int)->TmOp {return TmOp.{code=code,arg=arg};}
// count prints n, n-1, ..., 1; stride prints 2n, 2n-2, ..., 2; squares
// prints n*n, ..., 1; sum prints 1+2+...+n. Each keeps its counter in local
// 0 and loops on a backward branch. Returns the program's length, or 0.
es_program :: (family:EsFamily,n:int,out:[]TmOp)->int {
    if n<1 || n>ES_N_MAX {return 0;}
    p:[17]TmOp;k:int;
    T:=TmCode.push;G:=TmCode.get;S:=TmCode.set;
    p[0]=es_op(T,ifx family==EsFamily.stride then 2*n else n);p[1]=es_op(S,0);
    if family==EsFamily.sum {
        loop:=[15]TmOp.[es_op(T,0),es_op(S,1),es_op(G,1),es_op(G,0),es_op(TmCode.add,0),es_op(S,1),
            es_op(G,0),es_op(T,1),es_op(TmCode.sub,0),es_op(S,0),es_op(G,0),es_op(TmCode.brif,4),
            es_op(G,1),es_op(TmCode.out,0),es_op(TmCode.ret,0)];
        for op,i:loop {p[2+i]=op;}k=17;
    }
    else if family==EsFamily.squares {
        loop:=[11]TmOp.[es_op(G,0),es_op(G,0),es_op(TmCode.mul,0),es_op(TmCode.out,0),
            es_op(G,0),es_op(T,1),es_op(TmCode.sub,0),es_op(S,0),es_op(G,0),es_op(TmCode.brif,2),es_op(TmCode.ret,0)];
        for op,i:loop {p[2+i]=op;}k=13;
    }
    else {
        loop:=[9]TmOp.[es_op(G,0),es_op(TmCode.out,0),es_op(G,0),es_op(T,ifx family==EsFamily.stride then 2 else 1),
            es_op(TmCode.sub,0),es_op(S,0),es_op(G,0),es_op(TmCode.brif,2),es_op(TmCode.ret,0)];
        for op,i:loop {p[2+i]=op;}k=11;
    }
    if out.count<k {return 0;}
    for i:0..<k {out[i]=p[i];}
    return k;
}
```

Each family keeps its counter in local 0 and loops on a backward branch.
`count` and `sum` are the training families. `stride` uses only instructions
seen in training, with one different constant. `squares` adds `mul`.

## Features

<!-- source: psionic/lib/students.coax features -->
```jai
// The state a row is written from: [1, arg, top, second, the local named by
// arg, depth, outputs, ip], with 0 for what is not there.
es_base :: (program:[]TmOp,f:RxFold)->[ES_BASE]int {
    op:=program[f.ip];d:=f.state.depth;b:[ES_BASE]int;
    b[0]=1;b[1]=op.arg;b[5]=d;b[6]=f.state.outputs;b[7]=f.ip;
    if d>=1 {b[2]=f.state.stack[d-1];}
    if d>=2 {b[3]=f.state.stack[d-2];}
    if op.arg>=0 && op.arg<TM_LOCALS {b[4]=f.state.locals[op.arg];}
    return b;
}
es_width :: (kind:EsKind)->int {
    if kind==EsKind.table {return ES_TABLE;}
    return ifx kind==EsKind.gated then 2*ES_BASE else ES_BASE;
}
// The nonzero features as (index, value) pairs; returns how many.
es_pairs :: (kind:EsKind,b:[ES_BASE]int,idx:[]int,val:[]f64)->int {
    if kind==EsKind.table {
        idx[0]=0;val[0]=1;n:=1;
        for j:1..<ES_BASE {
            v:=b[j];
            if v>=-ES_SPAN && v<=ES_SPAN {idx[n]=1+(j-1)*(2*ES_SPAN+1)+v+ES_SPAN;val[n]=1;n+=1;}
        }
        return n;
    }
    for j:0..<ES_BASE {idx[j]=j;val[j]=cast(f64)b[j]/ES_SCALE;}
    if kind==EsKind.linear {return ES_BASE;}
    z:=ifx b[2]!=0 then 1.0 else 0.0;
    for j:0..<ES_BASE {idx[ES_BASE+j]=ES_BASE+j;val[ES_BASE+j]=val[j]*z;}
    return 2*ES_BASE;
}
// Model (code, target) owns weights [at, at + width); target 0 is x, 1 is y.
es_predict :: (kind:EsKind,w:[]f64,code:int,target:int,b:[ES_BASE]int)->f64 {
    at:=(code*2+target)*es_width(kind);idx:[16]int;val:[16]f64;s:=0.0;
    for i:0..<es_pairs(kind,b,idx[..],val[..]) {s+=w[at+idx[i]]*val[i];}
    return s;
}
es_token :: (p:f64)->int {
    if !(p>-1e15 && p<1e15) {return ES_FAR;}
    return cast(int)floor(p+0.5);
}
```

A student is one affine model per instruction code and per token, x and y.
The row's ip and code are not predicted: they come from the program, at the
ip the student's own earlier rows reached. Three kinds share these inputs:
- **linear** divides each of the eight by 16;
- **gated** also repeats each one multiplied by [top ≠ 0], which is what a
  ReLU gate gives chapter 226's branches;
- **table** has a constant plus one weight per value in −64 to 64 for each of
  the other seven: a learned lookup table, added up.

A prediction is rounded to the nearest integer. One too large to round is
written as 2^31, which no row of the machine holds.

## Training rows

<!-- source: psionic/lib/students.coax examples -->
```jai
// Appends one example per row of an admitted trace at ex[n..], each with
// the state chapter 232's fold rebuilds from the rows before it. Returns the
// new count, or -1.
es_examples :: (program:[]TmOp,rows:[]int,steps:int,ex:[]EsExample,n:int)->int {
    if n<0 || steps<0 || rows.count<steps*TM_TOKENS || n+steps>ex.count {return -1;}
    for k:0..<steps {
        f:=rx_fold(program,rows,k);
        if !f.ok {return -1;}
        o:=k*TM_TOKENS;
        ex[n+k]=EsExample.{code=rows[o+1],base=es_base(program,f),x=rows[o+2],y=rows[o+3]};
    }
    return n+steps;
}
```

The training rows are exactly the rows of traces that [chapter
245](ch245-outcomes-become-training-rows.md) admitted, each after [chapter
232](ch232-exact-replay-as-a-verification-class.md) replayed it whole. The
state for each row is chapter 232's fold of the rows before it. A trace with
one token changed, and its digest recomputed to match, fails replay and
never reaches training. The held-out traces are admitted too, as `heldout`
rows, so chapter 245's ledger keeps their groups out of any training set.

## Gradient descent

<!-- source: psionic/lib/students.coax train -->
```jai
// Full-batch gradient descent on the mean squared error of each model, from
// zero, for the given epochs. The loss and the tokens right are over all
// examples with their true states: what training sees, not what a rollout
// does.
es_train :: (kind:EsKind,ex:[]EsExample,w:[]f64,epochs:int)->EsFit {
    width:=es_width(kind);r:EsFit;
    if w.count!=ES_CODES*2*width || epochs<0 || ex.count<1 {r.status=EsStatus.shape;return r;}
    // Each example's code is counted once; one outside the codes is not.
    counts:[ES_CODES]int;seen:=0;
    for code:0..<ES_CODES {
        for e:ex {if e.code==code {counts[code]+=1;}}
        seen+=counts[code];
    }
    if seen!=ex.count {r.status=EsStatus.shape;return r;}
    for i:0..<w.count {w[i]=0;}
    idx:[16]int;val:[16]f64;g:[ES_TABLE]f64;
    for code:0..<ES_CODES {
        if counts[code]==0 {continue;}
        // The rate is 1 over the mean squared norm; with the mean's 1/m in
        // the gradient, each step is 2 g / (the sum of squared norms).
        norm:=0.0;
        for e:ex {
            if e.code!=code {continue;}
            for i:0..<es_pairs(kind,e.base,idx[..],val[..]) {norm+=val[i]*val[i];}
        }
        for target:0..<2 {
            at:=(code*2+target)*width;
            for epoch:0..<epochs {
                for i:0..<width {g[i]=0;}
                for e:ex {
                    if e.code!=code {continue;}
                    n:=es_pairs(kind,e.base,idx[..],val[..]);s:=0.0;
                    for i:0..<n {s+=w[at+idx[i]]*val[i];}
                    miss:=s-f64(ifx target==0 then e.x else e.y);
                    for i:0..<n {g[idx[i]]+=miss*val[i];}
                }
                for i:0..<width {w[at+i]-=2*g[i]/norm;}
            }
        }
    }
    total:=0.0;
    for e:ex {
        for target:0..<2 {
            want:=ifx target==0 then e.x else e.y;p:=es_predict(kind,w,e.code,target,e.base);
            total+=(p-f64(want))*(p-f64(want));
            if es_token(p)==want {r.right+=1;}
        }
    }
    r.loss=total/f64(2*ex.count);r.tokens=2*ex.count;
    return r;
}
```

Each model starts at zero. Every epoch is one step down the gradient of the
mean squared error over all of that code's rows. The rate is 1 over the mean
squared norm of the feature vectors, which bounds the largest curvature, so
the descent can't diverge.

From zero, gradient descent on a least-squares problem converges to the
minimum-norm fit. The oracle uses that: for each student trained to
convergence, it solves the minimum-norm fit in exact rational arithmetic and
requires the same output. The gated student stopped at 10,000 epochs is not
at that limit, which is why it is right on short runs and wrong on long ones.
Its slowest directions are the ones training barely constrains, and long runs
reach values where they matter. The oracle retrains it in binary64 instead.

## Rollout

<!-- source: psionic/lib/students.coax rollout -->
```jai
// The student runs the program alone for up to budget steps. Each row's ip
// and code come from the program at the ip the student's own rows reached;
// x and y are its rounded predictions. The state is chapter 232's fold of
// the rows written so far, so a row the machine could not have written ends
// the run as broken.
es_rollout :: (kind:EsKind,w:[]f64,program:[]TmOp,budget:int,rows:[]int,outs:[]int)->EsRun {
    r:EsRun;
    if budget<1 || rows.count<budget*TM_TOKENS || outs.count<TM_OUTPUTS || w.count!=ES_CODES*2*es_width(kind) ||
        tm_admit(program)!=TmStatus.ok {
        r.status=EsStatus.shape;return r;
    }
    for step:0..<budget {
        f:=rx_fold(program,rows,step);
        if !f.ok || f.ip>=program.count {r.end=EsEnd.broken;r.steps=step;return r;}
        op:=program[f.ip];code:=cast(int)op.code;b:=es_base(program,f);o:=step*TM_TOKENS;
        rows[o]=f.ip;rows[o+1]=code;
        rows[o+2]=es_token(es_predict(kind,w,code,0,b));rows[o+3]=es_token(es_predict(kind,w,code,1,b));
        if op.code==TmCode.out && r.outputs<TM_OUTPUTS {outs[r.outputs]=rows[o+2];r.outputs+=1;}
        if op.code==TmCode.ret {r.end=EsEnd.halted;r.steps=step+1;return r;}
    }
    r.steps=budget;
    return r;
}
```

The rollout writes rows and nothing else. The state is rebuilt from those
rows by chapter 232's fold, the same code a verifier uses on a claim. A row
the machine could not have written, such as a branch to neither target, ends
the run as broken.

## Scoring

<!-- source: psionic/lib/students.coax score -->
```jai
// Rolls the student out for as many steps as the machine takes, then
// claims its rows as chapter 232's claimants do and has chapter 232 replay
// the claim: exact, or failed at the first token that differs.
es_score :: (kind:EsKind,w:[]f64,program:[]TmOp,rows:[]int,truth:[]int,outs:[]int)->EsScore {
    s:EsScore;
    m:=rx_claim(program,TM_STEPS_MAX,truth,outs);
    if m.status!=TmStatus.ok {s.status=EsStatus.shape;return s;}
    run:=es_rollout(kind,w,program,m.claim.steps,rows,outs);
    if run.status!=EsStatus.ok {s.status=run.status;return s;}
    n:=run.steps*TM_TOKENS;
    claim:=RxClaim.{program=m.claim.program,trace=rx_ints_digest(rows,n),outputs=rx_ints_digest(outs,run.outputs),steps=run.steps};
    r:=rx_check(program,claim,rows[0..<n],0,run.steps,run.steps);
    s.tokens=m.claim.steps*TM_TOKENS;s.token=-1;
    if r.verdict==RxVerdict.exact {s.exact=true;return s;}
    #assert r.reason==RxReason.row;
    s.token=r.step*TM_TOKENS+cast(int)r.field-1;s.wrote=rows[s.token];s.want=truth[s.token];
    return s;
}
```

The student gets as many steps as the machine needs. Its rows become a claim
like any claimant's, and chapter 232 replays it. The verdict is exact, or
failed at the first divergent step and field, which gives the first wrong
token.

## The example

The [example](../examples/students.coax) admits the traces, trains the four
students and scores each one on 16 runs.

<!-- source: psionic/examples/students.coax example -->
```jai
main :: () {
    print("chapter 224's machine; four program families, each counting n down to 1 in local 0:\n");
    print("  count prints n, ..., 1; sum prints 1 + ... + n; stride prints 2n, 2n-2, ..., 2; squares prints n*n, ..., 1\n");

    // Training traces are admitted by chapter 245, which replays each one
    // with chapter 232 and gives its group a role from the time cuts.
    rows:[64]TrRow;entry:[64]TrEntry;s:=tr_open(17,CUTS,rows[..],entry[..],0);
    ex:[400]EsExample;examples:=0;time:=0;
    for family:0..<2 {
        for n:1..<5 {
            o:=offer(cast(EsFamily)family,n);a:=admit(s,o,family*1000+n,time);s=a.set;
            #assert a.reason==TrReason.admitted && s.row[s.count-1].role==TrRole.train;
            examples=es_examples(o.program[0..<o.count],o.rows[..],o.claim.steps,ex[..],examples);#assert examples>0;
            time+=1;
        }
    }
    print("training traces: count and sum at n = 1, 2, 3, 4, each replayed exactly and admitted as train rows\n");
    print("  % rows of the machine, % tokens\n",examples,examples*TM_TOKENS);
    bad:=offer(EsFamily.count,4);bad.rows[3*TM_TOKENS+2]+=1;
    bad.claim.trace=rx_ints_digest(bad.rows[..],bad.claim.steps*TM_TOKENS);
    print("  count 4 again with step 3's x changed and its digest redone: %\n",REASON[cast(int)admit(s,bad,1004,time-1).reason]);
    held:=0;
    for family:0..<4 {
        for n:TESTS {
            if family<2 && n==4 {continue;}
            a:=admit(s,offer(cast(EsFamily)family,n),family*1000+n,8+held);s=a.set;
            #assert a.reason==TrReason.admitted && s.row[s.count-1].role==TrRole.heldout;
            held+=1;
        }
    }
    c:=tr_counts(s);
    print("held out: count and sum at n = 8, 16, 32, stride and squares at n = 4, 8, 16, 32: % heldout rows, never trained on\n",held);
    print("  row set ");hex6(tr_digest(s));print(": % rows, % train, % heldout\n",s.count,c[1],c[4]);

    print("students: one affine model per instruction and token, trained from zero by full-batch gradient descent;\n");
    print("scored by rollout alone: the state is folded from the student's own rows, chapter 232 replays the run,\n");
    print("and a miss is the first token that differs, of the machine's\n");
    store:[ES_CODES*2*ES_TABLE]f64;w:=store[..];
    tally:[4]int;
    tally[0]=student(EsKind.linear,10000,ex[0..<examples],w);
    tally[1]=student(EsKind.gated,10000,ex[0..<examples],w);
    tally[2]=student(EsKind.gated,100000,ex[0..<examples],w);
    tally[3]=student(EsKind.table,10000,ex[0..<examples],w);
    print("exact runs of 16: linear %, gated after 10000 epochs %, after 100000 %, table %\n",tally[0],tally[1],tally[2],tally[3]);
}
```

## How it is checked

- **15 Coax checks:**
  - the four families against chapter 224's interpreter, and which
    instructions carry an argument;
  - features from known folded states, including missing stack entries and
    an argument that names no local;
  - each kind's feature pairs, and the table's span at ±64 and ±65;
  - one epoch of descent by hand, a rule descent can fit, and a branch the
    gated kind learns and the linear kind cannot;
  - rollouts that halt, run out of budget, break on a bad branch, follow the
    other legal branch, and run off a program's end;
  - scores of the rules written by hand as gated weights (exact), of zero
    weights, of a wrong loop exit and of a wrong output.
- **[`students_oracle.py`](../tests/students_oracle.py)** rebuilds the
  example's output line for line, with its own machine, digests and students.
  Converged students are exact minimum-norm least-squares fits in rational
  arithmetic.
- **Mutation:** 2,071 mutants, all killed.
