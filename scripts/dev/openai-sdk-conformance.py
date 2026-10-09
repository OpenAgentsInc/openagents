#!/usr/bin/env python3
"""The OpenAI Python SDK against our Chat Completions surface (#11068).

One small request per row of docs/inference/gateway.md section 3's Chat
Completions table, streaming and not, through the official `openai`
package with only its base URL and key changed. Each check prints PASS or
FAIL with what it saw; the exit code is the number of failures.

    OPENAGENTS_API_KEY=oak_... scripts/dev/openai-sdk-conformance.py \\
        --base-url http://127.0.0.1:8790/v1 --model google/gemini-3.8-flash

The key is read from the environment and never printed. Requests are a few
tokens each; the whole run is about fifteen calls.
"""

import argparse
import base64
import json
import os
import sys

import openai

# A 1x1 red PNG, so the image row costs almost nothing.
PIXEL = base64.b64encode(
    bytes.fromhex(
        "89504e470d0a1a0a0000000d4948445200000001000000010802000000907753de"
        "0000000c4944415408d763f8cfc000000301010018dd8db00000000049454e44ae426082"
    )
).decode()

WEATHER = {
    "type": "function",
    "function": {
        "name": "get_weather",
        "description": "The weather in a city.",
        "parameters": {
            "type": "object",
            "properties": {"city": {"type": "string"}},
            "required": ["city"],
        },
    },
}


