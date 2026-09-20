#!/usr/bin/env python3
"""The cheap baseline, served as a `POST /v1/systemone` door.

`baseline.py` measured frozen sentence embeddings plus logistic regression
off-line and found them worth building a door for. This is that door. It
speaks the System One contract so `gym eval --door` scores it on the same
suite, through the same client and the same store, as hosted Jev, Kev, and
Lev. The Gym learns nothing about it beyond what `GET /v1/models` publishes.

What it serves: Choice, for a question whose option set matches one the door
fitted a head on. At startup it embeds the fitting partition of the suite
with a frozen encoder, chooses the L2 strength by cross-validation inside
that partition, and fits one multinomial head per Choice family. Nothing is
fitted after startup, and the scoring partition is never read.

What it refuses, with a typed code the Gym records as the door's answer
rather than as a harness failure:

- `unsupported_primitive` for a Noul or a Score. A Noul asks for the
  probability that a statement holds and a Score for a position on an ordered
  rubric; a classifier fitted on labels returns neither. The same argument
  declined Laya, and it holds when the model is ours.
- `option_set_drift` for a Choice whose options are not the set a head was
  fitted on. A fitted head is pinned to the labels it saw and cannot answer
  a caller-supplied option set, which is what Kev's pointer readout exists
  to do.

Every refusal answers `422` with `{"error": {"code", "message", "question"}}`,
the envelope `gym::eval::classify` reads. `422` is not a status the `jev`
client retries, so a refusal costs one call.

    python3 door.py \
        --suite ../../crates/gym/suites/support-v2-three-way.json \
        --encoder BAAI/bge-base-en-v1.5 \
        --revision a5beb1e3e68b9ab74eb54cfd186867f64f240e1a \
        --port 8020
"""

from __future__ import annotations

import argparse
import json
import sys
import threading
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import numpy as np
from sklearn.linear_model import LogisticRegression

from baseline import C_GRID, REFUSALS, FrozenEncoder, load_suite, option_set, pick_strength

# The one status every refusal answers with. Not in the client's retry set.
REFUSAL_STATUS = 422


class Refusal(Exception):
    """A request the door declines, with the code the Gym records."""

    def __init__(self, code: str, message: str, question: str | None):
        super().__init__(message)
        self.code = code
        self.message = message
        self.question = question


@dataclass
class Head:
    """One fitted family: its options, its head, and how the strength was chosen."""

    family: str
    options: list[str]
    model: LogisticRegression
    strength: float
    rule: str
    fitted_on: int

    def matches(self, options: list[str]) -> bool:
        return sorted(options) == sorted(self.options)


class Door:
    """The fitted heads and the encoder, answering one request at a time."""

    def __init__(self, name: str, encoder: FrozenEncoder, heads: list[Head], fitted: dict):
        self.name = name
        self.encoder = encoder
        self.heads = heads
        self.fitted = fitted
        # The encoder is not documented as thread-safe; requests take turns.
        self.lock = threading.Lock()

    def card(self) -> dict:
        """What `GET /v1/models` publishes, in the fields `gym eval` reads.

        `base_model_signature` is the encoder revision, so every row the Gym
        records names the encoder and the commit it was pinned to. The rows
        carry the door's name, which names the encoder as well.
        """
        described = self.encoder.describe()
        return {
            "id": self.name,
            "name": self.name,
            "description": (
                f"frozen {described['name']} embeddings and a multinomial logistic "
                "regression head per Choice family; Noul and Score are refused"
            ),
            "release_date": "2026-09-20",
            "base_model_signature": described["revision"],
            "adapter": "",
            "estimator": f"logistic-regression/{self.fitted['rule']}",
            "encoder": described,
            "fitted": self.fitted,
            "refuses": sorted(REFUSALS),
        }

    def answer(self, body: dict) -> dict:
        state = body.get("state")
        questions = body.get("questions")
        if not isinstance(state, str) or not state:
            raise Refusal("invalid_request", "`state` must be a non-empty string", None)
        if not isinstance(questions, dict) or not questions:
            raise Refusal("invalid_request", "`questions` must hold at least one question", None)

        # Every refusal is decided before the encoder runs, so a request the
        # door declines costs it nothing and answers the same way every time.
        plan: list[tuple[str, list[str], Head]] = []
        for question_id, question in questions.items():
            if not isinstance(question, dict):
                raise Refusal("invalid_request", f"question `{question_id}` is not an object", question_id)
            kind = question.get("type")
            if kind in REFUSALS:
                raise Refusal(REFUSALS[kind]["code"], REFUSALS[kind]["reason"], question_id)
            if kind != "choice":
                raise Refusal("invalid_request", f"question `{question_id}` has unsupported type `{kind}`", question_id)
            criteria = question.get("criteria")
            if not isinstance(criteria, (dict, list)) or not criteria:
                raise Refusal("invalid_request", f"question `{question_id}` names no options", question_id)
            options = option_set({"question": question})
            head = next((head for head in self.heads if head.matches(options)), None)
            if head is None:
                raise Refusal(
                    "option_set_drift",
                    (
                        f"question `{question_id}` names options {options}, which no fitted head "
                        "covers. A fitted head is pinned to the labels it saw; it cannot answer "
                        "a caller-supplied option set. Fitted: "
                        + ", ".join(f"{head.family}={head.options}" for head in self.heads)
                    ),
                    question_id,
                )
            plan.append((question_id, options, head))

        with self.lock:
            features = self.encoder.encode([state])
        answers = {}
        for question_id, options, head in plan:
            row = head.model.predict_proba(features)[0]
            by_label = {label: float(p) for label, p in zip(head.model.classes_, row)}
            # Probabilities in the caller's order; the pick is the largest,
            # and equal leaders resolve to the first the caller listed.
            probabilities = {option: by_label[option] for option in options}
            choice = max(options, key=probabilities.get)
            answers[question_id] = {
                "type": "choice",
                "choice": choice,
                "confidence": probabilities[choice],
                "probabilities": probabilities,
            }
        return {"model": self.name, "answers": answers, "usage": {}}


