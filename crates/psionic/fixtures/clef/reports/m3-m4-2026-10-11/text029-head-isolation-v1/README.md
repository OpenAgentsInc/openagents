# Text 029 pooled-head replay

This diagnostic keeps the official CPU-f32 head, full saved MPS hidden memory, and same-GGUF lexical rows. It replaces selected pooled context inputs with the native Stage 6 f16 exports. It does not execute a trunk or use a GPU.

The reference replay reproduces all saved logits bit for bit. For the Japanese question `日本1`, the reference assigns true probability 0.4930319231. Replacing only its false-option pooled row changes that probability to 0.5002159178 and flips the answer. Replacing every pooled row yields 0.5019953666, close to the recorded native value 0.5020910901. Rounding reference pools to f16 changes the probability by only 0.0000124028.

This places the material discrepancy before the head projections and scoring. It does not yet separate trunk arithmetic from hidden normalization or pooling. Native full hidden memory was not exported, so this is not a complete native-head equivalence test. All three questions, eight controls, and prior/joint contributions remain in the receipt. The frozen 40-request, 104-question acceptance gate is unchanged and still fails one top answer.

The owned CPU process completed in 4.488 seconds with 1.169 GB peak accounted memory. Bounds were 4 GiB process group, 16 GiB starting available memory, 12 GiB available-memory floor, two CPU threads, and 60 seconds. Cleanup succeeded. Raw arrays and memory samples remain outside Git. File hashes preserve original and path-substituted bytes; the private path map remains with the preceding full reference publication.