class Run:
    def __init__(self) -> None:
        self.results: list[tuple[str, bool, str]] = []

    def check(self, name: str, fn) -> None:
        try:
            detail = fn()
            self.results.append((name, True, detail or ""))
        except AssertionError as error:
            self.results.append((name, False, f"assertion: {error}"))
        except openai.APIStatusError as error:
            self.results.append((name, False, f"HTTP {error.status_code}: {error.message}"))
        except Exception as error:  # noqa: BLE001 - every failure is a row
            self.results.append((name, False, f"{type(error).__name__}: {error}"))
        ok, detail = self.results[-1][1], self.results[-1][2]
        print(f"{'PASS' if ok else 'FAIL'}  {name}  {detail}", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--model", default="google/gemini-3.8-flash")
    parser.add_argument("--json", help="write the results here as JSON")
    args = parser.parse_args()
    key = os.environ.get("OPENAGENTS_API_KEY")
    if not key:
        print("Set OPENAGENTS_API_KEY.", file=sys.stderr)
        return 2
    client = openai.OpenAI(base_url=args.base_url, api_key=key, max_retries=0, timeout=90)
    model = args.model
    run = Run()

    def text_reply():
        reply = client.chat.completions.create(
            model=model,
            messages=[
                {"role": "system", "content": "Answer with one word."},
                {"role": "user", "content": "Say hello."},
            ],
            max_tokens=200,
            temperature=0.2,
            top_p=0.9,
            seed=7,
            user="conformance",
        )
        choice = reply.choices[0]
        assert reply.object == "chat.completion", reply.object
        assert choice.message.role == "assistant"
        assert choice.message.content, "no content"
        assert choice.finish_reason == "stop", choice.finish_reason
        assert reply.usage and reply.usage.prompt_tokens > 0 and reply.usage.completion_tokens > 0
        return f"{choice.message.content!r}, usage {reply.usage.prompt_tokens}/{reply.usage.completion_tokens}"

    run.check("system + user text, temperature/top_p/seed/user, max_tokens, usage, finish stop", text_reply)

    def developer_and_max_completion():
        reply = client.chat.completions.create(
            model=model,
            messages=[
                {"role": "developer", "content": "Reply in lowercase."},
                {"role": "user", "content": "Name a color."},
            ],
            max_completion_tokens=200,
            stop=["\n\n"],
        )
        assert reply.choices[0].message.content
        return repr(reply.choices[0].message.content)

    run.check("developer message, max_completion_tokens, stop", developer_and_max_completion)

    def streamed():
        stream = client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": "Count to three."}],
            max_tokens=200,
            stream=True,
            stream_options={"include_usage": True},
        )
        text, finish, usage, chunks = "", None, None, 0
        for chunk in stream:
            chunks += 1
            assert chunk.object == "chat.completion.chunk", chunk.object
            if chunk.choices:
                delta = chunk.choices[0].delta
                text += delta.content or ""
                finish = chunk.choices[0].finish_reason or finish
            if chunk.usage:
                usage = chunk.usage
        assert text, "no streamed text"
        assert finish == "stop", finish
        assert usage and usage.completion_tokens > 0, "no usage chunk"
        return f"{chunks} chunks, {text!r}"

    run.check("stream with include_usage: deltas, finish stop, usage chunk", streamed)

    def tool_call():
        reply = client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": "What's the weather in Paris? Use the tool."}],
            tools=[WEATHER],
            tool_choice="required",
            parallel_tool_calls=False,
            max_tokens=300,
        )
        choice = reply.choices[0]
        assert choice.finish_reason == "tool_calls", choice.finish_reason
        call = choice.message.tool_calls[0]
        assert call.type == "function" and call.function.name == "get_weather"
        json.loads(call.function.arguments)
        return f"{call.function.name}({call.function.arguments})"

    run.check("tools + tool_choice required: tool_calls, finish tool_calls", tool_call)

    def named_tool_streamed():
        stream = client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": "Weather in Oslo?"}],
            tools=[WEATHER],
            tool_choice={"type": "function", "function": {"name": "get_weather"}},
            max_tokens=300,
            stream=True,
        )
        name, arguments, finish = "", "", None
        for chunk in stream:
            if not chunk.choices:
                continue
            for call in chunk.choices[0].delta.tool_calls or []:
                name += call.function.name or "" if call.function else ""
                arguments += call.function.arguments or "" if call.function else ""
            finish = chunk.choices[0].finish_reason or finish
        assert name == "get_weather", name
        json.loads(arguments)
        assert finish == "tool_calls", finish
        return f"{name}({arguments})"

    run.check("named tool_choice, streamed tool call deltas", named_tool_streamed)

    def tool_round_trip():
        reply = client.chat.completions.create(
            model=model,
            messages=[
                {"role": "user", "content": "What's the weather in Paris?"},
                {
                    "role": "assistant",
                    "content": None,
                    "tool_calls": [
                        {
                            "id": "call_1",
                            "type": "function",
                            "function": {"name": "get_weather", "arguments": '{"city":"Paris"}'},
                        }
                    ],
                },
                {"role": "tool", "tool_call_id": "call_1", "content": "Sunny, 21 C"},
            ],
            tools=[WEATHER],
            max_tokens=200,
        )
        content = reply.choices[0].message.content or ""
        assert "21" in content or "sunny" in content.lower(), content
        return repr(content)

    run.check("assistant tool_calls + tool message in history", tool_round_trip)

    def multi_turn():
        reply = client.chat.completions.create(
            model=model,
            messages=[
                {"role": "user", "content": "My name is Ada."},
                {"role": "assistant", "content": "Nice to meet you, Ada."},
                {"role": "user", "content": "What is my name? One word."},
            ],
            max_tokens=200,
        )
        content = reply.choices[0].message.content or ""
        assert "ada" in content.lower(), content
        return repr(content)

    run.check("multi-turn assistant history", multi_turn)

    def json_object():
        reply = client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": "Return a JSON object with key a set to 1."}],
            response_format={"type": "json_object"},
            max_tokens=200,
        )
        value = json.loads(reply.choices[0].message.content)
        assert isinstance(value, dict), value
        return json.dumps(value)

    run.check("response_format json_object", json_object)

    def json_schema():
        reply = client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": "Pick a primary color."}],
            response_format={
                "type": "json_schema",
                "json_schema": {
                    "name": "color",
                    "strict": True,
                    "schema": {
                        "type": "object",
                        "properties": {"color": {"type": "string", "enum": ["red", "yellow", "blue"]}},
                        "required": ["color"],
                        "additionalProperties": False,
                    },
                },
            },
            max_tokens=200,
        )
        value = json.loads(reply.choices[0].message.content)
        assert value.get("color") in ("red", "yellow", "blue"), value
        return json.dumps(value)

    run.check("response_format json_schema (strict)", json_schema)

    def image():
        reply = client.chat.completions.create(
            model=model,
            messages=[
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "What color is this one-pixel image? One word."},
                        {"type": "image_url", "image_url": {"url": f"data:image/png;base64,{PIXEL}"}},
                    ],
                }
            ],
            max_tokens=200,
        )
        content = reply.choices[0].message.content or ""
        assert content, "no content"
        return repr(content)

    run.check("user image_url part", image)

    def reasoning_effort():
        reply = client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": "What is 17 * 3? Number only."}],
            reasoning_effort="low",
            max_completion_tokens=2000,
        )
        content = reply.choices[0].message.content or ""
        assert "51" in content, content
        details = reply.usage.completion_tokens_details if reply.usage else None
        reasoning = getattr(details, "reasoning_tokens", None) if details else None
        return f"{content!r}, reasoning tokens {reasoning}"

    run.check("reasoning_effort", reasoning_effort)

    def length_finish():
        reply = client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": "Write a long paragraph about the sea."}],
            # Gemini 3 through Vercel refuses `none` (400); low keeps the
            # few tokens for the answer.
            reasoning_effort="low",
            max_tokens=16,
        )
        finish = reply.choices[0].finish_reason
        assert finish == "length", finish
        return finish

    run.check("max_tokens reached: finish length", length_finish)

    def n_above_one():
        try:
            client.chat.completions.create(
                model=model, messages=[{"role": "user", "content": "hi"}], n=2, max_tokens=5
            )
        except openai.BadRequestError as error:
            return f"400 as documented: {error.message}"
        raise AssertionError("n=2 was accepted")

    run.check("n > 1 is 400", n_above_one)

    def unknown_model():
        try:
            client.chat.completions.create(
                model="nobody/no-such-model", messages=[{"role": "user", "content": "hi"}]
            )
        except openai.NotFoundError as error:
            return f"404: {error.message}"
        raise AssertionError("an unknown model answered")

    run.check("unknown model is 404", unknown_model)

    def models_list():
        ids = [model_.id for model_ in client.models.list()]
        assert model in ids, ids[:10]
        return f"{len(ids)} models"

    run.check("models.list()", models_list)

    failures = sum(1 for _, ok, _ in run.results if not ok)
    print(f"\n{len(run.results) - failures} passed, {failures} failed (openai {openai.__version__})")
    if args.json:
        with open(args.json, "w", encoding="utf-8") as out:
            json.dump(
                {
                    "sdk": f"openai-python {openai.__version__}",
                    "model": model,
                    "results": [{"check": n, "ok": ok, "detail": d} for n, ok, d in run.results],
                },
                out,
                indent=2,
            )
    return failures


if __name__ == "__main__":
    sys.exit(main())
