"""Head parity: Cloudflare's torch JointSchemaHead (f32, and bf16 as a
characterization row) on the hidden rows Psionic's backbone produced.

usage: head_parity.py <clef-flash dir> <dump dir>

The dump comes from the `dump_head_parity_inputs` test in
`psionic-serve/src/clef/tests.rs`: `hidden.f32` (L x 4096), and in
`dump.json` the prompt, spans, the option tokens' LM-head rows, and the
logits of Psionic's GGUF head and Hugging Face head on the same rows.
Option token ids are remapped into a small table, since the head reads
`output_embedding_weight` only at option tokens.
"""

import json
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import torch  # noqa: E402
from safetensors.torch import load_file  # noqa: E402
from joint_schema_model import EncodedQuestion, EncodedRecord, JointSchemaHead  # noqa: E402

ref = Path(sys.argv[1])
dump_dir = Path(sys.argv[2])
dump = json.loads((dump_dir / "dump.json").read_text())
d = dump["hidden_size"]
hidden = torch.frombuffer(bytearray((dump_dir / "hidden.f32").read_bytes()), dtype=torch.float32).reshape(-1, d)
ids = dump["input_ids"]
assert hidden.shape[0] == len(ids)
lexical_ids = sorted(int(k) for k in dump["lexical"])
remap = {token: index for index, token in enumerate(lexical_ids)}
table = torch.tensor([dump["lexical"][str(token)] for token in lexical_ids], dtype=torch.float32)
head_ids = [remap.get(token, 0) for token in ids]
questions = tuple(
    EncodedQuestion(q["id"], q["type"], tuple(q["question_span"]), tuple(tuple(s) for s in q["option_spans"]), tuple(q["option_ids"]))
    for q in dump["questions"]
)
record = EncodedRecord(input_ids=tuple(head_ids), questions=questions, record_id="dump")
config = json.loads((ref / "joint_head_config.json").read_text())
state = load_file(str(ref / "joint_head.safetensors"))


def run(dtype):
    head = JointSchemaHead(**config)
    head.load_state_dict(state, strict=True)
    head = head.to(dtype).eval()
    with torch.no_grad():
        out = head(hidden.to(dtype).unsqueeze(0), torch.tensor([head_ids]), torch.ones(1, len(ids), dtype=torch.long), [record], table.to(dtype))[0]
    return [row.float().tolist() for row in out]


def max_delta(a, b):
    return max(abs(x - y) for ra, rb in zip(a, b) for x, y in zip(ra, rb))


def max_dp(a, b):
    worst = 0.0
    for ra, rb in zip(a, b):
        pa = torch.tensor(ra).softmax(-1)
        pb = torch.tensor(rb).softmax(-1)
        worst = max(worst, float((pa - pb).abs().max()))
    return worst


f32 = run(torch.float32)
bf16 = run(torch.bfloat16)
report = {
    "prompt_tokens": len(ids),
    "questions": len(questions),
    "options": sum(len(q.option_spans) for q in questions),
    "torch": torch.__version__,
    "psionic_hf_head_vs_torch_f32_max_abs_dlogit": max_delta(dump["hf_head_logits"], f32) if dump["hf_head_logits"] else None,
    "psionic_gguf_q8_head_vs_torch_f32_max_abs_dlogit": max_delta(dump["gguf_head_logits"], f32),
    "psionic_gguf_q8_head_vs_torch_f32_max_abs_dp": max_dp(dump["gguf_head_logits"], f32),
    "torch_bf16_vs_f32_max_abs_dlogit": max_delta(bf16, f32),
    "torch_bf16_vs_f32_max_abs_dp": max_dp(bf16, f32),
}
print(json.dumps(report, indent=1))
