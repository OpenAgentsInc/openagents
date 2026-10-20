#!/usr/bin/env python3
"""Refresh R1 prompt/token fixtures from pinned public metadata, without weights.

Requires tokenizers==0.22.2 and Jinja2==3.1.6. Run from the repository root.
"""

import hashlib
from importlib.metadata import version
import json
from pathlib import Path
import unicodedata
import urllib.request

from jinja2 import Environment, StrictUndefined
from tokenizers import Tokenizer


REVISION = "ad9f0ae0864d7fbcd1cd905e3c6c5b069cc8b562"
BASE = "https://huggingface.co/deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B/resolve/" + REVISION
DEST = Path("crates/psionic/crates/psionic-models/src/testdata/deepseek_r1")


def main():
    for package, expected in (("tokenizers", "0.22.2"), ("Jinja2", "3.1.6")):
        if version(package) != expected:
            raise RuntimeError(f"Install {package}=={expected} before refreshing fixtures")
    raw = {name: urllib.request.urlopen(BASE + "/" + name, timeout=60).read()
           for name in ("tokenizer.json", "tokenizer_config.json")}
    config = json.loads(raw["tokenizer_config.json"])
    definition = json.loads(raw["tokenizer.json"])
    tokenizer = Tokenizer.from_str(raw["tokenizer.json"].decode())
    template = config["chat_template"]
    bos = config["bos_token"]
    if isinstance(bos, dict):
        bos = bos["content"]
    jinja = Environment(undefined=StrictUndefined).from_string(template)
    prompts = [
        ("user", [{"role": "user", "content": "What is 1 + 1?"}], True),
        ("system", [{"role": "system", "content": " Keep whitespace. "},
                    {"role": "user", "content": "Hello\n"}], True),
        ("continuation", [{"role": "user", "content": "What is 1 + 1?"},
                          {"role": "assistant", "content": "<think>work</think>\n\n2"},
                          {"role": "user", "content": "Add one to that."}], True),
        ("history", [{"role": "user", "content": "Hi"},
                     {"role": "assistant", "content": "<think>work</think>old</think> final "}], False),
        ("system_last_wins", [{"role": "system", "content": "first"},
                              {"role": "user", "content": "Hi"},
                              {"role": "system", "content": "last"}], True),
        ("assistant_end", [{"role": "assistant", "content": "2"}], False),
    ]
    cases = []
    for name, messages, generation in prompts:
        rendered = jinja.render(messages=messages, bos_token=bos,
                                add_generation_prompt=generation)
        cases.append(dict(name=name, messages=messages, generation=generation,
                          rendered=rendered, token_ids=tokenizer.encode(rendered, add_special_tokens=False).ids))
    samples = ["Hello world!", "1234567890 2026 3.14159", "ABCDef I'M don't we'll", "café cafe\u0301",
               "你好，世界 🌍", "def f(x):\n\treturn x + 1\n\n", "  hi\r\n\nnext  ",
               "<｜User｜>Hi<｜Assistant｜><think>\nwork</think>\n2<｜end▁of▁sentence｜>",
               "a\u0301\u0327 ́", "<｜begin▁of▁sentence｜>Hi"]
    token_cases = []
    for text in samples:
        ids = tokenizer.encode(text, add_special_tokens=False).ids
        token_cases.append(dict(text=text, token_ids=ids, decoded=tokenizer.decode(ids)))
    # Keep every candidate merge that can occur in this corpus, with its real ID.
    # The Rust test fills omitted entries with empty ordinary tokens. This is a
    # bounded rank fixture, not a replacement tokenizer for arbitrary inputs.
    byte_chars = list(range(33, 127)) + list(range(161, 173)) + list(range(174, 256))
    extra = 0
    alphabet = {}
    for value in range(256):
        if value in byte_chars:
            alphabet[value] = chr(value)
        else:
            alphabet[value] = chr(256 + extra)
            extra += 1
    inverse = {char: value for value, char in alphabet.items()}
    corpus = [unicodedata.normalize("NFC", text).encode() for text in
              samples + [case["rendered"] for case in cases]]
    vocab = []
    for token, token_id in definition["model"]["vocab"].items():
        data = bytes(inverse[c] for c in token)
        if len(data) == 1 or any(data in text for text in corpus):
            vocab.append([token_id, token, 1])
    for added in definition["added_tokens"]:
        vocab.append([added["id"], added["content"], 3 if added["special"] else 4])
    fixture = dict(schema="psionic.deepseek_r1_distill_qwen_1_5b.v1",
                   source=dict(repo="deepseek-ai/DeepSeek-R1-Distill-Qwen-1.5B", revision=REVISION,
                               tokenizer_sha256=hashlib.sha256(raw["tokenizer.json"]).hexdigest(),
                               template_sha256=hashlib.sha256(template.encode()).hexdigest(),
                               reference="tokenizers 0.22.2; Jinja2 3.1.6"),
                   vocabulary_size=151936, bos_id=151646, eos_id=151643,
                   vocabulary=sorted(vocab), prompt_cases=cases, token_cases=token_cases)
    DEST.mkdir(parents=True, exist_ok=True)
    (DEST / "template.jinja").write_text(template)
    (DEST / "golden.json").write_text(json.dumps(fixture, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
