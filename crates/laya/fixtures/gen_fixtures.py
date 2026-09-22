"""Generate the laya conformance fixtures the Rust port is tested against.

Run inside the reference checkout (the layout ~/work/laya-artifacts ships):

    .venv/bin/python crates/laya/fixtures/gen_fixtures.py \
        --checkpoint english --model-dir ~/work/laya-artifacts/english \
        --out crates/laya/fixtures

Emits, under --out:

- requests-<name>.json   TypeSafe-shaped request bodies plus the reference's
                         full system_one answers, fp32 CPU
- sequences-<name>.json  build_sequence() output: input ids and marker
                         positions per question
- manifest-<name>.json   sha256 of every artifact file the Rust side loads

No model weights are copied; the manifest records digests so a run can
prove it loaded the same bytes the goldens came from.
"""
import argparse, hashlib, json, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


# The requests the Rust port is conformance-tested against. Shapes cover
# the three types, a mixed batch, a structured (non-string) state, an
# options-fit refusal case, and option-cardinality temperature buckets.
def requests_corpus():
    return [
        {
            "name": "support_mixed",
            "request": {
                "state": "I was charged twice on the March invoice.",
                "model": "",
                "questions": {
                    "refund": {
                        "type": "noul",
                        "instructions": "Does the customer ask for money back?",
                    },
                    "topic": {
                        "type": "choice",
                        "instructions": "What is this about?",
                        "criteria": {"billing": None, "shipping": None, "other": None},
                    },
                    "severity": {
                        "type": "score",
                        "instructions": "How severe is this complaint?",
                        "criteria": ["trivial", "minor", "serious", "critical"],
                    },
                },
            },
        },
        {
            "name": "structured_state",
            "request": {
                "state": {
                    "ticket": {"subject": "refund", "priority": 2},
                    "history": ["opened 2024-03-01", "charged twice"],
                    "resolved": False,
                },
                "model": "",
                "questions": {
                    "close": {
                        "type": "noul",
                        "instructions": "Can this ticket be closed?",
                    },
                    "next": {
                        "type": "choice",
                        "instructions": "What should happen next?",
                        "criteria": ["investigate", "refund", "close"],
                    },
                },
            },
        },
        {
            "name": "choice_cardinalities",
            "request": {
                "state": "The export job failed at step 3 of 7.",
                "model": "",
                "questions": {
                    "bucket2": {
                        "type": "choice",
                        "instructions": "Retry or abort?",
                        "criteria": {"retry": None, "abort": None},
                    },
                    "bucket35": {
                        "type": "choice",
                        "instructions": "Which subsystem?",
                        "criteria": {"api": None, "worker": None, "db": None, "ui": None},
                    },
                    "bucket610": {
                        "type": "choice",
                        "instructions": "Which region?",
                        "criteria": {r: None for r in ["us1", "us2", "eu1", "eu2", "ap1", "ap2", "sa1"]},
                    },
                },
            },
        },
        {
            "name": "noul_defaults",
            "request": {
                "state": "Please cancel my subscription effective immediately.",
                "model": "",
                "questions": {
                    "cancel": {"type": "noul", "instructions": "Is the user cancelling?"},
                },
            },
        },
    ]


# One question per case so the sequence vectors stay readable.
def sequences_corpus():
    return [
        {
            "name": "noul",
            "state": "Refund please.",
            "question": {"type": "noul", "instructions": "Money back?"},
        },
        {
            "name": "choice_with_descriptions",
            "state": "Package never arrived.",
            "question": {
                "type": "choice",
                "instructions": "Which queue?",
                "criteria": {"shipping": "lost or late parcels", "billing": None},
            },
        },
        {
            "name": "score",
            "state": "This is the third outage this week.",
            "question": {
                "type": "score",
                "instructions": "How severe?",
                "criteria": ["mild", "bad", "critical"],
            },
        },
        {
            "name": "structured_state",
            "state": {"event": "signup", "plan": "pro", "seats": 12},
            "question": {"type": "noul", "instructions": "Paid plan?"},
        },
        {
            "name": "long_options_shrink",
            "state": "x",
            "question": {
                "type": "choice",
                "instructions": "pick one",
                "criteria": {" ".join(["opt", str(i)] * 40): None for i in range(30)},
            },
        },
        {
            "name": "list_criteria",
            "state": "hello",
            "question": {
                "type": "choice",
                "instructions": "which?",
                "criteria": ["a", "b", "c"],
            },
        },
    ]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--checkpoint", required=True, help="fixture name prefix, e.g. english")
    p.add_argument("--model-dir", required=True)
    p.add_argument("--out", required=True)
    args = p.parse_args()

    # rl_agent_api.py and rl_common.py live at the bundle root beside the
    # checkpoint directories.
    sys.path.insert(0, os.path.dirname(os.path.abspath(args.model_dir)))

    from rl_agent_api import RLAgent
    from rl_common import QTYPES, build_sequence, collate_items, render_options

    agent = RLAgent(args.model_dir, device="cpu")
    cfg = agent.cfg

    requests = []
    for case in requests_corpus():
        req = dict(case["request"])
        out = agent.system_one(req["state"], req["questions"])
        requests.append({"name": case["name"], "request": req, "response": out})
    with open(os.path.join(args.out, f"requests-{args.checkpoint}.json"), "w") as f:
        json.dump(requests, f, indent=1, ensure_ascii=False)

    sequences = []
    for case in sequences_corpus():
        q = agent._to_internal(case["question"])
        ids, markers = build_sequence(
            agent.tok, case["state"], q, cfg["max_len"], cfg["head_max_len"]
        )
        sequences.append(
            {
                "name": case["name"],
                "state": case["state"],
                "question": case["question"],
                "ids": ids,
                "markers": markers,
                "options": render_options(q),
            }
        )
    with open(os.path.join(args.out, f"sequences-{args.checkpoint}.json"), "w") as f:
        json.dump(sequences, f, indent=1, ensure_ascii=False)

    manifest = {"checkpoint": args.checkpoint, "files": {}}
    for name in [
        "rl_agent_config.json",
        "encoder/config.json",
        "tokenizer/tokenizer.json",
        "tokenizer/tokenizer_config.json",
        "model.safetensors",
    ]:
        path = os.path.join(args.model_dir, name)
        manifest["files"][name] = {"sha256": sha256(path), "bytes": os.path.getsize(path)}
    with open(os.path.join(args.out, f"manifest-{args.checkpoint}.json"), "w") as f:
        json.dump(manifest, f, indent=1)


if __name__ == "__main__":
    main()
