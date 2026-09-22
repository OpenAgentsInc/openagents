"""A constant baseline door for the caller pilot.

Answers `POST /v1/systemone` with the first option of every question, at
maximum confidence — the deterministic floor a real model must beat.
`GET /v1/models` publishes a card so the record can name the door's
identity. Not a door you deploy.
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer


def constant_answer(question):
    kind = question.get("type")
    if kind == "choice":
        criteria = question.get("criteria", {})
        options = list(criteria.keys()) if isinstance(criteria, dict) else criteria
        pick = options[0] if options else ""
        return {
            "type": "choice",
            "choice": pick,
            "confidence": 1.0,
            "probabilities": {o: (1.0 if o == pick else 0.0) for o in options},
        }
    if kind == "noul":
        return {"type": "noul", "answer": False, "probability": 0.01}
    if kind == "score":
        criteria = question.get("criteria", [])
        levels = criteria if isinstance(criteria, list) else list(criteria)
        probs = [0.0] * len(levels)
        if probs:
            probs[0] = 1.0
        return {"type": "score", "score": 0, "confidence": 1.0, "probabilities": probs}
    return {"type": kind, "refused": "unknown_question_type"}


class Door(BaseHTTPRequestHandler):
    def _send(self, status, body):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path == "/v1/models":
            self._send(200, {
                "models": [{
                    "id": "constant",
                    "name": "constant",
                    "artifact_identity": {"digest": "sha256:" + "1" * 64},
                    "execution": {"lane": "unmetered-local"},
                }]
            })
        else:
            self._send(404, {"error": {"code": "not_found", "message": self.path}})

    def do_POST(self):
        length = int(self.headers.get("content-length", 0))
        body = json.loads(self.rfile.read(length) or b"{}")
        if self.path == "/v1/systemone":
            questions = body.get("questions", {})
            self._send(200, {
                "model": "constant",
                "answers": {name: constant_answer(q) for name, q in questions.items()},
            })
        else:
            self._send(404, {"error": {"code": "not_found", "message": self.path}})

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 19090
    HTTPServer(("127.0.0.1", port), Door).serve_forever()
