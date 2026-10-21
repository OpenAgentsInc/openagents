#!/usr/bin/env python3
"""Check the native Flint HTTP endpoint without creating account data."""
import argparse
import concurrent.futures
import json
import time
import urllib.error
import urllib.request
from pathlib import Path

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("--url", default="http://127.0.0.1:8080")
p.add_argument("--output", type=Path, required=True)
args = p.parse_args()


def call(prompt=None, **options):
    body = dict(model="flint", messages=[dict(role="user", content=prompt or "Hello")],
                max_tokens=128, temperature=0, seed=42)
    body.update(options)
    request = urllib.request.Request(args.url + "/v1/chat/completions", data=json.dumps(body).encode(), headers={"Content-Type": "application/json"})
    start = time.monotonic()
    try:
        with urllib.request.urlopen(request, timeout=920) as response:
            return dict(status=response.status, body=json.load(response), seconds=time.monotonic()-start)
    except urllib.error.HTTPError as error:
        return dict(status=error.code, body=json.load(error), seconds=time.monotonic()-start)


rows = []
for options in [dict(stream=True), dict(tools=[]), dict(max_tokens=9000)]:
    result = call(**options)
    rows.append(dict(check="refusal", options=options, result=result))
    assert result["status"] == 400, result

prompt = "What is 2 + 2? Reply with just the number."
first = call(prompt)
second = call(prompt)
for result in [first, second]:
    assert result["status"] == 200, result
    choice = result["body"]["choices"][0]
    assert choice["finish_reason"] == "stop", result
    assert choice["message"]["content"].strip() == "4", result
assert first["body"]["psionic"]["token_ids"] == second["body"]["psionic"]["token_ids"]
rows.append(dict(check="arithmetic_and_repeat", first=first, second=second))

with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
    future = executor.submit(call, "Write a Rust function named add that takes two i32 values and returns their sum. Return only the function, without explanations.", max_tokens=512)
    time.sleep(.2)
    busy = call()
    code = future.result()
assert busy["status"] == 429, busy
assert code["status"] == 200 and code["body"]["choices"][0]["finish_reason"] == "stop", code
rows.append(dict(check="code_and_serial_admission", code=code, busy=busy))
args.output.write_text(json.dumps(rows, indent=2) + "\n")
print(args.output)
for row in rows:
    print(row["check"], "passed")
