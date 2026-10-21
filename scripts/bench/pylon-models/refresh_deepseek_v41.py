#!/usr/bin/env python3
"""Refresh the DeepSeek-V4.1-Flash protocol fixtures from pinned public files.

Downloads only the tokenizer, the reference encoder, and its five encoding
tests at the pinned checkpoint revision, never the weights. Each download must
match its SHA-256 pin. The script then:

- copies the encoding tests verbatim, after checking that the reference
  encoder reproduces each expected output;
- records the official tokenizer's token IDs for each expected output and for
  the streaming cases that the Rust mock-backend tests replay;
- checks that the official tokenizer encodes each oracle prompt that a
  stage test renders (`ORACLE_PROMPTS`) to the oracle's own token IDs;
- writes a reduced tokenizer that keeps every vocabulary entry and merge these
  texts can use, with their original IDs, and checks that it encodes and
  decodes every text exactly as the full tokenizer does. It is a fixture, not
  a replacement for the 6 MB checkpoint tokenizer.

Requires tokenizers==0.22.2. Run from the repository root.
"""

import hashlib
import importlib.util
from importlib.metadata import version
import json
from pathlib import Path
import tempfile
import urllib.request

from tokenizers import Tokenizer


REPO = "deepseek-ai/DeepSeek-V4.1-Flash-1017"
REVISION = "1bae0e1088993b9b82de08f59de5be5a34e42170"
RECIPE_COMMIT = "935817c66f5d6e64838f419692545f06787e485f"
BASE = f"https://huggingface.co/{REPO}/resolve/{REVISION}"
DEST = Path("crates/psionic/crates/psionic-models/src/testdata/deepseek_v41")
PINS = {
    "tokenizer.json": "c90dfa01249db1be4245780a052ede752e1361c612ac6d08e2bdada7d599476b",
    "tokenizer_config.json": "6ac8c8dc065ed118161d02dd532749ae3f52c243deac27872134fae2f50d8547",
    "encoding/encoding.py": "13231c592e958fb39e2af6001dae69d90e7a36b265702b9b390236fb387599e2",
    "encoding/tests/test_input_1.json":
        "6a5a86694e47169c8f0c9243a7e24cc5db05447be0d3d3a9891034e913c518de",
    "encoding/tests/test_input_2.json":
        "fd985cda88e8640b8e2404d8e659f7006cad204eb41f73c9e6662b1f218c01aa",
    "encoding/tests/test_input_3.json":
        "4207613f15c79b95ad92074bdb8b2b81c5ce9cfdf15b7a67f58ded6bb3ec4fa5",
    "encoding/tests/test_input_4.json":
        "4c3890a7bc042ec6396d98d024a39a73cdcffbe1d0253e852da0c35e65296f3e",
    "encoding/tests/test_input_5.json":
        "88289b0aac8d650aa55c4ffeb392818fff3a0ac83c2293ddfd90546804d5e5c4",
    "encoding/tests/test_output_1.txt":
        "91f79aa955717b414a82aabfb38bcd83ab5a998bd47db89d2a26338e8d3de550",
    "encoding/tests/test_output_2.txt":
        "9fa162587b73ab9b315be5c88d433f9e0e681b4b36926952f289099ed9bc1dab",
    "encoding/tests/test_output_3.txt":
        "cf2b801d0738c88abb691bf4a096000c0e0737faab8a54a4ba5243c5513a3711",
    "encoding/tests/test_output_4.txt":
        "94433b68370dcc69fa61efc5011b5d0e28685e4716fee1ee77fbbfc0c9935b54",
    "encoding/tests/test_output_5.txt":
        "4d673091746553d1cb2e40a3fc01dc09a7c53c40627096a3e49c97a269a81f64",
}

DSML = "｜DSML｜"
EOS = "<｜end▁of▁sentence｜>"
THINK_END = "</think>"
VOCABULARY_SIZE = 128000
LONG_QUERY = ("weather \"history\" for Beijing, Shanghai and Guangzhou\nwith a\\backslash, "
              "温度和湿度 " * 12).strip()

# Oracle prompts as the V4.1 template renders them in chat mode, with the
# SHA-256 of the rendered text and the token IDs from the oracle's manifest
# (gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/dump/manifest.json).
# psionic-v41-stage's tests render them and compare the IDs.
ORACLE_PROMPTS = {
    "p2-code": (
        "<｜begin▁of▁sentence｜><｜User｜>Write a Rust function `fn fib(n: u32) -> Option<u128>` "
        "that returns the n-th Fibonacci number, using checked arithmetic so it returns None on "
        "overflow.<｜Assistant｜></think>",
        "9967ee5de3771e56791192382556d86fe6319eb104f9ab4aadaa61bff659137d",
        [0, 128803, 21750, 260, 58408, 2019, 3608, 28952, 10150, 3913, 28, 549, 2111, 11, 6248,
         30397, 30, 87, 7833, 80225, 396, 10340, 270, 313, 7784, 26509, 1167, 14, 1812, 21002,
         29568, 832, 436, 10340, 9952, 377, 37126, 16, 128804, 128822],
    ),
}

