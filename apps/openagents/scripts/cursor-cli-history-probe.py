#!/usr/bin/env python3
"""Opt-in native CLI comparison: requires Cursor login and uses Muse Spark quota.

Run: python3 scripts/cursor-cli-history-probe.py
All prompts are synthetic; never resumes an existing user conversation.
"""

import json
import subprocess
import tempfile
import uuid


def main():
    tokens = [f"CLI-{uuid.uuid4()}" for _ in range(8)]
    session = None
    with tempfile.TemporaryDirectory(prefix="cursor-cli-history-") as workspace:
        def send(prompt):
            nonlocal session
            args = [
                "cursor-agent", "--workspace", workspace, "--trust",
                "--model", "muse-spark-1.3", "--print", "--output-format", "json",
            ]
            if session:
                args += ["--resume", session]
            response = subprocess.run(
                args + [prompt], capture_output=True, text=True, timeout=120,
            )
            assert response.returncode == 0, response.stderr[-1000:]
            result = json.loads(response.stdout)
            returned_session = result.get("session_id")
            assert returned_session, "CLI did not return a resumable session ID"
            assert session is None or returned_session == session, "session changed"
            session = returned_session
            return result.get("result", "")

        for i, token in enumerate(tokens):
            prompt = f"Remember token {token}. Reply with this token"
            if i:
                prompt += " and the token from the immediately previous user message"
            prompt += (
                ". Do not use tools or files. Retain all tokens in conversation history."
            )
            answer = send(prompt)
            assert token in answer, f"turn {i}: current token missing"
            if i:
                assert tokens[i - 1] in answer, f"turn {i}: previous token missing"
            print(json.dumps({
                "nativeCliTurn": i + 1, "previousAndCurrentRecalled": True,
            }), flush=True)

        answer = send("List every CLI token from all user messages in order. No tools.")
        offset = 0
        for token in tokens:
            offset = answer.index(token, offset) + len(token)
        print(json.dumps({
            "nativeCliHistoryTokens": len(tokens), "resumed": True, "failures": 0,
        }), flush=True)


if __name__ == "__main__":
    main()
