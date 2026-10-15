#!/usr/bin/env python3
"""Download pinned router artifacts or regenerate independent reference fixtures.

Reference tooling only: pip install onnxruntime==1.28.0 tokenizers==0.23.2.
Production preparation and execution are Rust.
"""

import argparse
import hashlib
import json
from pathlib import Path
import urllib.request

REPO = Path(__file__).resolve().parents[2]
FIXTURES = REPO / "crates/psionic/crates/psionic-inference/fixtures"
MANIFEST = FIXTURES / "s1-router-manifest.json"
LANGUAGES = [
    "Fix the failing Rust test and open a pull request.",
    "Fasse diesen Text in drei Sätzen zusammen.",
    "Résume cette conversation en trois phrases.",
    "Resume esta conversación en tres frases.",
    "Riassumi questa conversazione in tre frasi.",
    "Resuma esta conversa em três frases.",
    "Vat dit gesprek samen in drie zinnen.",
    "Sammanfatta den här konversationen i tre meningar.",
    "Opsummer denne samtale i tre sætninger.",
    "Oppsummer denne samtalen i tre setninger.",
    "Tiivistä tämä keskustelu kolmeen lauseeseen.",
    "Podsumuj tę rozmowę w trzech zdaniach.",
    "Shrň tuto konverzaci do tří vět.",
    "Σύνοψε αυτή τη συνομιλία σε τρεις προτάσεις.",
    "Bu konuşmayı üç cümlede özetle.",
    "Обобщи этот разговор в трёх предложениях.",
    "لخص هذه المحادثة في ثلاث جمل.",
    "इस बातचीत का सारांश तीन वाक्यों में दें।",
    "请用三句话总结这段对话。",
    "この会話を三文で要約してください。",
    "이 대화를 세 문장으로 요약해 주세요.",
    "Tóm tắt cuộc trò chuyện này trong ba câu.",
]


def checksum(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify(root, manifest):
    for name, expected in manifest["files"].items():
        path = root / name
        if path.stat().st_size != expected["bytes"] or checksum(path) != expected["sha256"]:
            raise ValueError(f"Checksum mismatch: {name}")


def download(root, manifest):
    for name, expected in manifest["files"].items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists() and checksum(path) == expected["sha256"]:
            continue
        url = f'https://huggingface.co/{manifest["repository"]}/resolve/{manifest["revision"]}/{name}'
        part = path.with_suffix(path.suffix + ".part")
        urllib.request.urlretrieve(url, part)
        if part.stat().st_size != expected["bytes"] or checksum(part) != expected["sha256"]:
            part.unlink()
            raise ValueError(f"Downloaded file failed verification: {name}")
        part.replace(path)
    verify(root, manifest)


def cases():
    for i in range(1000):
        request = LANGUAGES[i % len(LANGUAGES)]
        request += f" Case {i}: crates/pylon/src/verify.rs; issue #{10000 + i}."
        if i % 19 == 0:
            request += "\nfn main() { println!(\"hello\"); }"
        if i % 97 == 0:
            request += " Review this change." * 300
        context = None if i % 3 else "Continue the previous task."
        if i == 1:
            context = "a " * 299 + "🦀" * 3
        yield {"id": str(i), "request": request, "context": context}


def generate(root, manifest, destination):
    import numpy as np
    import onnxruntime as ort
    import tokenizers

    if ort.__version__ != "1.28.0" or tokenizers.__version__ != "0.23.2":
        raise ValueError("Use the pinned reference versions in this script's docstring")
    verify(root, manifest)
    tokenizer = tokenizers.Tokenizer.from_file(str(root / "tokenizer/tokenizer.json"))
    tokenizer.enable_truncation(512, strategy="only_second")
    tokenizer.no_padding()
    options = ort.SessionOptions()
    options.intra_op_num_threads = 4
    options.inter_op_num_threads = 1
    session = ort.InferenceSession(str(root / "model.gq.onnx"), options, providers=["CPUExecutionProvider"])
    tokenizer_cases = []
    requests = []
    labels = {
        "category": ["coding", "agentic", "math", "knowledge", "long_context", "tool_use", "design", "summarisation", "general"],
        "difficulty": ["trivial", "easy", "moderate", "hard", "frontier"],
        "stakes": ["negligible", "low", "medium", "high"],
    }
    flags = ["needs_tools", "needs_vision", "needs_long_context", "follow_up"]
    for i, case in enumerate(cases()):
        context = (case["context"] if case["context"] is not None else "(new conversation)")[:600]
        ids = tokenizer.encode(context, case["request"]).ids
        tokenizer_cases.append(dict(case, input_ids=ids))
        if i >= 100:
            continue
        inputs = {"input_ids": np.array([ids], dtype=np.int64), "attention_mask": np.ones((1, len(ids)), dtype=np.int64)}
        outputs = dict(zip([x.name for x in session.get_outputs()], session.run(None, inputs)))
        answers = {}
        for name, names in labels.items():
            values = outputs[name][0] / manifest["temperatures"][name]
            p = np.exp(values - values.max()); p /= p.sum()
            answers[name] = {"label": names[int(p.argmax())], "probabilities": dict(zip(names, p.tolist())),
                             "expected": float(np.arange(len(names)) @ p) if name != "category" else None}
        pflags = 1 / (1 + np.exp(-outputs["flags"][0] / manifest["temperatures"]["flags"]))
        answers["flags"] = dict(zip(flags, pflags.tolist()))
        tensors = lambda tensors, dtype: {name: {"shape": list(v.shape), "data": {"dtype": dtype, "values": v.flatten().tolist()}} for name, v in tensors.items()}
        requests.append(dict(case, inputs=tensors(inputs, "i64"), outputs=tensors(outputs, "f32"), answer=answers))
    result = {"schema": "psionic.router.reference.v1", "revision": manifest["revision"],
              "onnxruntime": ort.__version__, "tokenizers": tokenizers.__version__,
              "requests": requests, "tokenizer_cases": tokenizer_cases}
    destination.write_text(json.dumps(result, ensure_ascii=False, separators=(",", ":")) + "\n")
    print(f"Wrote {len(requests)} inference cases and {len(tokenizer_cases)} tokenizer cases to {destination}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["download", "verify", "fixtures"])
    parser.add_argument("model_dir", type=Path)
    parser.add_argument("--output", type=Path, default=FIXTURES / "s1-router-reference.json")
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_text())
    {"download": download, "verify": verify, "fixtures": lambda r, m: generate(r, m, args.output)}[args.action](args.model_dir, manifest)
