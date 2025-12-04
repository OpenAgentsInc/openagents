"""
Harbor agent adapter for MechaCoder.

This module implements the BaseInstalledAgent interface to run MechaCoder
in Terminal-Bench evaluations via Harbor.

MechaCoder uses Claude Code as its subagent for code execution, providing
high-quality code generation with comprehensive tool support.
"""

import json
import os
import shlex
from pathlib import Path

from harbor.agents.installed.base import BaseInstalledAgent, ExecInput
from harbor.models.agent.context import AgentContext
from harbor.models.trial.paths import EnvironmentPaths


class MechaCoderAgent(BaseInstalledAgent):
    """
    Harbor agent adapter for MechaCoder (OpenAgents).

    MechaCoder is installed via bun and uses Claude Code as its subagent.
    It outputs structured JSON files (metrics.json, trajectory.json) that
    are parsed to populate the agent context.
    """

    @staticmethod
    def name() -> str:
        return "mechacoder"

    @property
    def _install_agent_template_path(self) -> Path:
        return Path(__file__).parent / "install-mechacoder.sh.j2"

    def create_run_agent_commands(self, instruction: str) -> list[ExecInput]:
        """
        Create commands to run MechaCoder on the given instruction.

        Uses the tbench CLI wrapper which:
        - Accepts the task instruction
        - Runs Claude Code subagent
        - Outputs events.jsonl, trajectory.json, metrics.json
        """
        escaped_instruction = shlex.quote(instruction)

        env: dict[str, str] = {}

        # Pass through API keys for Claude Code
        for key in [
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_OAUTH_TOKEN",
            "OPENAI_API_KEY",
            "OPENROUTER_API_KEY",
        ]:
            if key in os.environ:
                env[key] = os.environ[key]

        # Build model arguments
        model_args = ""
        if self.model_name:
            model_args = f"--model {self.model_name}"

        output_dir = EnvironmentPaths.agent_dir

        # MechaCoder uses bun as its runtime
        # The tbench CLI is the entry point for Terminal-Bench runs
        mechacoder_dir = "/opt/mechacoder"
        tbench_cli = f"{mechacoder_dir}/src/cli/tbench.ts"

        return [
            # Ensure output directory exists
            ExecInput(
                command=f"mkdir -p {output_dir}",
                env=env,
            ),
            # Run MechaCoder tbench CLI
            ExecInput(
                command=(
                    f"cd {mechacoder_dir} && "
                    f"bun {tbench_cli} "
                    f"--instruction {escaped_instruction} "
                    f"--output-dir {output_dir} "
                    f"{model_args} "
                    f"--timeout 3600 "
                    f"2>&1 | tee {output_dir}/stdout.txt"
                ),
                env=env,
            ),
        ]

    def populate_context_post_run(self, context: AgentContext) -> None:
        """
        Populate the agent context with metrics from MechaCoder's output.

        Reads metrics.json which contains:
        - tokens: {input, output, total}
        - cost: USD cost of the run
        - success: whether the task completed
        - turns: number of agent turns
        - toolsUsed: tool call counts
        """
        metrics_file = self.logs_dir / "metrics.json"

        if not metrics_file.exists():
            print(f"MechaCoder metrics file not found: {metrics_file}")
            return

        try:
            with open(metrics_file) as f:
                metrics = json.load(f)

            # Extract token counts
            tokens = metrics.get("tokens", {})
            context.n_input_tokens = tokens.get("input", 0)
            context.n_output_tokens = tokens.get("output", 0)

            # Extract cost
            cost = metrics.get("cost")
            if cost is not None and cost > 0:
                context.cost_usd = cost

            # Store additional metadata
            context.metadata = {
                "success": metrics.get("success", False),
                "turns": metrics.get("turns", 0),
                "duration_ms": metrics.get("durationMs", 0),
                "files_modified": metrics.get("filesModified", []),
                "tools_used": metrics.get("toolsUsed", {}),
            }

        except json.JSONDecodeError as e:
            print(f"Failed to parse MechaCoder metrics: {e}")
        except Exception as e:
            print(f"Error reading MechaCoder metrics: {e}")
