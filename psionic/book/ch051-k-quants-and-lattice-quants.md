# 51. K-quants and lattice quants

> **Status: unimplemented — design draft.** Nothing here runs yet.

Most published quantized checkpoints use the K-quant and IQ block formats.
This chapter adds decoders for the 2- to 6-bit K-quant blocks and for the
non-uniform and lattice-codebook formats, so Psionic can read those files and
measure their quality.

## What this chapter covers

- K-quant block decoders from 2 to 6 bits, including their packed scales.
- IQ4: a non-uniform 4-bit value table.
- IQ3 and IQ2: lattice codebooks, where each code indexes a grid of values.
- Independent decode vectors for every format.
- The quality card from chapter 50 for each format:
  - mean KL divergence against the original weights;
  - the same-top-token rate;
  - perplexity with its error.
- License handling: copied tables (IQ grids, IQ4 values, K-quant scale
  packing) carry the MIT notice in a Psionic `NOTICE` and in this chapter.

## What it builds

- Planned decoders for each block format, read through chapter 49's GGUF
  reader.

## Dependencies

- [Chapter 49](ch049-read-a-gguf-file.md): the GGUF container.
- [Chapter 50](ch050-quantized-weights-and-caches.md): the quality card and the
  profile rules.

## Done means

- Every decoder matches its independent vectors exactly.
- Each format has a quality card measured against the original weights of a
  named model.
- Malformed blocks and truncated rows are refused.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): format sources
  and licensing.
