# Original V2-Lite metadata

Pinned `deepseek-ai/DeepSeek-V2-Lite-Chat` revision
`85864749cd611b4353ce1decdb286193298f64c7`. These are inert original
configuration, tokenizer recipe and tensor/shard inventory DATA. They are not
an executable dependency. Original remote Python was read, never executed.

The four BF16 shards total 31,413,626,576 file bytes. All four were downloaded
outside the repository and each complete SHA-256 matched the pinned Hub LFS
identity in `pins.json`. That is artifact preparation for chapter 58, not
numerical execution or model qualification. Weights are not redistributed here.

The model has 27 layers, width 2,048, 16 attention heads, KV latent width 512,
128 non-rotary and 64 rotary query/key dimensions, and value width 128. Its
first layer has a dense FFN; later layers use 64 routed experts, six selected,
and two shared experts. The router uses full-expert softmax probabilities
without selected-only normalization. YaRN and the tokenizer remain separate
implementation/verification gates.
