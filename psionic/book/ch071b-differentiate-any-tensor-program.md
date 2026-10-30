# 71b. Differentiate any tensor program

[Chapter 71](ch071-gradients-as-a-program.md) wrote a model as a list of
scalar operations and got every gradient from one backward pass. That tape is
for teaching: f64 scalars, a fixed set of thirteen operations, and every value
kept until the end. This chapter is the permanent version. It differentiates
the tensor programs of [chapter 26a](ch026a-a-tensor-program-becomes-an-execution-plan.md):
- matrix products, broadcasting sums and products, ReLU, sums over an axis,
  layer norm and gathers;
- views and reshapes that share storage with the tensor they view;
- operators that consumers register for themselves, as in
  [chapter 26c](ch026c-add-your-own-operator.md).

It also handles what chapter 71 avoided:
- **Shared values.** A value read twice gets the sum of both gradients.
- **Unused inputs.** An input that asks for a gradient but never reaches the
  output is reported as unused, not given zeros.
- **Gradient control.** Gradients can be stopped at a node, and a whole tape
  can run forward only.
- **Custom derivatives.** A node's derivative can be replaced by the caller's.
- **Saved-tensor lifetimes.** Each value is kept only as long as a backward
  rule still needs it.

```shell
coax/bin/coax run psionic/examples/autodiff-mlp.coax
coax/bin/coax test psionic/lib/autodiff.coax
python3 -I psionic/tests/autodiff_oracle.py
```

```jai
tape: 10 forward steps, 9 backward steps; statuses ok and ok
values arena: 193 floats in 12 blocks while training, 128 floats in 10 blocks forward only, 234 if every value were kept
gradient arena: 161 doubles in 13 blocks
values saved for the backward pass: 3; 8 9 13

gradient check at the start, central differences with h = 1/64:
  W1: 16 elements, ok, gradient norm 0.831658, largest difference 0.000074
  b1: 8 elements, ok, gradient norm 0.717109, largest difference 0.000286
  W2: 8 elements, ok, gradient norm 1.644728, largest difference 0.000016
  b2: 1 element, ok, gradient norm 4.650803, largest difference 0.000015
  worst difference 0.000286: within 0.01

training, learning rate 0.01:
  step 0: loss 10.75579
  step 100: loss 1.637489
  step 200: loss 0.30915
  step 300: loss 0.193845
  step 400: loss 0.145242
  step 500: loss 0.113336
  step 600: loss 0.087372
  step 700: loss 0.064215
  step 800: loss 0.045014

predictions after training (ok), against the targets:
  (-0.75 -0.5): 1.522047 for 1.5
  (-0.25 -0.5): 0.475214 for 0.5
  (0.25 -0.5): -0.526302 for -0.5
  (0.75 -0.5): -1.473953 for -1.5
  (-0.75 0.5): -1.358504 for -1.5
  (-0.25 0.5): -0.57965 for -0.5
  (0.25 0.5): 0.395235 for 0.5
  (0.75 0.5): 1.571067 for 1.5
```

In short:
- **One network, every gradient from the tape.** The
  [example](../examples/autodiff-mlp.coax) is a two-layer network whose hidden
  activation is chapter 26c's consumer-owned squared ReLU. Every one of its 33
  parameter gradients matches central differences to within 0.0003, and
  800 steps of gradient descent take the loss from 10.76 to 0.045.
- **The tape keeps only what backward reads.** Training needs 193 floats of
  values, against 234 if every value were kept and 128 for the forward pass
  alone. Of ten intermediate values, three are saved for the backward pass.
- **The forward pass is the IR's own.** Each built-in step runs chapter 26a's
  command kernel with its metadata, and a test checks the tape's output bit
  for bit against the IR running its own plan.
- **Gradients are f64 until published.** They are accumulated in f64 and
  rounded once to f32 when the caller asks for them.

## What gradients are wanted

<!-- source: psionic/lib/autodiff.coax mode -->
```jai
// What gradients are wanted. grad marks inputs that require one; detach
// stops the gradient at a node's output; custom gives a node a derivative
// the caller computes; enabled false runs forward only and saves nothing.
AdMode :: struct {enabled:bool=true;grad:[32]bool;detach:[32]bool;custom:[32]bool;}
```

