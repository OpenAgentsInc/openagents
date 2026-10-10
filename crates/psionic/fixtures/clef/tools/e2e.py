"""End-to-end Clef comparison helpers (stdlib only).

  e2e.py select <corpus.jsonl> <expected.jsonl> <max_tokens> <count> > requests.jsonl
  e2e.py run <base_url> <requests.jsonl> <out.jsonl>        # POST /v1/systemone each
  e2e.py compare <a.jsonl> <b.jsonl> [<same-prompt-as.jsonl>]
      top answer agreement and |dp|; with a third file, only requests where
      a, b and that file report the same input_tokens (the same prompt).
"""

import json
import sys
import time
import urllib.error
import urllib.request


def select(corpus, expected, max_tokens, count):
    picked = 0
    for body, want in zip(open(corpus, encoding="utf-8"), open(expected, encoding="utf-8")):
        if picked >= count:
            break
        if len(json.loads(want)["input_ids"]) > max_tokens:
            continue
        print(fill_instructions(body))
        picked += 1


class Raw(str):
    pass


def dump(value):
    if isinstance(value, Raw):
        return str(value)
    if isinstance(value, dict):
        return "{" + ",".join(json.dumps(k, ensure_ascii=False) + ":" + dump(v) for k, v in value.items()) + "}"
    if isinstance(value, list):
        return "[" + ",".join(dump(v) for v in value) + "]"
    return json.dumps(value, ensure_ascii=False)


def fill_instructions(body):
    """llama.cpp requires `instructions`; the reference reads a missing,
    null or empty one as the question id, so spell that out (the prompt is
    unchanged). Number spellings are kept."""
    record = json.loads(body, parse_float=Raw, parse_int=Raw)
    for qid, question in record["questions"].items():
        if question.get("instructions") in (None, ""):
            question["instructions"] = qid
    return dump(record)


def run(base, requests, out):
    with open(out, "w", encoding="utf-8") as sink:
        for index, line in enumerate(open(requests, encoding="utf-8")):
            began = time.time()
            request = urllib.request.Request(
                base.rstrip("/") + "/v1/systemone",
                data=line.encode("utf-8"),
                headers={"content-type": "application/json"},
            )
            try:
                with urllib.request.urlopen(request, timeout=3600) as response:
                    status, body = response.status, json.loads(response.read())
            except urllib.error.HTTPError as error:
                status, body = error.code, json.loads(error.read() or b"null")
            elapsed = time.time() - began
            sink.write(json.dumps({"index": index, "status": status, "seconds": round(elapsed, 3), "body": body}, ensure_ascii=False) + "\n")
            sink.flush()
            print(index, status, round(elapsed, 1), file=sys.stderr, flush=True)


def probabilities(answer):
    if answer["type"] == "noul":
        return {"true": answer["noul"], "false": 1 - answer["noul"]}
    return {str(k): float(v) for k, v in answer["probabilities"].items()}


def top(answer):
    if answer["type"] == "noul":
        return answer["noul"] >= 0.5
    p = probabilities(answer)
    return max(p, key=p.get)


def tokens(row):
    return row["body"].get("usage", {}).get("input_tokens") if row["status"] == 200 else None


def compare(left, right, same_as=None):
    a = [json.loads(line) for line in open(left, encoding="utf-8")]
    b = [json.loads(line) for line in open(right, encoding="utf-8")]
    if same_as:
        c = [json.loads(line) for line in open(same_as, encoding="utf-8")]
        keep = [tokens(x) is not None and tokens(x) == tokens(y) == tokens(z) for x, y, z in zip(a, b, c)]
        a = [x for x, k in zip(a, keep) if k]
        b = [y for y, k in zip(b, keep) if k]
    questions = agree = 0
    worst = 0.0
    worst_at = None
    deltas = []
    near_ties = []
    for x, y in zip(a, b):
        if x["status"] != 200 or y["status"] != 200:
            print("skip", x["index"], x["status"], y["status"])
            continue
        for qid, ax in x["body"]["answers"].items():
            ay = y["body"]["answers"][qid]
            px, py = probabilities(ax), probabilities(ay)
            questions += 1
            same = top(ax) == top(ay)
            agree += same
            if not same:
                near_ties.append((x["index"], qid, sorted(px.values())[-2:], sorted(py.values())[-2:]))
            for key in px:
                delta = abs(px[key] - py[key])
                deltas.append(delta)
                if delta > worst:
                    worst, worst_at = delta, (x["index"], qid, key)
    deltas.sort()
    print(json.dumps({
        "requests": min(len(a), len(b)),
        "request_indexes": [x["index"] for x in a],
        "questions": questions,
        "top_answer_agreement": agree / max(questions, 1),
        "max_abs_dp": worst,
        "max_at": worst_at,
        "median_abs_dp": deltas[len(deltas) // 2] if deltas else None,
        "p99_abs_dp": deltas[int(len(deltas) * 0.99)] if deltas else None,
        "disagreements": near_ties,
    }, indent=1))


if __name__ == "__main__":
    command = sys.argv[1]
    if command == "select":
        select(sys.argv[2], sys.argv[3], int(sys.argv[4]), int(sys.argv[5]))
    elif command == "run":
        run(sys.argv[2], sys.argv[3], sys.argv[4])
    elif command == "compare":
        compare(sys.argv[2], sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else None)
