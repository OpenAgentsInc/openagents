#!/usr/bin/env python3
"""Set up pinned embedding/vision models and regenerate ORT reference fixtures.

Tooling dependencies: onnx==1.20.1, onnxruntime==1.28.0, numpy==2.5.4,
tokenizers==0.23.2, safetensors==0.7.0. Production execution is Rust.
"""

import argparse
import hashlib
import json
from pathlib import Path
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "crates/psionic/crates/psionic-inference/fixtures"


def checksum(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def verify(root, manifest, native=False):
    for name, file in manifest["files"].items():
        path = root / name
        if path.stat().st_size != file["bytes"] or checksum(path) != file["sha256"]:
            raise ValueError(f"Checksum mismatch: {name}")
    if native and manifest.get("native"):
        file = manifest["native"]
        path = root / file["file"]
        if path.stat().st_size != file["bytes"] or checksum(path) != file["sha256"]:
            raise ValueError("Native weights do not match the pinned conversion")


def download(root, manifest):
    for name, file in manifest["files"].items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.exists() and checksum(path) == file["sha256"]:
            continue
        part = path.with_suffix(path.suffix + ".part")
        urllib.request.urlretrieve(file["url"], part)
        if part.stat().st_size != file["bytes"] or checksum(part) != file["sha256"]:
            part.unlink()
            raise ValueError(f"Download failed checksum verification: {name}")
        part.replace(path)
    verify(root, manifest)


def convert_mnist(root):
    import numpy as np
    import onnx
    from onnx import numpy_helper
    from safetensors.numpy import save_file

    model = onnx.load(root / "model.onnx")
    initializers = {t.name: numpy_helper.to_array(t) for t in model.graph.initializer}
    mapping = {
        "conv1.weight": ("Parameter5", (8, 1, 5, 5)), "conv1.bias": ("Parameter6", (8,)),
        "conv2.weight": ("Parameter87", (16, 8, 5, 5)), "conv2.bias": ("Parameter88", (16,)),
        "fc.weight": ("Parameter193", (256, 10)), "fc.bias": ("Parameter194", (10,)),
    }
    arrays = {name: np.ascontiguousarray(initializers[source].reshape(shape)) for name, (source, shape) in mapping.items()}
    save_file(dict(sorted(arrays.items())), str(root / "native.safetensors"))


def fixtures(root, manifest, family):
    import numpy as np
    import onnx
    import onnxruntime as ort
    from onnx import numpy_helper
    from tokenizers import Tokenizer
    from safetensors.numpy import load_file

    verify(root, manifest, native=True)
    options = ort.SessionOptions()
    options.intra_op_num_threads = 1
    options.inter_op_num_threads = 1
    session = ort.InferenceSession(str(root / manifest["onnx"]), options, providers=["CPUExecutionProvider"])
    cases = []
    tokenizer_cases = []
    def tensors(values):
        return {name: {"shape": list(value.shape), "data": {"dtype": "i64" if value.dtype == np.int64 else "f32", "values": value.flatten().tolist()}}
                for name, value in values.items()}
    if family == "potion":
        model = onnx.load(root / manifest["onnx"])
        expected = numpy_helper.to_array(model.graph.initializer[0])
        assert np.array_equal(expected, load_file(str(root / "model.safetensors"))["embeddings"])
        tokenizer = Tokenizer.from_file(str(root / "tokenizer.json"))
        tokenizer.no_padding()
        tokenizer.no_truncation()
        texts = ["", "Rust memory safety and borrowing", "Find a Lightning payment invoice", "Design a calendar reminder", "🦀", "Résumé: a café in Paris"]
        for i in range(100):
            group = [texts[i % len(texts)] + (f" local note {i}" if i % len(texts) else "")]
            if i % 7 == 0:
                group += ["", "The Rust compiler checks ownership.", "A simple pasta recipe."]
            if i == 99:
                group = ["Review this Rust file. " * 1000]
            ids, offsets = [], []
            for text in group:
                offsets.append(len(ids))
                encoded = tokenizer.encode(text, add_special_tokens=False).ids
                ids.extend(encoded)
                tokenizer_cases.append({"text": text, "ids": encoded})
            inputs = {"input_ids": np.asarray(ids, dtype=np.int64), "offsets": np.asarray(offsets, dtype=np.int64)}
            outputs = dict(zip([o.name for o in session.get_outputs()], session.run(None, inputs)))
            cases.append({"texts": group, "inputs": tensors(inputs), "outputs": tensors(outputs)})
    else:
        with tarfile.open(root / "tests.tar.gz") as archive:
            tensor = onnx.TensorProto()
            tensor.ParseFromString(archive.extractfile("mnist-12/test_data_set_0/input_0.pb").read())
            published = numpy_helper.to_array(tensor).astype(np.float32)
            expected = onnx.TensorProto()
            expected.ParseFromString(archive.extractfile("mnist-12/test_data_set_0/output_0.pb").read())
            published_output = numpy_helper.to_array(expected)
        rng = np.random.default_rng(20261015)
        for i in range(100):
            pixels = published if i == 0 else (np.zeros((1, 1, 28, 28), dtype=np.float32) if i == 1 else rng.integers(0, 256, size=(1, 1, 28, 28)).astype(np.float32) / 255)
            inputs = {"Input3": pixels}
            output = session.run(None, inputs)[0]
            if i == 0:
                assert np.allclose(output, published_output, atol=1e-4)
            outputs = {"Plus214_Output_0": output}
            cases.append({"inputs": tensors(inputs), "outputs": tensors(outputs), "digit": int(output.argmax())})
    result = {"schema": "psionic.small-model.reference.v1", "model": manifest["model"], "revision": manifest["revision"],
              "onnxruntime": ort.__version__, "seed": 20261015, "cases": cases, "tokenizer_cases": tokenizer_cases}
    path = FIXTURES / f"{family}-reference.json"
    path.write_text(json.dumps(result, ensure_ascii=False, separators=(",", ":")) + "\n")
    print(f"Wrote {len(cases)} reference cases to {path}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["download", "prepare", "verify", "fixtures"])
    parser.add_argument("family", choices=["potion", "mnist"])
    parser.add_argument("model_dir", type=Path)
    args = parser.parse_args()
    manifest = json.loads((FIXTURES / f"{args.family}-manifest.json").read_text())
    if args.action == "download":
        download(args.model_dir, manifest)
    elif args.action == "prepare":
        verify(args.model_dir, manifest)
        if args.family == "mnist":
            convert_mnist(args.model_dir)
        verify(args.model_dir, manifest, native=True)
    elif args.action == "verify":
        verify(args.model_dir, manifest, native=True)
    else:
        fixtures(args.model_dir, manifest, args.family)
