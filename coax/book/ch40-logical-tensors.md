# 40. Logical tensors over chosen layouts

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R21 (spec stage 11).

A numerical program has to say two things about an array: what it means and
how it is stored. Today a Coax type says both at once, so `[N] Pulse` and
`SOA` storage need different code
([chapter 13](ch13-data-layout.md)). R21 separates them. You write indexing
against the logical shape, and the compiler lowers it through the layout you
choose. Changing the layout changes addresses and measured cost, never a
result.

## What this chapter will cover

- The five checked parts of an array declaration: logical shape, element
  type, layout, placement and owner.
- Static, bounded and dynamic dimensions, and which ones a kernel is
  specialized on.
- A batch dimension, so a solver for one world runs over many.
- Why a reordered reduction is refused as a layout change.

## Outline

1. **Meaning apart from storage.** A batch of worlds, each with bodies, each
   with a pose. Uncompiled sketch:

   ```jai
   // Sketch only: no syntax is accepted yet.
   Bodies :: tensor [worlds: dynamic, bodies: 64] Body
             layout soa, tiled(worlds, 32)
             placement device;
   ```

2. **Layouts.** Row-major, column-major, strided, tiled, structure of arrays,
   array of structures and padded. Each one lowers the same index expression
   to different addresses.
3. **Placement and ownership.** Host, device or shared, with the owner named
   by R3's rules. A device tensor read on the host needs an explicit transfer
   ([chapter 42](ch42-device-graphs.md)).
4. **Visible specialization.** Static dimensions and layout choices are part
   of a kernel's identity. Bounded and dynamic ones are runtime parameters.
   `coax explain` lists what each kernel was specialized on and which change
   recompiles it.
5. **The batch dimension.** A leading logical dimension. The same solver code
   runs at batch 1 and batch 1,024.
6. **What a layout change can't do.** A change that reorders a reduction can
   change a result, so the compiler refuses it as a layout change.

## The consumer and the gate

The consumer is the Aethyr physics engine's solver state
([Aethyr Part IV](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch017-a-scene-owns-the-state.md)).
It declares its bodies, contacts and particles once. The gate:

- The solvers run in two layouts and at batch sizes 1 and 1,024 with
  bit-identical results under the strict profile.
- A hidden recompilation caused by a value the program called dynamic is a
  failing fixture.

## Done when

- R21 has shipped with the gate above.
- This chapter's examples run under `check-examples.sh` in debug and release.
- `coax explain` output for one kernel appears in the chapter as a transcript.
- Chapter 13's planned section links here and states what shipped.

## Dependencies

- R4 generics ([chapter 8](ch08-generics.md)), R5 layout metadata
  ([chapter 13](ch13-data-layout.md)), R6 storage transformations and R9
  resource declarations ([chapter 17](ch17-gpu.md)).
- [Chapter 16](ch16-determinism.md) for the strict profile the gate uses.
