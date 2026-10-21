#!/usr/bin/env python3
"""W2 (#12389): check complete V4.1 answers against the oracle prompts.

    w2_answers.py ANSWER.json [ANSWER.json ...] > checks.json

Each ANSWER.json is a `psionic-v41-stage chain` report (its `answer`
object) or a `{"id": ..., "text": ..., "source": ...}` record of a Pylon
answer, which carries the final text only. The script pairs each answer with
the oracle prompt it names (p1-chat-thinking or p2-code, from the retained
#12371 manifest), and reports:

  - the token comparison with the oracle's complete answer (chain reports
    only): the first differing position and the equal prefix;
  - stop behavior: the end token, the finish reason, the output cap;
  - the reasoning/final split from the #12372 adapter: thinking-mode answers
    need reasoning and a final answer, chat-mode answers no reasoning, and no
    `</think>` text in a final answer;
  - task assertions: p1 has two sentences about Rayleigh scattering; p2's
    Rust function compiles, returns `F(n)` for every input whose value fits
    in `u128` (0, 1, 2, 10, 93, 185, and 186, as 30ba21cb1e requires), and
    `None` past it. The oracle's answers are scored by the same assertions,
    for pairing; agreeing with the oracle doesn't excuse a failure.

Task assertions are designated by this script, not a frozen G2 v2 policy;
the report says so. It needs `rustc` on PATH for p2.
"""

import gzip
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MANIFEST = ROOT / (
    "crates/psionic/crates/psionic-models/src/testdata/deepseek_v41/oracle/manifest.json.gz"
)
EOS = 1
THINK_END = 128822

FIB_TESTS = """
#[cfg(test)]
mod w2 {
    use super::fib;
    #[test]
    fn must_small_values() {
        assert_eq!(fib(0), Some(0));
        assert_eq!(fib(1), Some(1));
        assert_eq!(fib(2), Some(1));
        assert_eq!(fib(10), Some(55));
        assert_eq!(fib(93), Some(12200160415121876738));
    }
    #[test]
    fn must_overflow_is_none() {
        assert_eq!(fib(187), None);
        assert_eq!(fib(200), None);
        assert_eq!(fib(u32::MAX), None);
    }
    #[test]
    fn must_largest_values() {
        // F(185) and F(186) fit in u128; an implementation that computes one
        // term ahead overflows at 186 (30ba21cb1e).
        assert_eq!(fib(185), Some(205697230343233228174223751303346572685));
        assert_eq!(fib(186), Some(332825110087067562321196029789634457848));
    }
}
"""


def oracle():
    with gzip.open(MANIFEST, "rt") as f:
        manifest = json.load(f)
    found = {}
    for prompt in manifest["prompts"]:
        if "complete_answer" in prompt:
            found[prompt["id"]] = prompt
    return found


def sentences(text):
    parts = [p for p in re.split(r"(?<=[.!?])\s+", text.strip()) if p.strip()]
    return len(parts)


def p1_tasks(content):
    lower = content.lower()
    checks = {
        "two_sentences": sentences(content) == 2,
        "names_scattering": "scatter" in lower,
        "blue_is_shorter_wavelength": "blue" in lower and "wavelength" in lower,
    }
    return checks


def p2_tasks(content):
    blocks = re.findall(r"```rust\n(.*?)```", content, re.S)
    checks = {"one_rust_block": len(blocks) == 1}
    if not blocks:
        return checks, "no rust block"
    code = blocks[0]
    checks["signature"] = "fn fib(n: u32) -> Option<u128>" in code
    with tempfile.TemporaryDirectory() as d:
        src = Path(d) / "fib.rs"
        src.write_text("#![allow(dead_code)]\n" + code + FIB_TESTS)
        build = subprocess.run(
            ["rustc", "--edition", "2021", "--test", "-O", str(src), "-o", str(Path(d) / "fib")],
            capture_output=True,
            text=True,
            timeout=300,
        )
        checks["compiles"] = build.returncode == 0
        if build.returncode != 0:
            return checks, build.stderr[-2000:]
        run = subprocess.run(
            [str(Path(d) / "fib"), "--test-threads=1"],
            capture_output=True,
            text=True,
            timeout=300,
        )
        for name in ("must_small_values", "must_overflow_is_none", "must_largest_values"):
            checks[name] = f"w2::{name} ... ok" in run.stdout
        return checks, run.stdout[-2000:]


