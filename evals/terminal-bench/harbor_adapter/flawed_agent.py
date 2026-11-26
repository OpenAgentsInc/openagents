# Firm reason: 6 (Harbor Python BaseInstalledAgent SDK — has no C API; all Flawed agent loop, tools, compaction, SSE streaming, and prompts live in Coax coder/tui/flawedworker.coax & flawed/*.coax).
"""First-class Harbor BaseInstalledAgent running OpenAgents Flawed natively inside Terminal-Bench containers."""

from __future__ import annotations

import json
import os
import shlex
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Literal, override

from pydantic import Field

from harbor.agents.capabilities import AgentCapabilities
from harbor.agents.installed.base import BaseInstalledAgent, with_prompt_template
from harbor.agents.options import InstalledAgentOptions
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext
from harbor.models.trajectories import (
    Agent,
    FinalMetrics,
    Metrics,
    Observation,
    ObservationResult,
    Step,
    ToolCall,
    Trajectory,
)

_Effort = Literal["low", "medium", "high", "xhigh", "max"]


class FlawedAgentOptions(InstalledAgentOptions):
    reasoning_effort: _Effort | None = Field(
        default="high",
        description="Model reasoning effort (low|medium|high|xhigh|max).",
    )
    thinking_mode: str | None = Field(
        default=None,
        description="Flawed thinking mode (adaptive|extended|off).",
    )
    max_turns: int | None = Field(
        default=None,
        description="Maximum agent tool turns (default: flawedworker built-in 64).",
    )
    bash_timeout_s: int | None = Field(
        default=None,
        description="Per-command Bash timeout seconds.",
    )
    budget_s: int | None = Field(
        default=None,
        description="Optional wall-clock pacing budget seconds (sets FLAWED_BUDGET_S).",
    )
    binary_path: str | None = Field(
        default=None,
        description="Path to static Linux flawedworker binary.",
    )
    curl_path: str | None = Field(
        default=None,
        description="Path to static Linux curl >= 8.3 binary.",
    )
    install_ripgrep: bool = Field(
        default=False,
        description="Optionally ensure ripgrep is installed in the container.",
    )


def _load_gemini_key_from_secrets() -> str:
    for env_name in ("GEMINI_API_KEY", "GOOGLE_API_KEY", "FLAWED_API_KEY"):
        val = (os.environ.get(env_name) or "").strip()
        if val:
            return val
    sec = Path.home() / "work" / ".secrets" / "openagents-gemini-provider.env"
    if sec.is_file():
        for raw in sec.read_text(encoding="utf-8", errors="replace").splitlines():
            ln = raw.strip()
            if ln.startswith("export "):
                ln = ln[7:].strip()
            if ln.startswith("GEMINI_API_KEY="):
                val = ln.split("=", 1)[1].strip().strip('"').strip("'")
                if val:
                    return val
    return ""


