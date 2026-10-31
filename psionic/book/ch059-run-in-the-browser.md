# 59. Run in the browser

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter moves chapter 54's search onto openagents.com. Psionic CPU kernels
compile to Wasm through Coaxial, and WebGPU compute follows once Coax emits it.
The browser is a separate target with its own evidence; a Mac result does not
qualify it.

## What this chapter covers

- Psionic CPU kernels compiled to Wasm through Coaxial.
- WebGPU compute once Coax has a qualified compute lowering. Bounded WGSL
  graphics modules exist; they do not qualify compute.
- Document embeddings precomputed natively and shipped with the page.
- A query encoder small enough to download, selected separately.
- Compatibility between that query encoder and the stored document
  embeddings, shown by one of:
  - a measured alignment;
  - distillation from the original query encoder;
  - a matched query and document encoder pair.
- Equal vector dimensions prove nothing about compatibility.
- The target's own ABI, memory growth and numerical evidence.

## What it builds

- A planned browser search page and a Wasm build of the needed kernels.

## Dependencies

- [Chapter 54](ch054-search-the-coax-book.md): the search being moved.
- [Chapter 37](ch037-a-coax-compute-subset.md): the compute subset, for the
  later WebGPU step.
- A qualified Psionic WebGPU compute path in Coax, which remains work (see the
  [Coax roadmap](../../coax/ROADMAP.md)).

## Done means

- Browser query embeddings match native ones within a stated tolerance for the
  same query encoder.
- Retrieval quality with the browser query encoder is reported on chapter 54's
  held-out set beside the native result.
- Memory growth and page load size are measured in a named browser.

## Sources

- [Chapter plan](PLAN.md), Part VI and the Coax feature map.
