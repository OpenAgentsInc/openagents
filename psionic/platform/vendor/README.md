# DLPack declarations

Only the public header from [dmlc/dlpack v1.2](https://github.com/dmlc/dlpack/tree/v1.2)
is vendored. No producer implementation, build scripts or runtime dependency
is imported. Keep its [Apache 2.0 license](DLPACK-LICENSE) with the header.

| File | Upstream source | SHA-256 |
| --- | --- | --- |
| [dlpack.h](dlpack.h) | `include/dlpack/dlpack.h` at tag `v1.2` | `2f755aa1df9855a1c8744d570bd87d7506d2f46493180a64701aac395d12e5d9` |
| [DLPACK-LICENSE](DLPACK-LICENSE) | `LICENSE` at tag `v1.2` | `3e8f6bc39276a586d60c8c5aaece2ff989262181cf2b3d2a128db58f97a9300c` |

Psionic's ABI is separately versioned in [interchange.h](../interchange.h).
Header compatibility alone does not imply support for every DLPack device,
dtype or layout; see the [interchange profile](../../fixtures/interchange-profile.json).