A node wants a gradient when one of its operands does, unless it is
detached. Detaching works only on nodes that compute something; a view
shares its root's storage, so stopping the gradient there would be
ambiguous. With `enabled` false no input wants a gradient, so nothing is
saved and there are no backward steps.

## Which values a backward step reads

<!-- source: psionic/lib/autodiff.coax needs -->
```jai
// Which operand values a node's rule reads in backward, given which of its
// operands need a gradient.
ad_reads :: (n:TiNode,custom:bool,need_a:bool,need_b:bool)->[2]bool {
    r:[2]bool;
    if custom {r[0]=true;r[1]=ti_binary(n.op);}
    else if n.op==TiOp.registered || n.op==TiOp.relu || n.op==TiOp.layernorm {r[0]=true;}
    else if n.op==TiOp.matmul || n.op==TiOp.multiply {r[0]=need_b;r[1]=need_a;}
    return r;
}
```

This short table decides memory. A product's gradient for one operand needs
the other operand, so `x @ W` keeps `x` only when `W` wants a gradient. In
the example, the first layer's input is data, so the product `x @ W1` is not
saved; the hidden activation is, because `W2`'s gradient needs it. Sums,
broadcast adds and gathers need no values at all. ReLU and the registered
operator need their input. Layer norm also needs only its input, because its
rule recomputes the mean and deviation from it. A custom derivative gets both
operands and the output, because the library cannot know which it uses.

## The tape

<!-- source: psionic/lib/autodiff.coax tape -->
```jai
ad_tape :: (p:TiProgram,mode:AdMode,registry:OrRegistry)->AdTape {
    t:AdTape;t.program=p;t.mode=mode;
    t.plan=ti_compile(p,TiTarget.{},false);
    if t.plan.status!=TiStatus.ok {t.status=AdStatus.program;return t;}
    for i:0..<p.count {
        n:=p.nodes[i];t.root[i]=t.plan.values[i].root;t.grad_born[i]=-1;
        if (mode.detach[i] || mode.custom[i]) && !ad_material(n.op) {t.status=AdStatus.mode;return t;}
        if n.op==TiOp.registered && (n.index_count!=0 || n.axis>=registry.count) {t.status=AdStatus.operation;return t;}
        if n.op==TiOp.input {t.wants[i]=mode.enabled && mode.grad[i];}
        else if !mode.detach[i] {t.wants[i]=t.wants[n.a] || (ad_both(n,mode.custom[i]) && t.wants[n.b]);}
    }
    // Walking back from the output through nodes that want a gradient finds
    // the nodes whose backward step is needed, and the inputs that get one.
    t.reaches[p.output]=true;
    for k:0..<p.count {
        i:=p.count-1-k;n:=p.nodes[i];
        if !t.reaches[i] || !t.wants[i] || n.op==TiOp.input {continue;}
        t.stepped[i]=ad_material(n.op);
        // A registered operator without a registered VJP needs a custom one.
        if n.op==TiOp.registered && !mode.custom[i] && !registry.schemas[n.axis].backward {t.status=AdStatus.operation;return t;}
        t.reaches[n.a]=true;if ad_both(n,mode.custom[i]) {t.reaches[n.b]=true;}
    }
    // One forward step for each material node, then the backward steps.
    // Node 0 can only be an input.
    for i:1..<p.count {
        n:=p.nodes[i];if !ad_material(n.op) {continue;}
        kind:=AdKind.builtin;if n.op==TiOp.registered {kind=AdKind.registered;}
        t.steps[t.count]=AdStep.{node=i,kind=kind};t.value_born[i]=t.count;t.value_dies[i]=t.count;t.count+=1;
    }
    t.forward=t.count;
    for k:0..<p.count {
        i:=p.count-1-k;if !t.stepped[i] {continue;}
        kind:=AdKind.builtin;
        if mode.custom[i] {kind=AdKind.custom;} else if p.nodes[i].op==TiOp.registered {kind=AdKind.registered;}
        t.steps[t.count]=AdStep.{node=i,backward=true,kind=kind};t.count+=1;
    }
    // A value lives to its last forward reader, a saved value to its last
    // backward reader, and the output to the end. Step 0 reads only inputs.
    for s:1..<t.count {
        i:=t.steps[s].node;n:=p.nodes[i];reads:=[2]bool.[true,ti_binary(n.op)];
        if t.steps[s].backward {
            custom:=t.steps[s].kind==AdKind.custom;
            reads=ad_reads(n,custom,t.wants[n.a],ad_both(n,custom) && t.wants[n.b]);
            if custom {t.saved[i]=true;t.value_dies[i]=s;}
        }
        for o:0..<2 {
            x:=ifx o==0 then n.a else n.b;
            if !reads[o] || p.nodes[t.root[x]].op==TiOp.input {continue;}
            if t.steps[s].backward {t.saved[t.root[x]]=true;}
            t.value_dies[t.root[x]]=s;
        }
    }
    t.value_dies[t.root[p.output]]=t.count;
    // A gradient is born at its first contributor and dies after its own
    // step; an input's lives to the end, to be published.
    if t.forward<t.count {t.grad_born[t.root[p.output]]=t.forward;}
    for s:t.forward..<t.count {
        i:=t.steps[s].node;n:=p.nodes[i];t.grad_dies[i]=s;
        for o:0..<2 {
            if o==1 && !ad_both(n,t.steps[s].kind==AdKind.custom) {continue;}
            x:=ifx o==0 then n.a else n.b;r:=t.root[x];
            if !t.wants[x] {continue;}
            if t.grad_born[r]==-1 {t.grad_born[r]=s;}
            if p.nodes[r].op==TiOp.input {t.grad_dies[r]=t.count;}
        }
    }
    return ad_layout(t);
}
```

