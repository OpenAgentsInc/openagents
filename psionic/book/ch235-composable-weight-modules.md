# 235. Composable weight modules

[Chapter 227](ch227-compile-a-gate-graph-into-a-block.md) compiles a gate
graph into a block of gated layers. Each block is a single program. This
chapter treats a compiled block as a **module**: a weight artifact pinned by
a digest, which can be linked to another module into one block without
recompiling either.

Linking must not change any answer. The linked block has to give exactly
what running the two modules one after the other gives, and exactly what
evaluating the composed gate graph gives. Every module also carries a label:
how a user who receives it can check it.

```shell
coax/bin/coax run psionic/examples/link-modules.coax
coax/bin/coax test psionic/lib/weight-module.coax
python3 -I psionic/tests/module_oracle.py
```

```jai
step module: 3 layers, width 6, 396 weights, digest 6fcd0391, exact, admitted: ok
max module: 2 layers, width 5, 165 weights, digest e659d8fb, tests, admitted: ok
linked: 6 layers, width 10, 1050 weights, digest aa3cdd24, tests, seam after layer 4
linked again: the same digest: true
105 inputs: the linked block, the chain and the composed graph agree; checksum 68178
multiply 10000 by 10000: linked range at layer 4; chain range; graph range at node 9
one linked weight changed: admitted: digest
```

The [example](../examples/link-modules.coax) links chapter 227's arithmetic
step to a small module that returns the larger of two values, whether the
first is zero, and the second.

## A module is its digest

<!-- source: psionic/lib/weight-module.coax digest -->
```jai
// The digest's bytes are staged 512 at a time: put writes eight bytes at
// buf[n] and returns the new length, hashing a full buffer first.
wm_put :: (sha:[]u8,buf:[]u8,n:int,w:u64)->int {
    at:=n;
    if at+8>buf.count {#assert sha256_update_checked(sha,buf[0..<at],false)==SHA_OK;at=0;}
    for i:0..<8 {buf[at+i]=cast(u8)((w>>cast(u64)(i*8))&0xff);}
    return at+8;
}
// A signed value as its two's complement.
wm_u64 :: (v:int)->u64 {
    w:=cast(u64)(v&0x7fffffffffffffff);
    if v<0 {w|=0x8000000000000000;}
    return w;
}
// SHA-256 over the block's shape, its seams, its output forms and the bits
// of every weight, each as eight little-endian bytes.
wm_digest :: (m:WmModule,weights:[]f64)->[32]u8 {
    sha:[112]u8;d:[32]u8;buf:[512]u8;n:=0;p:=m.plan;#assert sha256_init(sha[..])==SHA_OK;
    head:[8]int;head[0]=p.layers;head[1]=p.width;head[2]=p.inputs;head[3]=p.outputs;head[4]=m.nseams;head[5]=p.weights;
    for i:0..<6 {n=wm_put(sha[..],buf[..],n,wm_u64(head[i]));}
    for L:0..<p.layers {n=wm_put(sha[..],buf[..],n,wm_u64(p.units[L]));}
    for s:0..<m.nseams {
        e:=m.seams[s];
        n=wm_put(sha[..],buf[..],n,wm_u64(e.layer));n=wm_put(sha[..],buf[..],n,wm_u64(e.slot));n=wm_put(sha[..],buf[..],n,wm_u64(e.count));
    }
    for k:0..<p.outputs {
        f:=p.out[k];n=wm_put(sha[..],buf[..],n,wm_u64(f.count));
        for t:0..<f.count {n=wm_put(sha[..],buf[..],n,wm_u64(f.slots[t]));n=wm_put(sha[..],buf[..],n,wm_u64(f.coefs[t]));}
    }
    for i:0..<p.weights {n=wm_put(sha[..],buf[..],n,f64_bits(weights[i]));}
    #assert sha256_update_checked(sha[..],buf[0..<n],false)==SHA_OK;
    #assert sha256_final_checked(sha[..],d[..],false)==SHA_OK;
    return d;
}
```

The digest covers:
- the block's shape: layers, residual width, inputs, outputs, units per
  layer and the weight count;
- its seams, described below;
- its output forms;
- the exact bits of every weight.

Change one weight by one unit in the last place, and the digest changes. The
label isn't covered: it is a claim about the module, checked when the module
is admitted.

## Linking two modules

