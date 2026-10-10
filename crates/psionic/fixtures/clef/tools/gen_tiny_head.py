"""Tiny random JointSchemaHead fixture computed by the reference torch code.

usage: gen_tiny_head.py <clef-flash dir (for joint_schema_model.py)> <out dir>
Writes joint_head.safetensors (f32), joint_head_config.json, case.json.
"""

import json
import math
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import torch  # noqa: E402
from safetensors.torch import save_file  # noqa: E402
from joint_schema_model import EncodedQuestion, EncodedRecord, JointSchemaHead  # noqa: E402

out = Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
torch.manual_seed(1159)
config = {"hidden_size": 64, "width": 32, "routing_layers": 2, "layers": 2, "heads": 4, "feedforward": 48}
head = JointSchemaHead(**config).eval()
with torch.no_grad():
    for name, param in head.named_parameters():
        if param.ndim == 0:
            continue
        if name.endswith("bias") or "norm" in name:
            param.copy_(torch.randn_like(param) * 0.3 + (1.0 if ("norm" in name and name.endswith("weight")) else 0.0))
    head.prior_logit_scale.fill_(1.7)
    head.joint_logit_scale.fill_(5.2)  # clamped at ln 100
    head.residual_gate.fill_(-0.4)
state = {k: v.contiguous().float() for k, v in head.state_dict().items()}
save_file(state, str(out / "joint_head.safetensors"))
(out / "joint_head_config.json").write_text(json.dumps(config) + "\n")

vocab = 50
length = 40
input_ids = torch.randint(0, vocab, (length,)).tolist()
hidden = torch.randn(length, config["hidden_size"]) * 2.0 + 0.5
embedding = torch.randn(vocab, config["hidden_size"])
questions = (
    EncodedQuestion("a", 0, (5, 8), ((10, 12), (12, 15)), ("true", "false")),
    EncodedQuestion("b", 1, (16, 19), ((20, 21), (21, 24), (24, 28)), ("x", "y", "z")),
    EncodedQuestion("c", 2, (29, 30), ((31, 33), (33, 34), (34, 36), (36, 39)), ("0", "1", "2", "3")),
)
record = EncodedRecord(input_ids=tuple(input_ids), questions=questions, record_id="tiny")
with torch.no_grad():
    logits = head(hidden.unsqueeze(0), torch.tensor([input_ids]), torch.ones(1, length, dtype=torch.long), [record], embedding)[0]
case = {
    "input_ids": input_ids,
    "hidden": hidden.tolist(),
    "embedding": embedding.tolist(),
    "questions": [
        {"id": q.question_id, "type": q.question_type, "question_span": list(q.question_span),
         "option_spans": [list(s) for s in q.option_spans], "option_ids": list(q.option_ids)}
        for q in questions
    ],
    "logits": [l.tolist() for l in logits],
    "torch": torch.__version__,
}
(out / "case.json").write_text(json.dumps(case) + "\n")
print("ok", [l.tolist() for l in logits])