The tape is planned before anything runs:
1. **Which nodes take part.** Walking back from the output through nodes that
   want a gradient marks the nodes whose backward step is needed. An input
   reached this way gets a gradient; an input that wanted one and was not
   reached is unused.
2. **The steps.** Every node that computes something gets a forward step.
   Every marked node gets a backward step, in reverse node order, so all of a
   value's readers have contributed before its own step runs.
3. **Lifetimes.** A value lives from its forward step to its last reader. A
   saved value's last reader is a backward step. A gradient is born at the
   first step that contributes to it and dies after its own step. An input's
   gradient lives to the end, to be published.

Views and reshapes get no steps. Their value is their root's storage, read
through their own view. Their gradient lands in the root's gradient, through
the same view. When `x` is read both directly and through a strided view,
both contributions add into one gradient buffer for `x`'s storage.

<!-- source: psionic/lib/autodiff.coax place -->
```jai
// First fit: the lowest offset that overlaps no block live at any of the
// same steps. Lifetimes are inclusive, so a step never writes what it reads.
ad_place :: (arena:AdArena,size:int,first:int,last:int)->AdArena {
    a:=arena;best:=0;moved:=true;
    while moved {
        moved=false;
        for j:0..<a.count {
            if a.last[j]<first || a.first[j]>last {continue;}
            if a.at[j]<best+size && best<a.at[j]+a.size[j] {best=a.at[j]+a.size[j];moved=true;}
        }
    }
    a.at[a.count]=best;a.first[a.count]=first;a.last[a.count]=last;a.size[a.count]=size;a.count+=1;
    if best+size>a.top {a.top=best+size;}
    return a;
}
```

Values, and the staging buffers used by external calls, share one f32 arena;
gradients get an f64 arena of their own. Each block takes the lowest offset
not used by any block live at one of the same steps. Lifetimes include both
ends, so a step never writes over something it reads.

## The rules

