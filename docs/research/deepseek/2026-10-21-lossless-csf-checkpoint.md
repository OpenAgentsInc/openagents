# Local Inference Lab's lossless CSF checkpoint of V4.1-Flash

Sources, read on 2026-10-21:

- The model card and file list for
  [`local-inference-lab/DeepSeek-V4.1-Flash-1017-lossless-CSF`](https://huggingface.co/local-inference-lab/DeepSeek-V4.1-Flash-1017-lossless-CSF),
  revision `90d665c19e1a9ff2097cc787515e370d86b616f0`, published 2026-10-18.
- Its license file, `LICENSES/LicenseRef-LIL-1.0.txt`.
- Local Inference Lab's announcement, which claims up to 30% more KV cache
  with no loss in quality.

This page summarizes the release in our own words and says whether it
helps Psionic's native V4.1 work (#12369). **Recommendation: don't adopt
the checkpoint.** It saves little for us, it needs a special loader, and
its license conflicts with how Pylons get their weights. Keep serving the
original `deepseek-ai` checkpoint.

## What it is

It's a repackaging of the same checkpoint we serve:
`deepseek-ai/DeepSeek-V4.1-Flash-1017` at revision
`1bae0e1088993b9b82de08f59de5be5a34e42170`. The model card shows these
changes:

- **Only the block-scale planes are compressed.** These are the UE8M0
  scale bytes that accompany the FP4 experts (MXFP4 blocks). They're stored
  with a lossless integer code (`row-base-offset1-u24-exceptions/1`): a
  per-row base, small offsets, and an exception list. Nothing is
  requantized, and every other tensor keeps its original bytes.
- **Scale bytes:** 46,080 matrices shrink from 16.99 GB to 2.32 GB, about
  7.3 times smaller.
- **Whole checkpoint:** 510.30 GB shrinks to 495.63 GB. That saves
  14.66 GB, or **2.9%**.
- **Loading needs a CSF-aware loader.** The compressed scales sit in
  `.mxfp4_csf_fixed` and `.mxfp4_csf_exceptions` files beside the original
  shard names. `receipts/` keeps each original shard header and SHA-256,
  so the original shards can be rebuilt and checked byte for byte. There's
  no root safetensors index; the original config, tokenizer, and index live
  under `metadata/`.
- **Qualification is storage only.** It's qualified as "storage and CPU
  reconstruction", and the card says plainly that this doesn't qualify
  model serving.
- **The files carry identifying marks:** a canary string and a phrase,
  which the license requires recipients to keep.

## Does "30% more KV cache" apply to us?

Only under a narrow condition, and it doesn't address our current limit.

- **Where the saving comes from.** The 14.66 GB is the whole model's
  scale savings. It turns into free GPU memory only if the scales stay
  compressed on the GPU and the kernels decode them on the fly. A loader
  that rebuilds the original scales at load time saves download and disk
  space, not GPU memory.
- **Where 30% could come from.** It's plausible on a deployment whose KV
  budget is small next to 14.66 GB. That's a ratio claim, not a fixed
  amount.
- **What it would mean for our group.** Split over our four-GPU group,
  it's about 3.7 GB per GPU. W3 measured stage peaks of 76.5 to 78.1 GB
  on 102 GB cards.
- **KV isn't what limits our context.** W3 found that our step buffers
  dominate long-context memory (184,356 bytes per step token, or 24.2 GB
  per stage at 131,072 positions). KV and indexer state is only 640 to
  1,280 bytes per position per layer. Our first lever is sizing step
  buffers by chunk capacity rather than context capacity; see the
  [inference engineering lessons](../../psionic/inference-engineering-lessons.md).
- **Using it would mean kernel work.** Psionic's FP4 expert kernels read
  UE8M0 scales directly. Using compressed scales on the GPU would need new
  decode-in-kernel work and a new G1 v2 qualification of that path.

## License

The container uses the Local Inference Lab License 1.0. It's Apache 2.0
plus additional conditions that take precedence. The upstream DeepSeek
material stays under MIT. The terms that matter to us:

- **No reuploading.** You can't reupload the work, or any "substantially
  similar copy", to anyone by any means. Substantially similar copies
  include re-serialized and quantized versions.
  - Pylons fetch weights for contributors from our bucket.
  - Serving this package to Pylon operators that way would be a prohibited
    reupload.
- **Serving as an API is allowed, as long as users never get the model
  files.**
- **A required attribution notice.** Every page that runs or derives
  from the work must start with a prescribed attribution notice as its
  first visible paragraph. For us that would include openagents.com's main
  page.
- **Strict enforcement.** A breach ends all rights automatically, with no
  cure period. Takedowns are due within 24 hours, and the governing law
  and venue are Kentucky.

**Decision for us.** These terms are incompatible with a public network
where contributors download weights and run them. They would also put
another organization's notice at the top of our product page. The
original checkpoint has none of these conditions.

## What to take from it

- **The scales compress well.** That idea is useful, and costs nothing to
  note. UE8M0 scales vary little along a row, so a base-and-offset code
  shrinks them about seven-fold.
- **Possible future work.** If disk or transfer size ever matters, for
  example for Pylon operators downloading weights over home connections,
  we could design our own scale encoding from first principles and credit
  the idea. Never derive one from their files or code.
- **What it would save today.** It would save about 15 GB of a 510 GB
  download: not worth it now.
- **Their release practice is worth copying.** Each reconstruction step
  checks against receipted original headers and SHA-256 hashes, and the
  model card says storage verification doesn't qualify serving. That
  matches our own split between build identity (#12410) and G1/G2
  qualification.
