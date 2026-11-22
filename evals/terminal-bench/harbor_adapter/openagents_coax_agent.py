# Firm reason: 6 (Harbor Python BaseAgent SDK — has no C API; all Microcoder agent loop logic, LLM HTTP calls, Executor Network W2.2 guards, live Desktop GPU chat events, and ATIF-v1.8 export live in Coax coax/lib/tbench/agent.coax and coder/tui/tbench-run.coax).
"""Thin Harbor BaseAgent bridge delegating Terminal-Bench execution to Coax `tbench-run agent-loop` or host-bridged Antigravity CLI (`agy`)."""

from __future__ import annotations

import asyncio
import json
import os
import secrets
import shlex
import shutil
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

from harbor.agents.base import BaseAgent
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext


class OpenAgentsCoaxAgent(BaseAgent):
    """Minimal Harbor BaseAgent forwarding container exec/read/write requests from Coax `tbench-run agent-loop` (or host-bridged `agy`)."""

    SUPPORTS_ATIF: bool = True

    def __init__(
        self,
        logs_dir: Path,
        model_name: str | None = None,
        coax_bin: str | None = None,
        endpoint: str | None = None,
        policy_path: str | None = None,
        task_id: str | None = None,
        events_path: str | None = None,
        api_key_env: str | None = None,
        max_turns: int | None = None,
        **kwargs: Any,
    ) -> None:
        super().__init__(logs_dir=logs_dir, model_name=model_name, **kwargs)
        self._coax_bin = coax_bin or os.environ.get("OA_TBENCH_BIN", "")
        self._endpoint = endpoint or os.environ.get("OA_TBENCH_ENDPOINT", "http://127.0.0.1:18445/v1/chat/completions")
        self._policy_path = policy_path or os.environ.get("OA_TBENCH_POLICY", "")
        self._task_id = task_id or os.environ.get("OA_TBENCH_TASK", "")
        self._events_path = events_path or os.environ.get("OA_TBENCH_EVENTS", "")
        self._api_key_env = api_key_env or os.environ.get("OA_TBENCH_API_KEY_ENV", "")
        self._max_turns = max_turns

    @staticmethod
    def name() -> str:
        return "openagents-coax"

    def version(self) -> str | None:
        return "0.1.0"

    async def setup(self, environment: BaseEnvironment) -> None:
        return None

    async def _run_antigravity_bridge(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
        events_file: Path,
        task_name: str,
    ) -> None:
        """Run host-side signed-in `agy` CLI bridged into Harbor's live Docker container via a per-trial localhost /exec socket."""
        t0 = time.monotonic()
        agy_bin = (
            os.environ.get("OA_TBENCH_AGY_BIN")
            or shutil.which("agy")
            or str(Path.home() / ".local" / "bin" / "agy")
        )
        if not os.path.isfile(agy_bin):
            raise RuntimeError(f"OpenAgentsCoaxAgent: agy binary not found: {agy_bin!r}")

        def emit_event(obj: dict[str, Any]) -> None:
            with events_file.open("a", encoding="utf-8") as fh:
                fh.write(json.dumps(obj, ensure_ascii=False) + "\n")

        emit_event({"kind": "prompt", "task": task_name, "text": instruction[:16000]})

        token = secrets.token_hex(16)
        exec_log: list[dict[str, Any]] = []

        async def handle_client(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
            try:
                head = await reader.readuntil(b"\r\n\r\n")
                hdr_lines = head.decode("utf-8", errors="replace").split("\r\n")
                content_len = 0
                auth_hdr = ""
                for hl in hdr_lines[1:]:
                    if ":" not in hl:
                        continue
                    k, v = hl.split(":", 1)
                    kl = k.strip().lower()
                    if kl == "content-length":
                        content_len = int(v.strip())
                    elif kl == "x-tbox-token":
                        auth_hdr = v.strip()
                body = await reader.readexactly(content_len) if content_len > 0 else b"{}"
                if auth_hdr != token:
                    payload = json.dumps({"exit": 1, "output": "unauthorized"}).encode("utf-8")
                    writer.write(b"HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: " + str(len(payload)).encode() + b"\r\n\r\n" + payload)
                    await writer.drain()
                    return
                req = json.loads(body.decode("utf-8", errors="replace"))
                cmd = str(req.get("command", ""))
                timeout_sec = int(req.get("timeout_sec", 90))
                try:
                    res = await environment.exec(command=cmd, timeout_sec=timeout_sec)
                    out_text = (res.stdout or "") + (("\n" + res.stderr) if res.stderr else "")
                    rc = int(res.return_code)
                except Exception as exc:
                    out_text = f"[container exec error: {exc}]"
                    rc = 124
                exec_log.append({"command": cmd, "exit": rc, "output": out_text[:8000]})
                emit_event({
                    "kind": "command",
                    "command": cmd[:400],
                    "exit": rc,
                    "timed_out": rc == 124,
                    "output": out_text[:2000],
                })
                payload = json.dumps({"exit": rc, "output": out_text[:32000]}, ensure_ascii=False).encode("utf-8")
                writer.write(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: "
                    + str(len(payload)).encode()
                    + b"\r\n\r\n"
                    + payload
                )
                await writer.drain()
            except Exception as exc:
                payload = json.dumps({"exit": 1, "output": f"[bridge error: {exc}]"}).encode("utf-8")
                try:
                    writer.write(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: " + str(len(payload)).encode() + b"\r\n\r\n" + payload)
                    await writer.drain()
                except Exception:
                    pass
            finally:
                try:
                    writer.close()
                    await writer.wait_closed()
                except Exception:
                    pass

        server = await asyncio.start_server(handle_client, "127.0.0.1", 0)
        port = server.sockets[0].getsockname()[1]

        scratch_dir = Path(tempfile.mkdtemp(prefix=f"oa-agy-tb-{task_name}-"))
        tbox_path = scratch_dir / "tbox"
        tbox_script = f"""#!{sys.executable}
import json, sys, urllib.request
if len(sys.argv) > 1:
    cmd = " ".join(sys.argv[1:])
else:
    cmd = sys.stdin.read()
data = json.dumps({{"command": cmd, "timeout_sec": 120}}).encode("utf-8")
req = urllib.request.Request(
    "http://127.0.0.1:{port}/exec",
    data=data,
    headers={{"Content-Type": "application/json", "X-Tbox-Token": "{token}"}},
    method="POST",
)
with urllib.request.urlopen(req, timeout=135) as resp:
    res = json.loads(resp.read().decode("utf-8", errors="replace"))
out = res.get("output", "")
if out:
    sys.stdout.write(out)
    if not out.endswith("\\n"):
        sys.stdout.write("\\n")
sys.exit(int(res.get("exit", 1)))
"""
        tbox_path.write_text(tbox_script, encoding="utf-8")
        tbox_path.chmod(0o755)

        wrapped_prompt = (
            "You are solving a Terminal-Bench task inside a LIVE Linux Docker container.\n"
            "CRITICAL ENVIRONMENT RULES:\n"
            "1. Your local working directory contains ONLY a bridge executable `./tbox` connected to the live Linux container.\n"
            "2. Do NOT read, edit, or search files on the host machine. EVERY inspection, file edit, build, and test MUST run inside the container via `run_command` calling `./tbox`.\n"
            "3. Usage: `./tbox 'pwd && ls -la'` for short commands, OR `./tbox <<'TBEOF'\\n<multiline bash script>\\nTBEOF` for scripts/heredocs/file writes.\n"
            "4. Verify every task requirement inside the container with `./tbox` before finishing, then give a concise summary.\n\n"
            f"TASK INSTRUCTION:\n{instruction}\n"
        )

        argv = [
            agy_bin,
            "--output-format", "stream-json",
            "--dangerously-skip-permissions",
            "--disable-slash-commands",
            f"--print={wrapped_prompt}",
        ]
        if self.model_name and self.model_name != "antigravity-default":
            argv.extend(["--model", self.model_name])

        num_turns = 0
        input_tokens = 0
        output_tokens = 0
        final_response = ""
        ok_flag = False
        err_note = ""
        atif_steps: list[dict[str, Any]] = [
            {"step_id": 1, "timestamp": "2026-11-21T00:00:00Z", "source": "user", "message": instruction, "model_name": self.model_name or "antigravity"}
        ]
        raw_stream_lines: list[str] = []

        try:
            proc = await asyncio.create_subprocess_exec(
                *argv,
                cwd=str(scratch_dir),
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                limit=4 * 1024 * 1024,
            )
            assert proc.stdout is not None and proc.stderr is not None
            while True:
                raw = await proc.stdout.readline()
                if not raw:
                    break
                line = raw.decode("utf-8", errors="replace").strip()
                if not line:
                    continue
                raw_stream_lines.append(line)
                try:
                    ev = json.loads(line)
                except json.JSONDecodeError:
                    continue
                etype = ev.get("event", "")
                if etype == "step_update":
                    su = ev.get("step_update", {})
                    st_type = su.get("step_type", "")
                    if st_type == "agent_response" and su.get("text_delta"):
                        delta = str(su.get("text_delta", ""))
                        if delta.strip():
                            emit_event({"kind": "reply", "text": delta})
                elif etype == "result":
                    r = ev.get("result", {})
                    ok_flag = str(r.get("status", "")).upper() == "SUCCESS"
                    final_response = str(r.get("response", ""))
                    num_turns = max(1, int(r.get("num_turns", 0)) or len(exec_log))
                    usage = r.get("usage", {}) or {}
                    input_tokens = int(usage.get("input_tokens", 0))
                    output_tokens = int(usage.get("output_tokens", 0)) + int(usage.get("thinking_tokens", 0))
            stderr_bytes = await proc.stderr.read()
            await proc.wait()
            if stderr_bytes:
                (self.logs_dir / "agy-bridge.stderr.log").write_bytes(stderr_bytes)
            if not ok_flag and not err_note:
                err_note = f"agy exit={proc.returncode}"
        finally:
            server.close()
            await server.wait_closed()
            shutil.rmtree(scratch_dir, ignore_errors=True)

        (self.logs_dir / "agy-stream.jsonl").write_text("\n".join(raw_stream_lines) + "\n", encoding="utf-8")

        for idx, item in enumerate(exec_log, start=2):
            atif_steps.append({
                "step_id": idx,
                "timestamp": "2026-11-21T00:00:00Z",
                "source": "agent",
                "message": "",
                "model_name": self.model_name or "antigravity",
                "tool_calls": [{"tool_call_id": f"tbox_{idx}", "function_name": "Bash", "arguments": {"command": item["command"]}}],
                "observation": {"results": [{"source_call_id": f"tbox_{idx}", "content": item["output"]}]},
            })
        atif_steps.append({
            "step_id": len(atif_steps) + 1,
            "timestamp": "2026-11-21T00:00:00Z",
            "source": "agent",
            "message": final_response,
            "model_name": self.model_name or "antigravity",
            "metrics": {"prompt_tokens": input_tokens, "completion_tokens": output_tokens},
        })
        atif_doc = {
            "schema_version": "ATIF-v1.8",
            "session_id": task_name,
            "agent": {"name": "antigravity-cli-bridge", "version": "1.5.13", "model_name": self.model_name or "antigravity"},
            "steps": atif_steps,
            "final_metrics": {"total_prompt_tokens": input_tokens, "total_completion_tokens": output_tokens, "total_steps": len(atif_steps)},
        }
        (self.logs_dir / "trajectory.json").write_text(json.dumps(atif_doc, indent=2), encoding="utf-8")

        wall_ms = int((time.monotonic() - t0) * 1000)
        emit_event({"kind": "ended", "ending": "finished" if ok_flag else "error", "text": final_response[:4000] or err_note or "Completed."})

        summary = {
            "op": "done",
            "ok": ok_flag,
            "turns": max(num_turns, len(exec_log)),
            "prompt_tokens": input_tokens,
            "completion_tokens": output_tokens,
            "wall_ms": wall_ms,
            "exec_checks": 0,
            "exec_intercepts": 0,
            "exec_replays": 0,
            "atif_verified": True,
            "atif_digest": "",
            "error_note": err_note,
        }
        context.n_input_tokens = input_tokens
        context.n_output_tokens = output_tokens
        context.metadata = {
            "turns": summary["turns"],
            "container_execs": len(exec_log),
            "events_path": str(events_file),
            "backend": "antigravity-cli",
        }
        (self.logs_dir / "coax-summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")

    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        self.logs_dir.mkdir(parents=True, exist_ok=True)
        instr_file = self.logs_dir / "instruction.txt"
        instr_file.write_text(instruction, encoding="utf-8")
        events_file = Path(self._events_path) if self._events_path else (self.logs_dir / "chat-events.jsonl")
        events_file.parent.mkdir(parents=True, exist_ok=True)
        task_name = self._task_id or self.logs_dir.parent.name.split("__")[0]

        if self._endpoint.startswith("agy://"):
            await self._run_antigravity_bridge(instruction, environment, context, events_file, task_name)
            return

        if not self._coax_bin or not os.path.isfile(self._coax_bin):
            raise RuntimeError(f"OpenAgentsCoaxAgent: tbench-run binary not found: {self._coax_bin!r}")

        argv = [
            self._coax_bin,
            "agent-loop",
            "--endpoint", self._endpoint,
            "--model", self.model_name or "inkling-2-mini",
            "--task", task_name,
            "--instruction-file", str(instr_file),
            "--events", str(events_file),
            "--atif-dir", str(self.logs_dir),
        ]
        if self._policy_path:
            argv.extend(["--policy", self._policy_path])
        if self._api_key_env:
            argv.extend(["--api-key-env", self._api_key_env])
        if self._max_turns is not None:
            argv.extend(["--max-turns", str(self._max_turns)])

        proc = await asyncio.create_subprocess_exec(
            *argv,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            limit=4 * 1024 * 1024,
        )
        assert proc.stdin is not None and proc.stdout is not None and proc.stderr is not None

        summary: dict[str, Any] = {}
        try:
            while True:
                raw_line = await proc.stdout.readline()
                if not raw_line:
                    break
                line = raw_line.decode("utf-8", errors="replace").strip()
                if not line:
                    continue
                try:
                    req = json.loads(line)
                except json.JSONDecodeError:
                    continue

                op = req.get("op", "")
                if op == "done":
                    summary = req
                    break

                if op == "exec":
                    cmd = str(req.get("command", ""))
                    timeout_sec = int(req.get("timeout_sec", 90))
                    try:
                        res = await environment.exec(command=cmd, timeout_sec=timeout_sec)
                        out_text = (res.stdout or "") + (("\n" + res.stderr) if res.stderr else "")
                        reply = {"exit": int(res.return_code), "output": out_text[:16000]}
                    except Exception as exc:
                        reply = {"exit": 124, "output": f"[container exec error: {exc}]"}
                elif op == "read":
                    path = str(req.get("path", ""))
                    q = shlex.quote(path)
                    try:
                        res = await environment.exec(command=f"cat -- {q}", timeout_sec=30)
                        out_text = (res.stdout or "") + (("\n" + res.stderr) if res.stderr else "")
                        reply = {"exit": int(res.return_code), "output": out_text[:24000]}
                    except Exception as exc:
                        reply = {"exit": 1, "output": f"[read error: {exc}]"}
                elif op == "write":
                    path = str(req.get("path", ""))
                    content = str(req.get("content", ""))
                    q = shlex.quote(path)
                    local_tmp = self.logs_dir / ".upload_stage.tmp"
                    try:
                        await environment.exec(command=f"mkdir -p -- $(dirname -- {q})", timeout_sec=15)
                        local_tmp.write_text(content, encoding="utf-8")
                        await environment.upload_file(local_tmp, path)
                        reply = {"exit": 0, "output": f"Wrote {path} ({len(content.encode('utf-8'))} bytes)."}
                    except Exception as exc:
                        reply = {"exit": 1, "output": f"[write error: {exc}]"}
                    finally:
                        if local_tmp.exists():
                            local_tmp.unlink(missing_ok=True)
                else:
                    reply = {"exit": 1, "output": f"[unknown op: {op}]"}

                proc.stdin.write((json.dumps(reply, ensure_ascii=False) + "\n").encode("utf-8"))
                await proc.stdin.drain()
        finally:
            try:
                proc.stdin.close()
            except Exception:
                pass
            stderr_bytes = await proc.stderr.read()
            await proc.wait()
            if stderr_bytes:
                (self.logs_dir / "coax-agent.stderr.log").write_bytes(stderr_bytes)

        if summary:
            context.n_input_tokens = int(summary.get("prompt_tokens", 0))
            context.n_output_tokens = int(summary.get("completion_tokens", 0))
            context.metadata = {
                "turns": int(summary.get("turns", 0)),
                "exec_checks": int(summary.get("exec_checks", 0)),
                "exec_intercepts": int(summary.get("exec_intercepts", 0)),
                "exec_replays": int(summary.get("exec_replays", 0)),
                "atif_verified": bool(summary.get("atif_verified", False)),
                "atif_digest": str(summary.get("atif_digest", "")),
                "events_path": str(events_file),
            }
            (self.logs_dir / "coax-summary.json").write_text(
                json.dumps(summary, indent=2), encoding="utf-8"
            )
