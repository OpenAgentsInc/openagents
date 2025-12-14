"""
Harbor agent adapter for Claude Code CLI with testgen skill.

This module implements the BaseInstalledAgent interface to run Claude Code CLI
in Terminal-Bench evaluations via Harbor, with the testgen skill for improved
test coverage and task completion.

Uses Harbor's built-in ClaudeCode agent as base to get ATIF trajectory support.
"""

import os
from pathlib import Path
from typing import Any

from harbor.agents.installed.base import BaseInstalledAgent, ExecInput
from harbor.agents.installed.claude_code import ClaudeCode
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


class ClaudeCodeAgent(ClaudeCode):
    """
    Harbor agent adapter for Claude Code CLI with testgen skill.

    Extends Harbor's built-in ClaudeCode agent to:
    - Prepend testgen skill protocol to instructions
    - Use custom install template with non-root user setup
    - Inject OAuth credentials from local machine

    Inherits ATIF trajectory support from ClaudeCode base class.
    """

    @staticmethod
    def name() -> str:
        return "claude-code-testgen"

    @property
    def _install_agent_template_path(self) -> Path:
        return Path(__file__).parent / "install-agent.sh.j2"

    def create_run_agent_commands(self, instruction: str) -> list[ExecInput]:
        """Create commands to run Claude Code CLI with testgen skill prepended."""
        # Prepend testgen skill to instruction
        full_instruction = TESTGEN_SKILL + "\n\n# TASK:\n\n" + instruction

        # Get base commands from parent (handles --output-format stream-json, CLAUDE_CONFIG_DIR, etc.)
        # But we need to modify to use our instruction
        return self._create_testgen_commands(full_instruction)

    def _create_testgen_commands(self, instruction: str) -> list[ExecInput]:
        """Create commands with testgen-modified instruction and credential injection."""
        import shlex

        escaped_instruction = shlex.quote(instruction)

        env = {
            "ANTHROPIC_API_KEY": os.environ.get("ANTHROPIC_API_KEY", ""),
            "CLAUDE_CODE_OAUTH_TOKEN": os.environ.get("CLAUDE_CODE_OAUTH_TOKEN", ""),
            "FORCE_AUTO_BACKGROUND_TASKS": "1",
            "ENABLE_BACKGROUND_TASKS": "1",
        }

        # Remove empty auth credentials
        env = {k: v for k, v in env.items() if v}

        if self.model_name:
            env["ANTHROPIC_MODEL"] = self.model_name.split("/")[-1]
        elif "ANTHROPIC_MODEL" in os.environ:
            env["ANTHROPIC_MODEL"] = os.environ["ANTHROPIC_MODEL"]

        if "MAX_THINKING_TOKENS" in os.environ:
            env["MAX_THINKING_TOKENS"] = os.environ["MAX_THINKING_TOKENS"]

        env["CLAUDE_CONFIG_DIR"] = (EnvironmentPaths.agent_dir / "sessions").as_posix()

        # Inject credentials from local machine if available
        cred_setup = self._create_credential_setup_command()

        return [
            ExecInput(
                command=(
                    "mkdir -p $CLAUDE_CONFIG_DIR/debug $CLAUDE_CONFIG_DIR/projects/-app "
                    "$CLAUDE_CONFIG_DIR/shell-snapshots $CLAUDE_CONFIG_DIR/statsig "
                    "$CLAUDE_CONFIG_DIR/todos"
                ),
                env=env,
            ),
            ExecInput(
                command=cred_setup,
                env=env,
            ),
            ExecInput(
                command=(
                    f"source $HOME/.nvm/nvm.sh && "
                    f"claude --verbose --output-format stream-json "
                    f"-p {escaped_instruction} --allowedTools "
                    f"{' '.join(self.ALLOWED_TOOLS)} 2>&1 </dev/null | tee "
                    f"/logs/agent/claude-code.txt"
                ),
                env=env,
            )
        ]

    def _create_credential_setup_command(self) -> str:
        """
        Create command to setup Claude credentials from local machine.

        Reads credentials from ~/.claude/.credentials.json and injects into container.
        """
        cred_file = Path.home() / ".claude" / ".credentials.json"
        if cred_file.exists():
            try:
                cred_content = cred_file.read_text()
                return f"""
mkdir -p $CLAUDE_CONFIG_DIR
cat > $CLAUDE_CONFIG_DIR/.credentials.json << 'CRED_EOF'
{cred_content}
CRED_EOF
chmod 600 $CLAUDE_CONFIG_DIR/.credentials.json
echo "Credentials file injected from host"
"""
            except Exception as e:
                print(f"Warning: Could not read credentials file: {e}")

        return "echo 'No local credentials file found, using env vars'"
