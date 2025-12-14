"""
Harbor agent adapter for Claude Code CLI with testgen skill.

This module implements the BaseInstalledAgent interface to run Claude Code CLI
in Terminal-Bench evaluations via Harbor, with the testgen skill for improved
test coverage and task completion.
"""

import os
from pathlib import Path
from typing import Any

from harbor.agents.installed.base import BaseInstalledAgent, ExecInput
from harbor.models.agent.context import AgentContext
from harbor.models.trial.paths import EnvironmentPaths


# Testgen skill protocol to prepend to instructions
TESTGEN_SKILL = """
# TestGen Protocol

**Workflow**: ANALYZE -> EXPAND -> REVIEW (loop) -> IMPLEMENT -> ITERATE

## Step 1: Analyze Task

Output a structured analysis in THIS EXACT FORMAT:

```markdown
### ENTITIES
- entity_name: description, format, validation_rules

### CONSTRAINTS
- constraint_id: description, applies_to: [entity1, entity2]

### MATRIX
| Constraint | Entity1 | Entity2 |
|------------|---------|---------|
| c1         | ✓       | ✓       |
```

## Step 2: Generate Test Scaffold

Create comprehensive tests covering ALL entity-constraint combinations from your matrix.
Save tests to `/app/testgen_tests.py`. Update `SOLUTION_PATH` for your task.

## Step 3: Review Tests

Review your generated tests for:
1. Missing entity-constraint combinations (every ✓ in matrix needs a test)
2. Missing edge cases implied by the task
3. Overly weak assertions (tests that always pass)

Loop until tests are thorough enough (max 5 iterations).

## Step 4: Implement Solution

Fill in actual solution logic. Run tests frequently:
```
pytest /app/testgen_tests.py -v
```

## Step 5: Iterate

1. Write initial solution
2. Run: pytest /app/testgen_tests.py -v
3. If FAIL: fix solution (not tests!), go to 2
4. If PASS: done

## Rules

- NEVER read `/tests/*` or `test_outputs.py` (benchmark files)
- Derive tests from task description ONLY
- Fix the solution, not the tests

---

"""


class ClaudeCodeAgent(BaseInstalledAgent):
    """
    Harbor agent adapter for Claude Code CLI with testgen skill.

    Claude Code CLI is spawned with:
    - --dangerously-skip-permissions (required for unattended execution)
    - --max-turns 50 (prevent infinite loops)
    - --model (configurable via Harbor -m flag)
    - Testgen skill prepended to instruction
    """

    @staticmethod
    def name() -> str:
        return "claude-code"

    @property
    def _install_agent_template_path(self) -> Path:
        return Path(__file__).parent / "install-agent.sh.j2"

    def create_run_agent_commands(self, instruction: str) -> list[ExecInput]:
        """Create commands to run Claude Code CLI with testgen skill."""

        # Prepend testgen skill to instruction
        full_instruction = TESTGEN_SKILL + "\n\n# TASK:\n\n" + instruction

        env: dict[str, str] = {}

        # Support OAuth token (preferred) or API key
        # OAuth tokens work if credentials are exported from Mac Keychain
        for key in [
            "ANTHROPIC_OAUTH_TOKEN",
            "ANTHROPIC_API_KEY",
        ]:
            if key in os.environ:
                env[key] = os.environ[key]

        # Build model arguments
        model_args = ""
        if self.model_name:
            # Harbor format: provider/model -> extract model part
            model = self._parse_model_name(self.model_name)
            model_args = f"--model {model}"

        output_dir = EnvironmentPaths.agent_dir
        log_file = output_dir / "claude-code.txt"
        instruction_file = output_dir / "instruction.txt"

        # Escape instruction for heredoc (just need to escape backslashes and dollar signs)
        escaped_for_heredoc = full_instruction.replace("\\", "\\\\").replace("$", "\\$")

        return [
            ExecInput(
                command=f"mkdir -p {output_dir}",
                env=env,
            ),
            # Write instruction to file (avoids complex shell quoting)
            ExecInput(
                command=f"cat > {instruction_file} << 'INSTRUCTION_EOF'\n{full_instruction}\nINSTRUCTION_EOF",
                env=env,
            ),
            # Setup credentials for claude user
            ExecInput(
                command=self._create_credential_setup_command(),
                env=env,
            ),
            # Run as non-root user (Claude CLI refuses --dangerously-skip-permissions as root)
            ExecInput(
                command=self._build_claude_command(instruction_file, model_args, log_file, env),
                env=env,
            ),
        ]

    def _parse_model_name(self, model_name: str) -> str:
        """Parse Harbor model format (provider/model) into Claude format."""
        if "/" in model_name:
            # e.g., "anthropic/claude-haiku-4-5-20251001" -> "claude-haiku-4-5-20251001"
            return model_name.split("/", 1)[1]
        return model_name

    def _build_claude_command(
        self,
        instruction_file: Any,
        model_args: str,
        log_file: Any,
        env: dict[str, str],
    ) -> str:
        """Build the full Claude CLI command to run as non-root user."""
        # Build environment exports for API key (as fallback)
        env_exports = ""
        for key in ["ANTHROPIC_API_KEY", "ANTHROPIC_OAUTH_TOKEN"]:
            if key in env:
                # Escape single quotes in the value
                value = env[key].replace("'", "'\"'\"'")
                env_exports += f"export {key}='{value}' && "

        return (
            f"su - claude -c '"
            f"export NVM_DIR=\"$HOME/.nvm\" && "
            f". \"$NVM_DIR/nvm.sh\" && "
            f"{env_exports}"
            f"cd /app && "
            f"cat {instruction_file} | "
            f"claude --print --dangerously-skip-permissions "
            f"--max-turns 50 "
            f"{model_args} "
            f"-p - "
            f"2>&1' | tee {log_file}"
        )

    def _create_credential_setup_command(self) -> str:
        """
        Create command to setup Claude credentials for the claude user.

        Reads credentials from Mac Keychain and writes to ~/.claude/.credentials.json
        for the claude user in the container.
        """
        # Read credentials from local machine and inject into container
        cred_file = Path.home() / ".claude" / ".credentials.json"
        if cred_file.exists():
            try:
                cred_content = cred_file.read_text()
                # Escape for shell
                escaped_cred = cred_content.replace("'", "'\"'\"'")
                return f"""
mkdir -p /home/claude/.claude
cat > /home/claude/.claude/.credentials.json << 'CRED_EOF'
{cred_content}
CRED_EOF
chmod 600 /home/claude/.claude/.credentials.json
chown -R claude:claude /home/claude/.claude
echo "Credentials file created for claude user"
"""
            except Exception as e:
                print(f"Warning: Could not read credentials file: {e}")

        return "echo 'No credentials file found, using env vars'"

    def populate_context_post_run(self, context: AgentContext) -> None:
        """
        Populate the agent context with token usage from Claude Code's output log.

        Note: Token tracking not available without session file.
        """
        log_file = self.logs_dir / "claude-code.txt"

        if not log_file.exists():
            print(f"Claude Code log file not found: {log_file}")
            return

        # Token tracking would require session file which isn't supported
        # Just verify the log file exists
        print(f"Claude Code log file found: {log_file}")
