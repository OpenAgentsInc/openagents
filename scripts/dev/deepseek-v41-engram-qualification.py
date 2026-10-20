#!/usr/bin/env python3
"""Compare a Rust Engram map with the pinned reference's unmodified function.

Requires Python tokenizers. The reference source and tokenizer are supplied
locally; this check neither downloads weights nor imports PyTorch/TileLang.
"""

import argparse
import ast
import hashlib
import json
from pathlib import Path
import struct

import tokenizers

REVISION = "1bae0e1088993b9b82de08f59de5be5a34e42170"
SOURCE_SHA256 = "11f35ecbead8150c35aa002b3d180ef290b05a25afe883a11884f94d476d3897"
TOKENIZER_SHA256 = "c90dfa01249db1be4245780a052ede752e1361c612ac6d08e2bdada7d599476b"


def qualify(reference_path, tokenizer_path, rust_report):
    source = reference_path.read_bytes()
    tokenizer_bytes = tokenizer_path.read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise ValueError("Reference source does not match the pinned Engram file")
    if hashlib.sha256(tokenizer_bytes).hexdigest() != TOKENIZER_SHA256:
        raise ValueError("Tokenizer does not match the pinned checkpoint")
    node = next(
        node for node in ast.parse(source).body
        if isinstance(node, ast.FunctionDef)
        and node.name == "build_compressed_token_map"
    )
    namespace = {}
    # Execute exactly this pinned function, avoiding module-level torch imports.
    exec(compile(ast.Module(body=[node], type_ignores=[]), str(reference_path), "exec"), namespace)
    backend = tokenizers.Tokenizer.from_str(tokenizer_bytes.decode())

    class TokenizerWrapper:
        backend_tokenizer = backend

        def __len__(self):
            return backend.get_vocab_size(with_added_tokens=True)

    lookup, size = namespace["build_compressed_token_map"](TokenizerWrapper())
    reference = {
        "revision": REVISION,
        "tokenizer_sha256": TOKENIZER_SHA256,
        "token_count": len(lookup),
        "compressed_vocab_size": size,
        "compressed_pad_id": lookup[2],
        "map_u32_le_sha256": hashlib.sha256(
            b"".join(struct.pack("<I", value) for value in lookup)
        ).hexdigest(),
    }
    for key, expected in reference.items():
        if rust_report.get(key) != expected:
            raise ValueError(f"Rust/reference mismatch for {key}: {rust_report.get(key)} != {expected}")
    if rust_report.get("table_rows") != [384006168, 384016682]:
        raise ValueError("Rust Engram table sizes do not match the checkpoint")
    return {
        "schema": "psionic.deepseek_v41.engram_tokenizer_qualification.v1",
        "scope": "full pinned tokenizer map; no checkpoint activations or device oracle",
        "reference_source_sha256": SOURCE_SHA256,
        "python_tokenizers_version": tokenizers.__version__,
        "reference": reference,
        "rust": rust_report,
        "map_matches": True,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-engram", required=True, type=Path)
    parser.add_argument("--tokenizer", required=True, type=Path)
    parser.add_argument("--rust-report", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    report = qualify(
        args.reference_engram, args.tokenizer,
        json.loads(args.rust_report.read_text()),
    )
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Matched all {report['reference']['token_count']} token-map entries by SHA-256")


if __name__ == "__main__":
    main()