def fit(suite_path: Path, encoder: FrozenEncoder, rule: str, seed: int) -> tuple[list[Head], dict]:
    """Fits one head per Choice family on the suite's fitting partition."""
    suite, key, fit_partition, _ = load_suite(suite_path)
    items = suite["items"]
    heads: list[Head] = []
    families: dict[str, dict] = {}
    for family in sorted({item["family"] for item in items}):
        family_items = [item for item in items if item["family"] == family]
        kind = family_items[0]["kind"]
        fit_items = [item for item in family_items if item[key] == fit_partition]
        if kind != "choice":
            families[family] = {"kind": kind, "served": False, "code": REFUSALS[kind]["code"]}
            continue
        options = option_set(fit_items[0])
        for item in fit_items:
            if option_set(item) != options:
                raise SystemExit(f"{item['id']} carries a different option set from its family")
        x_fit = encoder.encode([item["state"] for item in fit_items])
        y_fit = np.asarray([item["truth"] for item in fit_items])
        selection = pick_strength(x_fit, y_fit, options, seed)
        strength = selection["strength_by_rule"][rule]
        model = LogisticRegression(C=strength, max_iter=5000, random_state=0)
        model.fit(x_fit, y_fit)
        heads.append(Head(family, options, model, strength, rule, len(fit_items)))
        families[family] = {
            "kind": kind,
            "served": True,
            "options": options,
            "fitted_on": len(fit_items),
            "strength": strength,
            "strength_at_grid_edge": selection["at_grid_edge"][rule],
        }
    fitted = {
        "suite": suite.get("name", suite_path.stem),
        "suite_digest": suite.get("digest"),
        "partition": fit_partition,
        "rule": rule,
        "fold_seed": seed,
        "strength_grid": C_GRID,
        "families": families,
    }
    return heads, fitted


def serve(door: Door, host: str, port: int) -> None:
    class Handler(BaseHTTPRequestHandler):
        server_version = "baseline-door/0.1"

        def log_message(self, format: str, *args) -> None:  # noqa: A002
            sys.stderr.write("%s %s\n" % (self.address_string(), format % args))

        def send_json(self, status: int, payload: dict) -> None:
            body = json.dumps(payload).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def refuse(self, refusal: Refusal) -> None:
            self.send_json(
                REFUSAL_STATUS,
                {
                    "detail": refusal.message,
                    "error": {
                        "code": refusal.code,
                        "message": refusal.message,
                        "question": refusal.question,
                    },
                },
            )

        def do_GET(self) -> None:  # noqa: N802
            if self.path in ("/v1/models", "/v1/models/"):
                self.send_json(200, {"models": [door.card()]})
            elif self.path == "/health":
                self.send_json(200, {"status": "ok", "model": door.name})
            else:
                self.send_json(404, {"detail": f"no route {self.path}"})

        def do_POST(self) -> None:  # noqa: N802
            if self.path not in ("/v1/systemone", "/v1/systemone/"):
                self.send_json(404, {"detail": f"no route {self.path}"})
                return
            length = int(self.headers.get("Content-Length") or 0)
            raw = self.rfile.read(length)
            try:
                body = json.loads(raw)
                if not isinstance(body, dict):
                    raise Refusal("invalid_request", "the request body must be an object", None)
                self.send_json(200, door.answer(body))
            except json.JSONDecodeError as error:
                self.refuse(Refusal("invalid_request", f"the request body is not JSON: {error}", None))
            except Refusal as refusal:
                self.refuse(refusal)

    server = ThreadingHTTPServer((host, port), Handler)
    print(f"{door.name} listening on http://{host}:{port}", file=sys.stderr)
    print(json.dumps(door.card()["fitted"], indent=2), file=sys.stderr)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--suite", required=True, type=Path, help="the suite whose fitting partition trains the heads")
    parser.add_argument("--encoder", required=True, help="a sentence-transformers encoder name")
    parser.add_argument("--revision", required=True, help="the encoder commit the door is pinned to")
    parser.add_argument("--name", default=None, help="the door's name; `baseline-<encoder>` by default")
    parser.add_argument("--rule", choices=("argmin", "one-se"), default="argmin", help="the strength-selection rule")
    parser.add_argument("--seed", type=int, default=0, help="the fold seed the strength selection uses")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8020)
    args = parser.parse_args()

    encoder = FrozenEncoder(args.encoder, args.revision)
    heads, fitted = fit(args.suite, encoder, args.rule, args.seed)
    if not heads:
        raise SystemExit("the suite holds no Choice family; the door would refuse everything")
    name = args.name or f"baseline-{args.encoder.rsplit('/', 1)[-1]}"
    serve(Door(name, encoder, heads, fitted), args.host, args.port)
    return 0


if __name__ == "__main__":
    sys.exit(main())
