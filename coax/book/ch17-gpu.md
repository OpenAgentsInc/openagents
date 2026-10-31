# GPU programs

The OpenAgents iOS app draws its home screen and the Grid with Metal.
Every glowing node, every line, the hotbar and the tone curve that maps
scene light to the screen are small functions that run once per pixel on
the GPU. Those functions are math, and math is exactly what we want to
test. But shader code is hard to test: it runs on another processor,
inside a draw call, with no `print` and no assertions.

`coax0` has a narrow answer to that today. The command `coax metal`
compiles the `f32` and `bool` procedures of a Coax file to Metal Shading
Language. The same procedures can be tested on the CPU with `coax test`
and called from C, so a fragment shader and a unit test run the same
source. In this chapter we'll write a small light library, test it,
emit it as Metal, wire it to a hand-written vertex and fragment stage,
and then look at how the real app does the same thing with
`verse/light.coax`. We'll end with the broader GPU layout and
shader-declaration designs that remain separate work.

> Note: `coax metal` and its scalar profile work in `coax0` 0.10.0, and
> the iOS app ships with it. The `@gpu` struct and `shader` procedure
> syntax under "Planned" remains design work (spec §15). The bounded
> WGSL graphics command below is implemented through ordinary checked
> Coax procedures and separate entry/binding inputs.
>
> A separate compute profile, `coax cuda` (0.23.0), compiles `#export_c`
> kernels with loops, integers, fixed arrays and views to CUDA C. It is
> described in the spec (§15) and used by the Psionic book's chapters 37 and
> 44; this chapter stays with the Metal scalar profile.

<!-- capability-claim: coax.wgsl.current -->
A separate bounded graphics path is implemented:
`coax wgsl FILE --vertices A,B --fragments C,D --bindings F [--strip P]`
writes a WGSL module from ordinary checked Coax procedures. Its admitted
subset includes vectors, records, graphics entry wrappers and resource
bindings; unsupported entry shapes, bindings and layouts are refused.
Everglade's main graphics module is generated from `verse/everglade/gpu.coax`
with `gpu.module` and `gpu.bindings`, and its particle module from
`gpu-fx.coax` with `gpu-fx.module` and `gpu-fx.bindings`. Other realm shaders
still have their separately documented paths. This does not implement the
planned `@gpu`/`shader` syntax, a general CPU/GPU layout model, or a qualified
Psionic browser compute backend. Device and numerical evidence remains scoped
to each actual consumer.
<!-- /capability-claim -->

## What `coax metal` accepts

Before we write anything, here are the rules, because they shape the
code. A file you compile with `coax metal` must stay inside what the
compiler calls the Metal scalar profile:

- Every value, parameter, local and return type is `f32` or `bool` (or
  no return value at all).
- Control flow is `if` and `else`. There are no loops.
- Procedures may call each other, as long as no call chain is recursive.
- The built-in math functions `sqrt`, `exp`, `log`, `pow`, `sin`, `cos`,
  `tan`, `floor` and `abs` are allowed; they map to Metal's own.
- No strings, no integers, no `f64`, no `print`, and no `#foreign` calls.

That's a small language. It's still enough to write the light of a
glowing dot, a smooth step, a signed-distance rounded rectangle or a
tone curve, which is most of what a 2D effect shader computes. What it
leaves out, on purpose, is everything that touches GPU resources:
buffers, textures, samplers, vertex attributes and vector types. Those
stay in Metal, in a small hand-written file we'll meet shortly.

## A light library

Let's write the light for one glowing dot. The fragment shader will call
it with a position on a square quad running from -1 to 1 on each axis,
and it returns how much light that pixel gets: a bright core, a soft
halo, and nothing at the quad's edge so the square never shows. We'll
also want a tone curve to turn scene light into a screen value.

Filename: examples/lib/ch17_glow.coax

