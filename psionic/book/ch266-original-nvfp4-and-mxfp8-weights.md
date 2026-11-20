# 266. Original NVFP4 and MXFP8 weights

[Chapter 60](ch060-original-mxfp4-weights.md) decoded MXFP4 weights for
gpt-oss, and [chapter 82](ch082-original-fp8-weights.md) decoded
block-scaled FP8 weights for Mistral. Inkling 2 Mini combines two native
microscaled formats in one ~20 GB checkpoint: **NVFP4** across most layers and
**MXFP8** across the last four layers (see the
[Inkling 2 Mini target specification](../docs/targets/inkling.md)).

This chapter implements exact bounded CPU dequantization for both formats and
the layer-format admission rule in
[`psionic/lib/inkling-weights.coax`](../lib/inkling-weights.coax), demonstrated
by [`psionic/examples/original-nvfp4-mxfp8.coax`](../examples/original-nvfp4-mxfp8.coax),
so later chapters read Inkling 2 Mini's native tensors directly without
converting to a third-party quantized container.

```shell
coax/build.sh
coax/bin/coax run psionic/examples/original-nvfp4-mxfp8.coax
coax/bin/coax test psionic/lib/inkling-weights.coax
```

```jai
NVFP4: 16 packed bytes + 2 E4M3 scales (global 0.5) -> 32 f32 values
NVFP4 block 0: 0.0, 0.25, 3.0, -3.0
NVFP4 block 1: 0.0, 0.5, 6.0, -6.0
MXFP8: 32 E4M3 bytes + 1 UE8M0 scale (2^1) -> 32 f32 values
MXFP8 values: 0.0, 0.00390625, 3.0, -896.0
layer format check: layer 0 NVFP4, layer 20 MXFP8
```

## NVFP4 micro-blocks

NVFP4 packs 16 four-bit E2M1 values into 8 bytes (low nibble first, then high
nibble), paired with one unsigned finite FP8 E4M3 block scale (`0x00`..`0x7e`)
and one positive finite FP32 per-tensor global scale:

```jai
value = f32(E2M1(nibble) * E4M3(block_scale) * f64(global_scale))
```

Negative zero (`code == 8`) preserves its sign bit in the emitted `f32` word.
Invalid shapes, mismatched byte counts, aliased buffers, non-positive or
non-finite global scales, and negative or NaN block scales are rejected before
any destination byte is written.

## MXFP8 micro-blocks and layer assignment

The last four layers of Inkling 2 Mini use **MXFP8**: 32-element blocks of
FP8 E4M3 codes (`0x7f` and `0xff` NaN codes refused) sharing one UE8M0
power-of-two block scale `2^(s - 127)` with `s <= 246` so `448 * 2^(s - 127)`
stays finite in `f32`.

`inkling_layer_format(layer, total_layers)` enforces the checkpoint split:
when `layer >= total_layers - 4`, the layer's quantized weights are admitted
as `InklingWeightFormat.mxfp8`; all earlier layers are admitted as
`InklingWeightFormat.nvfp4`.

## Runnable example

<!-- source: psionic/examples/original-nvfp4-mxfp8.coax example -->
```jai
main :: () {
    packed:=[16]u8.[0x10,0x32,0x54,0x76,0x98,0xba,0xdc,0xfe, 0x10,0x32,0x54,0x76,0x98,0xba,0xdc,0xfe];
    nv_scales:=[2]u8.[0x38,0x40];
    nv_out:[32]f32;
    nv:=nvfp4_decode(1,32,packed[..],nv_scales[..],0x3f000000,nv_out[..],false);
    #assert nv.status==InklingWeightStatus.ok;
    print("NVFP4: 16 packed bytes + 2 E4M3 scales (global 0.5) -> % f32 values\n",nv.written);
    print("NVFP4 block 0: %, %, %, %\n",nv_out[0],nv_out[1],nv_out[7],nv_out[15]);
    print("NVFP4 block 1: %, %, %, %\n",nv_out[16],nv_out[17],nv_out[23],nv_out[31]);

    mx_data:[32]u8;
    for i:0..<32 {mx_data[i]=0x38;}
    mx_data[0]=0x00; mx_data[1]=0x01; mx_data[2]=0x3c; mx_data[3]=0xfe;
    mx_scales:=[1]u8.[128];
    mx_out:[32]f32;
    mx:=mxfp8_decode(1,32,mx_data[..],mx_scales[..],mx_out[..],false);
    #assert mx.status==InklingWeightStatus.ok;
    print("MXFP8: 32 E4M3 bytes + 1 UE8M0 scale (2^1) -> % f32 values\n",mx.written);
    print("MXFP8 values: %, %, %, %\n",mx_out[0],mx_out[1],mx_out[2],mx_out[3]);

    #assert inkling_layer_format(0,24)==InklingWeightFormat.nvfp4;
    #assert inkling_layer_format(20,24)==InklingWeightFormat.mxfp8;
    print("layer format check: layer 0 NVFP4, layer 20 MXFP8\n");
}
```
