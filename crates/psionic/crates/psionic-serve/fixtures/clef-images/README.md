# Clef image preprocessing fixtures

These synthetic images test base64 decoding, EXIF handling, resize dimensions,
antialiased bicubic pixels, normalization, and patch order without loading model weights.
They do not establish vision-encoder or decision-output parity.

[Clef's reference loader](https://huggingface.co/Cloudflare/clef/blob/main/joint_schema_model.py)
calls `AutoProcessor.from_pretrained(path)` without a backend override. With
Transformers 5.10.2 and the released processor configuration, this constructs
`Qwen3VLProcessor` with `Qwen2VLImageProcessor`, whose backend is
[`TorchvisionBackend`](https://github.com/huggingface/transformers/blob/v5.10.2/src/transformers/models/qwen2_vl/image_processing_qwen2_vl.py).
The generator checks that class rather than selecting the separate PIL backend.
It runs on CPU with one Torch thread.

Clef passes base64 strings directly to this processor. Its byte decoder uses
Torchvision's `decode_image` with `apply_exif_orientation=False`, so an EXIF
rotation tag does not change the pixel order or dimensions. The separate
Transformers PIL loader applies EXIF orientation and is not interchangeable with
this entry point. The generator captures the actual byte decoder's pixels and
checks that their resize, grid, and patch values match the complete processor.

The resize implementation follows
[PyTorch 2.9.1's CPU uint8 antialias kernel](https://github.com/pytorch/pytorch/blob/v2.9.1/aten/src/ATen/native/cpu/UpSampleKernel.cpp):
Keys cubic coefficients with parameter -0.5, adaptive int16 coefficient
precision, horizontal then vertical filtering, and uint8 rounding and clipping
after each pass. Pillow's coefficient precision differs, so a Pillow resize is
not an interchangeable golden.

`reference.json` records exact library versions, the released processor
configuration hash, input file hashes, expected dimensions, grids, resized RGB
hashes, and complete float32 patch hashes. Each `*.expected.png` stores the
reference's resized RGB pixels. The generator passes base64 strings through the
full processor, including its image loader. PNG and lossless WebP comparisons
require exact pixels. Each JPEG also stores the reference's decoded RGB in
`*.decoded.png`, bound to the existing `decoded_rgb_sha256`. Resizing those
identical RGB values must match Torchvision exactly.

The pinned Rust and Torchvision JPEG decoders differ by up to three RGB levels
before resizing: 7,905 of 274,659 channels differ in each JPEG fixture, with
nine channels differing by three and a mean absolute difference of 0.03415 or
less. The complete JPEG decode-and-resize
path also differs by at most three levels. Tests check these two bounds
separately from exact bicubic resizing; the allowance applies only to the JPEG
decoder and complete JPEG path. Normalization and patch order must match the
complete reference hash when supplied the same RGB pixels.

The fixtures also record the complete media token sequence. A two-image case
compares all three position axes with `Qwen3_5Model.get_rope_index`, called on a
weight-free adapter that supplies only the spatial merge size. This checks the
positions before, between, and after two different image grids.

The production bound is separate from the reference resize policy. The reference
uses 65,536 minimum pixels, 16,777,216 maximum pixels, and a factor of 32. Clef
preserves that target, then refuses images whose resized output exceeds the
server's 262,144-pixel limit. It does not silently lower the reference maximum.

To regenerate, install the versions in `reference.json`, then run:

```sh
python generate.py --model-metadata /path/to/clef-processor-metadata
```

The metadata directory must contain the released `config.json`,
`processor_config.json`, `tokenizer.json`, and `tokenizer_config.json`; symlinks
to verified local blobs are sufficient. No weight files are needed.