```jai
// A glowing dot and a tone curve, for a fragment shader to call.
// Every value is an f32 or a bool, and nothing loops.

lo :: (a: f32, b: f32) -> f32 { if a < b { return a; } return b; }
hi :: (a: f32, b: f32) -> f32 { if a > b { return a; } return b; }
sat :: (a: f32) -> f32 { return lo(1.0f32, hi(0.0f32, a)); }

// 0 below edge0, 1 above edge1, a smooth S between.
smooth :: (edge0: f32, edge1: f32, x: f32) -> f32 {
  t := sat((x - edge0) / (edge1 - edge0));
  return t * t * (3.0f32 - 2.0f32 * t);
}

// Light at (x, y) on a quad that runs from -1 to 1: a bright core,
// a soft halo, and nothing at the quad's edge.
glow_dot :: (x: f32, y: f32, light: f32) -> f32 #export_c {
  r2 := x * x + y * y;
  if r2 >= 1.0f32 { return 0.0f32; }
  core := exp(-r2 * 12.0f32);
  halo := 0.2f32 * exp(-r2 * 3.0f32);
  fade := 1.0f32 - smooth(0.8f32, 1.0f32, sqrt(r2));
  return light * (core + halo) * fade;
}

// Map scene light (0 and up) to a screen value below 1.
glow_tone :: (light: f32) -> f32 #export_c {
  return 1.0f32 - exp(-light * 1.35f32);
}

#test "the dot is brightest in the middle and dark at its edge" {
  #assert glow_dot(0.0f32, 0.0f32, 1.0f32) == 1.2f32;
  #assert glow_dot(1.0f32, 0.0f32, 1.0f32) == 0.0f32;
  #assert glow_dot(0.0f32, 0.99f32, 1.0f32) < 0.01f32;
}

#forall "the dot fades outward" i: 0..<20 {
  r := f32(i) / 20.0f32;
  #assert glow_dot(r + 0.05f32, 0.0f32, 1.0f32) <= glow_dot(r, 0.0f32, 1.0f32);
}

#forall "tone never reaches 1" i: 0..<100 {
  t := glow_tone(f32(i) * 0.1f32);
  #assert t >= 0.0f32 && t < 1.0f32;
}
```

*Listing 17-1: A light library inside the Metal scalar profile*

This file has no `main`. It's a library, and it will be compiled three
ways: as Metal for the GPU, as C for a CPU host, and by the checker for
its tests. That's also why it lives in `examples/lib/` rather than next
to the runnable examples.

The helpers come first. `lo` and `hi` are minimum and maximum, written
with `if` because a shader has no other branch. `sat` clamps to the
range 0 to 1 (shader programmers call that "saturate"), and `smooth` is
the familiar smoothstep curve built from it.

Every literal is written with its type, as in `1.0f32`. In this file
that matters: a bare `1.0` is an untyped constant that would happily
become an `f64`, and `f64` is outside the profile.

`glow_dot` does the real work. It computes the squared distance `r2`
from the quad's center and returns no light at all outside the unit
circle. Inside, the core falls off fast (`exp(-r2 * 12)`), the halo
falls off slowly at a fifth of the strength, and `fade` takes both down
to zero over the outer fifth of the radius. `glow_tone` is a simple
exponential tone curve: zero light stays black and any amount of light
approaches, but never reaches, full white.

The two procedures marked `#export_c` are the library's public surface.
In the Metal output they become ordinary non-`static` functions that a
fragment shader can call by name. The helpers stay `static`.

## Testing it on the CPU

The three checks at the bottom are why we wrote this in Coax rather than
straight in Metal. The first `#test` pins the center value and the edge.
The two `#forall` checks sweep inputs: the dot must never get brighter
as we move outward, and the tone curve must stay in [0, 1) for light
from 0 to 9.9. [Chapter 11](ch11-writing-checks.md) covers these
checks in depth.

```shell
$ cd examples/lib
$ coax test ch17_glow.coax
ok   #test the dot is brightest in the middle and dark at its edge (1 case, 207 steps)
ok   #forall the dot fades outward (20 cases, 3685 steps)
ok   #forall tone never reaches 1 (100 cases, 2402 steps)
3 checks, 3 passed, 0 failed
```

All three run inside the compiler's evaluator, with the same
one-rounding-per-operation arithmetic described in
[Chapter 16](ch16-determinism.md). No GPU is involved.

## Emitting Metal

Now let's see what the GPU gets:

```shell
$ coax metal ch17_glow.coax
/* Generated by coax0 from ch17_glow.coax. Do not edit: change the .coax file. */
#include <metal_stdlib>
using namespace metal;

static float coax_p_lo(float l_a, float l_b);
static float coax_p_hi(float l_a, float l_b);
static float coax_p_sat(float l_a);
static float coax_p_smooth(float l_edge0, float l_edge1, float l_x);
float glow_dot(float l_x, float l_y, float l_light);
float glow_tone(float l_light);

// ...

float glow_dot(float l_x, float l_y, float l_light) {
  float l_r2 = ((l_x * l_x) + (l_y * l_y));
  if (l_r2 >= 1.0f)
  {
    return 0.0f;
  }
  float l_core = exp(((-l_r2) * 12.0f));
  float l_halo = (0.200000003f * exp(((-l_r2) * 3.0f)));
  float l_fade = (1.0f - coax_p_smooth(0.800000012f, 1.0f, sqrt(l_r2)));
  return ((l_light * (l_core + l_halo)) * l_fade);
}

float glow_tone(float l_light) {
  return (1.0f - exp(((-l_light) * 1.35000002f)));
}
```

