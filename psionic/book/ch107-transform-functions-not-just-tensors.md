# 107. Transform functions, not just tensors

A gradient becomes more useful when it is another function we can evaluate,
differentiate and batch. This chapter builds checked function values from the
permanent tensor IR, then applies `grad`, `value_and_grad`, Jacobian, JVP, VJP,
HVP and `vmap` transforms to them. The runnable example contains a cross term,
so its Hessian has real off-diagonal entries.

These function values are **32-node permanent TiIR graphs**, not Coax procedure
values. The library transforms a checked graph description and its requested
partials; it does not add language-level `grad`, callbacks or arbitrary Coax
function transformation. Execution is currently CPU only. The
[example](../examples/function-transforms.coax) uses the actual generic graph
APIs, with independent arithmetic assertions for its particular function.

[Chapter 106](ch106-differentiate-any-tensor-program.md) gave the permanent IR
reverse rules. Here we add analytic forward linearization and forward-over-
reverse curvature, reuse those rules and compose transforms through order two.
Registered or opaque operators are refused: a name or an asserted derivative
cannot supply an unverified second-order rule.

## Build one function, then reuse it

Our example is `f(x) = x[0] * x[1] + x[2] * x[2]`. At `(2,3,4)` its value is 22,
gradient is `[3,2,8]`, and Hessian in row-major order is
`[0,1,0; 1,0,0; 0,0,2]`. We use those values as checks, not as the transform
implementation.

The graph has one three-coordinate input. Three gather nodes select the
coordinates, two multiply nodes form the terms, and add combines them. The
same graph can be evaluated at other points; no derivative coefficients are
embedded in its metadata.

<!-- source: psionic/examples/function-transforms.coax graph -->
```jai
ft_example_program :: ()->TiProgram {
    p:TiProgram;p.count=7;p.output=6;
    p.nodes[0]=ti_input(TvShape.{rank=1,dims=[4]int.[3,0,0,0]},0);
    for j:0..<3 {
        node:=TiNode.{op=TiOp.gather,a=0,axis=0,index_count=1};
        node.indices[0]=j;p.nodes[1+j]=node;
    }
    p.nodes[4]=TiNode.{op=TiOp.multiply,a=1,b=2};
    p.nodes[5]=TiNode.{op=TiOp.multiply,a=3,b=3};
    p.nodes[6]=TiNode.{op=TiOp.add,a=4,b=5};
    return p;
}
```

Run both native build modes from the repository root:

```shell
coax/bin/coax run psionic/examples/function-transforms.coax
coax/bin/coax run psionic/examples/function-transforms.coax --release
```

The program reserves caller-owned forward, flag, tangent, adjoint, directional-
adjoint, seed, direction, staging and point buffers once. These are distinct
allocations. Each checked entrypoint validates layout, capacities, numeric
inputs, requested derivative order and the declared work budget before publishing
its public result.

`fx_function` admits the original graph. `fx_grad` requires a single-output
function and returns one first-partial leaf per flat input coordinate.
`fx_value_and_grad` prepends that function value to its gradient. Taking the
Jacobian of the gradient yields a Hessian function: one second-partial leaf
for every ordered pair of input coordinates, with the inner input coordinate
varying fastest.

<!-- source: psionic/examples/function-transforms.coax transforms -->
```jai
    built:=fx_function(ft_example_program());
    #assert built.status==FxStatus.ok && built.function.count==1;
    f:=built.function;
    grad:=fx_grad(f);vg:=fx_value_and_grad(f);
    #assert grad.status==FxStatus.ok && vg.status==FxStatus.ok;
    hessian:=fx_jacobian(grad.function);
    #assert hessian.status==FxStatus.ok && hessian.function.count==9;
    #assert fx_eval(f,x[..],y[..],work,false).status==FxStatus.ok && y[0]==22;
    print("f(2,3,4): %\n",cast(int)y[0]);
    #assert fx_eval(grad.function,x[..],y[..],work,false).status==FxStatus.ok;
    #assert y[0]==3 && y[1]==2 && y[2]==8;
    print("grad: % % %\n",cast(int)y[0],cast(int)y[1],cast(int)y[2]);
    #assert fx_eval(vg.function,x[..],y[..],work,false).status==FxStatus.ok;
    #assert y[0]==22 && y[1]==3 && y[2]==2 && y[3]==8;
    print("value_and_grad: % % % %\n",cast(int)y[0],cast(int)y[1],cast(int)y[2],cast(int)y[3]);
    #assert fx_eval(hessian.function,x[..],y[..],work,false).status==FxStatus.ok;
    expected:=[9]f64.[0,1,0,1,0,0,0,0,2];
    for i:0..<9 {#assert y[i]==expected[i];}
    print("Jacobian(grad), row-major: % % %; % % %; % % %\n",
        cast(int)y[0],cast(int)y[1],cast(int)y[2],cast(int)y[3],cast(int)y[4],
        cast(int)y[5],cast(int)y[6],cast(int)y[7],cast(int)y[8]);
```

