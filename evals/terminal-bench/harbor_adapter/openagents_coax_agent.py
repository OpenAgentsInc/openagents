# Firm reason: 6 (Harbor Python BaseAgent SDK — has no C API; all agent loop logic, LLM HTTP calls, Executor Network W2.2 guards, live Desktop GPU chat events, and ATIF-v1.8 export live in Coax coax/lib/tbench/agent.coax and coder/tui/tbench-run.coax).
"""Thin Harbor BaseAgent bridge delegating Terminal-Bench execution to Coax `tbench-run agent-loop`."""

from __future__ import annotations

import asyncio
import json
import os
import shlex
from pathlib import Path
from typing import Any

from harbor.agents.base import BaseAgent
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext


class OpenAgentsCoaxAgent(BaseAgent):
    """Minimal Harbor BaseAgent forwarding container exec/read/write requests from Coax `tbench-run agent-loop`."""

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

    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        if not self._coax_bin or not os.path.isfile(self._coax_bin):
            raise RuntimeError(f"OpenAgentsCoaxAgent: tbench-run binary not found: {self._coax_bin!r}")

        self.logs_dir.mkdir(parents=True, exist_ok=True)
        instr_file = self.logs_dir / "instruction.txt"
        instr_file.write_text(instruction, encoding="utf-8")
        events_file = Path(self._events_path) if self._events_path else (self.logs_dir / "chat-events.jsonl")
        events_file.parent.mkdir(parents=True, exist_ok=True)
        task_name = self._task_id or self.logs_dir.parent.name.split("__")[0]

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