*Listing 17-2: Part of the Metal that `coax metal` prints for Listing 17-1*

It looks a lot like the C that `coax c` produces, and it comes from the
same checked tree. Internal procedures get a `coax_p_` prefix and are
`static`; exported ones keep their names. Every subexpression is
parenthesized, so Metal's precedence rules can't change the meaning.

Notice the constants, too: `0.2f32` became `0.200000003f`. That's not a
rounding error introduced by the compiler. `0.2` has no exact binary
representation, and `0.200000003` is the nearest `f32`, printed with
nine significant digits, which is enough for any `f32` to survive the
trip to text and back. (The C emitter writes hexadecimal float
literals, which are exact; Metal gets decimal text.)

The `#test` and `#forall` checks don't appear. They belong to the CPU.

## Wiring up the stages

The generated functions compute light, but they don't know about
vertices, instances, buffers or render targets. That's the job of the
stage code, which we write by hand in Metal. Here's a vertex stage that
draws one quad per dot, and a fragment stage that calls our library:

Filename: examples/lib/ch17_stage.metal

```c
// Hand-written stage code: vertex layout, buffers and the quad.
// The light itself comes from ch17_glow.coax.
struct Dot { float4 a; };   // x, y, radius, light
struct View { float2 size; };
struct DotOut { float4 position [[position]]; float2 uv; float light; };

constant float2 corners[6] = {{-1,-1},{1,-1},{-1,1},{-1,1},{1,-1},{1,1}};

vertex DotOut dot_vertex(uint id [[vertex_id]], uint inst [[instance_id]],
                         const device Dot* dots [[buffer(0)]],
                         constant View& v [[buffer(1)]]) {
  Dot d = dots[inst];
  float2 uv = corners[id];
  float2 p = d.a.xy + uv * d.a.z;
  return {float4(p.x / v.size.x * 2 - 1, 1 - p.y / v.size.y * 2, 0, 1), uv, d.a.w};
}

fragment float4 dot_fragment(DotOut in [[stage_in]]) {
  float c = glow_tone(glow_dot(in.uv.x, in.uv.y, in.light));
  return float4(c, c, c, 1);
}
```

*Listing 17-3: Metal stages that call the generated light functions*

`dot_vertex` reads one `Dot` per instance from buffer 0 and emits the
six corners of a quad around it, passing each corner's position on the
quad (`uv`, from -1 to 1) and the dot's brightness on to the fragment
stage. `dot_fragment` hands those values to `glow_dot` and `glow_tone`
and writes a gray pixel. Everything with a `float2`, `float4`, `[[...]]`
attribute or buffer index is here; everything that's just arithmetic is
in Coax.

The two parts become one Metal source by plain concatenation, generated
code first so the functions are declared before the stages use them. To check that the
result really is valid Metal, compile it with Apple's Metal compiler:

```shell
$ coax metal ch17_glow.coax > Glow.metal
$ cat ch17_stage.metal >> Glow.metal
$ xcrun -sdk macosx metal -c Glow.metal -o Glow.air
```