<!-- source: psionic/lib/weight-module.coax link -->
```jai
// A module's slot s in the linked block: slot 0 is the shared constant one,
// and its other slots start at `base`.
wm_slot :: (s:int,base:int)->int {
    if s==0 {return 0;}
    return base+s-1;
}
// Copies one layer of a module of width w into the linked block of width W.
// A layer's weights are the value rows, the gate rows (units of each, w
// wide), then the output matrix (w rows of units). Row 0 of the output
// matrix would write the shared constant slot: a module whose row 0 isn't
// zero changes the one the other module reads, so the copy refuses it.
wm_copy :: (from:[]f64,fb:int,w:int,units:int,base:int,to:[]f64,tb:int,W:int)->bool {
    for row:0..<2*units {for s:0..<w {to[tb+row*W+wm_slot(s,base)]=from[fb+row*w+s];}}
    for i:0..<w {for u:0..<units {
        v:=from[fb+2*units*w+i*units+u];
        if i==0 && v!=0 {return false;}
        to[tb+2*units*W+wm_slot(i,base)*units+u]=v;
    }}
    return true;
}
wm_link :: (a:WmModule,wa:[]f64,b:WmModule,wb:[]f64,wire:[]int,out:[]f64)->WmModule {
    r:WmModule;pa:=a.plan;pb:=b.plan;
    r.plan.status=GgStatus.shape;
    if pa.status!=GgStatus.ok || pb.status!=GgStatus.ok || wire.count!=pb.inputs {return r;}
    r.plan.status=GgStatus.operand;
    for w:wire {if w<0 || w>=pa.outputs {return r;}}
    W:=pa.width+pb.width-1;L:=pa.layers+1+pb.layers;g:=wire.count;
    r.plan.status=GgStatus.width;if W>RA_WIDTH_MAX {return r;}
    r.plan.status=GgStatus.layers;if L>GG_LAYERS_MAX {return r;}
    r.plan.status=GgStatus.capacity;
    if a.nseams+1+b.nseams>WM_SEAMS || wa.count<pa.weights || wb.count<pb.weights {return r;}
    total:=ra_weight_count(W,g);
    for k:0..<pa.layers {total+=ra_weight_count(W,pa.units[k]);}
    for k:0..<pb.layers {total+=ra_weight_count(W,pb.units[k]);}
    if total>out.count {return r;}
    // A refused copy leaves the status shape; out's contents are then unspecified.
    r.plan.status=GgStatus.shape;
    for i:0..<total {out[i]=0;}
    // The first module keeps its slots; the second's follow them.
    ba:=1;bb:=pa.width;
    tb:=0;fb:=0;
    for k:0..<pa.layers {
        if !wm_copy(wa,fb,pa.width,pa.units[k],ba,out,tb,W) {return r;}
        fb+=ra_weight_count(pa.width,pa.units[k]);tb+=ra_weight_count(W,pa.units[k]);r.plan.units[k]=pa.units[k];
    }
    // The glue layer: unit j's value is the wired output's form, its gate
    // reads the constant one, and it adds into input slot j of the second module.
    for j:0..<g {
        f:=pa.out[wire[j]];
        for t:0..<f.count {out[tb+j*W+f.slots[t]]=f64(f.coefs[t]);}
        out[tb+(g+j)*W]=1;
        out[tb+2*g*W+(pa.width+j)*g+j]=1;
    }
    r.plan.units[pa.layers]=g;tb+=ra_weight_count(W,g);fb=0;
    for k:0..<pb.layers {
        if !wm_copy(wb,fb,pb.width,pb.units[k],bb,out,tb,W) {return r;}
        fb+=ra_weight_count(pb.width,pb.units[k]);tb+=ra_weight_count(W,pb.units[k]);r.plan.units[pa.layers+1+k]=pb.units[k];
    }
    r.plan.status=GgStatus.ok;r.plan.layers=L;r.plan.width=W;r.plan.weights=total;
    r.plan.inputs=pa.inputs;r.plan.outputs=pb.outputs;
    for k:0..<pb.outputs {
        f:=pb.out[k];r.plan.out[k]=f;
        for t:0..<f.count {r.plan.out[k].slots[t]=wm_slot(f.slots[t],bb);}
    }
    for s:0..<a.nseams {r.seams[s]=a.seams[s];}
    r.seams[a.nseams]=WmSeam.{layer=pa.layers+1,slot=pa.width,count=g};
    for s:0..<b.nseams {
        e:=b.seams[s];
        r.seams[a.nseams+1+s]=WmSeam.{layer=e.layer+pa.layers+1,slot=wm_slot(e.slot,bb),count=e.count};
    }
    r.nseams=a.nseams+1+b.nseams;
    // The weaker label: x + y less the larger.
    x:=cast(int)a.label;y:=cast(int)b.label;r.label=cast(WmLabel)(x+y-gg_max(x,y));
    return r;
}
```

