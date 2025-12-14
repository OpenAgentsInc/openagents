"""
Harbor agent adapter for Pi coding agent.

This module implements the BaseInstalledAgent interface to run the Pi coding
agent (from pi-mono) in Terminal-Bench evaluations via Harbor.
"""

import json
import os
import shlex
from pathlib import Path
from typing import Any

from harbor.agents.installed.base import BaseInstalledAgent, ExecInput
from harbor.models.agent.context import AgentContext
from harbor.models.trial.paths import EnvironmentPaths


class PiAgent(BaseInstalledAgent):
    """
    Harbor agent adapter for the Pi coding agent.

    Pi is installed via npm and run in headless mode with --mode json
    to capture structured output for trajectory construction.
    """

    @staticmethod
    def name() -> str:
        return "pi"

    @property
    def _install_agent_template_path(self) -> Path:
        return Path(__file__).parent / "install-agent.sh.j2"

    def create_run_agent_commands(self, instruction: str) -> list[ExecInput]:
        """Create commands to run Pi agent."""
        escaped_instruction = shlex.quote(instruction)

        env: dict[str, str] = {}

        # Support multiple providers
        # ANTHROPIC_OAUTH_TOKEN is preferred over ANTHROPIC_API_KEY
        for key in [
            "ANTHROPIC_OAUTH_TOKEN",
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "GEMINI_API_KEY",
            "GROQ_API_KEY",
            "XAI_API_KEY",
            "OPENROUTER_API_KEY",
        ]:
            if key in os.environ:
                env[key] = os.environ[key]

        # Build model arguments
        model_args = ""
        if self.model_name:
            provider, model = self._parse_model_name(self.model_name)
            model_args = f"--provider {provider} --model {model}"

        output_dir = EnvironmentPaths.agent_dir
        session_file = output_dir / "session.jsonl"
        json_output_file = output_dir / "pi-output.jsonl"

        return [
            ExecInput(
                command=f"mkdir -p {output_dir}",
                env=env,
            ),
            ExecInput(
                command=(
                    f"pi --print --mode json --session {session_file} "
                    f"{model_args} "
                    f"{escaped_instruction} "
                    f"2>&1 | tee {json_output_file}"
                ),
                env=env,
            ),
        ]

    def _parse_model_name(self, model_name: str) -> tuple[str, str]:
        """Parse Harbor model format (provider/model) into pi format."""
        if "/" in model_name:
            parts = model_name.split("/", 1)
            return parts[0], parts[1]
        # Default to anthropic if no provider specified
        return "anthropic", model_name

    def populate_context_post_run(self, context: AgentContext) -> None:
        """
        Populate the agent context with token usage from pi's JSON output.

        Parses the JSONL output to extract token counts from assistant messages.
        """
        json_output_file = self.logs_dir / "pi-output.jsonl"

        if not json_output_file.exists():
            print(f"pi output file not found: {json_output_file}")
            return

        total_input_tokens = 0
        total_output_tokens = 0
        total_cache_read_tokens = 0
        total_cache_write_tokens = 0
        total_cost = 0.0

        with open(json_output_file) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    event = json.loads(line)

                    # Extract usage from message_end events for assistant messages
                    if event.get("type") == "message_end":
                        message = event.get("message", {})
                        if message.get("role") == "assistant":
                            usage = message.get("usage", {})
                            total_input_tokens += usage.get("input", 0)
                            total_output_tokens += usage.get("output", 0)
                            total_cache_read_tokens += usage.get("cacheRead", 0)
                            total_cache_write_tokens += usage.get("cacheWrite", 0)
                            cost = usage.get("cost", {})
                            total_cost += cost.get("total", 0.0)

                except json.JSONDecodeError:
                    continue

        context.n_input_tokens = total_input_tokens
        context.n_output_tokens = total_output_tokens
        context.n_cache_tokens = total_cache_read_tokens + total_cache_write_tokens
        context.cost_usd = total_cost if total_cost > 0 else None