No output means it compiled. (The `metal` command comes with Xcode; on
some installs it's an optional component.)

## What the profile refuses

Let's step outside the profile and see what happens. Suppose we want
several rings of dots and add this to Listing 17-1:

```jai
glow_ring :: (x: f32, y: f32, rings: int) -> f32 #export_c {
  total := 0.0f32;
  for i: 0..<rings {
    total += glow_dot(x * f32(i), y, 1.0f32);
  }
  return total;
}
```

```shell
$ coax metal ch17_glow.coax
ch17_glow.coax:46:31: error[E0377]: a int parameter is outside the Metal scalar profile
  glow_ring :: (x: f32, y: f32, rings: int) -> f32 #export_c {
                                ^
  fix: Metal libraries use f32 and bool, if/else and calls; no loops, strings, integers or host calls
ch17_glow.coax:48:3: error[E0377]: a loop is outside the Metal scalar profile
    for i: 0..<rings {
    ^
  fix: Metal libraries use f32 and bool, if/else and calls; no loops, strings, integers or host calls
2 errors
```

Two problems, both reported: the `int` parameter and the loop. The fix
line tells you what the profile allows, and `coax explain E0377` says a
bit more:

```shell
$ coax explain E0377
E0377: outside the Metal scalar profile
  coax metal accepts f32 and bool values, if/else, locals and acyclic calls. Loops, strings, integers, f64, print, #assert and #foreign calls stay in the C library.
```

Interestingly, `coax check ch17_glow.coax` accepts this version without
complaint. `glow_ring` is perfectly good Coax; it just can't be a Metal
function. The profile is checked only when you ask for Metal output, so
a file can be valid as a C library and still be refused by
`coax metal`. If a loop really belongs in a shader, it goes in the
hand-written stage code. The real app's skinning loop, which blends four
joint matrices per vertex, lives in its Metal stages for exactly this
reason.

Mixing `f64` in by accident is caught earlier, by the ordinary type
checker. Change the tone curve's constant to `1.35f64` and even
`coax check` stops:

```shell
$ coax metal ch17_glow.coax
ch17_glow.coax:27:30: error[E0309]: '*' needs two numbers of one type, found f32 and f64
    return 1.0f32 - exp(-light * 1.35f64);
                               ^
  fix: convert one: cast(f32) x
1 error
```

And a file that imports protocol codecs (Chapter 18) can't be compiled
as Metal at all; the compiler says `error[E0504]: proto codecs cannot be
compiled as Metal` at the import.

## In the real world: `light.coax`

The iOS app does all of this at a larger scale. `verse/light.coax`
holds 18 exported light functions and their helpers: the sky gradient,
the home screen's dots and lines, the HUD bar with its sweeping glint,
bloom, the tone curve, sRGB decoding and encoding, the Grid's sun-and-sky
lighting, the robots' line widths, and the hotbar slots with their
icons. Here's one of the smaller ones, the conversion from a display
(sRGB) value to linear light:

```jai
verse_decode :: (v:f32)->f32 #export_c {
  if v<=0.04045f32 { return v/12.92f32; }
  return pow((v+0.055f32)/1.055f32,2.4f32);
}
```

The file ends with seven checks, among them that encoding undoes
decoding, that robot lines thin with distance but never drop under a
pixel, and that the dance icon has a head, a stem and a flag where you'd
expect them. From the repository root:

```shell
$ coax test verse/light.coax
ok   #test display values round trip through linear light (1 case, 1070 steps)
ok   #test unlit light tones back to its display value (1 case, 826 steps)
ok   #test the sun and sky light a surface as aethyr's Grid (1 case, 470 steps)
ok   #forall robot lines thin and dim with distance, never under a pixel (180 cases, 78791 steps)
ok   #test robot line widths and light at their ends (1 case, 1316 steps)
ok   #test a hotbar slot: glass inside, nothing far outside, brighter picked and pressed (1 case, 7363 steps)
ok   #test the dance icon is a note: a head, a stem, a flag (1 case, 1779 steps)
7 checks, 7 passed, 0 failed
```

The stages live in `apps/openagents-ios/OpenAgents/Platform/Stages.metal`,
which begins with the comment "All scalar visual math is generated from
Coax." Its fragment stages are mostly one line that unpacks vectors and
calls a generated function:

```c
fragment float4 dot_fragment(DotOut in [[stage_in]]) {float c=verse_dot(in.uv.x,in.uv.y,in.light,in.soft,in.crisp);return float4(c,c,c,0);}
```

The app's build script, `apps/openagents-ios/generate.sh`, joins the two
exactly as we did:

```shell
"$compiler" metal verse/light.coax > "$out/HomeShaders.txt.tmp"
cat apps/openagents-ios/OpenAgents/Platform/Stages.metal >> "$out/HomeShaders.txt.tmp"
mv "$out/HomeShaders.txt.tmp" "$out/HomeShaders.txt"
```

The result is bundled as a text resource, and the renderer compiles it
when the app starts, in `HomeRenderer.swift`:

```swift
let library = try device.makeLibrary(source: String(contentsOf: source, encoding: .utf8), options: nil)
```

So the app needs no offline Metal toolchain at build time. The cost is a
shader compile at launch, which [Chapter 19](ch19-builds-hot-reload.md)
counts as part of the loop you actually wait on.

> Note: The CPU checks and the GPU don't promise bit-identical results.
> `options: nil` means Metal's default compile options, which allow fast
> math, and Metal's `exp` and `pow` aren't the C library's. The checks
> above test the math the shader is meant to compute; they don't prove
> the GPU computes it to the last bit. That's fine for light, and it's
> why simulation state stays on the CPU (Chapter 16).

## The problem that's left: layouts written twice

`coax metal` shares functions, not data. Look at how one dot gets from
the CPU to the GPU in the real app. The Coax home calls the host's
`host_dot` with six numbers. The C host packs them into this struct,
from `verse/platform/host.h`:

```c
typedef struct { float x,y,size,light,soft,crisp,pad0,pad1; } MRDot;
```

And the Metal stages read the same bytes as this one:

```c
struct Dot { float4 a; float4 b; };
```

Two declarations of one layout, in two languages, and nothing checks
that they agree. If someone adds a field to `MRDot` and forgets
`Dot`, nothing crashes. The GPU reads shifted bytes and draws garbage.
That's the problem the planned part of this chapter is about.

## Planned: shared GPU declarations

> Note: Not implemented. `@gpu` is a design (spec §15).

```jai
Instance :: struct @gpu { model: m4; color: v4; }
```

A `@gpu` struct would have one checked layout that follows Metal's
alignment rules (and WGSL's, for WebGPU). CPU code that fills the buffer
would use the same type, and the compiler would check offsets, strides,
alignment and upload lengths for each pipeline. The `coax layout`
command from [Chapter 13](ch13-data-layout.md) already prints CPU
layouts; a GPU layout would be reported the same way and recorded on
both sides.

## Planned: shader procedures

> Note: Not implemented, and not decided. See "The experiment" below.

```jai
node_vs :: (v: Vertex_In, @instance inst: Instance, @uniform frame: Frame_Uniforms) -> Vertex_Out
           shader vertex {
  out: Vertex_Out;
  out.position = frame.view_proj * inst.model * v4.{v.position.x, v.position.y, v.position.z, 1};
  out.color = inst.color;
  return out;
}
```

A `shader` procedure would be a whole stage written in a restricted
Coax: no general pointers, no allocation, no context and no recursion,
with explicit access modes for each resource. It would lower to MSL for
Metal and WGSL for WebGPU, each validated separately. The `coax metal`
scalar profile has no vectors or resources. The implemented WGSL command
uses its own admitted graphics subset, rather than this planned syntax.

## Things that stay true either way

WebAssembly isn't a GPU. Compiling Coax CPU code to Wasm for the
browser doesn't make it a shader. WebGPU takes WGSL through the
browser's API, and native Metal takes its own pipelines. Jai, which has
no shader language, shows the same split: Polygon's Jai engine uses
wgpu natively and WebGPU in the browser, with shaders kept separate.

Metal features are target features. Argument buffers, indirect command
buffers, memoryless render targets, tile shaders and imageblocks have to
be validated for the device, with stated fallbacks.

GPU resources belong to the host. Buffers, textures and pipelines are
resident objects whose queues and fences the host owns. Replacing a
shader means building a new pipeline between frames and retiring the
old one after the GPU is done with it, never reinterpreting a live
buffer under a different layout.

## The experiment

Spec stage 6 settles the open question by building one real pipeline two
ways. In the first, Coax generates the shared layouts and a header for
the existing shader language, and the shaders stay in that language. In
the second, Coax compiles `shader` procedures itself. Whichever is
cheaper to build and maintain, and catches more mismatches, wins. The
scalar profile we used in this chapter is the step both paths share:
the math is in Coax and tested, and the stage code stays in Metal.

## Summary

`coax metal` compiles a Coax file's `f32` and `bool` procedures to Metal
Shading Language. Inside that profile you get `if`/`else`, locals,
non-recursive calls and the built-in math functions, and the same file
can carry `#test` and `#forall` checks that run on the CPU. In that iOS
scalar path, stage code, vectors, buffers and loops stay in hand-written
Metal that calls the generated functions, and the two are concatenated
into one source. The separate WGSL path generates bounded graphics modules
for the named consumers. General CPU/GPU layout qualification and the
planned `@gpu`/`shader` declaration forms remain separate work.

Next we'll look at the other boundary Coax code crosses constantly: the
protocol messages OpenAgents sends and receives.

## A CPU layout consumer in Verse

The actual app's Haunted House buffer gate loads `verse/haunt-layout.coax`.
It describes the existing packed vertex as fixed arrays of f32, u8 and u32,
then derives its stride, texture-layer byte and scalar offsets with
`type_info`/`field_info`. A `#run` table supplies admission offsets; independent
checks pin the shipped 48-byte stride, four-byte alignment and colour offset.
This removes duplicated numeric layout decisions from the admission loop.

This is CPU reflection over an existing binary format. It does not infer Metal
vector alignment or rewrite a shader. The packed Metal declaration remains a
platform contract; `verse/test_haunt.sh` checks the native layout and real
bundled geometry, while the simulator test exercises the actual GPU path.
