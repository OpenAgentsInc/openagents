# 45a. Original weights stay on the GPU

[Chapter 45](ch45-execute-gpt-oss-20b.md) established a complete numerical
reference. Usable inference also needs a different execution layout: keep the
weights and working state resident, and move the computation to them. We start
with original gpt-oss-20b on the owned RTX 4080. The 120b model needs a larger
resident-memory target; the planner refuses it on 16 GiB before allocation.

## Count the complete session

The [Coax planner](../lib/gptoss/device-plan.coax) is the authority for weight
packing, workspace sizes, the reserve and every kernel argument. C executes
its allocations, byte copies and launches. The complete original artifact
identity and parameter interval checks remain those of chapter 45. Authentication
happens once per session, followed by one upload of each parameter.

Tensor bytes are copied unchanged into eight-byte-aligned intervals. This
alignment matters: a valid BF16 tensor can begin two bytes past an aligned
address in a Safetensors file. A CUDA view of packed 64-bit words must not
borrow that unaligned address. Opaque relocation changes the address, preserving
every original BF16, packed E2M1 and scale byte. There is no matrix expansion,
conversion or requantization.

The [runnable plan example](../examples/gptoss-device-plan.coax) needs no GPU:

<!-- source: psionic/examples/gptoss-device-plan.coax example -->
```jai
main :: () {
    s:=g_original_shape(0,2048);pins:=G20_ARTIFACTS;
    p:=gd_plan(s,pins[..],17179869184,17179869184);
    #assert p.status==GStatus.ok;
    print("original 20b packed weight bytes: %\n",p.weights);
    print("resident workspace bytes: %; reserve bytes: %\n",p.workspace,GD_RESERVE);
    print("prefill chunk: % tokens; generated kernel entry points: %\n",p.batch,gd_kernel_count());
    print("1024 prompt tokens: % full chunks; final tail at 1023: %\n",1024/p.batch,gd_chunk(1023,1024,p.batch));
    large:=G120_ARTIFACTS;
    #assert gd_plan(g_original_shape(1,2048),large[..],17179869184,17179869184).status==GStatus.budget;
    print("120b on 16 GiB: refused before allocation\n");
}
```

Its exact output is:

```jai
original 20b packed weight bytes: 13761300984
resident workspace bytes: 527933440; reserve bytes: 134217728
prefill chunk: 128 tokens; generated kernel entry points: 43
1024 prompt tokens: 8 full chunks; final tail at 1023: 1
120b on 16 GiB: refused before allocation
```

The 128 MiB reserve is an admission requirement, separate from workspace.
Actual free memory is observed after driver/module setup and checked against
this plan. The display server, CUDA context and driver allocations consume
memory too; a nominal 16 GiB card does not expose 16 GiB free. Failed allocation
is still a platform failure. A plan cannot reserve memory against another
process allocating concurrently.

## Read selected packed coefficients directly

The [kernels](../lib/gptoss/device-kernels.coax) decode BF16 with its exact bit
shift. Each original MXFP4 group supplies 32 coefficients, two 64-bit packed
words and one E8M0 scale. The low/high nibble order is the chapter 41 order.
All sixteen E2M1 codes, negative zero and the smallest admitted scale have
hand-derived checks. Invalid scale bytes become numeric failure flags.

A selected expert's matrix product reads its original blocks directly. It
never creates a dense copy of the expert or transfers its route to the host.
The `gpt-oss-cuda-f32-v1` arithmetic profile accumulates short products in f32
lanes, then combines lanes in an ordered f64 reduction and stores f32.
It has its own numerical qualification; chapter 45's wide CPU profile remains
an independent reference.

## Check the real path cheaply

```shell
COAX=coax/bin/coax psionic/tests/check_gptoss_device.sh
PSIONIC_CUDA=1 COAX=coax/bin/coax psionic/tests/check_gptoss_device.sh
```

The default check runs the same Coax kernels compiled as C against owned tiny
weights, in debug/release with ASan/UBSan. The explicit CUDA check needs a
`coax cuda` compiler and NVIDIA driver/NVRTC. A missing device refuses execution;
there is no hidden CPU fallback. Original weights are an additional opt-in.
Full mutation roots and whole-original CPU decisions are excluded from this
iteration path at the owner's direction.

Continue with [the complete CUDA decoder](ch45b-a-complete-cuda-decoder.md).
