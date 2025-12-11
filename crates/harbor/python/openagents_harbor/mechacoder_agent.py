"""
Harbor agent adapter for MechaCoder.

This module implements the BaseInstalledAgent interface to run MechaCoder
in Terminal-Bench evaluations via Harbor.

MechaCoder uses Claude Code as its subagent for code execution, providing
high-quality code generation with comprehensive tool support.
"""

import json
import os
import platform
import re
import shlex
import subprocess
import tempfile
from pathlib import Path
from shutil import rmtree

from harbor.agents.installed.base import BaseInstalledAgent, ExecInput
from harbor.models.agent.context import AgentContext
from harbor.models.trial.paths import EnvironmentPaths


class MechaCoderAgent(BaseInstalledAgent):
    """
    Harbor agent adapter for MechaCoder (OpenAgents).

    MechaCoder is installed via cargo and uses Claude Code as its subagent.
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

        Uses the tbench Rust binary which:
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

        # If ANTHROPIC_API_KEY is not set but ANTHROPIC_OAUTH_TOKEN is,
        # use the OAuth token as the API key (Claude CLI accepts this)
        if "ANTHROPIC_API_KEY" not in env and "ANTHROPIC_OAUTH_TOKEN" in env:
            env["ANTHROPIC_API_KEY"] = env["ANTHROPIC_OAUTH_TOKEN"]

        # Build model arguments
        model_args = ""
        if self.model_name:
            model_args = f"--model {self.model_name}"

        output_dir = EnvironmentPaths.agent_dir

        # MechaCoder tbench binary location (installed via cargo)
        tbench_bin = "/opt/mechacoder/target/release/tbench"

        return [
            # Ensure output directory exists
            ExecInput(
                command=f"mkdir -p {output_dir}",
                env=env,
            ),
            # Run MechaCoder tbench binary
            ExecInput(
                command=(
                    f"export PATH=\"$(npm bin -g):$HOME/.bun/bin:$HOME/.cargo/bin:$PATH\" && "
                    f"{tbench_bin} "
                    f"--instruction {escaped_instruction} "
                    f"--output-dir {output_dir} "
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
                "duration_ms": metrics.get("duration_ms", 0),
                "files_modified": metrics.get("filesModified", []),
                "tools_used": metrics.get("toolsUsed", {}),
            }

        except json.JSONDecodeError as e:
            print(f"Failed to parse MechaCoder metrics: {e}")
        except Exception as e:
            print(f"Error reading MechaCoder metrics: {e}")

    async def run(
        self,
        instruction: str,
        environment,
        context: AgentContext,
    ) -> None:
        """
        Run the agent with best-effort credential injection for Claude Code.

        When API keys are not provided via environment variables, attempt to
        read Claude Code OAuth credentials from the macOS Keychain and upload
        them into the container at ~/.claude/.credentials.json so the Claude
        CLI can authenticate in Harbor sandboxes.
        """
        await self._maybe_inject_credentials(environment)
        await super().run(instruction, environment, context)

    def _extract_credentials_from_keychain(self) -> str | None:
        """
        Extract Claude Code credentials from macOS Keychain.

        Returns the raw JSON credential string when present, otherwise None.
        """
        if platform.system() != "Darwin":
            return None

        try:
            result = subprocess.run(
                ["security", "find-generic-password", "-s", "Claude Code-credentials", "-g"],
                capture_output=True,
                text=True,
                check=False,
            )
        except Exception as exc:  # pragma: no cover - defensive
            print(f"[mechacoder] Failed to invoke security for credentials: {exc}")
            return None

        if result.returncode != 0:
            return None

        output = result.stderr or ""
        match = re.search(r'^password:\s*"(.+)"$', output, re.MULTILINE)
        if not match:
            return None

        raw = match.group(1)
        json_str = raw.replace(r'\\"', '"').replace(r"\\\\", "\\")

        try:
            json.loads(json_str)
        except json.JSONDecodeError:
            return None

        return json_str

    async def _maybe_inject_credentials(self, environment) -> None:
        """
        If env vars are missing, try to inject credentials file into container.
        """
        if "ANTHROPIC_API_KEY" in os.environ or "ANTHROPIC_OAUTH_TOKEN" in os.environ:
            return

        credentials = self._extract_credentials_from_keychain()
        if not credentials:
            return

        tmp_dir = Path(tempfile.mkdtemp(prefix="mechacoder-creds-"))
        creds_path = tmp_dir / ".credentials.json"
        try:
            tmp_dir.mkdir(parents=True, exist_ok=True)
            creds_path.write_text(credentials)

            await environment.exec(command="mkdir -p /root/.claude")
            await environment.upload_file(
                source_path=creds_path,
                target_path="/root/.claude/.credentials.json",
            )
            await environment.exec(command="chmod 600 /root/.claude/.credentials.json")
        except Exception as exc:  # pragma: no cover - defensive
            print(f"[mechacoder] Failed to inject Claude credentials into sandbox: {exc}")
        finally:
            rmtree(tmp_dir, ignore_errors=True)
