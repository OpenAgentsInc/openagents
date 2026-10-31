# 38. A tensor program becomes an execution plan

IR means **intermediate representation**: a structured description between the
program a person writes and the instructions a machine executes. Here it is
a small typed list of tensor operations. It says what each operation reads,
what shape and dtype it produces, which device executes it and whether its
result owns storage or aliases an earlier value.

An execution plan adds decisions: when values die, where their storage goes,
which commands execute and what makes a cached plan stale. Keeping those
questions separate lets us express a feed-forward block once and execute it
several ways. Later chapters can differentiate or fuse the same representation.

This chapter implements **tensor-ir-f32-v1** in
[Coax](../lib/tensor-ir.coax), with a [CPU interpreter](../lib/tensor-ir-host.coax),
[shared device kernels](../lib/tensor-ir-kernels.coax), an
[owned native runner](../platform/tensor-ir-runtime.c) and a
[machine-readable profile](../fixtures/tensor-ir-profile.json).
It is a library IR constructed explicitly in Coax. Automatic extraction from
an arbitrary Coax function, dynamic shapes, differentiation and fusion follow
in their own chapters.

## Run it

The [Coax example](../examples/tensor-program.coax) compares the CPU procedures
with the planned arithmetic and prints the workspace reduction. The native
checks also use an owned compiled object with persistent workspace, so its
replays reuse the actual allocations and lowered command metadata.

```shell
coax/bin/coax run psionic/examples/tensor-program.coax
COAX=/path/to/qualified/coax psionic/check.sh --only tensor-ir
COAX=/path/to/qualified/coax psionic/tests/check_tensor_ir.sh --metal
# On the qualified CUDA host; adjust the NVRTC library path for your install:
LD_LIBRARY_PATH="$HOME/cuda-13.2/lib:${LD_LIBRARY_PATH:-}" \
  COAX=/path/to/qualified/coax psionic/tests/check_tensor_ir.sh --cuda
```

A qualified compiler needs `cuda` or `metal-compute` for the requested device.
The focused runner performs no compiler bootstrap or full mutation run. Metal
uses release GPU emission: a bounds-checked development candidate triggered
an Apple pipeline compiler failure. The host and Coax checks still run in
both debug and release, and the same admitted graph fixtures qualify the
release Metal artifact. The [verification record](../../docs/verification/psionic-ch26a-2026-10-29.md)
contains the exact artifact identities and the limitation.

The [frozen output](../fixtures/tensor-program.out) is:

```jai
CPU reference: 3.5, 5.5, 1.5, 2.5
Planned execution: 3.5, 5.5, 1.5, 2.5
Workspace: 16 floats reused, 32 with fresh buffers; 6 commands
Exact graph and target cache match: true
Compiler changed, cache match: false
```

## What the representation carries

Chapter 5 supplied CPU operators and shape inference. Chapter 6 supplied
device and dtype contracts; chapter 3 supplied the address rules for views.
The IR uses those rules with one deliberately concrete execution dtype: f32.
Other dtype annotations return `dtype`, rather than silently converting.

<!-- source: psionic/lib/tensor-ir.coax types -->
```jai
TiOp :: enum u8 {input;matmul;add;multiply;relu;sum;layernorm;view;reshape;gather;registered=11;}
TiStatus :: enum u8 {ok;operation;dependency;shape;dtype;device;layout;index;limit;capacity;alias;number;arithmetic;identity;budget;driver;absent;state;}
TiPlacement :: enum u8 {target;cpu;cuda;metal;}
TiNode :: struct {
    op:TiOp;a:int=-1;b:int=-1;
    shape:TvShape;strides:[4]int;offset:int;storage:int;input_base:int;
    dtype:TdDtype;placement:TiPlacement;
    axis:int;epsilon:f32=0.00001f32;index_count:int;indices:[16]int;
}
TiProgram :: struct {count:int;output:int;nodes:[32]TiNode;}
TiTarget :: struct {device:TdDevice;compiler:[4]u64;backend:[4]u64;kernel:[4]u64;operators:[4]u64;budget:int=67108864;}
```

Node indices are value IDs. Every operand must precede its consumer, making
this a straight-line single-assignment program: each materialized node writes
a new logical value exactly once. An input describes a region of a caller's
packed binding array. Its storage length, shape, signed strides and offset
are validated before reading values. Read-only input regions may overlap.

`placement=target` makes a graph portable across the three backends. An
explicit CPU/CUDA/Metal placement must agree with the selected target. There
are no implicit copies between devices. The native runner selects CUDA
device 0 or the default Metal device, as in the first device profile.

`TiTarget` carries SHA-256 identities for the producing Coax compiler, backend
and generated module, plus the buffer budget. The trusted builder supplies
the compiler digest; the native runner obtains the backend description from
the driver and uses Coax SHA-256 to hash that description and the actual module
bytes. CPU execution uses the host compiler/ABI description and an empty module
identity. This is process-local cache identity, not remote execution attestation.

