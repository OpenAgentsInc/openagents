# Retained task metadata for Gym tests

These are exact public Terminal-Bench task instructions and metadata from
[harbor-framework/terminal-bench at `452bf305c6daa62fc59061d22133a7cbc7c1572e`](https://github.com/harbor-framework/terminal-bench/tree/452bf305c6daa62fc59061d22133a7cbc7c1572e/tasks).
The original canaries and author metadata remain unchanged. The upstream
[Apache 2.0 license](LICENSE) applies. This fixture contains no verifier,
reference solution, or private implementation.

The retained run configurations point to the original Linux operator's task
checkout. Tests previously read it implicitly when present and lost task
instructions, categories, and limits on other machines. That changed the
learning-evidence digest and prevented three recorded answers from replaying.
`runs::fixture_sources` now rewrites only the temporary copies of those config
paths to these retained files. Original run records, recorded answers, their
digests, and assertions remain unchanged. The four original answer keys match
again without a Jev request.

These are test inputs, not training data or agent runtime knowledge. The
metadata is complete so existing readers calculate the same evidence as the
original recorded run; it must not be abbreviated or regenerated to fit a key.

| File | SHA-256 of retained bytes |
| --- | --- |
| `coq-block-bound/instruction.md` | `9e86df1ddf96f1d6ee037b9cfa3aa69bab88b477c83d7c46e8167ff806915a16` |
| `coq-block-bound/task.toml` | `3d3887dcf8e54a863e3dd47dc15af8a721d31be069622cacde895077399bf8f2` |
| `fin-saccr-rwa/instruction.md` | `7b49f7e95dc8f991ef80ef18fb6f1431cb27cea7506f2e0327edacbf30d5b4ef` |
| `fin-saccr-rwa/task.toml` | `8c65d87bfc462bebccd5df55cbc0efb40c9bbd27dfb2ad9359d700640bd8b9e1` |
| `uefi-bootkit/instruction.md` | `7d16c962b5f58fe0edd301cac698ed5986666f0b8e988313a0b544d3056a3c96` |
| `uefi-bootkit/task.toml` | `a188dd807c9157b150b8f1a12a3ae4944f591db3df8a77150438c2601ac66537` |
| `wal-recovery-ordering/instruction.md` | `f8e44418b9cb1329b1ac226d88a03c8c9654c496e1a32950ce31d768139a4ad3` |
| `wal-recovery-ordering/task.toml` | `147d7d1f314a9a0240335984d8ecb025baed41cb4e80f352e82dfba37b174fe0` |
