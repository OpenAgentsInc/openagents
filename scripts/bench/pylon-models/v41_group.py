"""DeepSeek-V4.1 answers through a Pylon model group (W14, #12401).

`run.py --v41-group` sends each qualification prompt through a running
group with `pylon ask --model WANT --deadline SECS`: the router, with
`Settings::want`, sends the job only to a ready group (or pylon) serving that
exact model, under `pylon.deadline.v1` with this buyer's budget. Each answer
must be complete:

- the job ended with finish `stop`, inside the deadline the group's beacon
  declares and the buyer's budget, measured from sending the request;
- the answer has a final part. The real group's last stage splits each
  token into reasoning or final answer at the model's own `</think>` token
  (#12372's parser), and the head returns only the final answer, as every
  pylon keeps reasoning private; the head refuses an answer with reasoning
  and no final part. A group whose stages send plain text (the tests'
  synthetic stages) is split here the way the adapter starts a thinking-mode
  answer: in reasoning, until the first `</think>`;
- the final answer scores as `run.py` scores the same prompt.

The group is started elsewhere: `pylon cloud up` for the real one, or a
test's synthetic stages. The buyer key lives in the output folder, never in
the owner's home, and no receipts are published.
"""

import json
import time
from pathlib import Path

# The V4.1 thinking markers of the #12372 adapter (`deepseek_v41.rs`).
THINK_END = "</think>"
END_OF_SENTENCE = "<｜end▁of▁sentence｜>"
# What `pylon ask` says when no ready group has its slot free.
NO_PYLON = "no live pylon serves the requested model"
BUSY_RETRIES = 12
BUSY_WAIT_SECS = 5


def split(text):
    """Split a V4.1 thinking-mode answer into (reasoning, final).

    The adapter starts a thinking-mode answer in reasoning
    (`ThinkingInitialStage::ReasoningStart`), so everything before the first
    `</think>` is reasoning and unfinished reasoning never becomes the final
    answer: without the marker, final is None. The final answer is kept byte
    for byte, without the end-of-sentence marker. This reads decoded text;
    a marker the model wrote as ordinary characters can't be told apart here,
    which the token-level adapter does.
    """
    text = text or ""
    if text.startswith("<think>"):
        text = text[len("<think>"):]
    reasoning, marker, final = text.partition(THINK_END)
    if not marker:
        return text, None
    if final.endswith(END_OF_SENTENCE):
        final = final[: -len(END_OF_SENTENCE)]
    return reasoning, final


def check(name, response, score):
    """The checks for one CLI answer, as (name, passed, detail) rows."""
    data = response.get("data") or {}
    declared = data.get("declared") or {}
    text = data.get("text")
    reasoning, final = split(text)
    if final is None and text and declared.get("finish") == "stop":
        # The head split the answer at the model's token and sent the final
        # part only.
        reasoning, final = None, text
    ceiling = declared.get("ceiling_secs")
    deadline = declared.get("deadline_secs")
    answer_ms = data.get("answer_ms")
    scored = score(name, final or "") if final is not None else False
    return [
        ("answered", bool(response.get("ok")) and bool(data.get("text")),
         {"error": data.get("error") or response.get("summary")}),
        ("extended", deadline is not None and declared.get("admitted_secs") == deadline,
         {"declared": declared}),
        ("finish_stop", declared.get("finish") == "stop", {}),
        ("within_declared_deadline", answer_ms is not None and ceiling is not None
         and deadline is not None and answer_ms <= min(ceiling, deadline) * 1000,
         {"answer_ms": answer_ms, "ceiling_secs": ceiling, "deadline_secs": deadline}),
        ("final_answer", bool((final or "").strip()),
         {"reasoning_chars": None if reasoning is None else len(reasoning),
          "split_by": "head" if reasoning is None else "harness", "final": final}),
        ("final_scores", scored is not False, {"score": scored}),
    ]


def qualify(args, cases, score, cli):
    """Send `cases` through the group and check every answer."""
    folder = Path(args.output) / "v41-group"
    folder.mkdir(parents=True, exist_ok=True)
    home = folder / "buyer-home"
    report = folder / "results.json"
    result = {"scope": "v41_group_through_pylon", "relay": args.relay, "want": args.v41_want,
              "budget_secs": args.v41_budget, "qualified": False, "cases": []}
    try:
        for name, prompt in cases:
            if args.cases and name not in args.cases:
                continue
            command = ["ask", prompt, "--model", args.v41_want, "--deadline", str(args.v41_budget),
                       "--no-receipt", "--relay", args.relay]
            answer = cli(args.pylon_bin, home, command, timeout=args.v41_budget + 60)
            # The head's beacon says its one slot is busy for a moment after
            # an answer; discovery then finds no free group. Ask again.
            for _ in range(BUSY_RETRIES):
                if NO_PYLON not in str(answer["response"].get("summary")):
                    break
                time.sleep(BUSY_WAIT_SECS)
                answer = cli(args.pylon_bin, home, command, timeout=args.v41_budget + 60)
            rows = check(name, answer["response"], score)
            case = {"name": name, "prompt": prompt, "cli": answer,
                    "checks": [{"name": n, "passed": bool(p), **d} for n, p, d in rows],
                    "passed": all(p for _, p, _ in rows)}
            result["cases"].append(case)
            report.write_text(json.dumps(result, indent=2))
            print(json.dumps({"event": "v41-group-answer", "case": name, "passed": case["passed"],
                              "failed": [n for n, p, _ in rows if not p]}), flush=True)
        result["qualified"] = bool(result["cases"]) and all(c["passed"] for c in result["cases"])
    except Exception as error:
        result["error"] = str(error)
    finally:
        report.write_text(json.dumps(result, indent=2))
    return result