<!-- source: psionic/lib/autodiff.coax rules -->
```jai
// The vector-Jacobian product of one built-in node, added into its
// operands' gradients. dy is the node's own gradient, contiguous.
ad_rule :: (t:AdTape,i:int,inputs:[]f32,values:[]f32,grads:[]f64) {
    n:=t.program.nodes[i];s:=t.plan.values[i].view.shape;dy:=t.grad_at[i];
    a:=t.plan.values[n.a].view.shape;need_a:=t.wants[n.a];need_b:=ti_binary(n.op) && t.wants[n.b];
    for j:0..<ad_count(s) {
        g:=grads[dy+j];c:=to_coords(s,j);
        if n.op==TiOp.add || n.op==TiOp.multiply {
            b:=t.plan.values[n.b].view.shape;ja:=to_broadcast_index(a,s,j);jb:=to_broadcast_index(b,s,j);
            ga:=g;gb:=g;
            if n.op==TiOp.multiply {ga=g*ad_read(t,n.b,jb,inputs,values);gb=g*ad_read(t,n.a,ja,inputs,values);}
            if need_a {ad_add(t,n.a,ja,ga,grads);}
            if need_b {ad_add(t,n.b,jb,gb,grads);}
        } else if n.op==TiOp.matmul {
            b:=t.plan.values[n.b].view.shape;
            for k:0..<a.dims[1] {
                ja:=c[0]*a.dims[1]+k;jb:=k*b.dims[1]+c[1];
                if need_a {ad_add(t,n.a,ja,g*ad_read(t,n.b,jb,inputs,values),grads);}
                if need_b {ad_add(t,n.b,jb,g*ad_read(t,n.a,ja,inputs,values),grads);}
            }
        } else if n.op==TiOp.relu {
            if ad_read(t,n.a,j,inputs,values)>0 {ad_add(t,n.a,j,g,grads);}
        } else if n.op==TiOp.sum {
            point:[4]int;cursor:=0;
            for d:0..<a.rank {if d!=n.axis {point[d]=c[cursor];cursor+=1;}}
            for k:0..<a.dims[n.axis] {point[n.axis]=k;ad_add(t,n.a,to_flat(a,point),g,grads);}
        } else if n.op==TiOp.gather {
            point:=c;point[n.axis]=n.indices[c[n.axis]];ad_add(t,n.a,to_flat(a,point),g,grads);
        }
    }
    if n.op==TiOp.layernorm {ad_layernorm(t,i,inputs,values,grads);}
}
// Layer norm over the last axis, recomputed from x in f64:
// dx = (dy - mean(dy) - xhat * mean(dy * xhat)) / sigma.
ad_layernorm :: (t:AdTape,i:int,inputs:[]f32,values:[]f32,grads:[]f64) {
    n:=t.program.nodes[i];a:=t.plan.values[n.a].view.shape;width:=a.dims[a.rank-1];dy:=t.grad_at[i];
    for row:0..<ad_count(a)/width {
        base:=row*width;mean:f64=0;
        for k:0..<width {mean+=ad_read(t,n.a,base+k,inputs,values);}
        mean/=f64(width);variance:f64=0;
        for k:0..<width {d:=ad_read(t,n.a,base+k,inputs,values)-mean;variance+=d*d;}
        sigma:=sqrt(variance/f64(width)+f64(n.epsilon));m1:f64=0;m2:f64=0;
        for k:0..<width {g:=grads[dy+base+k];m1+=g;m2+=g*(ad_read(t,n.a,base+k,inputs,values)-mean)/sigma;}
        m1/=f64(width);m2/=f64(width);
        for k:0..<width {
            xhat:=(ad_read(t,n.a,base+k,inputs,values)-mean)/sigma;
            ad_add(t,n.a,base+k,(grads[dy+base+k]-m1-xhat*m2)/sigma,grads);
        }
    }
}
```

Each rule adds its contribution into its operands' gradients and never
overwrites them. That is why a value read twice gets the sum of both
gradients. A broadcast operand adds every output position that read it, and
a gather adds every position that selected the same row. ReLU's gradient at
exactly zero is zero, matching chapter 3a's convention.

## Running a tape