The linked block has a wider residual. Slot 0 is the constant 1, which both
modules share. Next come the first module's slots, and then the second
module's slots other than its slot 0. In the example the widths are 6 and
5, so the linked width is 10. The layers run in this order:
1. **The first module's layers,** unchanged, with zero weights on the second
   module's slots.
2. **One glue layer** with a unit for each of the second module's inputs.
   - Its value row is the first module's output form for the wired output.
   - Its gate reads the constant slot, so the ReLU gate is exactly 1.
   - It adds the value into the second module's input slot, which is still
     zero.
3. **The second module's layers,** with its slots moved and zero weights on
   the first module's slots.

Every value either module computes is computed with the same weights in the
same order. The extra terms are zero weights times finite values, and they
add exactly zero, so nothing rounds differently.

Two refusals protect this:
- **Neither module may write slot 0.** Both read it as the constant 1. If
  the first module wrote it, the second would read another value inside the
  linked block than in the chain. A compiled block never writes slot 0, but
  the copy checks every layer of both modules and refuses one that does.
- **A wire names an output, never an input.** Chapter 227's compiler reuses
  slots: by the end of a block, its input slots may hold other values or
  zero. An earlier version of this linker read inputs from their slots, and
  the tests caught it. A module that needs to pass an input on exports it as
  an output. The step module here exports its input `a`.

## The seam

<!-- source: psionic/lib/weight-module.coax run -->
```jai
// Chapter 227's run, with each seam checked after its layer.
wm_run :: (m:WmModule,weights:[]f64,inputs:[]int,outputs:[]f64)->WmRun {
    r:WmRun;p:=m.plan;
    if p.status!=GgStatus.ok {r.status=p.status;return r;}
    if weights.count<p.weights || inputs.count!=p.inputs || outputs.count<p.outputs {r.status=GgStatus.capacity;return r;}
    for v:inputs {if v< -RA_VALUE_MAX || v>RA_VALUE_MAX {r.status=GgStatus.range;return r;}}
    x:[64]f64;x[0]=1;
    for k:0..<inputs.count {x[k+1]=f64(inputs[k]);}
    base:=0;
    for L:0..<p.layers {
        n:=ra_weight_count(p.width,p.units[L]);
        #assert ra_forward(p.width,p.units[L],weights[base..<base+n],x[0..<p.width],x[0..<p.width],false,false)==RaStatus.ok;
        base+=n;
        for s:0..<m.nseams {
            e:=m.seams[s];
            if e.layer==L+1 {
                for i:e.slot..<e.slot+e.count {
                    if x[i]< -f64(RA_VALUE_MAX) || x[i]>f64(RA_VALUE_MAX) {r.status=GgStatus.range;r.layer=L+1;return r;}
                }
            }
        }
    }
    for k:0..<p.outputs {
        f:=p.out[k];sum:f64=0;
        for t:0..<f.count {sum=sum+f64(f.coefs[t])*x[f.slots[t]];}
        outputs[k]=sum;
    }
    return r;
}
```

Run on its own, the second module refuses inputs outside chapter 225's range
of ±2²⁶. Inside the linked block, nothing would refuse them, so the link
records a **seam**: after the glue layer, the second module's input slots
must be in range. A multiply of 10,000 by 10,000 is refused at the seam, just
as running the two modules one after the other refuses it. The evaluator
refuses the composed graph at the multiply itself.

Linking a linked module carries its seams along, moved to their new layers
and slots. The tests link four modules this way, with three seams.

## The references

<!-- source: psionic/lib/weight-module.coax chain -->
```jai
// The reference for a link: run the first module, wire its results into the
// second, and run that. A result outside the second module's input range is
// refused by the second run, as the linked block refuses it at the seam.
wm_chain :: (a:WmModule,wa:[]f64,b:WmModule,wb:[]f64,wire:[]int,inputs:[]int,outputs:[]f64)->WmRun {
    mid:[16]f64;r:=wm_run(a,wa,inputs,mid[0..<a.plan.outputs]);
    if r.status!=GgStatus.ok {return r;}
    next:[16]int;
    for j:0..<wire.count {
        next[j]=cast(int)mid[wire[j]];
    }
    return wm_run(b,wb,next[0..<wire.count],outputs);
}
// The composed gate graph: the first graph, then the second with each input
// node replaced by its wired output, scaled by one.
wm_compose :: (na:[]GgNode,oa:[]int,nb:[]GgNode,ob:[]int,wire:[]int,nc:[]GgNode,oc:[]int)->int {
    if na.count+nb.count>nc.count || ob.count>oc.count {return -1;}
    for i:0..<na.count {nc[i]=na[i];}
    shift:=na.count;
    for i:0..<nb.count {
        n:=nb[i];
        if n.op==GgOp.input {
            nc[shift+i]=GgNode.{op=GgOp.scale,a=oa[wire[n.k]],k=1};
        } else {
            n.a+=shift;n.b+=shift;n.c+=shift;nc[shift+i]=n;
        }
    }
    for k:0..<ob.count {oc[k]=ob[k]+shift;}
    return na.count+nb.count;
}
```

