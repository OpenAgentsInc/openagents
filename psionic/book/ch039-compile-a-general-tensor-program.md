# 39. Compile a general tensor program

A fixed graph need not mean a fixed batch. Chapter 38's kernel module can
execute different shapes without compiling new machine code. This chapter
adds a bounded symbolic template, guards, declared branch/loop specialization
and a four-entry LRU plan cache. The [Coax policy](../lib/tensor-template.coax)
chooses concrete graphs and cache victims; the [native runner](../platform/tensor-template-runtime.c)
executes those choices and shares one immutable backend module.

## Run it

```shell
coax/bin/coax run psionic/examples/dynamic-tensor-program.coax
COAX=/path/to/qualified/coax psionic/check.sh --only tensor-template
COAX=/path/to/qualified/coax psionic/tests/check_tensor_template.sh --metal
LD_LIBRARY_PATH="$HOME/cuda-13.2/lib:${LD_LIBRARY_PATH:-}" \
  COAX=/path/to/qualified/coax psionic/tests/check_tensor_template.sh --cuda
```

The [example](../examples/dynamic-tensor-program.coax) prints these concrete
bindings from one template:

```jai
batch 1: 29 input floats, 2 output floats; repeat is a cache hit: true
batch 2: 32 input floats, 4 output floats; repeat is a cache hit: true
batch 4: 38 input floats, 8 output floats; repeat is a cache hit: true
batch 8: 50 input floats, 16 output floats; repeat is a cache hit: true
One backend module; shape misses lower plans, not new kernels.
```

## Declare the variation

<!-- source: psionic/lib/tensor-template.coax types -->
```jai
TgSymbol :: struct {minimum:int=1;maximum:int=1;step:int=1;}
TgTemplate :: struct {symbol_count:int;symbols:[4]TgSymbol;programs:[2]TiProgram;choice:bool;max_repeat:int;loop:TiNode;}
TgBinding :: struct {symbols:[4]int;branch:bool;repeat:int;}
TgBound :: struct {status:TiStatus;program:TiProgram;}
TgEntry :: struct {live:bool;binding:TgBinding;bytes:int;stamp:int;}
TgCache :: struct {entries:[4]TgEntry;clock:int;bytes:int;limit:int;}
TgSelect :: struct {status:TiStatus;hit:bool;slot:int;evict:u32;next:TgCache;}
```

A negative shape dimension `-1` refers to symbol zero, `-2` to symbol one,
and so on. Up to four integer symbols have minimum, maximum and step guards.
The whole Cartesian product, branches and repeat counts together may describe
at most 64 variants. Open validates **every** admitted variant, including an
unselected branch. A source program that is valid only on some allowed shapes
is refused rather than leaving a deferred failure in its cache.

Template inputs are packed logical tensors. Their storage, contiguous strides
and binding offsets are derived after symbol substitution; those inherited
input fields from the source IR are not user-selected strided bindings. Use
explicit read-only `view` nodes for striding, transposes or broadcasting.
Reshape and view shape fields can contain symbols too. Output shapes follow
normal operator inference. Dtypes remain f32 and placement remains explicit.

The declared control-flow subset is a boolean choice between two complete
acyclic programs and zero to four repetitions of a unary ReLU/LayerNorm body
after the selected output. The repeat body is unrolled into SSA nodes. Branch
and trip-count bindings are part of the specialization key. They are caller
control values, not a condition read from a tensor on the CPU; tensor-dependent
branches, unbounded loops and arbitrary Coax control flow are outside this
profile.

The fixture varies the first matrix's batch from one through eight. Its
branches choose ReLU or a square activation, and its optional final loop applies
ReLU. The original affine weights and biases keep fixed shapes.

## Compile code once, specialize plans as needed

`ptg_open` opens one small module owner. `pti_derive` creates concrete plans
with their own five payload buffers while borrowing that owner's compiled
CUDA/Metal kernels. Coax module leases prevent closing the owner while any
specialization exists. Closing a specialization returns its lease; cache close
releases all entries before the owner. CPU uses the already compiled native
Coax kernel and requires no runtime module compilation.

A miss resolves symbols, validates/infer shapes and lowers a plan. A hit uses
its existing immutable command metadata and workspace. The cache is limited
to four plans and an aggregate payload budget that includes its owner. LRU
victims are chosen by Coax and released before replacement allocation. A
replacement allocation failure may leave entries evicted, but publishes no
output and installs no incomplete plan.

The cache binds the copied template and the compiler/backend/module identities
from chapter 38. Within one object, changing an admitted batch, branch or
repeat may lower a new plan; it does not recompile backend code. Eviction loses
a plan and its buffers, so revisiting that binding lowers/allocates again.
Changing source, guards, dtype, layout policy, compiler or device requires a
new template object; `ptg_matches` rejects the old identity. No on-disk machine
code cache or cross-process compatibility is claimed.

Inputs, capacities, overlap and finite values are checked before execution.
The same staged publication, synchronous waits, numerical-error recovery and
driver poisoning rules as 38 apply. Driver/control overhead and host metadata
remain outside the tracked payload budget. Host access is exclusive.

## Check the specializations, not just the template

The [independent Decimal oracle](../tests/tensor_template_oracle.py) evaluates
16 batch/branch/loop combinations. Each runs twice on CPU, CUDA and Metal.
The [native harness](../tests/tensor_template.c) observes 16 hits, 16 created
specializations, 12 evictions and exactly one backend compilation on each GPU.
Every tested answer is exact for these binary-valued fixtures; the inherited
comparison ceiling remains 0.0001 scaled error.

Checks cover rejected symbols/steps, unused symbols, bounds, future operands,
invalid unselected branches, budget decisions, real native metadata aliases,
stale template/compiler keys and refusal to unload a leased module. Native
ASan/UBSan checks host code; GPU memory is not instrumented. Metal retains the
qualified release emission from 38. The [verification record](../../docs/verification/psionic-ch26b-2026-10-29.md)
records build modes and hardware. Full mutation runs are omitted under the
owner's focused-check instruction.
