# Original DeepSeek R1 distill DATA

Repository: `deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B`, revision
`ad9f0ae0864d7fbcd1cd905e3c6c5b069cc8b562`.

The tokenizer JSON is retained as deterministic gzip. `r1.pst` packs its
byte vocabulary and merge ranks with the already pinned Unicode-17 DATA
from chapter 9. Configs and the reviewed Safetensors header remain inert
DATA. No downloaded source code is executed. The original weight payload
is not committed; `manifest.json` pins its complete size and SHA-256.
`targets.coax` is generated from this reviewed header and the fixed Qwen2
architecture, never from a candidate model answer. `LICENSE` is the pinned
repository’s MIT license. The distill’s original model card identifies its
Qwen base and terms; this directory introduces no new weight license.

Rebuild or check the DATA with:

```shell
python3 -I psionic/tests/deepseek_artifacts.py
python3 -I psionic/tests/deepseek_artifacts.py --original DIRECTORY --write
```

The independent regex/heap-BPE/Jinja authoring vectors are one directory
above; their recorded tooling versions and exact input/output pairs are
in `text-reference.json`. Reauthoring them requires `regex` and `jinja2`.
Ordinary checks consume the committed vectors and require neither package.
The independent original model oracle additionally needs NumPy; the Coax
engine and native host never need Python at run time.