class FlawedAgent(BaseInstalledAgent):
    """Runs OpenAgents Flawed (pure Coax static Linux binary) natively inside Harbor containers."""

    SUPPORTS_ATIF: bool = True
    capabilities = AgentCapabilities(
        reasoning_effort=True,
        atif=True,
        skills=True,
    )
    options_model = FlawedAgentOptions
    options: FlawedAgentOptions

    @staticmethod
    @override
    def name() -> str:
        return "openagents-flawed"

    @override
    def get_version_command(self) -> str | None:
        return "test -x /opt/flawed/bin/flawedworker && echo 0.8.0"

    def _default_build_dir(self) -> Path:
        return Path(
            os.environ.get(
                "FLAWED_LINUX_OUT_DIR",
                str(Path.home() / ".openagents" / "tbench" / "flawed-linux-build"),
            )
        )

    @override
    async def install(self, environment: BaseEnvironment) -> None:
        arch_res = await self.exec_as_root(environment, command="uname -m")
        arch = (arch_res.stdout or "").strip()
        suffix = "amd64" if arch in ("x86_64", "amd64") else "arm64"

        build_dir = self._default_build_dir()
        fw_bin = Path(
            self.options.binary_path
            or os.environ.get("FLAWED_LINUX_BIN", "")
            or str(build_dir / f"flawedworker-linux-{suffix}")
        ).expanduser()
        if not fw_bin.is_file():
            raise FileNotFoundError(
                f"Flawed static Linux binary not found: {fw_bin}. "
                "Run evals/terminal-bench/build_flawed_linux.sh first."
            )

        curl_bin = Path(
            self.options.curl_path
            or os.environ.get("FLAWED_LINUX_CURL", "")
            or str(build_dir / f"curl-linux-{suffix}")
        ).expanduser()
        ca_bundle = build_dir / "cacert.pem"

        deps: list[str] = []
        if not curl_bin.is_file():
            deps.extend(["ca_certificates", "curl"])
        if self.options.install_ripgrep:
            deps.append("ripgrep")
        if deps:
            await self.ensure_system_dependencies(environment, tuple(deps))

        await self.exec_as_root(environment, command="mkdir -p /opt/flawed/bin")
        await environment.upload_file(str(fw_bin), "/opt/flawed/bin/flawedworker")
        await self.exec_as_root(environment, command="chmod 755 /opt/flawed/bin/flawedworker")

        if curl_bin.is_file():
            await environment.upload_file(str(curl_bin), "/opt/flawed/bin/curl")
            await self.exec_as_root(environment, command="chmod 755 /opt/flawed/bin/curl")
        if ca_bundle.is_file():
            await environment.upload_file(str(ca_bundle), "/opt/flawed/cacert.pem")
            await self.exec_as_root(environment, command="chmod 644 /opt/flawed/cacert.pem")

    @with_prompt_template
    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        api_key = _load_gemini_key_from_secrets()
        if not api_key:
            raise ValueError(
                "FlawedAgent requires GEMINI_API_KEY (or ~/work/.secrets/openagents-gemini-provider.env)."
            )

        raw_model = self.model_name or "google/gemini-4-neon"
        model = raw_model.split("/", 1)[-1] if "/" in raw_model else raw_model
        effort = self.options.reasoning_effort or "high"

        pwd_res = await self.exec_as_agent(environment, command="pwd")
        workdir = (pwd_res.stdout or "").strip() or "/app"

        req_obj = {
            "task": instruction,
            "session": "tbench",
            "cwd": workdir,
            "sandbox": "danger-full-access",
            "policy": {"enabled": False, "allow": 0, "ceiling": 0, "roots": workdir},
            "model": model,
            "effort": effort,
        }
        req_line = json.dumps(req_obj, ensure_ascii=False) + "\n"

        with tempfile.NamedTemporaryFile("w", encoding="utf-8", delete=False, suffix=".json") as tf:
            tf.write(req_line)
            local_req = tf.name
        try:
            await environment.upload_file(local_req, "/tmp/.flawed_req.json")
            await self.exec_as_root(environment, command="chmod 644 /tmp/.flawed_req.json")
        finally:
            Path(local_req).unlink(missing_ok=True)

        env: dict[str, str] = {
            "GEMINI_API_KEY": api_key,
            "FLAWED_BACKEND": "gemini",
            "FLAWED_MODEL": model,
            "FLAWED_EFFORT": effort,
            "FLAWED_PERMISSION_MODE": "bypass",
            "FLAWED_SANDBOX_MODE": "off",
            "FLAWED_AUTO_COMPACT": "1",
            "FLAWED_AUTO_MEMORY": "1",
        }
        if self.options.thinking_mode:
            env["FLAWED_THINKING_MODE"] = self.options.thinking_mode
        if self.options.max_turns is not None:
            env["FLAWED_MAX_TURNS"] = str(self.options.max_turns)
        if self.options.bash_timeout_s is not None:
            env["FLAWED_BASH_TIMEOUT_S"] = str(self.options.bash_timeout_s)
        if self.options.budget_s is not None:
            env["FLAWED_BUDGET_S"] = str(self.options.budget_s)

        runner_cmd = (
            'export PATH="/opt/flawed/bin:$PATH"; '
            'if [ -f /opt/flawed/cacert.pem ]; then '
            'export CURL_CA_BUNDLE=/opt/flawed/cacert.pem; '
            'elif [ -f /etc/ssl/certs/ca-certificates.crt ]; then '
            'export CURL_CA_BUNDLE=/etc/ssl/certs/ca-certificates.crt; '
            'elif [ -f /etc/pki/tls/certs/ca-bundle.crt ]; then '
            'export CURL_CA_BUNDLE=/etc/pki/tls/certs/ca-bundle.crt; fi; '
            'tmpdir=$(mktemp -d); '
            'mkfifo "$tmpdir/in"; '
            'trap \'touch "$tmpdir/done"; rm -rf "$tmpdir"\' EXIT; '
            '( cat /tmp/.flawed_req.json; while [ ! -f "$tmpdir/done" ]; do sleep 0.2; done ) > "$tmpdir/in" & '
            'wpid=$!; '
            ': > /logs/agent/flawed-wire.jsonl; '
            '/opt/flawed/bin/flawedworker < "$tmpdir/in" 2>/logs/agent/flawed-stderr.log '
            '| while IFS= read -r line; do '
            'printf "%s\\n" "$line" >> /logs/agent/flawed-wire.jsonl; '
            'case "$line" in *"\\"kind\\":\\"ended\\""*) touch "$tmpdir/done"; break ;; esac; '
            'done; '
            'rc=$?; '
            'touch "$tmpdir/done"; '
            'wait "$wpid" 2>/dev/null || true; '
            'rm -f /tmp/.flawed_req.json; '
            'exit $rc'
        )
        await self.exec_as_agent(environment, command=runner_cmd, env=env)
        self._populate_from_wire(instruction, model, context)

    @override
    def populate_context_post_run(self, context: AgentContext) -> None:
        raw_model = self.model_name or "google/gemini-4-neon"
        model = raw_model.split("/", 1)[-1] if "/" in raw_model else raw_model
        self._populate_from_wire("", model, context)

    def _populate_from_wire(
        self, instruction: str, model: str, context: AgentContext
    ) -> None:
        wire_path = self.logs_dir / "flawed-wire.jsonl"
        if not wire_path.is_file():
            return

        total_in = 0
        total_out = 0
        total_cached = 0
        total_cost_micros = 0
        attempts = 0
        retries = 0
        tool_events = 0
        cmd_fails = 0
        ending = ""
        final_reply_parts: list[str] = []
        steps: list[Step] = []
        step_id = 1
        now_iso = datetime.now(timezone.utc).isoformat()

        if instruction:
            steps.append(
                Step(step_id=step_id, timestamp=now_iso, source="user", message=instruction)
            )
            step_id += 1

        for raw in wire_path.read_text(encoding="utf-8", errors="replace").split("\n"):
            raw = raw.strip()
            if not raw:
                continue
            try:
                ev = json.loads(raw)
            except json.JSONDecodeError:
                continue
            kind = ev.get("kind", "")
            if kind == "attempt":
                outc = ev.get("outcome", "")
                if outc == "done":
                    attempts += 1
                    u = ev.get("usage") or {}
                    if isinstance(u, dict):
                        total_in += max(0, int(u.get("input", 0) or 0))
                        total_out += max(0, int(u.get("output", 0) or 0))
                        total_cached += max(0, int(u.get("cached", 0) or 0))
                    # fallback flat keys on coder.observability.v2
                    for k_in in ("input_tokens", "in_tokens"):
                        if k_in in ev and isinstance(ev[k_in], int) and ev[k_in] > 0:
                            total_in += ev[k_in]
                    for k_out in ("output_tokens", "out_tokens"):
                        if k_out in ev and isinstance(ev[k_out], int) and ev[k_out] > 0:
                            total_out += ev[k_out]
                    for k_c in ("cached_tokens", "cache_read", "cache_read_tokens"):
                        if k_c in ev and isinstance(ev[k_c], int) and ev[k_c] > 0:
                            total_cached += ev[k_c]
                    cm = ev.get("usd_micros") or ev.get("provider_cost_usd_micros") or ev.get("cost_micros") or 0
                    if isinstance(cm, int) and cm > 0:
                        total_cost_micros += cm
                elif outc == "retried":
                    retries += 1
            elif kind == "view":
                tool_events += 1
                p = str(ev.get("path", ""))
                ok = bool(ev.get("found", False))
                cid = f"view_{step_id}"
                steps.append(
                    Step(
                        step_id=step_id,
                        timestamp=now_iso,
                        source="agent",
                        model_name=model,
                        message="",
                        tool_calls=[ToolCall(tool_call_id=cid, function_name="Read", arguments={"file_path": p})],
                        observation=Observation(
                            results=[ObservationResult(source_call_id=cid, content="found" if ok else "not found")]
                        ),
                    )
                )
                step_id += 1
            elif kind == "command":
                tool_events += 1
                cmd = str(ev.get("command", ""))
                rc = int(ev.get("exit", 0) or 0)
                if rc != 0:
                    cmd_fails += 1
                out_txt = str(ev.get("output", ""))[:8000]
                cid = f"cmd_{step_id}"
                steps.append(
                    Step(
                        step_id=step_id,
                        timestamp=now_iso,
                        source="agent",
                        model_name=model,
                        message="",
                        tool_calls=[ToolCall(tool_call_id=cid, function_name="Bash", arguments={"command": cmd})],
                        observation=Observation(
                            results=[ObservationResult(source_call_id=cid, content=f"[exit={rc}]\n{out_txt}")]
                        ),
                    )
                )
                step_id += 1
            elif kind == "reply":
                final_reply_parts.append(str(ev.get("text", "")))
            elif kind == "ended":
                ending = str(ev.get("ending", ""))
                txt = str(ev.get("text", ""))
                if txt and not final_reply_parts:
                    final_reply_parts.append(txt)

        final_msg = "".join(final_reply_parts).strip()
        steps.append(
            Step(
                step_id=step_id,
                timestamp=now_iso,
                source="agent",
                model_name=model,
                message=final_msg,
                metrics=Metrics(
                    prompt_tokens=total_in or None,
                    completion_tokens=total_out or None,
                    cached_tokens=total_cached or None,
                ),
            )
        )

        context.n_input_tokens = total_in or None
        context.n_output_tokens = total_out or None
        context.n_cache_tokens = total_cached or None
        if total_cost_micros > 0:
            context.cost_usd = total_cost_micros / 1_000_000.0
        context.metadata = {
            "attempts": attempts,
            "retries": retries,
            "tool_events": tool_events,
            "cmd_fails": cmd_fails,
            "ending": ending,
            "effort": self.options.reasoning_effort or "high",
        }

        traj = Trajectory(
            schema_version="ATIF-v1.8",
            session_id=self.logs_dir.parent.name,
            agent=Agent(name="openagents-flawed", version="0.8.0", model_name=model),
            steps=steps,
            final_metrics=FinalMetrics(
                total_prompt_tokens=total_in or None,
                total_completion_tokens=total_out or None,
                total_cached_tokens=total_cached or None,
                total_cost_usd=context.cost_usd,
                total_steps=len(steps),
            ),
        )
        (self.logs_dir / "trajectory.json").write_text(
            json.dumps(traj.to_json_dict(), indent=2), encoding="utf-8"
        )
