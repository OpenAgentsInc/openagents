"""Measure readable Luna summary settings without recording login credentials.

Uses the operator's existing Codex login, read-only. No tools execute and no
login refresh occurs. Each request and readable response is retained.
"""
import argparse
import base64
import hashlib
import json
import time
import urllib.error
import urllib.request
import uuid
from datetime import datetime, timezone
from pathlib import Path

TASK = """Review this cache implementation against the contract below. Identify the
correctness defects and propose a minimal correction plus three useful tests.
Return a concise review, not a rewritten program.
Contract: get returns a fresh item or None; reading does not extend its lifetime.
put replaces the value and expiration. The cache has capacity 2 and evicts the
least recently used live entry only when a new key exceeds capacity. The clock
is monotonic and an entry expires exactly at expires_at.
Code:
class Cache:
    def __init__(self): self.items = {}
    def put(self, key, value, now, ttl):
        if len(self.items) >= 2:
            del self.items[next(iter(self.items))]
        self.items[key] = [value, now + ttl]
    def get(self, key, now):
        if key not in self.items: return None
        value, expires_at = self.items[key]
        if now > expires_at: return None
        self.items[key][1] = now + 10
        return value
"""


def probe(output, effort="high", repeats=3):
    output.mkdir(parents=True, exist_ok=True)
    auth = json.loads((Path.home() / ".codex/auth.json").read_text())["tokens"]
    token = auth["access_token"]
    payload = token.split(".")[1]
    expires = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))["exp"]
    if expires - time.time() < 600:
        raise RuntimeError("The Codex login expires within ten minutes; no refresh attempted")
    orders = [("auto", "concise", "detailed"), ("detailed", "auto", "concise"),
              ("concise", "detailed", "auto")]
    records = []
    for repeat, order in enumerate(orders[:repeats], 1):
        for setting in order:
            target = output / f"{repeat}-{setting}.json"
            if target.exists():
                raise RuntimeError(f"Refusing to overwrite {target}")
            session = str(uuid.uuid4())
            body = {"model": "gpt-6-luna", "instructions": "You review code carefully and answer concisely.",
                    "input": [{"role": "user", "content": [{"type": "input_text", "text": TASK}]}],
                    "tools": [], "tool_choice": "auto", "parallel_tool_calls": False,
                    "store": False, "stream": True, "include": ["reasoning.encrypted_content"],
                    "prompt_cache_key": session, "reasoning": {"effort": "high", "summary": setting}}
            if effort == "default":
                body["reasoning"].pop("effort")
            request = urllib.request.Request("https://chatgpt.com/backend-api/codex/responses",
                data=json.dumps(body).encode(), method="POST", headers={
                    "Authorization": f"Bearer {token}", "ChatGPT-Account-ID": auth["account_id"],
                    "Content-Type": "application/json", "Accept": "text/event-stream",
                    "originator": "openagents_microluna_summary_probe", "User-Agent": "openagents-summary-probe/1",
                    "session-id": session, "thread-id": session})
            record = {"repeat": repeat, "setting": setting, "request": body,
                      "started_at": datetime.now(timezone.utc).isoformat(), "response": None}
            items = []
            started = time.monotonic()
            try:
                with urllib.request.urlopen(request, timeout=150) as stream:
                    record["http_status"] = stream.status
                    size = 0
                    for line in stream:
                        size += len(line)
                        if size > 8 * 1024 * 1024 or time.monotonic() - started > 180:
                            raise RuntimeError("Probe response exceeded its byte or time bound")
                        if not line.startswith(b"data: "): continue
                        data = line[6:].strip()
                        if data == b"[DONE]": continue
                        event = json.loads(data)
                        if event.get("type") == "response.output_item.done":
                            items.append(event["item"])
                        if event.get("type") in ("response.completed", "response.failed", "response.incomplete"):
                            record["response"] = event.get("response")
                            break
                        elif event.get("type") == "error": record["error"] = event
            except urllib.error.HTTPError as error:
                record["http_status"] = error.code
                record["error"] = error.read(8000).decode(errors="replace")
            except Exception as error:
                record["error"] = str(error)
            record["seconds"] = time.monotonic() - started
            response = record["response"] or {}
            if items:
                response["output"] = items
            summaries = []
            for item in response.get("output", []):
                item.pop("encrypted_content", None)
                if item.get("type") == "reasoning":
                    summaries.extend(part.get("text", "") for part in item.get("summary", []))
            record["summary_chars"] = sum(map(len, summaries))
            record["readable_summary"] = "\n".join(summaries)
            usage = response.get("usage") or {}
            if usage:
                cached = (usage.get("input_tokens_details") or {}).get("cached_tokens", 0)
                record["cost_usd"] = ((usage["input_tokens"] - cached) * 0.1 + cached * 0.01 + usage["output_tokens"] * 0.5) / 1e6
            else:
                record["cost_usd"] = None
            target.write_text(json.dumps(record, indent=2) + "\n")
            records.append({key: record.get(key) for key in ("repeat", "setting", "http_status", "seconds", "summary_chars", "cost_usd")})
            print(json.dumps(records[-1]), flush=True)
    (output / "summary.json").write_text(json.dumps({"schema": "openagents.microluna.summary-probe.v1",
        "source_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "pricing": "USD per million: input 0.10, cached 0.01, output 0.50; repository Luna usage valuation, not an invoice",
        "records": records}, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--effort", choices=["high", "default"], default="high")
    parser.add_argument("--repeats", type=int, choices=[1, 2, 3], default=3)
    args = parser.parse_args()
    probe(args.output, args.effort, args.repeats)