These transforms return status-bearing descriptors. Evaluating the descriptor
runs the graph and its generic derivative machinery; merely constructing it
does not compute a value. A Jacobian can have up to 1,024 leaves in this profile.
A request that exceeds this bound, or requires a third derivative, refuses.

## Ask for a product when the matrix is unnecessary

A Jacobian maps an input perturbation to an output perturbation. A JVP computes
that product directly. For direction `[1,-2,.5]`, the example's directional
change is `3*1 + 2*(-2) + 8*.5 = 3`.

A VJP starts with weights on output coordinates and returns the corresponding
weighted input gradient. With scalar output weight 2, the result is `[6,4,16]`.
This is the same reverse-mode idea used in training, exposed as a reusable
operation on a function value.

A Hessian-vector product asks how the gradient changes along the direction.
For this function and direction it is `[-2,1,1]`. `fx_hvp` returns both the
gradient and that product. `fx_jvp(grad(f))` gives the same directional change.
A VJP of the gradient multiplies its Jacobian transpose by a three-coordinate
cotangent. These compose through the supported first/second-order descriptors,
without constructing an arbitrary procedure or asking the caller to implement
a backward loop.

<!-- source: psionic/examples/function-transforms.coax products -->
```jai
    v:=[3]f64.[1,-2,0.5];cot:=[3]f64.[2,-1,3];
    #assert fx_jvp(f,x[..],v[..],y[..],dy[..],work,false).status==FxStatus.ok;
    #assert y[0]==22 && dy[0]==3;
    print("JVP: %\n",cast(int)dy[0]);
    #assert fx_vjp(f,x[..],cot[0..<1],y[..],dy[..],work,false).status==FxStatus.ok;
    #assert dy[0]==6 && dy[1]==4 && dy[2]==16;
    print("VJP(seed=2): % % %\n",cast(int)dy[0],cast(int)dy[1],cast(int)dy[2]);
    #assert fx_hvp(f,x[..],v[..],y[..],dy[..],work,false).status==FxStatus.ok;
    #assert y[0]==3 && y[1]==2 && y[2]==8 && dy[0]==-2 && dy[1]==1 && dy[2]==1;
    print("HVP: % % %\n",cast(int)dy[0],cast(int)dy[1],cast(int)dy[2]);
    #assert fx_jvp(grad.function,x[..],v[..],y[..],dy[..],work,false).status==FxStatus.ok;
    #assert dy[0]==-2 && dy[1]==1 && dy[2]==1;
    #assert fx_vjp(grad.function,x[..],cot[..],y[..],dy[..],work,false).status==FxStatus.ok;
    #assert dy[0]==-1 && dy[1]==2 && dy[2]==6;
    print("JVP(grad) and VJP(grad) compose: true\n");
```

The implementation in
[function-linearize.coax](../lib/function-linearize.coax) computes forward
F64 tangents and directional reverse adjoints analytically. Multiply and
matmul carry the cross terms between their operands. Gather, sum and views
route linear contributions through the original storage layout. Layer norm
recomputes full-row statistics and differentiates both its inverse standard
deviation and its centered values. Finite differences are an independent
verification tool, not how the implementation produces a derivative.

## State what the derivative means