The low-level lowering/kernel helpers require an admitted plan. The checked
Coax executor and opaque native API are the entry points for caller-supplied
graph descriptors.

The first operation set is:

| Operation | Admitted semantics |
| --- | --- |
| `input` | A finite f32 storage region and a validated rank-0–4 read-only view. |
| `matmul` | Rank-two `[M,K] × [K,N] → [M,N]`, including a zero inner dimension. |
| `add`, `multiply` | Right-aligned broadcasting under chapter 5's shape rules. |
| `relu` | Elementwise positive part, including canonical positive zero. |
| `sum` | Reduce one admitted axis, dropping that axis; an empty sum is zero. |
| `layernorm` | Normalize the last axis with population variance and positive finite epsilon inside the root; no affine scale/bias. |
| `view` | A read-only root-relative shape, offset and strides, including transpose, reverse and zero-stride broadcast. |
| `reshape` | Equal logical element count and contiguous source order; no hidden materialization. |
| `gather` | Gather along one axis using up to 16 constant, prevalidated indices. |

Computed values have contiguous storage; views and reshapes retain their root.
The root-relative `view` operation is an explicit strided view of the whole
allocation, rather than a promise that it stays within a parent view's logical
subset. Every referenced address must still lie inside the root storage.

Limits are 32 nodes, rank four, 1,048,576 elements per storage/logical view,
and 1,048,576 aggregate scalar-work units. Total flag entries and fresh
materialized values are also bounded. The caller sets a buffer budget of at
most 1 GiB; the default is 64 MiB. Empty tensors require no data reads.

## Express a block once

Our block computes two affine maps with a ReLU between them:

```jai
hidden = relu(input * first_weight + first_bias);
output = hidden * second_weight + second_bias;
```

The matrix convention here stores weights as `[input_width, output_width]`.
Five input nodes and five arithmetic nodes describe it:

<!-- source: psionic/lib/tensor-ir.coax fixture -->
```jai
ti_fixture :: ()->TiProgram #export_c {
    p:TiProgram;p.count=10;p.output=9;
    p.nodes[0]=ti_input(TvShape.{rank=2,dims=[4]int.[2,3,0,0]},0);
    p.nodes[1]=ti_input(TvShape.{rank=2,dims=[4]int.[3,4,0,0]},6);
    p.nodes[2]=ti_input(TvShape.{rank=1,dims=[4]int.[4,0,0,0]},18);
    p.nodes[3]=ti_input(TvShape.{rank=2,dims=[4]int.[4,2,0,0]},22);
    p.nodes[4]=ti_input(TvShape.{rank=1,dims=[4]int.[2,0,0,0]},30);
    p.nodes[5]=TiNode.{op=TiOp.matmul,a=0,b=1};
    p.nodes[6]=TiNode.{op=TiOp.add,a=5,b=2};
    p.nodes[7]=TiNode.{op=TiOp.relu,a=6};
    p.nodes[8]=TiNode.{op=TiOp.matmul,a=7,b=3};
    p.nodes[9]=TiNode.{op=TiOp.add,a=8,b=4};return p;
}
```

For the tiny fixture, the input is `[2,3]`, the hidden width four and the output
width two. The graph itself is identical on CPU, CUDA and Metal. Target
selection, allocation and launch binding happen outside its operation list.

The example's checked Coax entry recompiles/revalidates its metadata for each
call. The native `Pti` object instead copies the graph, compiles its plan once,
lowers commands once, opens the selected backend and allocates its five buffers
once. `pti_run` replays those fixed commands with new finite input bindings.
The CPU reference remains an interpreter over the existing CPU procedures.

## Turn use counts into reusable storage

A last-use pass follows every operand to its storage root. A use through a
view extends the root's lifetime. The selected output's root stays live until
publication, even when the output node precedes other nodes in the list.

The planner then chooses an expired slot with enough capacity, preferring
the smallest fitting slot. Otherwise it adds a slot. A slot expires only when
its last use is **strictly before** the current node: a node cannot overwrite
an operand that it is still reading. Input storage is never reused for writes.
This is bounded best-fit reuse; there is no claim of an optimal allocator.

| Materialized value | Floats | Last use | Workspace offset, floats |
| --- | --- | --- | --- |
| First matrix product, node 5 | 8 | 6 | 0 |
| Bias addition, node 6 | 8 | 7 | 8 |
| ReLU, node 7 | 8 | 8 | 0 |
| Second matrix product, node 8 | 4 | 9 | 8 |
| Final bias addition, node 9 | 4 | Publication | 0 |

Fresh storage needs 32 intermediate floats. Two eight-float slots need 16.
The native allocation tracker observes **64 workspace bytes versus 128** for
fresh intermediates. The larger 32×48→64→32 fixture similarly uses 4,096
workspace floats instead of 8,192.