<!-- source: psionic/lib/autodiff.coax run -->
```jai
// Run steps until the tape ends or a step needs the caller. Gradients born
// at a step are cleared first; the output's gradient is seeded with ones, so
// the result is the gradient of the sum of the output's elements.
ad_run :: (t:AdTape,cursor:AdCursor,inputs:[]f32,values:[]f32,grads:[]f64,flags:[]u8)->AdRun {
    run:=AdRun.{cursor=cursor};
    if t.status!=AdStatus.ok || cursor.pending {run.status=AdStatus.state;return run;}
    if inputs.count<t.plan.input_elements || values.count<t.values.top || grads.count<t.grads.top {run.status=AdStatus.capacity;return run;}
    while run.cursor.next<t.count {
        s:=run.cursor.next;st:=t.steps[s];i:=st.node;n:=t.program.nodes[i];
        call:=AdCall.{node=i,backward=st.backward,kind=st.kind,items=t.plan.values[i].storage,gain=n.epsilon,meta=ad_meta(t,i),y_at=t.value_at[i]};
        if flags.count<call.items {run.status=AdStatus.capacity;return run;}
        if !st.backward {
            if st.kind!=AdKind.builtin {run.external=true;run.cursor.pending=true;run.call=call;return run;}
            for item:0..<call.items {tik_run(item,call.meta[..],n.epsilon,inputs,values,values,flags);}
            for item:0..<call.items {if flags[item]!=0 {run.status=AdStatus.number;return run;}}
        } else {
            for r:0..<t.program.count {
                if t.grad_born[r]!=s {continue;}
                for k:0..<t.plan.values[r].storage {grads[t.grad_at[r]+k]=0;}
            }
            if s==t.forward {
                for j:0..<ad_count(t.plan.values[t.program.output].view.shape) {ad_add(t,t.program.output,j,1,grads);}
            }
            if st.kind==AdKind.builtin {ad_rule(t,i,inputs,values,grads);}
            else {
                for j:0..<call.items {values[st.stage_dy+j]=f32(grads[t.grad_at[i]+j]);}
                call.dy_at=st.stage_dy;call.ga_at=st.stage_dx;call.gb_at=st.stage_db;
                call.na=ad_count(t.plan.values[n.a].view.shape);
                if st.kind==AdKind.custom && ti_binary(n.op) {call.nb=ad_count(t.plan.values[n.b].view.shape);}
                if st.kind==AdKind.registered {
                    // The second operand's slot now describes the staged gradient. It
                    // already has the right shape; its strides become contiguous.
                    call.meta[1]=st.stage_dx;call.meta[48]=1;call.meta[19]=1;call.meta[20]=st.stage_dy;stride:=1;
                    for k:0..<call.meta[21] {d:=call.meta[21]-1-k;call.meta[26+d]=stride;stride*=call.meta[22+d];}
                }
                run.external=true;run.cursor.pending=true;run.call=call;return run;
            }
        }
        run.cursor.next+=1;
    }
    return run;
}
// The caller ran the call; fold its result in and move past the step.
ad_finish :: (t:AdTape,cursor:AdCursor,values:[]f32,grads:[]f64,flags:[]u8)->AdRun {
    run:=AdRun.{cursor=cursor};
    if !cursor.pending {run.status=AdStatus.state;return run;}
    st:=t.steps[cursor.next];n:=t.program.nodes[st.node];
    if st.kind!=AdKind.custom {
        for item:0..<t.plan.values[st.node].storage {if flags[item]!=0 {run.status=AdStatus.number;return run;}}
    }
    if st.backward {
        for j:0..<ad_count(t.plan.values[n.a].view.shape) {
            g:=values[st.stage_dx+j];if !tv_finite(g) {run.status=AdStatus.number;return run;}
            if t.wants[n.a] {ad_add(t,n.a,j,f64(g),grads);}
        }
        if st.kind==AdKind.custom && ti_binary(n.op) && t.wants[n.b] {
            for j:0..<ad_count(t.plan.values[n.b].view.shape) {
                g:=values[st.stage_db+j];if !tv_finite(g) {run.status=AdStatus.number;return run;}
                ad_add(t,n.b,j,f64(g),grads);
            }
        }
    }
    run.cursor=AdCursor.{next=cursor.next+1};
    return run;
}
```

Coax has no callbacks, so the library cannot call the consumer's kernel
itself. It hands back a call instead. For a registered operator, the call
holds chapter 26c's kernel metadata, which points the kernel at the value
arena:
- going forward, at the operands;
- going backward, at the output's gradient, staged in f32, and at a buffer
  for the operand's gradient.