Forward operators retain chapter 38's **F32 primal values** and rounding sites.
Tangents, adjoints, requested partials and published results use F64. These are
**formal analytic derivatives evaluated at the stored F32 primals**: the
operator equations are differentiated while floating-point rounding is not.
They are not literal derivatives of the discontinuous map from one F32 bit
pattern to another. This convention also applies to the second-order rules.

ReLU has derivative 1 for positive input and 0 for nonpositive input. Its second
rule is zero, including at the kink. That is the declared convention, not a
claim that a classical derivative exists at zero. Supported original graph
operators are input, add, multiply, matmul, ReLU, sum, layer norm, gather, view
and reshape. Unknown/registered operations and requests above order two refuse.
No rule is invented for an external simulator or a foreign callback.

The scalar coordinates are a flat input-storage domain of at most 256 values.
Input storage bindings may leave holes or overlap. All flat input values,
including holes, must be finite. Holes still occupy positions
in that domain and have zero derivative when unused. Two input nodes bound to
the same physical coordinate contribute to one summed derivative. The example
checks this with two one-element inputs at flat base 1: the domain is two,
coordinate 0 is a hole, and the gradient is`[0,2]`.

<!-- source: psionic/examples/function-transforms.coax bindings -->
```jai
    alias_program:TiProgram;alias_program.count=3;alias_program.output=2;
    scalar:=TvShape.{rank=1,dims=[4]int.[1,0,0,0]};
    alias_program.nodes[0]=ti_input(scalar,1);
    alias_program.nodes[1]=ti_input(scalar,1);
    alias_program.nodes[2]=TiNode.{op=TiOp.add,a=0,b=1};
    alias_function:=fx_function(alias_program);alias_grad:=fx_grad(alias_function.function);
    #assert alias_function.status==FxStatus.ok && alias_grad.status==FxStatus.ok;
    flat:=[2]f32.[99,3];
    #assert fx_eval(alias_grad.function,flat[..],y[..],work,false).status==FxStatus.ok;
    #assert y[0]==0 && y[1]==2;
    print("flat binding hole/overlap gradient: % %\n",cast(int)y[0],cast(int)y[1]);
```

This is storage identity, rather than the accidental order in which input nodes
appear. Views and reshapes likewise scatter their gradients into the correct
root storage. Coordinates outside the admitted domain, malformed derivative
leaves and unsupported placement refuse during layout admission.

## Batch the transformed function

`fx_vmap` describes repeated application of a function without a caller-written
execution loop. In this bounded flat-domain interface, each coordinate's axis
is either 0 (mapped over rows) or −1 (shared). This is not a general tensor
`in_axes`/`out_axes` implementation.

The example maps the first two coordinates and shares the third at 4. `rows`
is a dense row-major `[batch,domain]` buffer; `shared` is one domain-width
buffer. Their unused slots are ignored. Applying the mapped gradient returns
`[3,2,8]`, `[7,5,8]` and `[2,-1,8]` for its three rows. The API performs the
per-example gradient evaluation; the caller supplies no derivative loop.

<!-- source: psionic/examples/function-transforms.coax batch -->
```jai
    axes:=[3]int.[0,0,-1];mapped:=fx_vmap(grad.function,axes[..],3);
    #assert mapped.status==FxStatus.ok;
    shared:=[3]f32.[99,99,4];rows:=[9]f32.[2,3,77,5,7,77,-1,2,77];
    result:=fx_vmap_apply(mapped.map,shared[..],rows[..],y[..],work,false);
    #assert result.status==FxStatus.ok && result.values==9;
    batched_expected:=[9]f64.[3,2,8,7,5,8,2,-1,8];
    for i:0..<9 {#assert y[i]==batched_expected[i];}
    print("vmap(grad): % % %; % % %; % % %\n",
        cast(int)y[0],cast(int)y[1],cast(int)y[2],cast(int)y[3],cast(int)y[4],
        cast(int)y[5],cast(int)y[6],cast(int)y[7],cast(int)y[8]);
```

The entire batch stages privately before any output is published. If a later
row has an invalid consumed value, nonfinite derivative or other refusal,
earlier rows do not become a partial public batch. The batch limit is 64, and
its flattened output limit is 16,384 values. `vmap(grad(f))` and
`vmap(jacobian(f))` are supported descriptor compositions; arbitrary transform
nesting is not implied.