These are measured allocation-request bytes, not inferred process RSS or
reserved driver VRAM. The five persistent buffers hold input storage,
workspace, staged output, numerical flags and command metadata. The tiny
planned object requests 3,316 bytes across those buffers. Object headers,
host readback/metadata copies, per-launch driver argument/check buffers,
contexts/modules and allocator rounding
are outside this buffer budget. Their exclusion is explicit; this is not a
whole-process memory limit.

## Three execution paths

**CPU reference.** Fresh intermediates preserve each node's value. The
interpreter calls chapter 5's operator evaluator with the admitted views;
normalization uses compensated f64 population-statistic equations. Results
narrow to f32 once per node. This supplies a numerical reference independently
of the new planned kernel's f32 accumulation order.

**Planned CPU.** The immutable command list uses the shared Coax kernel and
reuses its admitted workspace slots. Coax supplies operation IDs, operand
address metadata, output offsets, item counts and flag offsets. The native
host executes that list without doing tensor arithmetic or choosing lifetimes.

**CUDA/Metal.** The same Coax kernel emits through chapter 37's compute subset.
The host binds persistent device buffers and launches the admitted commands
in order. Each work item writes one logical result. A final gather command
copies the selected output view into the independent staging buffer. There
is no host arithmetic fallback for an unsupported device.

This first lowering uses a generic opcode kernel, rather than generating a
specialized fused kernel for each model. It establishes semantics and memory
planning; it makes no throughput or tensor-core claim. Inputs, including model
parameters, are supplied as a packed binding array for each run. Direct binding
of chapter 6's opaque `PtdTensor` handles and keeping selected parameters resident
across runs need a later binding layer; there is no hidden device readback here.

## Keep publication separate from workspace writes

Every arithmetic command writes its own numerical flag interval. Reusing a
workspace slot never overwrites the earlier flags. A finite input that causes
an overflowing product, accumulation, variance or output produces an arithmetic
error. Later commands may run on private placeholder values, but one failed
flag refuses publication of the entire result.

The final copy stages the logical output, including strided aliases. Coax
checks all flags before copying to caller storage. Data errors leave the caller's
output and tail unchanged; private workspace/staging may change. Native address
observations reject output overlap with input bindings, metadata or owned host
buffers before execution. Invalid graphs and insufficient capacities/budgets
are refused before tensor execution.

A `Pti` object owns its graph, plan, module and buffers until `pti_close`.
Runs are synchronous with explicit driver waits inside the host; CUDA transfer
failure paths also attempt to drain submitted work before releasing borrowed
spans. The host API requires exclusive access and truthful pointer extents.
Numerical errors leave it reusable. Driver errors poison further replays;
close can still release an idle poisoned object. A real GPU driver fault is
not injected by this chapter's tests.

## A cache must know what changed

`pti_matches` compares the owned plan with a proposed graph and target. The
comparison includes every active node's operation, dependencies, shape,
strides, offsets, storage/binding metadata, dtype, placement and parameters,
plus target, budget, compiler, backend and module identities. A changed shape,
layout, compiler or device returns a miss, requiring an explicit new plan.

The graph is copied at open, so mutating the caller's original descriptor
cannot alter an already compiled object. Running that object always runs its
original graph. There is no on-disk cache and no automatic dynamic-shape
specialization yet; chapter 39 builds those on this contract.

## Check values and plans independently

The [tooling oracle](../tests/tensor_ir_oracle.py) builds twelve graph fixtures
and evaluates their equations with Decimal(100), independently of Psionic's
shape inference, liveness pass or kernels. They cover the two feed-forward
sizes, broadcasting/multiply, reverse/transposed views, reshapes, gathers,
normalization, reductions, alias lifetime, empty tensors and scalar/view outputs.

The [native harness](../tests/tensor_ir.c) compares the CPU reference and every
planned backend with those answers. Three replays per graph retain exactly
five tensor-buffer allocations. It also checks stale identities, actual memory
aliasing, invalid graphs/dtypes/budgets, non-finite inputs, output preservation,
overflow recovery and lifecycle poisoning. Native debug/release and ASan/UBSan
cover host code; they do not instrument GPU memory accesses.

The declared comparison is
`abs(actual-reference)/max(1,abs(reference)) ≤ 0.0001`. The CPU reference uses
wide compensated arithmetic while the plan uses sequential f32 arithmetic,
so general results need not be bit-identical. Metal inherits chapter 6's
subnormal-flush behavior. The exact normal tiny fixture and measured errors
for the larger fixtures are recorded separately from this frozen bound.

This representation is now a concrete input for fusion, general reverse mode
and reusable model APIs. Those transformations can preserve this chapter's
value, alias, lifetime and cache checks as they add capabilities.