`wm_chain` runs the two modules one after the other, and `wm_compose` builds
the gate graph they compose to. Over the example's 105 inputs, the linked
block, the chain and the composed graph give the same outputs.

## How a user checks a module

<!-- source: psionic/lib/weight-module.coax admit -->
```jai
// Admits a module against its pinned digest. Every case given must pass, and
// the label must have its evidence: an exact module's source graph compiles
// to the pinned digest, a tests one has cases, a statistical one has
// samples.
wm_admit :: (m:WmModule,weights:[]f64,pin:[32]u8,cases:[]WmCase,nodes:[]GgNode,outs:[]int,scratch:[]f64)->WmAdmit {
    r:WmAdmit;p:=m.plan;
    r.status=WmStatus.digest;
    if p.status!=GgStatus.ok || weights.count<p.weights || !wm_same(wm_digest(m,weights),pin) {return r;}
    r.status=WmStatus.cases;
    for c:0..<cases.count {
        r.failed=c;got:[16]f64;
        run:=wm_run(m,weights,cases[c].inputs[0..<p.inputs],got[0..<p.outputs]);
        if run.status!=GgStatus.ok {return r;}
        for k:0..<p.outputs {if got[k]!=f64(cases[c].outputs[k]) {return r;}}
    }
    r.failed=0;r.status=WmStatus.evidence;
    if m.label==WmLabel.tests && cases.count==0 {return r;}
    if m.label==WmLabel.statistical && m.samples<1 {return r;}
    if m.label==WmLabel.exact {
        if nodes.count==0 {return r;}
        r.status=WmStatus.source;
        if m.nseams!=0 {return r;}
        q:=gg_compile(nodes,outs,scratch);
        if q.status!=GgStatus.ok || !wm_same(wm_digest(wm_module(q,m.label),scratch),pin) {return r;}
    }
    r.status=WmStatus.ok;
    return r;
}
```

| Label | What the user can do | What admission requires |
| --- | --- | --- |
| `exact` | Recompile the shipped source graph and compare digests, then check any input against the evaluator | the source compiles to the pinned digest |
| `tests` | Rerun fixed conformance cases | at least one case, all passing |
| `statistical` | Trust a stated agreement rate on sampled inputs | a sample count |
| `none` | Nothing | nothing |

Whatever the label, every case given must pass and the digest must match.

A linked module gets the weaker of its two labels. A linked module labelled
`exact` is checked by relinking its two exact parts and comparing digests,
since no single source graph compiles to a linked block. Linking twice gives
the same digest, as the example shows.

## Supported profile

`module-v1`:
- chapter 227's blocks in binary64;
- wires from the first module's outputs to every input of the second;
- a linked residual within 64 slots and 32 layers;
- at most 4 seams.

## How it is checked

- **Coax checks:** the chapter's 23 checks, with the 25 from chapters 226 and 227 and SHA-256's 41 that `weight-module.coax` loads, all 89 passing in debug and release. They cover:
  - every digest field and the bytes of each word;
  - the run against chapter 227's own run and evaluator;
  - the link against the chain and the composed graph over a grid of
    inputs, for two and for three modules, and the seams of four;
  - the seam at the range's edge and one past it;
  - each of the link's refusals;
  - the label rule, and each admission rule.
- **An independent oracle,** [module_oracle.py](../tests/module_oracle.py),
  works only from the two modules' meanings and the linking rule. It derives
  the linked block's layers, width and weight count, the outputs over the
  example's inputs and their checksum, and the multiply's refusal. It
  compares each with the example's output.
- **Mutation testing** of `weight-module.coax`: 1,942 of 1,942 mutants killed.

The [verification record](../../docs/verification/psionic-ch176-2026-10-30.md)
has the commands and results.

## What this does not do yet

- **Learned modules.** Chapter 233's students would be `statistical`
  modules. Admitting one needs its samples to be checkable, not just a count.
- **binary32.** In binary32, the first module's outputs round before the
  second module reads them. Linking in binary32 needs its own rule.
- **Shared slots.** The linked residual is the sum of the two widths, so
  long chains run out of slots. Reusing slots across modules would need
  chapter 227's slot scheduler to cover the seam.