The caller runs its kernel over the call's items and passes its flags back
to `ad_finish`. A custom derivative gets the arena offsets of both operands,
the output, the output's gradient and buffers for both operand gradients.
Either way the result is folded in through the same view-aware addition as
a built-in rule.

<!-- source: psionic/lib/autodiff.coax publish -->
```jai
// A node's value, after its forward step has run.
ad_value :: (t:AdTape,cursor:AdCursor,x:int,inputs:[]f32,values:[]f32,out:[]f32)->AdStatus {
    if x<0 || x>=t.program.count {return AdStatus.state;}
    if t.program.nodes[t.root[x]].op!=TiOp.input && (cursor.next<=t.value_born[t.root[x]] || cursor.next>t.value_dies[t.root[x]]+1) {return AdStatus.state;}
    for j:0..<ad_count(t.plan.values[x].view.shape) {out[j]=f32(ad_read(t,x,j,inputs,values));}
    return AdStatus.ok;
}
// An input's gradient, rounded once to f32, after the whole tape has run.
ad_gradient :: (t:AdTape,cursor:AdCursor,x:int,grads:[]f64,out:[]f32)->AdStatus {
    if x<0 || x>=t.program.count || t.program.nodes[x].op!=TiOp.input || !t.mode.grad[x] {return AdStatus.state;}
    if !t.mode.enabled {return AdStatus.mode;}
    if cursor.next<t.count || cursor.pending {return AdStatus.state;}
    if !t.reaches[x] {return AdStatus.unused;}
    for j:0..<ad_count(t.plan.values[x].view.shape) {
        g:=grads[t.grad_at[x]+tv_offset(t.plan.values[x].view,j)];
        if !to_fits(g) {return AdStatus.number;}
        out[j]=f32(g);
    }
    return AdStatus.ok;
}
```

A value can be read from the step after its own until the step after its
last reader; after that its block may hold something else, and `ad_value`
refuses. A gradient is published only once the whole tape has run, and only
for an input that asked for one.

## A network built from registered operators

<!-- source: psionic/examples/autodiff-mlp.coax program -->
```jai
// A two-layer network as one tensor program. The hidden activation is
// chapter 26c's consumer-owned squared ReLU, 0.5 * relu(z)^2, registered
// with its own VJP; the loss is the sum of squared errors.
X :: 0;W1 :: 1;B1 :: 2;W2 :: 3;B2 :: 4;TARGET :: 5;NEG :: 6;LOSS :: 16;
INPUTS :: 58;
dims :: (rank:int,d0:int,d1:int)->TvShape {return TvShape.{rank=rank,dims=[4]int.[d0,d1,0,0]};}
network :: ()->TiProgram {
    p:TiProgram;p.count=17;p.output=LOSS;
    p.nodes[X]=ti_input(dims(2,8,2),0);p.nodes[W1]=ti_input(dims(2,2,8),16);p.nodes[B1]=ti_input(dims(1,8,0),32);
    p.nodes[W2]=ti_input(dims(2,8,1),40);p.nodes[B2]=ti_input(dims(1,1,0),48);
    p.nodes[TARGET]=ti_input(dims(2,8,1),49);p.nodes[NEG]=ti_input(dims(1,1,0),57);
    p.nodes[7]=TiNode.{op=TiOp.matmul,a=X,b=W1};
    p.nodes[8]=TiNode.{op=TiOp.add,a=7,b=B1};
    p.nodes[9]=or_node(0,8,8,false,0.5);
    p.nodes[10]=TiNode.{op=TiOp.matmul,a=9,b=W2};
    p.nodes[11]=TiNode.{op=TiOp.add,a=10,b=B2};
    p.nodes[12]=TiNode.{op=TiOp.multiply,a=TARGET,b=NEG};
    p.nodes[13]=TiNode.{op=TiOp.add,a=11,b=12};
    p.nodes[14]=TiNode.{op=TiOp.multiply,a=13,b=13};
    p.nodes[15]=TiNode.{op=TiOp.sum,a=14,axis=0};
    p.nodes[LOSS]=TiNode.{op=TiOp.sum,a=15,axis=0};
    return p;
}
```