# Each streaming case is a list of (kind, text) pieces: plain text encodes
# without the added vocabulary, and a special piece is one added-token ID.
STREAM_CASES = {
    "reasoning_then_final": [
        ("plain", "One plus one is two.\n"), ("special", THINK_END),
        ("plain", "\n\nThe answer is 2.\n"), ("special", EOS)],
    "tool_call_long_arguments": [
        ("plain", "I should search."), ("special", THINK_END),
        ("plain", "\n\n<"), ("special", DSML), ("plain", " calls>\n<"), ("special", DSML),
        ("plain", ' invoke name="search">\n<'), ("special", DSML),
        ("plain", ' parameter name="query" string="true">' + LONG_QUERY + "</"), ("special", DSML),
        ("plain", " parameter>\n<"), ("special", DSML),
        ("plain", ' parameter name="limit" string="false">5</'), ("special", DSML),
        ("plain", " parameter>\n</"), ("special", DSML), ("plain", " invoke>\n<"), ("special", DSML),
        ("plain", ' invoke name="get_weather">\n<'), ("special", DSML),
        ("plain", ' parameter name="location" string="true">Paris</'), ("special", DSML),
        ("plain", " parameter>\n</"), ("special", DSML), ("plain", " invoke>\n</"), ("special", DSML),
        ("plain", " calls>"), ("special", EOS)],
    "literal_markers_in_text": [
        ("plain", "Write </think> and <｜DSML｜ calls> as text."), ("special", EOS)],
    "split_utf8": [("plain", "Crab: 🦀 and 𝔘."), ("special", EOS)],
    "truncated_reasoning": [("plain", "Let me think about the first step and")],
    "no_finish": [("plain", "\n\nPartial answer")],
}


def fetch(name):
    data = urllib.request.urlopen(f"{BASE}/{name}", timeout=120).read()
    digest = hashlib.sha256(data).hexdigest()
    if digest != PINS[name]:
        raise RuntimeError(f"{name} has SHA-256 {digest}, expected {PINS[name]}")
    return data


def byte_alphabet():
    printable = list(range(33, 127)) + list(range(161, 173)) + list(range(174, 256))
    mapping, extra = {}, 0
    for value in range(256):
        if value in printable:
            mapping[value] = chr(value)
        else:
            mapping[value] = chr(256 + extra)
            extra += 1
    return {char: value for value, char in mapping.items()}


def reduced_tokenizer(definition, corpus):
    inverse = byte_alphabet()
    model = definition["model"]
    added = {token["content"] for token in definition["added_tokens"]}
    vocab = {}
    for token, token_id in model["vocab"].items():
        if any(char not in inverse for char in token):
            # Added tokens also appear in the model vocabulary unmapped.
            if token in added:
                vocab[token] = token_id
            continue
        data = bytes(inverse[char] for char in token)
        if len(data) == 1 or any(data in text for text in corpus):
            vocab[token] = token_id
    merges = []
    for merge in model["merges"]:
        left, right = merge.split(" ") if isinstance(merge, str) else merge
        if left in vocab and right in vocab and left + right in vocab:
            merges.append(merge)
    reduced = dict(definition)
    reduced["model"] = dict(model, vocab=vocab, merges=merges)
    return reduced


def expanded(reduced):
    """Fill the unused IDs below the model's vocabulary size, as the Rust loader does.

    The tokenizers library numbers added tokens from the model's vocabulary
    size, so the reduced model must keep that size. Each filler entry is a
    multi-character string outside the byte-level alphabet that no merge
    produces, so encoding never emits it.
    """
    model = reduced["model"]
    used = set(model["vocab"].values())
    vocab = dict(model["vocab"])
    for token_id in range(VOCABULARY_SIZE):
        if token_id not in used:
            vocab[f"\u0000unused{token_id}"] = token_id
    return dict(reduced, model=dict(model, vocab=vocab))


def without_added(definition):
    return Tokenizer.from_str(json.dumps(dict(definition, added_tokens=[])))