def check(path, oracle_prompts):
    record = json.loads(Path(path).read_text())
    answer = record.get("answer", record)
    pid = record.get("id") or answer.get("id")
    if pid is None:
        prompt_ids = answer.get("prompt_ids")
        for name, prompt in oracle_prompts.items():
            if prompt["prompt_token_ids"] == prompt_ids:
                pid = name
    reference = oracle_prompts.get(pid)
    out = {"file": Path(path).name, "id": pid, "source": record.get("source", "chain")}
    if reference is None:
        out["error"] = "no oracle prompt matches"
        return out
    ref = reference["complete_answer"]
    thinking = reference["thinking_mode"] == "thinking"
    out["oracle_mode"] = reference["thinking_mode"]
    if "ids" in answer:
        ids = answer["ids"]
        out["prompt_ids_equal_oracle"] = answer.get("prompt_ids") == reference["prompt_token_ids"]
        out["answer_mode_thinking"] = answer.get("thinking")
        first = next(
            (i for i, (a, b) in enumerate(zip(ids, ref["ids"])) if a != b),
            min(len(ids), len(ref["ids"])),
        )
        out["tokens"] = {
            "served": len(ids),
            "oracle": len(ref["ids"]),
            "exact_match": ids == ref["ids"],
            "equal_prefix": first,
            "first_difference": None
            if ids == ref["ids"]
            else {
                "position": first,
                "served": ids[first] if first < len(ids) else None,
                "oracle": ref["ids"][first] if first < len(ref["ids"]) else None,
            },
        }
        out["stop"] = {
            "ends_with_eos": bool(ids) and ids[-1] == EOS,
            "finish": answer.get("finish"),
            "cap": answer.get("max_tokens"),
        }
        out["split"] = {
            "think_end_tokens": ids.count(THINK_END),
            "reasoning_chars": len(answer.get("reasoning", "")),
            "content_chars": len(answer.get("content", "")),
        }
        content = answer.get("content", "")
        reasoning = answer.get("reasoning", "")
        out["split"]["ok"] = (
            (bool(reasoning) and ids.count(THINK_END) == 1 if thinking else not reasoning)
            and bool(content.strip())
            and "</think>" not in content
        )
        out["timing"] = {
            "prefill_ms": answer.get("prefill_ms"),
            "decode_tokens_per_s": answer.get("decode_tokens_per_s"),
        }
    else:
        content = record["text"]
        out["stop"] = {"finish": record.get("finish")}
    out["content"] = content
    if pid == "p1-chat-thinking":
        tasks = p1_tasks(content)
        detail = None
    else:
        tasks, detail = p2_tasks(content)
    out["tasks"] = tasks
    out["must_pass"] = all(tasks.values())
    if detail:
        out["task_output"] = detail
    return out


def oracle_tasks(oracle_prompts):
    """The same assertions on the oracle's own complete answers."""
    paired = {}
    for pid, prompt in oracle_prompts.items():
        content = prompt["complete_answer"]["parsed"]["content"]
        if pid == "p1-chat-thinking":
            tasks = p1_tasks(content)
        else:
            tasks, _ = p2_tasks(content)
        paired[pid] = {
            "tasks": tasks,
            "must_pass": all(tasks.values()),
        }
    return paired


def main():
    oracle_prompts = oracle()
    results = [check(p, oracle_prompts) for p in sys.argv[1:]]
    json.dump(
        {
            "schema": "openagents.deepseek_v41.w2_answer_checks.v1",
            "assertions": "designated by scripts/bench/deepseek-v41/w2_answers.py; "
            "G2 v2's frozen task suite and quality budget are not yet defined",
            "oracle": oracle_tasks(oracle_prompts),
            "results": results,
        },
        sys.stdout,
        indent=1,
    )
    print()


if __name__ == "__main__":
    main()