## Make storage and work explicit

`FxWork` borrows caller-owned typed spans. A successful `fx_layout` reports the
forward plan, flat domain, original codomain and root-storage cell count. The
main capacity requirements are:

| Work span | Minimum elements |
| --- | --- |
| `values` | Forward plan workspace, F32 |
| `flags` | Forward plan flag elements |
| `dots`, `grads`, `grad_dots` | Layout cell count each, F64 |
| `seed` | Original graph codomain, F64 |
| `direction`, `point` | Flat domain each; F64 and F32 respectively |
| `stage` for evaluation | Number of function leaves |
| `stage` for JVP | Twice the number of function leaves |
| `stage` for VJP | Function leaves plus flat domain |
| `stage` for vmap | Function leaves plus flattened batch output |

The derivative arena has at most 16,384 root-storage cells. Metadata permits at
most 32 IR nodes, 256 flat input coordinates and 1,024 function/derivative leaves.
The shared nominal work ceiling is 67,108,864 declared element-work units. Its
declared admission formula budgets repeated derivative work and
applies to checked evaluation/products as well as batching. It is not a CPU
instruction count, measured wall deadline, compiler resource proof or R14
`#bounded` contract. A static shape cap alone does not establish those claims.

The caller must provide a complete, honest overlap receipt covering every
input/output and every mutable work span. `false` means all required mutable
spans are disjoint from inputs and from one another; it does not ask Coax to
inspect native addresses. Buffer capacity is not evidence of disjointness.
The example deliberately aliases staging and public output, reports that
actual overlap and verifies refusal preserves all public values.

<!-- source: psionic/examples/function-transforms.coax refusal -->
```jai
    aliased:=work;aliased.stage=y[..];
    for i:0..<16 {y[i]=77;}
    refused:=fx_eval(f,x[..],y[..],aliased,true);
    #assert refused.status==FxStatus.alias;
    for value:y {#assert value==77;}
    print("honest overlap refusal preserves public output: true\n");
```

The checked wrappers are `fx_eval`, `fx_jvp`, `fx_vjp`, `fx_hvp` and
`fx_vmap_apply`, together with their checked descriptor builders. Internal
forward/tangent/reverse helpers assume an admitted layout and fitted, disjoint
work; calling those helpers directly does not repeat the public admission.
Caller storage must stay valid for the full call. Refusal may change private
work, while public outputs and spare tails remain unchanged.

## Keep qualification separate from applicability

The example's debug and release outputs match the committed
[golden transcript](../fixtures/function-transforms.out). It checks a cross-term
function's value, gradient, Hessian, both Jacobian products, curvature products,
shared/mapped batching, input holes/overlapping bindings and atomic overlap
refusal. These checks exercise the shared graph machinery; they do not provide
a numerical rule that the implementation can read back.

The [independent qualification](../fixtures/function-native-qualification.json)
records **704/704 root mutants killed, zero survivors** across layout 95,
linearization 279, budget 18, transforms 104, products 142 and batching 66 sites.
Each native debug/release/ASan-UBSan build passes 16,113 checks and emits exactly
the same result bytes as the frozen independent 33-point reference. Its 5,328
formal derivative values agree within 1.36e-15, with 84 primal values exact.
Thirty smooth finite-difference points have maximum Jacobian/Hessian error
9.44e-7/2.25e-6 under the independently frozen budgets. Kink conventions are
checked analytically rather than inferred by a finite difference through zero.

The final changed native build reuses the prior reference only after exact
output-byte comparison. Operator, strided-view, layer-norm and second-order
comparisons therefore remain independent of the book’s illustrative arithmetic.
The [verification record](../../docs/verification/psionic-ch107-2026-10-31.md)
keeps source identities, limitations and the targeted checks together.


This foundation lets later robotics code express a small owned state update as
a graph, ask how an action or parameter changes its output, and batch the same
calculation over independent cases. It does not differentiate the external
MuJoCo backend, the original GR00T device graph or arbitrary Coax procedures.
The owned differentiable solver and its parameter-recovery evidence remain
separate chapters.
