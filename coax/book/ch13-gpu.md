# 13. GPU programs

> **Design, and an open question.** Spec §15. Whether Coax compiles
> shaders itself, or generates shared layouts for existing shader tools,
> is decided by an experiment (Chapter 20, stage 6).

## 13.1 The problem

The native OpenAgents app renders with Metal through metal-cpp. Its
shaders live in `home.metal`, and its instance buffers are filled by C++
structs in `src/metal`. The struct on the CPU and the struct in the
shader are written twice, and nothing checks that they agree. A
mismatched offset doesn't crash. It draws garbage.

## 13.2 Shared declarations

```jai
Instance :: struct @gpu { model: m4; color: v4; }
```

A `@gpu` struct has one checked layout for the GPU, with alignment rules
for MSL (and WGSL for WebGPU). The CPU code that fills the buffer uses
the same type. The compiler validates offsets, strides, alignment and
upload lengths for each pipeline, and the layout fingerprint (Chapter 10)
is recorded on both sides.

## 13.3 Shader procedures (if the experiment says so)

```jai
node_vs :: (v: Vertex_In, @instance inst: Instance, @uniform frame: Frame_Uniforms) -> Vertex_Out
           shader vertex {
  out: Vertex_Out;
  out.position = frame.view_proj * inst.model * v4.{v.position.x, v.position.y, v.position.z, 1};
  out.color = inst.color;
  return out;
}
```

A `shader` procedure is a restricted subset of Coax: no general pointers,
no allocation, no context and no recursion, with explicit resource access
modes. It would lower to MSL for native Metal and WGSL for WebGPU, each
validated separately.

## 13.4 Things that stay true either way

- **WebAssembly is not a GPU.** Compiling Coax CPU code to Wasm doesn't
  make it a shader. WebGPU takes WGSL through the browser's API, and
  native Metal takes its own compiled pipelines. Jai, which has no
  shader language, shows the same split: Polygon's Jai engine uses wgpu
  natively and WebGPU in the browser, with shaders kept separate.
- **Metal features are target features.** Argument buffers, indirect
  command buffers, memoryless render targets, tile shaders and
  imageblocks are validated for the device, with stated fallbacks.
- **GPU resources belong to the host.** Buffers, textures and pipelines
  are resident objects with explicit queue and fence ownership.
  Replacing a shader creates a new pipeline between frames and retires
  the old one after the GPU is done. It never reinterprets a live buffer
  under a different layout.

## 13.5 The experiment

Before any shader backend exists, one real pipeline gets built two ways:

1. Coax generates the shared layout and an MSL or Slang header, and the
   shader stays in its existing language.
2. Coax compiles a `shader` procedure itself.

Whichever is cheaper to build and maintain, and catches more mismatches,
wins. A shader backend has to earn its place like every other compiler
feature.
