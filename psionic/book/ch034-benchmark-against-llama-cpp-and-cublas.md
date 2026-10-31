# 34. Benchmark against llama.cpp and cuBLAS

> **Status: unimplemented — design draft.** Nothing here runs yet.

A speed number means little without a comparator that runs the same model on
the same machine. This chapter runs Psionic and a packaged llama.cpp build on
the same file and the same prompt shapes, and it compares Psionic's own matrix
kernels with cuBLAS on the same shapes. Neither comparator becomes a
dependency.

## What this chapter covers

- Same weights on both sides: the same GGUF file for quantized runs, or a GGUF
  whose tensor bytes Psionic's reader proves equal to the pinned Safetensors.
- Matched settings: threads, device offload, attention mode, cache type and
  batch sizes.
- The llama-bench method, reimplemented in Coax:
  - prompt processing and generation at fixed lengths;
  - depth with a prefilled cache;
  - decision-shaped runs, with a shared prefix plus several suffixes;
  - long prompts.
- Warm-up, then at least five repetitions, reported as mean and standard
  deviation.
- A record of the machine, driver, clocks and load for every run.
- cuBLAS matrix products on the same shapes as an opt-in comparator for
  Psionic's generated kernels.
- The earlier claim that Psionic beat llama.cpp stays unverified until you
  reproduce it this way.

## What it builds

- A planned benchmark driver that writes one record per run with both
  engines' identities, versions and settings.

## Dependencies

- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): the fixed
  workload and the measurement method.
- [Chapter 49](ch049-read-a-gguf-file.md): the GGUF reader that proves byte
  equality with the original tensors.
- [Chapter 44](ch044-elephant-on-cuda.md): the CUDA path you compare.
- An owner-approved packaged llama.cpp build. You never run anything built
  from the reference clone.

## Done means

- Each comparison names both builds (llama.cpp version and commit), the file
  digest and every matched setting.
- Every reported figure is a mean and standard deviation over at least five
  warm repetitions on a quiet machine with recorded clocks.
- cuBLAS appears only in comparison records, and the Psionic build under test
  links no BLAS runtime.
- The reference bar from the survey (about 0.2 s for a 1,000-token decision on
  a 9B hybrid on the RTX 4080 class) is reported as an external figure, not a
  Psionic result.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md), sections on the
  benchmark method and fair comparison.
