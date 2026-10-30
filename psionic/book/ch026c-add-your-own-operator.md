# 26c. Add your own operator

A new operation should arrive as a name, schema and qualified implementation,
rather than another special case in the engine. This chapter adds a public
[Coax registry](../lib/operator-registry.coax) and
[native binding API](../platform/operator-runtime.h). Registered calls are real
IR nodes: their outputs participate in shape inference, alias/liveness
planning, workspace reuse and compiled execution.

The [first registry profile](../fixtures/operator-registry-profile.json) is unary, same-shape f32 operators with read-only
inputs, independent outputs, one finite scalar configuration parameter, an
optional input VJP and an optional pointwise batching rule. Wider schemas,
parameter gradients and general automatic differentiation remain later work.

## Run it

```shell
coax/bin/coax run psionic/examples/registered-operator.coax
COAX=/path/to/qualified/coax psionic/check.sh --only operator-registry
COAX=/path/to/qualified/coax psionic/tests/check_operator_registry.sh --metal
LD_LIBRARY_PATH="$HOME/cuda-13.2/lib:${LD_LIBRARY_PATH:-}" \
  COAX=/path/to/qualified/coax psionic/tests/check_operator_registry.sh --cuda
```

The [Coax example](../examples/registered-operator.coax) admits and executes the
consumer's kernel with registry metadata. Its [transcript](../fixtures/registered-operator.out)
is:

```jai
Registered squared-ReLU: 0.0, 0.0, 0.0625, 4.0
Batch rule: [2, 4]; registered backward rule: true
Compiled model identity includes operator source and schema.
```

The native harness additionally registers the module, compiles the whole MLP,
checks an input VJP and trains through it on CPU/CUDA/Metal.

## Register a contract

<!-- source: psionic/lib/operator-registry.coax registry -->
```jai
OrSchema :: struct {name:[48]u8;name_count:int;entry:[48]u8;entry_count:int;version:int=1;rank_limit:int=4;dtype:TdDtype;backward:bool=true;batching:bool=true;write_alias:bool;}
OrRegistry :: struct {count:int;schemas:[8]OrSchema;code:[8][4]u64;compiler:[4]u64;life:TiLife;}
OrSpec :: struct {status:TiStatus;schema:OrSchema;}
OrUpdate :: struct {status:TiStatus;id:int;registry:OrRegistry;}
OrBatch :: struct {status:TiStatus;shape:TvShape;}
```

`or_schema` copies bounded names into owned metadata and declares the initial
schema defaults. Registering records the schema, compiler provenance and code
identity. Names and entry symbols are validated; duplicate name/version pairs,
unsupported dtypes, invalid ranks and write aliases refuse. Up to eight entries
fit one registry. A registered operator's ID belongs to that registry.

`or_node` constructs an IR call. Its two declared reads are the input and,
in backward mode, an equal-shaped upstream gradient. Forward mode names its
input twice; the second read is unused by the computation. Core inference
preserves the first input's shape, validates f32 placement and marks a new
independent output root. The registry then checks ID, schema rank and backward
capability before execution. A registered node without an attached registry
returns `operation`; it never executes as a built-in placeholder.

The batching rule prepends a bounded batch dimension and checks the resulting
rank/element count. It is valid for this pointwise profile. A schema without
the rule refuses that transform; it does not silently invent a rule for an
arbitrary operation.

## Supply Coax code outside the core

The [consumer-owned module](../examples/operators/squared-relu.coax) supplies
squared-ReLU and a second linear-gain operation. Neither name appears in core
runtime dispatch. The first computes `gain * max(x,0)^2`, and its input VJP is
`2 * gain * max(x,0) * dy`:

<!-- source: psionic/examples/operators/squared-relu.coax custom -->
```jai
registered_squared :: (item:int,m:[]int,gain:f32,input:[]f32,work:[]f32,out:[]f32,flags:[]u8) #export_c {
    c:[4]int;for axis:0..<4 {if axis<m[3] {c[axis]=tik_coord(m,item,axis);}}
    x:=tik_read(m,8,c[0],c[1],c[2],c[3],input,work);positive:f32=0;if x>0 {positive=x;}
    value:=gain*(positive*positive);
    if m[48]==1 {dy:=tik_read(m,19,c[0],c[1],c[2],c[3],input,work);value=(2.0f32*gain*positive)*dy;}
    flag:=0;if !tik_finite(x) || !tik_finite(value) {flag=2;value=0;}work[m[1]+item]=value;flags[m[2]+item]=cast(u8)flag;
}
```

The shared ABI carries the admitted command's address metadata, configuration
parameter, input/workspace spans, staged output and numerical flags. It is the
same command layout on CPU, CUDA and Metal. The consumer also supplies a wide
CPU reference procedure. `por_register` binds trusted compiled Coax CPU
callbacks and looks up the declared entry in the actual generated GPU module.
The host performs binding and driver effects; numerical implementation stays
in Coax. Kernel authors must obey the declared read/write/flag contract and
qualify their own numerical behavior; this is not a sandbox for arbitrary code.

Each module handles forward and VJP mode explicitly. A VJP is an input-gradient
rule, not a derivative of the configuration parameter. The generic engine
needs no knowledge of either formula. Adding another operator means supplying
another descriptor and callbacks/kernel entry, without editing its core.

## Keep compilation and ownership connected

`por_compile` opens a normal IR program and attaches its admitted registry.
Built-ins still use the generic kernels; registered commands dispatch the
registry's compiled kernel in the same stream/batch and write the planned
workspace slot. The final staged gather and whole-result publication remain
26a's. There is no per-operator CPU fallback on a GPU.

A registry fingerprint includes name/entry lengths and bytes, versions,
schema properties, producer and actual supplied source/module identities.
The attached fingerprint becomes part of `TiTarget.operators`, so cached plans
cannot substitute a different registry. The trusted local producer binds CPU
callback provenance; the API does not attest arbitrary machine code.

Coax leases keep registry modules alive while compiled models or derived plans
borrow them. Closing a borrowed registry or changing its registration table
is refused. Derived plans inherit the hooks and their own leases; a parent
cannot unload code while children remain. Close models first, then the registry.
All host access remains exclusive and synchronous.

## Exercise inference, gradients and training

The [native checks](../tests/operator_registry.c) replace the MLP's activation
with the registered squared-ReLU. Its independent small-fixture answer is
`[9.5, 13.5, 3.5, 4.5]`. A compiled CPU reference, planned CPU and generated
CUDA/Metal execution use the same graph. A second registered linear operation
and a prepended batch execute without another core change.

The input VJP is checked both against exact answers and finite differences,
including the smooth zero boundary of squared-ReLU. A small model then learns
`y = (2x)^2` as `squared_relu(weight*x)`. Forward and VJP are compiled registered
calls on the selected device; [Coax](../lib/operator-training.coax) computes
MSE, reduces the scalar parameter gradient and applies the update. This is a
training consumer of the public rule, not a claim of a device-resident optimizer
or general autograd.

From weight 0.5, 80 updates reach `1.99999976` with loss about
`3.70370401e-13` on the tested profile. The forward/VJP normal fixtures are exact;
the finite-difference ceiling is 0.002 scaled error, and the model-wide
reference ceiling stays 26a's 0.0001. The
[verification record](../../docs/verification/psionic-ch26c-2026-10-29.md) names
hardware, build modes, refusals and limitations. Native ASan/UBSan cover host
code; Metal retains qualified release emission and its declared subnormal
profile. Full mutation runs are omitted under the owner's instruction.