<!-- source: psionic/examples/autodiff-mlp.coax drive -->
```jai
// Run a tape to the end, running the registered operator's kernel each
// time the tape hands back a call.
drive :: (t:AdTape,inputs:[]f32,values:[]f32,grads:[]f64,flags:[]u8)->AdRun {
    run:=ad_run(t,AdCursor.{},inputs,values,grads,flags);
    while run.status==AdStatus.ok && run.external {
        for i:0..<run.call.items {registered_squared(i,run.call.meta[..],run.call.gain,inputs,values,values,flags);}
        run=ad_finish(t,run.cursor,values,grads,flags);
        if run.status==AdStatus.ok {run=ad_run(t,run.cursor,inputs,values,grads,flags);}
    }
    return run;
}
loss_of :: (t:AdTape,inputs:[]f32,values:[]f32,grads:[]f64,flags:[]u8)->f64 {
    run:=drive(t,inputs,values,grads,flags);out:[1]f32;
    if run.status!=AdStatus.ok || ad_value(t,run.cursor,LOSS,inputs,values,out[..])!=AdStatus.ok {return -1;}
    return f64(out[0]);
}
```

<!-- source: psionic/examples/autodiff-mlp.coax example -->
```jai
main :: () {
    p:=network();registry:=OrRegistry.{count=1};
    train:AdMode;for k:PARAMS {train.grad[k]=true;}
    t:=ad_tape(p,train,registry);
    eval:AdMode;eval.enabled=false;e:=ad_tape(p,eval,registry);
    print("tape: % forward steps, % backward steps; statuses % and %\n",t.forward,t.count-t.forward,t.status,e.status);
    print("values arena: % floats in % blocks while training, % floats in % blocks forward only, % if every value were kept\n",
        t.values.top,t.values.count,e.values.top,e.values.count,t.kept);
    print("gradient arena: % doubles in % blocks\n",t.grads.top,t.grads.count);
    saved:=0;for i:0..<p.count {if t.saved[i] {saved+=1;}}
    print("values saved for the backward pass: %;",saved);
    for i:0..<p.count {if t.saved[i] {print(" %",i);}}
    print("\n");

    // Eight points; the target is the product of the two coordinates.
    inputs:[INPUTS]f32;
    for i:0..<8 {
        x0:=(f64(i%4)-1.5)/2;x1:=f64(i/4)-0.5;
        inputs[2*i]=f32(x0);inputs[2*i+1]=f32(x1);inputs[49+i]=f32(4*x0*x1);
    }
    inputs[57]=-1;state:u32=12345;
    for j:16..<49 {state=add_wrap(mul_wrap(state,(cast(u32)1103515245)),(cast(u32)12345));inputs[j]=f32(f64(int((state>>16)%33)-16)/32);}
    values:[256]f32;grads:[256]f64;flags:[64]u8;g:[16]f32;

    // Check every gradient against central differences of the forward-only tape.
    run:=drive(t,inputs[..],values[..],grads[..],flags[..]);
    print("\ngradient check at the start, central differences with h = 1/64:\n");
    worst:f64=0;
    for q,qi:PARAMS {
        n:=ad_count(t.plan.values[q].view.shape);
        status:=ad_gradient(t,run.cursor,q,grads[..],g[..]);
        largest:f64=0;norm:f64=0;
        for j:0..<n {
            base:=p.nodes[q].input_base+j;keep:=inputs[base];
            inputs[base]=keep+0.015625;up:=loss_of(e,inputs[..],values[..],grads[..],flags[..]);
            inputs[base]=keep-0.015625;down:=loss_of(e,inputs[..],values[..],grads[..],flags[..]);
            inputs[base]=keep;
            d:=abs((up-down)/0.03125-f64(g[j]));if d>largest {largest=d;}
            norm+=f64(g[j])*f64(g[j]);
        }
        if largest>worst {worst=largest;}
        print("  %: % element%, %, gradient norm %, largest difference %\n",PARAM_NAMES[qi],n,ifx n==1 then "" else "s",status,r6(sqrt(norm)),r6(largest));
    }
    print("  worst difference %: %\n",r6(worst),ifx worst<0.01 then "within 0.01" else "too large");

    // Plain gradient descent, every gradient from the tape.
    print("\ntraining, learning rate 0.01:\n");
    for step:0..<801 {
        run=drive(t,inputs[..],values[..],grads[..],flags[..]);
        if step%100==0 {
            out:[1]f32;if ad_value(t,run.cursor,LOSS,inputs[..],values[..],out[..])!=AdStatus.ok {print("  step %: no loss\n",step);return;}
            print("  step %: loss %\n",step,r6(f64(out[0])));
        }
        for q:PARAMS {
            if ad_gradient(t,run.cursor,q,grads[..],g[..])!=AdStatus.ok {print("  step %: no gradient\n",step);return;}
            for j:0..<ad_count(t.plan.values[q].view.shape) {
                base:=p.nodes[q].input_base+j;inputs[base]=f32(f64(inputs[base])-0.01*f64(g[j]));
            }
        }
    }
    // The same network up to its prediction, forward only.
    predict:=p;predict.count=12;predict.output=11;f:=ad_tape(predict,eval,registry);
    run=drive(f,inputs[..],values[..],grads[..],flags[..]);
    y:[8]f32;status:=ad_value(f,run.cursor,11,inputs[..],values[..],y[..]);
    print("\npredictions after training (%), against the targets:\n",status);
    for i:0..<8 {print("  (% %): % for %\n",f64(inputs[2*i]),f64(inputs[2*i+1]),r6(f64(y[i])),f64(inputs[49+i]));}
}
```