def main():
    if version("tokenizers") != "0.22.2":
        raise RuntimeError("Install tokenizers==0.22.2 before refreshing fixtures")
    raw = {name: fetch(name) for name in PINS}
    definition = json.loads(raw["tokenizer.json"])
    full = Tokenizer.from_str(raw["tokenizer.json"].decode())
    plain = without_added(definition)

    # Check the copied outputs against the pinned reference encoder.
    with tempfile.TemporaryDirectory() as directory:
        module_path = Path(directory) / "encoding_dsv41.py"
        module_path.write_bytes(raw["encoding/encoding.py"])
        spec = importlib.util.spec_from_file_location("encoding_dsv41", module_path)
        encoder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(encoder)
        for index in range(1, 6):
            input_path = Path(directory) / f"test_input_{index}.json"
            input_path.write_bytes(raw[f"encoding/tests/test_input_{index}.json"])
            (case,) = encoder.load_cases(str(input_path))
            rendered, _ = encoder.encode_case(case, "chat")
            if rendered != raw[f"encoding/tests/test_output_{index}.txt"].decode():
                raise RuntimeError(f"The reference encoder differs from test_output_{index}.txt")

    added = {token["content"]: token["id"] for token in definition["added_tokens"]}
    prompt_cases = []
    for index in range(1, 6):
        output = raw[f"encoding/tests/test_output_{index}.txt"].decode()
        prompt_cases.append(dict(
            name=f"test_{index}",
            input_sha256=hashlib.sha256(raw[f"encoding/tests/test_input_{index}.json"]).hexdigest(),
            output_sha256=hashlib.sha256(output.encode()).hexdigest(),
            token_ids=full.encode(output, add_special_tokens=False).ids))
    stream_cases = []
    for name, pieces in STREAM_CASES.items():
        ids = []
        for kind, text in pieces:
            ids.extend([added[text]] if kind == "special" else plain.encode(text, add_special_tokens=False).ids)
        stream_cases.append(dict(name=name, pieces=[list(piece) for piece in pieces], token_ids=ids))

    for name, (text, digest, ids) in ORACLE_PROMPTS.items():
        if hashlib.sha256(text.encode()).hexdigest() != digest:
            raise RuntimeError(f"The oracle prompt {name} isn't the text the oracle rendered")
        if full.encode(text, add_special_tokens=False).ids != ids:
            raise RuntimeError(f"The tokenizer encodes the oracle prompt {name} differently")

    corpus_texts = [case_output.decode() for name, case_output in raw.items() if "test_output" in name]
    corpus_texts += [text for text, _, _ in ORACLE_PROMPTS.values()]
    corpus_texts += [text for pieces in STREAM_CASES.values() for kind, text in pieces if kind == "plain"]
    reduced = reduced_tokenizer(definition, [text.encode() for text in corpus_texts])
    if len(definition["model"]["vocab"]) != VOCABULARY_SIZE:
        raise RuntimeError("The checkpoint's model vocabulary size changed")
    compact = Tokenizer.from_str(json.dumps(expanded(reduced)))
    compact_plain = without_added(expanded(reduced))
    for case in prompt_cases:
        text = raw[f"encoding/tests/test_output_{case['name'][5:]}.txt"].decode()
        if compact.encode(text, add_special_tokens=False).ids != case["token_ids"]:
            raise RuntimeError(f"The reduced tokenizer encodes {case['name']} differently")
    for text in corpus_texts:
        if compact_plain.encode(text, add_special_tokens=False).ids != plain.encode(text, add_special_tokens=False).ids:
            raise RuntimeError(f"The reduced tokenizer encodes {text!r} differently")
    for case in prompt_cases + stream_cases:
        for skip in (False, True):
            if compact.decode(case["token_ids"], skip_special_tokens=skip) != full.decode(case["token_ids"], skip_special_tokens=skip):
                raise RuntimeError(f"The reduced tokenizer decodes {case['name']} differently")
    split = plain.encode("🦀", add_special_tokens=False).ids
    if len(split) < 2 or "�" not in full.decode(split[:1]):
        raise RuntimeError("The split UTF-8 case no longer splits a character across tokens")

    reduced_text = json.dumps(reduced, ensure_ascii=False, separators=(",", ":")) + "\n"
    fixture = dict(
        schema="psionic.deepseek_v41_flash_protocol.v1",
        source=dict(repo=REPO, revision=REVISION, recipe_commit=RECIPE_COMMIT,
                    files={name: hashlib.sha256(data).hexdigest() for name, data in raw.items()},
                    reference="tokenizers 0.22.2; encoding/encoding.py with thinking_mode chat"),
        reduced_tokenizer=dict(file="tokenizer.reduced.json", vocabulary_size=VOCABULARY_SIZE,
                               sha256=hashlib.sha256(reduced_text.encode()).hexdigest(),
                               vocabulary=len(reduced["model"]["vocab"]),
                               merges=len(reduced["model"]["merges"])),
        prompt_cases=prompt_cases, stream_cases=stream_cases)
    (DEST / "encoding" / "tests").mkdir(parents=True, exist_ok=True)
    for index in range(1, 6):
        for stem, suffix in (("test_input", "json"), ("test_output", "txt")):
            name = f"encoding/tests/{stem}_{index}.{suffix}"
            (DEST / name).write_bytes(raw[name])
    (DEST / "tokenizer_config.json").write_bytes(raw["tokenizer_config.json"])
    (DEST / "tokenizer.reduced.json").write_text(reduced_text)
    (DEST / "golden.json").write_text(json.dumps(fixture, indent=1, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
