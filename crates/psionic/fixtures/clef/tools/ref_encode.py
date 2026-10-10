"""Reference encodings: Cloudflare's encode_record with the HF tokenizer.

usage: ref_encode.py <clef-flash dir> <corpus.jsonl> > encoder.jsonl
"""

import json
import sys

sys.path.insert(0, sys.argv[1])
from joint_schema_model import encode_record  # noqa: E402
from transformers import AutoTokenizer  # noqa: E402

tokenizer = AutoTokenizer.from_pretrained(sys.argv[1])
for line in open(sys.argv[2], encoding="utf-8"):
    record = json.loads(line)
    encoded = encode_record(tokenizer, record, max_length=10**9)
    print(json.dumps({
        "input_ids": list(encoded.input_ids),
        "questions": [
            {
                "id": q.question_id,
                "type": q.question_type,
                "question_span": list(q.question_span),
                "option_spans": [list(s) for s in q.option_spans],
                "option_ids": list(q.option_ids),
            }
            for q in encoded.questions
        ],
    }, ensure_ascii=False))