The network learns the product of two coordinates from eight points. One
hidden layer of eight squared ReLUs is enough: a product is a difference of
squares. The gradient check uses a step of 1/64 so that f32 rounding in the
two forward passes stays small next to the difference. A central difference
with a step that large is only accurate to a few ten-thousandths; the oracle
below checks the tape's gradients exactly.

## How it is checked

- **Ten Coax checks:**
  - every built-in rule against chapter 3a's dense-Jacobian VJPs, including
    broadcasting, a rank-3 sum and a gather that repeats an index;
  - a value used twice, shared nodes, unused inputs, detached nodes and
    forward-only tapes;
  - exact lifetimes, saved values and arena offsets for small programs,
    including a value nobody reads;
  - which operands each kind of step saves, with and without custom
    derivatives;
  - the kernel metadata against chapter 26a's `ti_lower`, the forward output
    bit for bit against the IR running its own plan, and the gradients of a
    program with a strided view and a reshape against central differences;
  - layer norm's rule against central differences of an f64 layer norm;
  - a registered operator forward and backward, its flags, its second
    operand and a registry without its VJP;
  - custom derivatives, including non-finite results, and every refusal.
- **[`autodiff_oracle.py`](../tests/autodiff_oracle.py)** rebuilds the
  example's output line for line from the Python standard library. It plans
  the tape itself, places the arenas first fit, emulates the f32 forward pass
  in chapter 26a's operation order, and writes the network's backward pass
  out by hand in f64, in the order this chapter documents.
- **Mutation:** every mutant of `autodiff.coax` is killed except two equivalents: the arena's high-water test (`>` and `>=` keep the same maximum) and `ti_compile`'s slot-reuse flag, which moves only the IR's own workspace bases, never read by the tape.

## Limits

- **The gradient of a sum.** The output's gradient is seeded with ones, so
  the result is the gradient of the sum of its elements. Other seeds, and
  `grad`, `vjp` and `jvp` as functions, are [chapter 71c](PLAN.md)'s.
- **First derivatives only.** The backward pass is not itself a tensor
  program, so it cannot be differentiated again.
- **A registered operator differentiates its first operand.** Its second
  operand only carries the shape, as in chapter 26c. A custom derivative can
  differentiate both.
- **Staged gradients are f32.** A registered operator's and a custom
  derivative's incoming gradient is rounded to f32, because their kernels
  read f32.
- **CPU, f32, 32 nodes.** The forward kernels compile for devices in chapter
  26a, but this chapter runs and checks them on the CPU only.
