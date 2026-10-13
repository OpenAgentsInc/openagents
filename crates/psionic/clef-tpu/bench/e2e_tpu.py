"""Run the 40 e2e fixture requests through the TPU runner (encoded records
from the reference encoder fixtures, which the Rust encoder matches exactly)
and write rows in `tools/e2e.py` format.

  python -I e2e_tpu.py <clef-flash dir> <fixtures/clef> <out.jsonl> [--chunk 2048] [--precision bf16] [--scan jax]
"""
import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from clef_tpu.runner import Runner, answer  # noqa: E402


def records(fx, max_tokens=1100, count=40):
    exp = []
    for f in ("expected-000.jsonl", "expected-100.jsonl"):
        exp += [json.loads(l) for l in open(fx / "encoder" / f, encoding="utf-8")]
    picked = [e for e in exp if len(e["input_ids"]) <= max_tokens][:count]
    reqs = [json.loads(l) for l in open(fx / "e2e" / "requests.jsonl", encoding="utf-8")]
    assert len(picked) == len(reqs) == count
    return list(zip(reqs, picked))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("model")
    ap.add_argument("fixtures")
    ap.add_argument("out")
    ap.add_argument("--chunk", type=int, default=0)
    ap.add_argument("--chunks", default="", help="extra chunk sizes for the invariance check, e.g. 512,64")
    ap.add_argument("--precision", default="bf16")
    ap.add_argument("--scan", default="jax")
    a = ap.parse_args()
    r = Runner(a.model, chunk=a.chunk, precision=a.precision, scan=a.scan)
    print("loaded", round(r.load_seconds, 1), "s", r.identity(), file=sys.stderr, flush=True)
    inv = {"chunks": [int(c) for c in a.chunks.split(",") if c], "max_abs_dlogit": 0.0, "argmax_flips": 0}
    with open(a.out, "w", encoding="utf-8") as sink:
        for i, (req, enc) in enumerate(records(Path(a.fixtures))):
            assert list(req["questions"]) == [q["id"] for q in enc["questions"]]
            t = time.time()
            lg = r.logits(enc["input_ids"], enc["questions"])
            dt = time.time() - t
            for c in inv["chunks"]:
                lc = r.logits(enc["input_ids"], enc["questions"], chunk=c)
                for x, y in zip(lg, lc):
                    inv["max_abs_dlogit"] = max(inv["max_abs_dlogit"], max(abs(p - q) for p, q in zip(x, y)))
                    inv["argmax_flips"] += int(max(range(len(x)), key=x.__getitem__) != max(range(len(y)), key=y.__getitem__))
            answers = {q["id"]: answer(req["questions"][q["id"]], q["option_ids"], l) for q, l in zip(enc["questions"], lg)}
            body = {"model": req["model"], "answers": answers,
                    "usage": {"input_tokens": len(enc["input_ids"]), "output_tokens": 0}, "psionic": r.identity()}
            sink.write(json.dumps({"index": i, "status": 200, "seconds": round(dt, 3), "body": body}, ensure_ascii=False) + "\n")
            print(i, len(enc["input_ids"]), round(dt, 3), file=sys.stderr, flush=True)
    if inv["chunks"]:
        print("INVARIANCE " + json.dumps(inv), flush=True)


if __name__ == "__main__":
    main()
